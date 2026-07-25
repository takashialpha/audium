//! Persisted playback state: queue, current track and position, playback
//! modes, so a restart resumes where the last session stopped.
//!
//! *Losable* state, neither library nor preferences, so it lives under
//! `$XDG_STATE_HOME` and references tracks by filename like the index.
//! Anything unreadable is ignored and the app starts fresh: being disposable,
//! it is never migrated or set aside.

use serde::{Deserialize, Serialize};

use crate::app::LoopMode;
use crate::library::Library;

/// Bumped only if this file's shape changes incompatibly; a mismatch is
/// treated as "no saved state", never an error.
const STATE_VERSION: u32 = 1;
const STATE_FILE: &str = "state.json";

/// A snapshot of playback, as written to disk.
///
/// `Clone`/`PartialEq` let the event loop keep the last-written snapshot and
/// skip identical writes, so an idle session stops touching the disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackState {
    version: u32,
    /// Queue entries, by filename.
    pub queue: Vec<String>,
    /// Index into `queue` of the track that was current, if any.
    pub now_playing: Option<usize>,
    /// Position within the current track, in seconds.
    pub position_secs: u64,
    pub loop_mode: LoopMode,
    pub speed: f32,
    pub volume: f32,
}

impl PlaybackState {
    /// Builds a snapshot from the parts the app tracks.
    pub const fn new(
        queue: Vec<String>,
        now_playing: Option<usize>,
        position_secs: u64,
        loop_mode: LoopMode,
        speed: f32,
        volume: f32,
    ) -> Self {
        Self {
            version: STATE_VERSION,
            queue,
            now_playing,
            position_secs,
            loop_mode,
            speed,
            volume,
        }
    }

    /// Loads saved state, or `None` if there is none, it cannot be read, or it
    /// is a version this build does not understand.
    pub fn load() -> Option<Self> {
        let path = Library::find_state_file(STATE_FILE)?;
        let raw = std::fs::read_to_string(path).ok()?;
        let state: Self = serde_json::from_str(&raw).ok()?;
        (state.version == STATE_VERSION).then_some(state)
    }

    /// Writes the snapshot atomically, like the index. Failure is non-fatal:
    /// losing resume state is no reason to interrupt playback or quitting.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Library::place_state_file(STATE_FILE)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Deletes any saved state, best effort. Used when resume is switched off,
    /// so a stale file cannot resurrect a session the user asked us to forget.
    pub fn discard() {
        if let Some(path) = Library::find_state_file(STATE_FILE) {
            let _ = std::fs::remove_file(path);
        }
    }
}
