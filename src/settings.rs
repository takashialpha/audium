use crate::library::Library;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

/// How colors are chosen for the UI.
///
/// The built-in themes are authored in 24-bit RGB, which a real tty or a
/// terminal without truecolor renders incorrectly.  When truecolor is not in
/// effect the UI falls back to the named-ANSI `console` theme instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    /// Detect truecolor support from the environment.
    #[default]
    Auto,
    /// Force full 24-bit color, using the selected RGB theme.
    Truecolor,
    /// Force the 16-color console fallback theme.
    Ansi16,
}

impl ColorMode {
    /// Whether truecolor is effectively active, given what auto-detection found.
    pub const fn truecolor(self, detected: bool) -> bool {
        match self {
            Self::Auto => detected,
            Self::Truecolor => true,
            Self::Ansi16 => false,
        }
    }

    /// The only override worth offering, given what detection found: forcing
    /// the mode detection already picked is indistinguishable from `Auto`.
    pub const fn override_for(detected: bool) -> Self {
        if detected {
            Self::Ansi16
        } else {
            Self::Truecolor
        }
    }

    /// Settings offers two states, so left and right are the same toggle.
    pub const fn toggle(self, detected: bool) -> Self {
        match self {
            Self::Auto => Self::override_for(detected),
            _ => Self::Auto,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Truecolor => "truecolor",
            Self::Ansi16 => "16-color",
        }
    }
}

fn default_console_theme() -> String {
    "console_dark".to_string()
}

/// Persistent user preferences.
///
/// Read from the highest-priority copy across `$XDG_CONFIG_HOME` and
/// `$XDG_CONFIG_DIRS`; always written to `$XDG_CONFIG_HOME/audium/settings.json`.
///
/// All fields have sensible defaults via `Default` so missing keys in an
/// older file are silently filled in on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Initial volume applied when audium starts (0.0-1.0).
    pub default_volume: f32,
    /// How many seconds <- / -> seek by.
    pub seek_step_secs: u64,
    /// Name of the active truecolor theme.  Must match one of the built-in
    /// theme names; unknown values fall back to "dark" silently on load.
    pub theme_name: String,
    /// Name of the active 16-color console theme.  Kept separate from
    /// `theme_name` so switching color modes does not overwrite either choice.
    #[serde(default = "default_console_theme")]
    pub console_theme_name: String,
    /// Whether background transparency is enabled.
    pub transparent: bool,
    /// How colors are selected.  `Auto` detects truecolor support and falls
    /// back to the console theme when it is missing.
    #[serde(default)]
    pub color_mode: ColorMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_volume: 0.7,
            seek_step_secs: 1,
            theme_name: "dark".to_string(),
            console_theme_name: default_console_theme(),
            transparent: false,
            color_mode: ColorMode::Auto,
        }
    }
}

impl Settings {
    const FILE: &'static str = "settings.json";

    /// Loads settings from disk.  Missing file -> `Default`.
    /// Corrupt file -> `Default` (non-fatal; we just overwrite on next save).
    pub fn load() -> Self {
        let Some(path) = Library::find_config_file(Self::FILE) else {
            return Self::default();
        };
        let Ok(raw) = fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Library::place_config_file(Self::FILE)?;
        let tmp = path.with_extension("json.tmp");
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, &raw).with_context(|| format!("writing settings to {}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .with_context(|| format!("replacing settings at {}", path.display()))?;
        Ok(())
    }

    // -- Validated setters -------------------------------------------------

    pub const fn set_default_volume(&mut self, v: f32) {
        self.default_volume = v.clamp(0.0, 1.0);
    }

    pub fn set_seek_step_secs(&mut self, s: u64) {
        self.seek_step_secs = s.clamp(1, 120);
    }

    pub fn set_theme(&mut self, name: &str) {
        self.theme_name = name.to_string();
    }

    pub fn set_console_theme(&mut self, name: &str) {
        self.console_theme_name = name.to_string();
    }
}
