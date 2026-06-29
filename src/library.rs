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

/// Builds a `Track` for `path`, reading metadata tags if available and
/// falling back to the filename stem for the display name.
fn track_from_file(id: TrackId, path: PathBuf) -> Track {
    let tags = read_file_tags(&path);
    let name = tags
        .as_ref()
        .and_then(|t| t.title.clone())
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Unknown".to_string());
    Track {
        id,
        name,
        artist: tags.as_ref().and_then(|t| t.artist.clone()),
        album: tags.as_ref().and_then(|t| t.album.clone()),
        year: tags.as_ref().and_then(|t| t.year),
        genre: tags.as_ref().and_then(|t| t.genre.clone()),
        lyrics: None,
        path,
    }
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

        let mut changed = false;

        // Re-locate tracks whose recorded path no longer exists but whose file
        // is still present under the current music directory (e.g. after the
        // data directory itself was moved).
        let music_dir = Self::music_dir()?;
        for t in &mut lib.tracks {
            if !t.path.exists()
                && let Some(name) = t.path.file_name()
            {
                let candidate = music_dir.join(name);
                if candidate.exists() {
                    t.path = candidate;
                    changed = true;
                }
            }
        }

        // Remove tracks whose files no longer exist.
        let mut removed = std::collections::HashSet::new();
        lib.tracks.retain(|t| {
            if t.path.exists() {
                true
            } else {
                removed.insert(t.id);
                changed = true;
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

        // Re-import any audio files sitting in the music directory but not
        // registered in the library, e.g. left behind after tracks were
        // pruned due to a stale recorded path.
        let known: std::collections::HashSet<PathBuf> = lib
            .tracks
            .iter()
            .filter_map(|t| t.path.canonicalize().ok())
            .collect();

        if let Ok(entries) = fs::read_dir(&music_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Ok(canon) = path.canonicalize() else {
                    continue;
                };
                if known.contains(&canon) || !crate::filepicker::is_audio(&path) {
                    continue;
                }

                let id = lib.next_track_id;
                lib.next_track_id += 1;
                lib.playlists[0].tracks.push(id);
                lib.tracks.push(track_from_file(id, path));
                changed = true;
            }
        }

        if changed {
            lib.save()?;
        }

        Ok(lib)
    }

    pub fn save(&self) -> Result<()> {
        let index = Self::index_path()?;
        let tmp = index.with_extension("json.tmp");
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, &raw)
            .with_context(|| format!("writing library index to {}", tmp.display()))?;
        fs::rename(&tmp, &index)
            .with_context(|| format!("replacing library index at {}", index.display()))?;
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
        crate::player::validate_decodable(source)
            .with_context(|| format!("cannot import \"{}\"", source.display()))?;

        let music_dir = Self::music_dir()?;

        let filename = source.file_name().context("source path has no filename")?;
        let default_dest = music_dir.join(filename);

        // If the default destination already exists, check whether it is already
        // registered — if so, the file was already imported and we're done.
        if default_dest.exists() {
            let dest_canon = default_dest.canonicalize().ok();
            if let Some(existing) = self
                .tracks
                .iter()
                .find(|t| t.path.canonicalize().ok() == dest_canon)
            {
                return Ok((existing.clone(), false));
            }
        }

        // Also handle the case where the source itself is already registered
        // (e.g. it lives inside the music directory under a different name).
        let source_canon = source.canonicalize().ok();
        if source_canon.is_some()
            && let Some(existing) = self
                .tracks
                .iter()
                .find(|t| t.path.canonicalize().ok() == source_canon)
        {
            return Ok((existing.clone(), false));
        }

        // Resolve the actual copy destination, generating a unique name when
        // a different file already occupies the default path.
        let dest = if default_dest.exists() {
            let stem = default_dest
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("track");
            let ext = default_dest
                .extension()
                .and_then(|s| s.to_str())
                .map(|e| format!(".{e}"))
                .unwrap_or_default();
            let mut i = 1u32;
            loop {
                let candidate = music_dir.join(format!("{stem}-{i}{ext}"));
                if !candidate.exists() {
                    break candidate;
                }
                i += 1;
            }
        } else {
            default_dest
        };

        fs::copy(source, &dest)
            .with_context(|| format!("copying {} -> {}", source.display(), dest.display()))?;

        let id = self.next_track_id;
        self.next_track_id += 1;

        let track = track_from_file(id, dest);
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
        let before = self.tracks.len();
        self.tracks.retain(|t| t.id != id);
        if self.tracks.len() != before {
            for pl in &mut self.playlists {
                pl.tracks.retain(|&tid| tid != id);
            }
            self.save()?;
        }
        Ok(())
    }

    /// Renames a track (display name only; does not touch the file).
    pub fn rename_track(&mut self, id: TrackId, new_name: impl Into<String>) -> Result<()> {
        if let Some(t) = self.tracks.iter_mut().find(|t| t.id == id) {
            t.name = new_name.into();
            self.save()?;
        }
        Ok(())
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
        let before = self.playlists.len();
        self.playlists.retain(|p| p.id != id);
        if self.playlists.len() != before {
            self.save()?;
        }
        Ok(())
    }

    /// Renames a user playlist.  Silently ignores "All Tracks".
    pub fn rename_playlist(&mut self, id: PlaylistId, name: impl Into<String>) -> Result<()> {
        if id == ALL_TRACKS_ID {
            return Ok(());
        }
        if let Some(pl) = self.playlists.iter_mut().find(|p| p.id == id) {
            pl.name = name.into();
            self.save()?;
        }
        Ok(())
    }

    /// Adds a track to a playlist (no-op if already present).
    pub fn playlist_add_track(&mut self, playlist_id: PlaylistId, track_id: TrackId) -> Result<()> {
        if let Some(pl) = self.playlists.iter_mut().find(|p| p.id == playlist_id)
            && !pl.tracks.contains(&track_id)
        {
            pl.tracks.push(track_id);
            self.save()?;
        }
        Ok(())
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
