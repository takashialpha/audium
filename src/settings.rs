use crate::library::Library;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

/// How colors are chosen for the UI.
///
/// The built-in themes are 24-bit RGB, which a tty renders wrongly, so without
/// truecolor the UI falls back to a named-ANSI console theme.
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
    "native".to_string()
}

/// Bumped only on an incompatible change. Informational: every field defaults
/// independently, so a file missing keys simply gains them.
const SETTINGS_VERSION: u32 = 1;

const fn default_version() -> u32 {
    SETTINGS_VERSION
}
const fn default_volume() -> f32 {
    0.7
}
const fn default_seek() -> u64 {
    1
}
const fn default_true() -> bool {
    true
}
fn default_theme() -> String {
    "dark".to_string()
}

/// Persistent user preferences.
///
/// Read from the highest-priority copy across `$XDG_CONFIG_HOME` and
/// `$XDG_CONFIG_DIRS`, always written to `$XDG_CONFIG_HOME`. Every field
/// defaults independently, so a missing key is filled in without disturbing
/// the rest. Field order matches the settings modal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Schema version of this file.  See `SETTINGS_VERSION`.
    #[serde(default = "default_version")]
    version: u32,
    /// Initial volume applied when audium starts (0.0-1.0).
    #[serde(default = "default_volume")]
    pub default_volume: f32,
    /// How many seconds <- / -> seek by.
    #[serde(default = "default_seek")]
    pub seek_step_secs: u64,
    /// Remember the session across restarts. When off, nothing is written to
    /// `$XDG_STATE_HOME` and any existing `state.json` is removed on launch.
    #[serde(default = "default_true")]
    pub resume_playback: bool,
    #[serde(default)]
    pub color_mode: ColorMode,
    /// Active truecolor theme; an unknown name falls back to "dark" on load.
    #[serde(default = "default_theme")]
    pub theme_name: String,
    /// Active console theme. Kept apart from `theme_name` so switching color
    /// modes does not overwrite either choice.
    #[serde(default = "default_console_theme")]
    pub console_theme_name: String,
    /// Whether background transparency is enabled.
    #[serde(default)]
    pub transparent: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            default_volume: 0.7,
            seek_step_secs: 1,
            resume_playback: true,
            color_mode: ColorMode::Auto,
            theme_name: "dark".to_string(),
            console_theme_name: default_console_theme(),
            transparent: false,
        }
    }
}

impl Settings {
    const FILE: &'static str = "settings.json";

    /// Loads settings from disk. Missing or corrupt -> `Default`, non-fatal:
    /// the next save overwrites it.
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
