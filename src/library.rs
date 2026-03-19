use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// A single track entry stored in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// Display name derived from the filename stem.
    pub name: String,
    /// Absolute path to the audio file inside ~/.audium/music/.
    pub path: PathBuf,
}

impl Track {
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        let path: PathBuf = path.into();
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unknown".to_string());
        Self { name, path }
    }
}

/// Persistent library state stored at ~/.audium/library.json.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Library {
    pub tracks: Vec<Track>,
}

impl Library {
    /// Returns the root Audium data directory: `~/.audium`.
    pub fn data_dir() -> Result<PathBuf> {
        let home = dirs_next::home_dir().context("could not determine home directory")?;
        Ok(home.join(".audium"))
    }

    /// Returns the music storage directory: `~/.audium/music`.
    pub fn music_dir() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("music"))
    }

    fn index_path() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("library.json"))
    }

    /// Loads the library from disk, creating directories and an empty index if
    /// they do not yet exist.
    pub fn load() -> Result<Self> {
        let data_dir = Self::data_dir()?;
        let music_dir = Self::music_dir()?;
        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(&music_dir)?;

        let index = Self::index_path()?;
        if !index.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&index)
            .with_context(|| format!("reading library index at {}", index.display()))?;

        // Filter out entries whose files have been deleted externally.
        let mut lib: Self = serde_json::from_str(&raw)
            .with_context(|| "parsing library.json — the file may be corrupted")?;
        lib.tracks.retain(|t| t.path.exists());
        Ok(lib)
    }

    /// Persists the current state to `~/.audium/library.json`.
    pub fn save(&self) -> Result<()> {
        let index = Self::index_path()?;
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(&index, raw)
            .with_context(|| format!("writing library index to {}", index.display()))?;
        Ok(())
    }

    /// Copies `source` into `~/.audium/music/` (if not already there) and
    /// registers it in the library.  Returns the in-library [`Track`].
    ///
    /// Calling this with a path that is already inside the music directory is
    /// a no-op on the filesystem; the track is still registered (deduped by
    /// canonical path).
    pub fn add_file(&mut self, source: &Path) -> Result<Track> {
        let music_dir = Self::music_dir()?;

        let filename = source.file_name().context("source path has no filename")?;
        let dest = music_dir.join(filename);

        if !dest.exists() {
            fs::copy(source, &dest)
                .with_context(|| format!("copying {} -> {}", source.display(), dest.display()))?;
        }

        let track = Track::from_path(&dest);

        // Deduplicate by canonical path so repeated invocations are idempotent.
        let already_present = self.tracks.iter().any(|t| {
            t.path
                .canonicalize()
                .ok()
                .zip(dest.canonicalize().ok())
                .map(|(a, b)| a == b)
                .unwrap_or(false)
        });

        if !already_present {
            self.tracks.push(track.clone());
            self.save()?;
        }

        Ok(track)
    }
}
