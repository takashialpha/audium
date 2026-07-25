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
/// Bump for any change another version could not read. There is no migration
/// path: [`Library::load`] sets a foreign version aside and re-scans the music
/// directory, so only playlists have to be rebuilt by hand.
const FORMAT_VERSION: u32 = 1;

/// Basename of the index inside the data directory.
///
/// Must stay unique to this schema. A name shared with an incompatible format
/// could be opened by something that cannot read it and half-loaded.
const INDEX_FILE: &str = "audium.json";

// -- Track ------------------------------------------------------------------

/// A single audio file registered in the library.
///
/// Purely a runtime type: nothing here is serialized. On disk a track is just
/// its filename (see [`IndexFile`]), the `id` is an in-memory handle assigned
/// at load, and the metadata comes from the file's own tags. Identity that
/// survives a restart is therefore the filename, unique by the filesystem.
#[derive(Debug, Clone)]
pub struct Track {
    pub id: TrackId,
    /// Absolute path to the audio file, always inside the music directory.
    pub path: PathBuf,
    /// Display name: title tag if present, otherwise the filename stem.
    pub name: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Raw LRC text or plain lyrics.
    pub lyrics: Option<String>,
    /// Track length in seconds, from the file's headers.
    pub duration_secs: Option<u64>,
}

impl Track {
    /// A track known only by id and path; metadata is empty until read from
    /// the file with [`Self::reload_metadata`].
    const fn at(id: TrackId, path: PathBuf) -> Self {
        Self {
            id,
            path,
            name: String::new(),
            artist: None,
            album: None,
            lyrics: None,
            duration_secs: None,
        }
    }

    /// Replaces the metadata fields from the file's own tags, falling back to
    /// the filename stem for the display name.
    fn reload_metadata(&mut self) {
        let tags = read_file_tags(&self.path);
        self.name = tags
            .as_ref()
            .and_then(|t| t.title.clone())
            .or_else(|| {
                self.path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "Unknown".to_string());
        self.artist = tags.as_ref().and_then(|t| t.artist.clone());
        self.album = tags.as_ref().and_then(|t| t.album.clone());
        self.lyrics = tags.as_ref().and_then(|t| t.lyrics.clone());
        self.duration_secs = tags.as_ref().and_then(|t| t.duration_secs);
    }

    /// The track's on-disk identity: its filename within the music directory.
    pub fn filename(&self) -> String {
        self.path
            .file_name()
            .map_or_else(|| self.path.to_string_lossy(), |n| n.to_string_lossy())
            .into_owned()
    }

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
    let mut track = Track::at(id, path);
    track.reload_metadata();
    track
}

/// Reads every track's metadata from its file, in parallel: the tracks are
/// disjoint and the reads are pure I/O, so there is no shared state to guard.
fn load_metadata(tracks: &mut [Track]) {
    if tracks.is_empty() {
        return;
    }
    let threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let chunk = tracks.len().div_ceil(threads.max(1));
    std::thread::scope(|s| {
        for slice in tracks.chunks_mut(chunk) {
            s.spawn(|| {
                for t in slice {
                    t.reload_metadata();
                }
            });
        }
    });
}

/// The index one step from `index` in the given direction, or `None` at the
/// end: a refused move must leave the cursor where it is, not clamp to itself.
fn neighbour(index: usize, down: bool, len: usize) -> Option<usize> {
    if down {
        index.checked_add(1).filter(|&i| i < len)
    } else {
        index.checked_sub(1)
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

    // from the container headers this probe already parsed: no decoding needed
    let duration_secs = Some(tagged.properties().duration().as_secs()).filter(|&d| d > 0);

    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        // no tags at all, but the length is still worth keeping
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
/// The file is the record and `audium.json` only caches it, so anything typed
/// in the app has to reach the tags or the next rescan would restore the old
/// values. Written into whichever tag the container treats as primary; for
/// every format audium accepts that one holds all four fields.
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

    // errors reach a dialog, so they name the file rather than its whole path
    let file = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    let mut tagged = Probe::open(path)
        .with_context(|| format!("opening {file}"))?
        .options(ParseOptions::new().read_cover_art(false))
        .read()
        .with_context(|| format!("reading tags from {file}"))?;

    // a file with no tag still needs one to write into
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

    // ID3v2 wants USLT, a separate lofty key; the wrong one is dropped silently
    let lyrics_key = if tag.tag_type() == lofty::tag::TagType::Id3v2 {
        ItemKey::UnsyncLyrics
    } else {
        ItemKey::Lyrics
    };
    set_or_clear(tag, lyrics_key, lyrics);

    // Written to a copy and renamed, never edited in place. Growing a tag
    // shifts everything after it, and the decoder may be streaming this very
    // file: a rename leaves it reading the old inode, so playback survives. It
    // is also what makes this crash-safe, since an interrupted copy leaves the
    // original untouched. The `.tmp` suffix keeps the scratch file out of
    // `is_audio`, so a leftover one is never imported.
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
        // best effort: the original is already intact, this only tidies up
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

/// A user-created, named, ordered collection of tracks. The full library is
/// not one of these: it is `Library::tracks`, surfaced separately.
///
/// Runtime-only like [`Track`]. On disk a playlist is a name and an ordered
/// list of filenames, so the `id` needs no identity beyond this session.
#[derive(Debug, Clone)]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    /// Ordered track ids; resolved from filenames on load.
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

/// Top-level state: a registry of tracks plus the user's playlists.
///
/// Invariants: every `TrackId` in a playlist exists in `tracks`, the id
/// counters strictly increase, and `by_id` maps every track's id to its index.
/// `tracks` is public for iteration only -- adding or removing goes through
/// this type's methods, which keep `by_id` in step.
#[derive(Debug, Default)]
pub struct Library {
    /// All registered tracks, keyed by insertion order.
    pub tracks: Vec<Track>,
    /// All user-created playlists.
    pub playlists: Vec<Playlist>,

    /// In-memory id allocators. Ids are assigned fresh each load and never
    /// written, so these are ordinary counters, not persisted state.
    next_track_id: TrackId,
    next_playlist_id: PlaylistId,

    /// `TrackId` -> index into `tracks`. Derived state: without it resolving a
    /// playlist costs one linear scan *per track*, on every render.
    by_id: FxHashMap<TrackId, usize>,
}

/// The on-disk index: version, the ordered list of track filenames, and each
/// playlist as a name and an ordered list of track filenames. No ids, no
/// counters -- everything is keyed by the filename, which is hand-editable and
/// the same across instances.
#[derive(Serialize, Deserialize)]
struct IndexFile {
    version: u32,
    tracks: Vec<String>,
    playlists: Vec<PlaylistRepr>,
}

#[derive(Serialize, Deserialize)]
struct PlaylistRepr {
    name: String,
    tracks: Vec<String>,
}

impl Default for IndexFile {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            tracks: Vec::new(),
            playlists: Vec::new(),
        }
    }
}

/// Reads just the version, to name a set-aside index and to tell a foreign
/// schema apart from a corrupt file.
#[derive(Deserialize)]
struct VersionPeek {
    #[serde(default)]
    version: u32,
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

    /// Highest-priority existing copy of a state file, or `None`. Playback
    /// position and the queue are losable, so they live under
    /// `$XDG_STATE_HOME`, apart from data and preferences.
    pub fn find_state_file(name: &str) -> Option<PathBuf> {
        Self::xdg().find_state_file(name)
    }

    /// Where a state file is written: `$XDG_STATE_HOME/audium`. Creates the
    /// directory if needed.
    pub fn place_state_file(name: &str) -> Result<PathBuf> {
        Self::xdg()
            .place_state_file(name)
            .context("could not determine state directory")
    }

    /// Where a config file is *written*: always `$XDG_CONFIG_HOME/audium`,
    /// never a system directory. Creates the directory if needed.
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
        let music_dir = Self::music_dir()?;
        fs::create_dir_all(&music_dir)?;

        let index = Self::index_path()?;
        let (file, reset) = Self::read_index(&index)?;
        let mut changed = reset;

        // lossy filename -> real path. Matching on the lossy name keeps the
        // index valid JSON (a non-UTF-8 name cannot be a JSON string) while the
        // path keeps the real bytes, so such a file round-trips instead of
        // looking missing and being re-imported every launch.
        let mut present: FxHashMap<String, PathBuf> = FxHashMap::default();
        if let Ok(entries) = fs::read_dir(&music_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && crate::filepicker::is_audio(&path)
                    && let Some(name) = path.file_name()
                {
                    present.insert(name.to_string_lossy().into_owned(), path);
                }
            }
        }

        // The library's order is whatever the index lists (that still exists),
        // followed by any present file the index does not mention.
        let mut order: Vec<String> = Vec::with_capacity(present.len());
        let mut seen: FxHashSet<String> = FxHashSet::default();
        for name in file.tracks {
            if present.contains_key(&name) && seen.insert(name.clone()) {
                order.push(name);
            } else {
                // listed file vanished, or was listed twice
                changed = true;
            }
        }
        // dropped in rather than imported, in a deterministic order
        let mut extra: Vec<String> = present
            .keys()
            .filter(|n| !seen.contains(*n))
            .cloned()
            .collect();
        extra.sort_unstable();
        changed |= !extra.is_empty();
        order.append(&mut extra);

        // fresh ids, remembering the filename each maps to so playlists resolve
        // below. The path comes from the listing, so it keeps the real bytes.
        let mut by_name: FxHashMap<String, TrackId> = FxHashMap::default();
        let mut tracks: Vec<Track> = Vec::with_capacity(order.len());
        let mut next_track_id: TrackId = 1;
        for name in order {
            let path = present
                .get(&name)
                .cloned()
                .unwrap_or_else(|| music_dir.join(&name));
            let id = next_track_id;
            next_track_id += 1;
            by_name.insert(name.clone(), id);
            tracks.push(Track::at(id, path));
        }

        // the index carries no metadata, so read it from every file now
        load_metadata(&mut tracks);

        let mut next_playlist_id: PlaylistId = 1;
        let playlists: Vec<Playlist> = file
            .playlists
            .into_iter()
            .map(|p| {
                // Resolve filenames to ids, dropping any the library lacks and
                // any repeat: a track belongs to a playlist at most once, the
                // same invariant `playlist_add_track` keeps.
                let mut seen = FxHashSet::default();
                let resolved: Vec<TrackId> = p
                    .tracks
                    .iter()
                    .filter_map(|n| by_name.get(n).copied())
                    .filter(|&id| seen.insert(id))
                    .collect();
                changed |= resolved.len() != p.tracks.len();
                let id = next_playlist_id;
                next_playlist_id += 1;
                Playlist {
                    id,
                    name: p.name,
                    tracks: resolved,
                }
            })
            .collect();

        let mut lib = Self {
            tracks,
            playlists,
            next_track_id,
            next_playlist_id,
            by_id: FxHashMap::default(),
        };
        lib.reindex();

        if changed {
            lib.save()?;
        }

        Ok(lib)
    }

    /// Reads the on-disk index, or sets it aside and starts fresh.
    ///
    /// A foreign version or a corrupt file is renamed, never migrated or
    /// deleted, so the old copy stays for hand recovery. Returns the index and
    /// whether a reset happened, which tells the caller to rewrite it.
    fn read_index(index: &Path) -> Result<(IndexFile, bool)> {
        if !index.exists() {
            return Ok((IndexFile::default(), false));
        }
        let raw = fs::read_to_string(index)
            .with_context(|| format!("reading library index at {}", index.display()))?;
        if let Ok(file) = serde_json::from_str::<IndexFile>(&raw)
            && file.version == FORMAT_VERSION
        {
            return Ok((file, false));
        }
        let version = serde_json::from_str::<VersionPeek>(&raw).map_or(0, |v| v.version);
        let aside = index.with_extension(format!("v{version}.json"));
        fs::rename(index, &aside)
            .with_context(|| format!("setting aside an unreadable index at {}", aside.display()))?;
        Ok((IndexFile::default(), true))
    }

    pub fn save(&self) -> Result<()> {
        // runtime ids become filenames on disk
        let file = IndexFile {
            version: FORMAT_VERSION,
            tracks: self.tracks.iter().map(Track::filename).collect(),
            playlists: self
                .playlists
                .iter()
                .map(|p| PlaylistRepr {
                    name: p.name.clone(),
                    tracks: p
                        .tracks
                        .iter()
                        .filter_map(|&id| self.track(id).map(Track::filename))
                        .collect(),
                })
                .collect(),
        };

        let index = Self::index_path()?;
        let tmp = index.with_extension("json.tmp");
        let raw = serde_json::to_string_pretty(&file)?;
        fs::write(&tmp, &raw)
            .with_context(|| format!("writing library index to {}", tmp.display()))?;
        fs::rename(&tmp, &index)
            .with_context(|| format!("replacing library index at {}", index.display()))?;
        Ok(())
    }

    // -- Track management -------------------------------------------------

    /// Copies `source` into the music directory (if not already there) and
    /// registers it. Idempotent: `is_new` is false if it was already present.
    pub fn add_file(&mut self, source: &Path) -> Result<(Track, bool)> {
        crate::player::validate_decodable(source)
            .with_context(|| format!("cannot import \"{}\"", source.display()))?;

        let music_dir = Self::music_dir()?;

        let filename = source.file_name().context("source path has no filename")?;
        let default_dest = music_dir.join(filename);

        // already at the default destination and registered: nothing to do
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

        // or registered under a different name inside the music directory
        let source_canon = source.canonicalize().ok();
        if source_canon.is_some()
            && let Some(existing) = self
                .tracks
                .iter()
                .find(|t| t.path.canonicalize().ok() == source_canon)
        {
            return Ok((existing.clone(), false));
        }

        // a *different* file holds the default path: pick a free name
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

    /// Removes a track from the library, and deletes audium's copy of the file.
    ///
    /// The music directory holds copies audium made on import, never the
    /// originals, and the load-time scan re-imports whatever is left in it, so
    /// a removal that spared the file would undo itself on the next launch.
    /// Best effort: if the delete fails the track is still gone this session.
    pub fn remove_track(&mut self, id: TrackId) -> Result<()> {
        let Some(pos) = self.tracks.iter().position(|t| t.id == id) else {
            return Ok(());
        };
        let path = self.tracks.remove(pos).path;
        // removal shifts every later index
        self.reindex();
        for pl in &mut self.playlists {
            pl.tracks.retain(|&tid| tid != id);
        }
        let _ = fs::remove_file(&path);
        self.save()
    }

    /// Replaces a track's user-editable metadata, in the index and in the
    /// file's own tags.
    ///
    /// The index is written first and always, since it is what the UI reads:
    /// an edit must survive on screen even when the file cannot be written.
    /// The tag error is returned so the caller can say so, rather than let the
    /// user find out at the next rescan that the edit did not stick.
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

    /// Replaces (or clears) a track's raw lyrics, in the index and in the
    /// file's tags. See [`Self::update_track_metadata`] for the write order.
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

    /// Finds a track by its filename. Used to resolve saved playback state at
    /// startup, which is a one-off, so a linear scan is fine.
    pub fn track_by_filename(&self, filename: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.filename() == filename)
    }

    /// Returns a reference to a track by id.
    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.by_id.get(&id).and_then(|&i| self.tracks.get(i))
    }

    /// Rebuilds [`Self::by_id`]. Call after anything reorders `tracks` or
    /// changes its length; O(n), and those events are rare.
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
        let target = neighbour(index, down, pl.tracks.len())?;
        pl.tracks.swap(index, target);
        let _ = self.save();
        Some(target)
    }

    /// Reorders the whole library. Playlists reference tracks by id, not
    /// position, so this changes display order only.
    pub fn move_track(&mut self, index: usize, down: bool) -> Option<usize> {
        let target = neighbour(index, down, self.tracks.len())?;
        self.tracks.swap(index, target);
        self.reindex();
        let _ = self.save();
        Some(target)
    }

    /// Reorders the sidebar's list of playlists.
    pub fn move_playlist(&mut self, index: usize, down: bool) -> Option<usize> {
        let target = neighbour(index, down, self.playlists.len())?;
        self.playlists.swap(index, target);
        let _ = self.save();
        Some(target)
    }

    /// Returns a reference to a playlist by id.
    pub fn playlist(&self, id: PlaylistId) -> Option<&Playlist> {
        self.playlists.iter().find(|p| p.id == id)
    }

    /// The tracks of a playlist, in order; dangling ids are skipped.
    pub fn playlist_tracks(&self, playlist_id: PlaylistId) -> Vec<&Track> {
        self.playlist(playlist_id)
            .map(|pl| pl.tracks.iter().filter_map(|id| self.track(*id)).collect())
            .unwrap_or_default()
    }
}
