use anyhow::{Context, Result};
use lofty::prelude::{Accessor, TaggedFileExt};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

// -- Identity ---------------------------------------------------------------

/// Opaque, stable identifier for a track.  Auto-incremented; never reused.
pub type TrackId = u64;

/// Opaque, stable identifier for a playlist.  Auto-incremented; never reused.
pub type PlaylistId = u64;

/// On-disk schema version of the index.
///
/// Bump this for any change that older audium versions cannot read, or that
/// this version cannot read from an older file.  There is deliberately no
/// migration path: [`Library::load`] sets an index of any other version aside
/// and starts fresh, and the music directory is re-scanned so the tracks come
/// back on their own.  Only playlists have to be rebuilt by hand.
const FORMAT_VERSION: u32 = 1;

/// Basename of the index inside the data directory.
///
/// Deliberately *not* `library.json`: every release up to 1.x wrote a
/// different, incompatible schema under that name.  Using a name they never
/// touched means neither version can read the other's file, so upgrading (or
/// downgrading) can never half-load a foreign index.
const INDEX_FILE: &str = "audium.json";

// -- Track ------------------------------------------------------------------

/// A single audio file registered in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    /// Display name: title tag if present, otherwise the filename stem.
    pub name: String,
    /// Absolute path to the audio file (inside $`XDG_DATA_HOME/audium/music`/ after import).
    pub path: PathBuf,

    // Optional metadata read from file tags on import.
    // All default to None so an index written before they existed still
    // deserializes cleanly.
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    /// Raw LRC text or plain lyrics, set by the user in-app.
    #[serde(default)]
    pub lyrics: Option<String>,
    /// Track length in seconds, read from the file's own headers at import.
    /// `None` for tracks registered before this was recorded, or for formats
    /// whose headers do not carry it.
    #[serde(default)]
    pub duration_secs: Option<u64>,
}

impl Track {
    /// Returns `"{artist} - {name}"` when an artist is set, otherwise `"{name}"`.
    pub fn display(&self) -> String {
        self.artist
            .as_deref()
            .filter(|s| !s.is_empty())
            .map_or_else(
                || self.name.clone(),
                |artist| format!("{artist} - {}", self.name),
            )
    }
}

// -- Tag reading ------------------------------------------------------------

struct FileTags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    lyrics: Option<String>,
    duration_secs: Option<u64>,
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
        lyrics: tags.as_ref().and_then(|t| t.lyrics.clone()),
        duration_secs: tags.as_ref().and_then(|t| t.duration_secs),
        path,
    }
}

fn read_file_tags(path: &Path) -> Option<FileTags> {
    use lofty::config::ParseOptions;
    use lofty::file::AudioFile;
    use lofty::probe::Probe;
    let tagged = Probe::open(path)
        .ok()?
        .options(ParseOptions::new().read_cover_art(false))
        .read()
        .ok()?;

    // Length comes from the container headers, which this probe already
    // parsed; it costs nothing extra and needs no decoding.
    let duration_secs = Some(tagged.properties().duration().as_secs()).filter(|&d| d > 0);

    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        // No tags at all, but the length is still worth keeping.
        return Some(FileTags {
            title: None,
            artist: None,
            album: None,
            lyrics: None,
            duration_secs,
        });
    };
    let nonempty = |s: String| if s.is_empty() { None } else { Some(s) };
    Some(FileTags {
        title: tag
            .title()
            .map(std::borrow::Cow::into_owned)
            .and_then(nonempty),
        artist: tag
            .artist()
            .map(std::borrow::Cow::into_owned)
            .and_then(nonempty),
        album: tag
            .album()
            .map(std::borrow::Cow::into_owned)
            .and_then(nonempty),
        // Either key may hold them depending on the container; ID3v2 uses
        // USLT, everything else the plain lyrics field.
        lyrics: tag
            .get_string(lofty::prelude::ItemKey::Lyrics)
            .or_else(|| tag.get_string(lofty::prelude::ItemKey::UnsyncLyrics))
            .map(ToString::to_string)
            .and_then(nonempty),
        duration_secs,
    })
}

/// Writes the editable fields back into the file's own tags.
///
/// The file is the record; `audium.json` only caches it. Anything typed in the
/// app therefore has to reach the tags, or re-importing the track (which any
/// rescan or a version bump does) would quietly restore the old values.
///
/// Writes into whichever tag the container treats as primary, which for every
/// format audium accepts is one that holds all four fields: `ID3v2` for MPEG,
/// WAV and AIFF, Vorbis comments for FLAC and Ogg, and an MP4 atom list for
/// M4A. Verified by writing each of those and reading them back.
fn write_file_tags(
    path: &Path,
    name: &str,
    artist: Option<&str>,
    album: Option<&str>,
    lyrics: Option<&str>,
) -> Result<()> {
    use lofty::config::{ParseOptions, WriteOptions};
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::prelude::ItemKey;
    use lofty::probe::Probe;
    use lofty::tag::Tag;

    // Errors reach a dialog, so they name the file rather than its whole path.
    let file = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    let mut tagged = Probe::open(path)
        .with_context(|| format!("opening {file}"))?
        .options(ParseOptions::new().read_cover_art(false))
        .read()
        .with_context(|| format!("reading tags from {file}"))?;

    // A file with no tag at all still needs one to write into; the primary
    // type is whichever the container natively carries.
    if tagged.primary_tag_mut().is_none() {
        let kind = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(kind));
    }
    let Some(tag) = tagged.primary_tag_mut() else {
        anyhow::bail!("{file} cannot hold tags");
    };

    tag.set_title(name.to_string());
    set_or_clear(tag, ItemKey::TrackArtist, artist);
    set_or_clear(tag, ItemKey::AlbumTitle, album);

    // ID3v2 has no unsynchronised-lyrics equivalent under `Lyrics`; it wants
    // USLT, which lofty exposes as a separate key. Writing the wrong one is
    // silently dropped, so pick by tag type.
    let lyrics_key = if tag.tag_type() == lofty::tag::TagType::Id3v2 {
        ItemKey::UnsyncLyrics
    } else {
        ItemKey::Lyrics
    };
    set_or_clear(tag, lyrics_key, lyrics);

    // Written to a copy and renamed into place, never edited where it lies.
    //
    // Growing a tag shifts everything after it, and the file may be open: the
    // decoder streams the track that is playing, and audium can edit exactly
    // that track. Rewriting underneath the reader moves the audio out from
    // under its file offset. A rename instead leaves the open file on the old
    // inode, so playback continues from what it already had.
    //
    // It is also the only way this is crash-safe. An interrupted in-place
    // rewrite leaves a truncated or half-shifted music file; an interrupted
    // copy leaves a stray temp file and an untouched original.
    //
    // The `.tmp` suffix keeps the scratch file out of `is_audio`, so a stray
    // one is never imported as a track.
    let tmp = path.with_extension("audium-tag.tmp");
    let write_via_tmp = || -> Result<()> {
        fs::copy(path, &tmp).with_context(|| format!("copying {file} to write its tags"))?;
        tagged
            .save_to_path(&tmp, WriteOptions::default())
            .with_context(|| format!("writing tags to {file}"))?;
        fs::rename(&tmp, path).with_context(|| format!("replacing {file}"))?;
        Ok(())
    };

    let result = write_via_tmp();
    if result.is_err() {
        // Best effort: the original is already intact, this only tidies up.
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Sets a tag item, or removes it when the value is empty: leaving the old
/// text in place would make clearing a field in the app look like it worked
/// until the next rescan put it back.
fn set_or_clear(tag: &mut lofty::tag::Tag, key: lofty::prelude::ItemKey, value: Option<&str>) {
    use lofty::tag::{ItemValue, TagItem};
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => {
            tag.insert(TagItem::new(key, ItemValue::Text(v.to_string())));
        }
        None => {
            tag.remove_key(key);
        }
    }
}

// -- Playlist ---------------------------------------------------------------

/// A named, ordered collection of track references.
///
/// A user-created playlist. The full library is not one of these -- it is
/// `Library::tracks` itself, surfaced separately in the sidebar.
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

// -- Library ----------------------------------------------------------------

/// Top-level persistent state: a registry of tracks + a set of playlists.
///
/// Invariants upheld at all times:
///  - Every playlist in `playlists` is user-created; the library as a whole is
///    `tracks`, never a playlist entry.
///  - Every `TrackId` inside any playlist exists in `tracks`.
///  - `next_track_id` / `next_playlist_id` are strictly increasing.
///  - `by_id` maps every track's id to its position in `tracks`.
///
/// `tracks` is public for iteration only.  Anything that adds or removes an
/// entry must go through this type's methods, which keep `by_id` in step.
#[derive(Debug, Serialize, Deserialize)]
pub struct Library {
    /// Schema version of this index; see [`FORMAT_VERSION`].
    #[serde(default)]
    version: u32,

    /// All registered tracks, keyed by insertion order.
    pub tracks: Vec<Track>,
    /// All user-created playlists.
    pub playlists: Vec<Playlist>,

    next_track_id: TrackId,
    next_playlist_id: PlaylistId,

    /// `TrackId` -> index into `tracks`.  Derived state, never persisted:
    /// without it every lookup is a linear scan, and the callers that resolve
    /// a whole playlist do one scan *per track*.
    ///
    /// `FxHash` rather than std's `SipHash`: the key is a `u64` we generate
    /// ourselves and this is read on every render.
    #[serde(skip)]
    by_id: FxHashMap<TrackId, usize>,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            tracks: Vec::new(),
            playlists: Vec::new(),
            next_track_id: 1,
            next_playlist_id: 1,
            by_id: FxHashMap::default(),
        }
    }
}

impl Library {
    // -- Filesystem paths -------------------------------------------------

    /// `$XDG_DATA_HOME/audium` -- the index and the imported audio files.
    pub fn data_dir() -> Result<PathBuf> {
        Self::xdg()
            .create_data_directory("")
            .context("could not determine data directory")
    }

    /// The XDG base directories for audium, used for every path we touch.
    fn xdg() -> xdg::BaseDirectories {
        xdg::BaseDirectories::with_prefix("audium")
    }

    /// Highest-priority existing copy of a config file: `$XDG_CONFIG_HOME`
    /// first, then each entry of `$XDG_CONFIG_DIRS`, so a distribution or
    /// sysadmin can ship defaults under `/etc/xdg/audium/`.
    ///
    /// Returns `None` when no copy exists anywhere; creates nothing.
    pub fn find_config_file(name: &str) -> Option<PathBuf> {
        Self::xdg().find_config_file(name)
    }

    /// Where a config file is *written*: always `$XDG_CONFIG_HOME/audium`,
    /// never a system directory. Creates the directory if needed.
    ///
    /// Config lives apart from the data directory so the spec's split holds:
    /// config is hand-editable and portable, data is ours to manage.
    pub fn place_config_file(name: &str) -> Result<PathBuf> {
        Self::xdg()
            .place_config_file(name)
            .context("could not determine config directory")
    }

    pub fn music_dir() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("music"))
    }

    fn index_path() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join(INDEX_FILE))
    }

    // -- Persistence ------------------------------------------------------

    /// Loads (or creates) the index from `$XDG_DATA_HOME/audium/audium.json`.
    /// Silently prunes tracks whose files have been deleted externally.
    pub fn load() -> Result<Self> {
        fs::create_dir_all(Self::data_dir()?)?;
        fs::create_dir_all(Self::music_dir()?)?;

        // No index, or one this version cannot read, still falls through to the
        // music-directory scan below rather than returning early -- that scan is
        // what rebuilds the collection, so short-circuiting it would leave a
        // fresh install (or a set-aside index) staring at an empty library with
        // its files sitting right there.
        let index = Self::index_path()?;
        let mut lib = if index.exists() {
            let raw = fs::read_to_string(&index)
                .with_context(|| format!("reading library index at {}", index.display()))?;
            serde_json::from_str::<Self>(&raw)
                .with_context(|| format!("parsing {INDEX_FILE}: the file may be corrupted"))?
        } else {
            Self::default()
        };

        // A foreign version is moved aside, never migrated and never deleted:
        // the old file stays on disk if anything needs recovering from it by
        // hand, and only playlists have to be rebuilt.
        if lib.version != FORMAT_VERSION {
            let aside = index.with_extension(format!("v{}.json", lib.version));
            fs::rename(&index, &aside).with_context(|| {
                format!("setting aside an unreadable index at {}", aside.display())
            })?;
            lib = Self::default();
        }

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
        let mut removed = FxHashSet::default();
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

        // Re-import any audio files sitting in the music directory but not
        // registered in the library, e.g. left behind after tracks were
        // pruned due to a stale recorded path.
        let known: FxHashSet<PathBuf> = lib
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
                lib.tracks.push(track_from_file(id, path));
                changed = true;
            }
        }

        // Backfill lengths for tracks registered before they were recorded.
        // Only files still missing one are probed (~0.1 ms each), so this is a
        // one-off cost that the next save makes permanent.
        for track in &mut lib.tracks {
            if track.duration_secs.is_none()
                && let Some(secs) = read_file_tags(&track.path).and_then(|f| f.duration_secs)
            {
                track.duration_secs = Some(secs);
                changed = true;
            }
        }

        lib.reindex();

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

    // -- Track management -------------------------------------------------

    /// Copies `source` into `$XDG_DATA_HOME/audium/music/` (if not already
    /// there) and registers it in the library.
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
        // registered: if so, the file was already imported and we're done.
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
        self.by_id.insert(id, self.tracks.len());
        self.tracks.push(track.clone());

        self.save()?;
        Ok((track, true))
    }

    /// Removes a track from the registry and from all playlists.
    /// Does NOT delete the file from disk.
    pub fn remove_track(&mut self, id: TrackId) -> Result<()> {
        let before = self.tracks.len();
        self.tracks.retain(|t| t.id != id);
        if self.tracks.len() != before {
            // Removal shifts every later index, so the whole map is rebuilt.
            self.reindex();
            for pl in &mut self.playlists {
                pl.tracks.retain(|&tid| tid != id);
            }
            self.save()?;
        }
        Ok(())
    }

    /// Replaces all user-editable metadata for a track (no file is touched).
    /// Replaces a track's user-editable metadata, in the index and in the
    /// file's own tags.
    ///
    /// The index is written first and always: it is what the UI reads, so an
    /// edit must survive on screen even when the file cannot be written (a
    /// read-only mount, a format with nowhere to put the field). The tag error
    /// is returned so the caller can say so, rather than leaving the user to
    /// discover on the next rescan that the edit did not stick.
    pub fn update_track_metadata(
        &mut self,
        id: TrackId,
        name: String,
        artist: Option<String>,
        album: Option<String>,
    ) -> Result<()> {
        let Some(t) = self.tracks.iter_mut().find(|t| t.id == id) else {
            return Ok(());
        };
        t.name = name;
        t.artist = artist;
        t.album = album;

        let (path, name, artist, album, lyrics) = (
            t.path.clone(),
            t.name.clone(),
            t.artist.clone(),
            t.album.clone(),
            t.lyrics.clone(),
        );
        self.save()?;
        write_file_tags(
            &path,
            &name,
            artist.as_deref(),
            album.as_deref(),
            lyrics.as_deref(),
        )
    }

    /// Replaces (or clears) the raw lyrics text for a track, in the index and
    /// in the file's own tags.  See [`Self::update_track_metadata`] for why the
    /// index is written even when the file write fails.
    pub fn set_track_lyrics(&mut self, id: TrackId, lyrics: Option<String>) -> Result<()> {
        let Some(t) = self.tracks.iter_mut().find(|t| t.id == id) else {
            return Ok(());
        };
        t.lyrics = lyrics;

        let (path, name, artist, album, lyrics) = (
            t.path.clone(),
            t.name.clone(),
            t.artist.clone(),
            t.album.clone(),
            t.lyrics.clone(),
        );
        self.save()?;
        write_file_tags(
            &path,
            &name,
            artist.as_deref(),
            album.as_deref(),
            lyrics.as_deref(),
        )
    }

    /// Returns a reference to a track by id.
    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.by_id.get(&id).and_then(|&i| self.tracks.get(i))
    }

    /// Rebuilds [`Self::by_id`].  Call after anything reorders `tracks` or
    /// changes its length; it is O(n) and those events are rare.
    fn reindex(&mut self) {
        self.by_id = self
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id, i))
            .collect();
    }

    // -- Playlist management ----------------------------------------------

    /// Creates a new user playlist.  Returns its id.
    pub fn create_playlist(&mut self, name: impl Into<String>) -> Result<PlaylistId> {
        let id = self.next_playlist_id;
        self.next_playlist_id += 1;
        self.playlists.push(Playlist::new(id, name));
        self.save()?;
        Ok(id)
    }

    /// Deletes a user playlist.
    pub fn delete_playlist(&mut self, id: PlaylistId) -> Result<()> {
        let before = self.playlists.len();
        self.playlists.retain(|p| p.id != id);
        if self.playlists.len() != before {
            self.save()?;
        }
        Ok(())
    }

    /// Renames a user playlist.
    pub fn rename_playlist(&mut self, id: PlaylistId, name: impl Into<String>) -> Result<()> {
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

    /// Drops a track from one playlist; the track itself stays in the library.
    pub fn playlist_remove_track(
        &mut self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        if let Some(pl) = self.playlists.iter_mut().find(|p| p.id == playlist_id) {
            pl.tracks.retain(|&tid| tid != track_id);
            self.save()?;
        }
        Ok(())
    }

    /// Moves the track at `index` one place towards the start or end of a
    /// playlist, and reports the index it ended up at.
    ///
    /// Returns `None` when the move would run off either end, so the caller
    /// can leave the cursor where it is rather than clamping it silently.
    pub fn playlist_move_track(
        &mut self,
        playlist_id: PlaylistId,
        index: usize,
        down: bool,
    ) -> Option<usize> {
        let pl = self.playlists.iter_mut().find(|p| p.id == playlist_id)?;
        let target = if down {
            index.checked_add(1).filter(|&i| i < pl.tracks.len())?
        } else {
            index.checked_sub(1)?
        };
        pl.tracks.swap(index, target);
        let _ = self.save();
        Some(target)
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
