use anyhow::{Context, Result};
use lofty::prelude::{Accessor, TaggedFileExt};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

// ── Identity ───────────────────────────────────────────────────────────────

/// Opaque, stable identifier for a track.  Auto-incremented; never reused.
pub type TrackId = u64;

/// Opaque, stable identifier for a playlist.  Auto-incremented; never reused.
pub type PlaylistId = u64;

/// The reserved id for the "All Tracks" virtual playlist.
pub const ALL_TRACKS_ID: PlaylistId = 0;

// ── Track ──────────────────────────────────────────────────────────────────

/// A single audio file registered in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    /// Display name: title tag if present, otherwise the filename stem.
    pub name: String,
    /// Absolute path to the audio file (inside $XDG_DATA_HOME/audium/music/ after import).
    pub path: PathBuf,

    // Optional metadata read from file tags on import.
    // All default to None so existing library.json files deserialize cleanly.
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub year: Option<u32>,
    #[serde(default)]
    pub genre: Option<String>,
    /// Raw LRC text or plain lyrics, set by the user in-app.
    #[serde(default)]
    pub lyrics: Option<String>,
}

impl Track {
    /// Returns `"{artist} — {name}"` when an artist is set, otherwise `"{name}"`.
    pub fn display(&self) -> String {
        match self.artist.as_deref().filter(|s| !s.is_empty()) {
            Some(artist) => format!("{artist} — {}", self.name),
            None => self.name.clone(),
        }
    }
}

// ── Tag reading ────────────────────────────────────────────────────────────

struct FileTags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    year: Option<u32>,
    genre: Option<String>,
}

fn read_file_tags(path: &Path) -> Option<FileTags> {
    use lofty::config::ParseOptions;
    use lofty::probe::Probe;
    let tagged = Probe::open(path)
        .ok()?
        .options(ParseOptions::new().read_cover_art(false))
        .read()
        .ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let nonempty = |s: String| if s.is_empty() { None } else { Some(s) };
    Some(FileTags {
        title: tag.title().map(|s| s.into_owned()).and_then(nonempty),
        artist: tag.artist().map(|s| s.into_owned()).and_then(nonempty),
        album: tag.album().map(|s| s.into_owned()).and_then(nonempty),
        genre: tag.genre().map(|s| s.into_owned()).and_then(nonempty),
        year: tag.date().map(|ts| u32::from(ts.year)),
    })
}

// ── Playlist ───────────────────────────────────────────────────────────────

/// A named, ordered collection of track references.
///
/// The virtual "All Tracks" playlist (id == `ALL_TRACKS_ID`) is managed
/// automatically by `Library` and cannot be deleted or renamed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    /// Ordered list of track ids belonging to this playlist.
    pub tracks: Vec<TrackId>,
}

impl Playlist {
    pub fn new(id: PlaylistId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            tracks: Vec::new(),
        }
    }
}

// ── Library ────────────────────────────────────────────────────────────────

/// Top-level persistent state: a registry of tracks + a set of playlists.
///
/// Invariants upheld at all times:
///  - `playlists[0]` is always the "All Tracks" virtual playlist
///    (`id == ALL_TRACKS_ID`).
///  - Every `TrackId` inside any playlist exists in `tracks`.
///  - `next_track_id` / `next_playlist_id` are strictly increasing.
#[derive(Debug, Serialize, Deserialize)]
pub struct Library {
    /// All registered tracks, keyed by insertion order.
    pub tracks: Vec<Track>,
    /// All playlists (index 0 is always "All Tracks").
    pub playlists: Vec<Playlist>,

    next_track_id: TrackId,
    next_playlist_id: PlaylistId,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            playlists: vec![Playlist::new(ALL_TRACKS_ID, "All Tracks")],
            next_track_id: 1,
            next_playlist_id: 1, // 0 is reserved for ALL_TRACKS_ID
        }
    }
}

impl Library {
    // ── Filesystem paths ─────────────────────────────────────────────────

    pub fn data_dir() -> Result<PathBuf> {
        xdg::BaseDirectories::with_prefix("audium")
            .create_data_directory("")
            .context("could not determine data directory")
    }

    pub fn music_dir() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("music"))
    }

    fn index_path() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("library.json"))
    }

    // ── Persistence ──────────────────────────────────────────────────────

    /// Loads (or creates) the library from `$XDG_DATA_HOME/audium/library.json`.
    /// Silently prunes tracks whose files have been deleted externally.
    pub fn load() -> Result<Self> {
        fs::create_dir_all(Self::data_dir()?)?;
        fs::create_dir_all(Self::music_dir()?)?;

        let index = Self::index_path()?;
        if !index.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&index)
            .with_context(|| format!("reading library index at {}", index.display()))?;

        let mut lib: Self = serde_json::from_str(&raw)
            .with_context(|| "parsing library.json — the file may be corrupted")?;

        // Remove tracks whose files no longer exist.
        let mut removed = std::collections::HashSet::new();
        lib.tracks.retain(|t| {
            if t.path.exists() {
                true
            } else {
                removed.insert(t.id);
                false
            }
        });
        if !removed.is_empty() {
            for pl in &mut lib.playlists {
                pl.tracks.retain(|id| !removed.contains(id));
            }
        }

        // Ensure the "All Tracks" virtual playlist is always at index 0.
        if lib.playlists.is_empty() || lib.playlists[0].id != ALL_TRACKS_ID {
            lib.playlists.retain(|pl| pl.id != ALL_TRACKS_ID);
            let mut all = Playlist::new(ALL_TRACKS_ID, "All Tracks");
            all.tracks = lib.tracks.iter().map(|t| t.id).collect();
            lib.playlists.insert(0, all);
        }

        Ok(lib)
    }

    pub fn save(&self) -> Result<()> {
        let index = Self::index_path()?;
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(&index, raw)
            .with_context(|| format!("writing library index to {}", index.display()))?;
        Ok(())
    }

    // ── Track management ─────────────────────────────────────────────────

    /// Copies `source` into `$XDG_DATA_HOME/audium/music/` (if not already there),
    /// registers it in the library and in every playlist that auto-includes
    /// all tracks (i.e., the virtual "All Tracks" playlist).
    ///
    /// Returns `(track, is_new)`.  `is_new` is false if the file was already
    /// present (idempotent).
    pub fn add_file(&mut self, source: &Path) -> Result<(Track, bool)> {
        let music_dir = Self::music_dir()?;

        let filename = source.file_name().context("source path has no filename")?;
        let dest = music_dir.join(filename);

        if !dest.exists() {
            fs::copy(source, &dest)
                .with_context(|| format!("copying {} -> {}", source.display(), dest.display()))?;
        }

        // Deduplication by canonical path.
        let dest_canon = dest.canonicalize().ok();
        if let Some(existing) = self
            .tracks
            .iter()
            .find(|t| t.path.canonicalize().ok() == dest_canon)
        {
            return Ok((existing.clone(), false));
        }

        let id = self.next_track_id;
        self.next_track_id += 1;

        let tags = read_file_tags(&dest);
        let name = tags
            .as_ref()
            .and_then(|t| t.title.clone())
            .or_else(|| dest.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "Unknown".to_string());
        let track = Track {
            id,
            name,
            path: dest.clone(),
            artist: tags.as_ref().and_then(|t| t.artist.clone()),
            album: tags.as_ref().and_then(|t| t.album.clone()),
            year: tags.as_ref().and_then(|t| t.year),
            genre: tags.as_ref().and_then(|t| t.genre.clone()),
            lyrics: None,
        };

        self.tracks.push(track.clone());

        // Keep "All Tracks" in sync.
        if let Some(all) = self.playlists.iter_mut().find(|p| p.id == ALL_TRACKS_ID) {
            all.tracks.push(id);
        }

        self.save()?;
        Ok((track, true))
    }

    /// Removes a track from the registry and from all playlists.
    /// Does NOT delete the file from disk.
    pub fn remove_track(&mut self, id: TrackId) -> Result<()> {
        self.tracks.retain(|t| t.id != id);
        for pl in &mut self.playlists {
            pl.tracks.retain(|&tid| tid != id);
        }
        self.save()
    }

    /// Renames a track (display name only; does not touch the file).
    pub fn rename_track(&mut self, id: TrackId, new_name: impl Into<String>) -> Result<()> {
        if let Some(t) = self.tracks.iter_mut().find(|t| t.id == id) {
            t.name = new_name.into();
        }
        self.save()
    }

    /// Replaces all user-editable metadata for a track (no file is touched).
    pub fn update_track_metadata(
        &mut self,
        id: TrackId,
        name: String,
        artist: Option<String>,
        album: Option<String>,
        year: Option<u32>,
        genre: Option<String>,
    ) -> Result<()> {
        if let Some(t) = self.tracks.iter_mut().find(|t| t.id == id) {
            t.name = name;
            t.artist = artist;
            t.album = album;
            t.year = year;
            t.genre = genre;
            self.save()
        } else {
            Ok(())
        }
    }

    /// Replaces (or clears) the raw lyrics text for a track.
    pub fn set_track_lyrics(&mut self, id: TrackId, lyrics: Option<String>) -> Result<()> {
        if let Some(t) = self.tracks.iter_mut().find(|t| t.id == id) {
            t.lyrics = lyrics;
            self.save()
        } else {
            Ok(())
        }
    }

    /// Returns a reference to a track by id.
    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    // ── Playlist management ──────────────────────────────────────────────

    /// Creates a new user playlist.  Returns its id.
    pub fn create_playlist(&mut self, name: impl Into<String>) -> Result<PlaylistId> {
        let id = self.next_playlist_id;
        self.next_playlist_id += 1;
        self.playlists.push(Playlist::new(id, name));
        self.save()?;
        Ok(id)
    }

    /// Deletes a user playlist.  Silently ignores attempts to delete
    /// "All Tracks" (id == `ALL_TRACKS_ID`).
    pub fn delete_playlist(&mut self, id: PlaylistId) -> Result<()> {
        if id == ALL_TRACKS_ID {
            return Ok(());
        }
        self.playlists.retain(|p| p.id != id);
        self.save()
    }

    /// Renames a user playlist.  Silently ignores "All Tracks".
    pub fn rename_playlist(&mut self, id: PlaylistId, name: impl Into<String>) -> Result<()> {
        if id == ALL_TRACKS_ID {
            return Ok(());
        }
        if let Some(pl) = self.playlists.iter_mut().find(|p| p.id == id) {
            pl.name = name.into();
        }
        self.save()
    }

    /// Adds a track to a playlist (no-op if already present).
    pub fn playlist_add_track(&mut self, playlist_id: PlaylistId, track_id: TrackId) -> Result<()> {
        if let Some(pl) = self.playlists.iter_mut().find(|p| p.id == playlist_id)
            && !pl.tracks.contains(&track_id)
        {
            pl.tracks.push(track_id);
        }
        self.save()
    }

    /// Returns a reference to a playlist by id.
    pub fn playlist(&self, id: PlaylistId) -> Option<&Playlist> {
        self.playlists.iter().find(|p| p.id == id)
    }

    /// Resolves the track objects for a given playlist, in order.
    /// Silently skips any dangling ids.
    pub fn playlist_tracks(&self, playlist_id: PlaylistId) -> Vec<&Track> {
        self.playlist(playlist_id)
            .map(|pl| pl.tracks.iter().filter_map(|id| self.track(*id)).collect())
            .unwrap_or_default()
    }
}
