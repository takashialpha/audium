use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::library::Library;

/// Persistent user preferences stored at `~/.audium/settings.json`.
///
/// All fields have sensible defaults via `Default` so missing keys in an
/// older file are silently filled in on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Initial volume applied when audium starts (0.0 – 1.0).
    pub default_volume: f32,

    /// How many seconds ← / → seek by.
    pub seek_step_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_volume: 0.7,
            seek_step_secs: 5,
        }
    }
}

impl Settings {
    fn path() -> Result<std::path::PathBuf> {
        Ok(Library::data_dir()?.join("settings.json"))
    }

    /// Loads settings from disk.  Missing file → `Default`.
    /// Corrupt file → `Default` (non-fatal; we just overwrite on next save).
    pub fn load() -> Self {
        let path = match Self::path() {
            Ok(p) => p,
            Err(_) => return Self::default(),
        };
        if !path.exists() {
            return Self::default();
        }
        let raw = match fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => return Self::default(),
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(&path, raw).with_context(|| format!("writing settings to {}", path.display()))?;
        Ok(())
    }

    // ── Validated setters ─────────────────────────────────────────────────

    pub fn set_default_volume(&mut self, v: f32) {
        self.default_volume = v.clamp(0.0, 1.0);
    }

    pub fn set_seek_step_secs(&mut self, s: u64) {
        // Clamp to a sensible range: 1 – 120 seconds.
        self.seek_step_secs = s.clamp(1, 120);
    }
}
