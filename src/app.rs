use anyhow::Result;
use rand::seq::SliceRandom;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

use crate::{
    cli::Cli,
    filepicker::{FilePicker, FilePickerOutcome},
    library::{ALL_TRACKS_ID, Library, PlaylistId, Track, TrackId},
    lyrics,
    modal::{
        Modal, ModalConfirm, ModalOutcome, RemoveTarget, SettingsState, TextInput,
        make_lyrics_textarea,
    },
    numeric,
    player::{PlayerEvent, PlayerHandle, resolve_duration, spawn_audio_thread},
    settings::{ColorMode, Settings},
    ui,
    ui::layout::{Theme, console_theme, theme_by_name},
};

// ── Terminal color detection ───────────────────────────────────────────────

/// Best-effort detection of 24-bit truecolor support.
///
/// There is no portable query for this, so we use the widely-agreed
/// heuristics: `NO_COLOR` disables color entirely, a real Linux tty
/// (`TERM=linux`) or `TERM=dumb` is 16-color only, and `COLORTERM` set to
/// `truecolor`/`24bit` is the standard opt-in signal.  Misdetection is
/// recoverable from the settings menu via the Color mode override.
fn detect_truecolor() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if let Ok("linux" | "dumb") = std::env::var("TERM").as_deref() {
        return false;
    }
    matches!(
        std::env::var("COLORTERM").as_deref(),
        Ok("truecolor" | "24bit")
    )
}

/// Resolves the live theme for the current color state.
///
/// With truecolor active the selected RGB theme is used (honoring
/// transparency); otherwise the named-ANSI console theme is substituted while
/// the user's real theme choice is left untouched in settings.
fn resolve_theme(theme_name: &str, transparent: bool, truecolor: bool) -> Theme {
    if truecolor {
        let mut t = theme_by_name(theme_name).clone();
        t.transparent = transparent;
        t
    } else {
        console_theme().clone()
    }
}

// ── Playback speed ─────────────────────────────────────────────────────────

const SPEED_STEP: f32 = 0.01;
const SPEED_MIN: f32 = 0.05;
const SPEED_MAX: f32 = 3.0;

// - LoopMode -

/// Playback loop mode, cycled with `l`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoopMode {
    #[default]
    Off,
    /// Restart from the first queue entry when the last track finishes.
    Queue,
    /// Replay the current track indefinitely.
    Track,
}

impl LoopMode {
    pub const fn cycle(self) -> Self {
        match self {
            Self::Off => Self::Queue,
            Self::Queue => Self::Track,
            Self::Track => Self::Off,
        }
    }
}

// ── Focus ──────────────────────────────────────────────────────────────────

/// Which panel currently owns keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    TrackList,
    Queue,
}

impl Focus {
    pub const fn cycle(self) -> Self {
        match self {
            Self::Sidebar => Self::TrackList,
            Self::TrackList => Self::Queue,
            Self::Queue => Self::Sidebar,
        }
    }
}

// ── AppState ───────────────────────────────────────────────────────────────

pub struct AppState {
    pub library: Library,
    pub player: PlayerHandle,

    // ── UI ──────────────────────────────────────────────────────────────
    pub focus: Focus,

    /// Which playlist is currently displayed in the tracklist panel.
    pub active_playlist: PlaylistId,

    /// Cursor inside the sidebar (playlist list).
    pub sidebar_cursor: usize,
    /// Cursor inside the tracklist (tracks of `active_playlist`).
    pub tracklist_cursor: usize,
    /// Cursor inside the queue panel.
    pub queue_cursor: usize,

    // ── Playback queue ──────────────────────────────────────────────────
    /// Ephemeral ordered list of tracks waiting to be played.
    pub queue: Vec<Track>,
    /// Index of the currently playing track inside `queue`, if any.
    pub now_playing: Option<usize>,

    // ── Progress tracking ───────────────────────────────────────────────
    pub track_start: Option<Instant>,
    pub seek_offset: Duration,
    pub track_duration: Option<Duration>,

    // ── Overlay state ───────────────────────────────────────────────────
    pub modal: Option<Modal>,
    pub file_picker: Option<FilePicker>,
    pub loop_mode: LoopMode,
    pub theme: Theme,
    pub settings: Settings,
    pub should_quit: bool,

    // ── Tracklist filter ─────────────────────────────────────────────────
    /// Current filter string applied to the active playlist's track list.
    pub tracklist_filter: String,
    /// Whether the filter bar is currently receiving keyboard input.
    pub filter_active: bool,

    // ── Lyrics overlay ───────────────────────────────────────────────────
    /// Whether the lyrics overlay is visible.
    pub show_lyrics: bool,
    /// Manual scroll offset for plain-text (unsynced) lyrics.
    pub lyrics_scroll: usize,
    /// Pre-parsed lyrics for the current track, updated on open/track-change/save.
    pub lyrics_lines: Vec<lyrics::LyricLine>,
    /// Cached track IDs for the active playlist + filter, rebuilt on every mutation.
    filtered_ids: Vec<TrackId>,
}

impl AppState {
    pub fn new(library: Library, player: PlayerHandle, settings: Settings) -> Self {
        let theme = resolve_theme(
            &settings.theme_name,
            settings.transparent,
            settings.color_mode.truecolor(detect_truecolor()),
        );
        let mut s = Self {
            library,
            player,
            focus: Focus::Sidebar,
            active_playlist: ALL_TRACKS_ID,
            sidebar_cursor: 0,
            tracklist_cursor: 0,
            queue_cursor: 0,
            queue: Vec::new(),
            now_playing: None,
            track_start: None,
            seek_offset: Duration::ZERO,
            track_duration: None,
            modal: None,
            file_picker: None,
            loop_mode: LoopMode::Off,
            theme,
            settings,
            should_quit: false,
            tracklist_filter: String::new(),
            filter_active: false,
            show_lyrics: false,
            lyrics_scroll: 0,
            lyrics_lines: Vec::new(),
            filtered_ids: Vec::new(),
        };
        s.rebuild_filter_cache();
        s
    }

    // ── Progress ─────────────────────────────────────────────────────────

    pub fn elapsed(&self) -> Duration {
        if self.now_playing.is_none() {
            return Duration::ZERO;
        }
        if self.player.is_paused {
            return self.seek_offset;
        }
        let wall = self.track_start.map_or(Duration::ZERO, |s| s.elapsed());
        self.seek_offset + wall.mul_f32(self.player.playback_speed)
    }

    pub fn progress_ratio(&self) -> f64 {
        let elapsed = self.elapsed().as_secs_f64();
        match self.track_duration {
            Some(d) if d.as_secs_f64() > 0.0 => (elapsed / d.as_secs_f64()).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    // ── Queue helpers ─────────────────────────────────────────────────────

    pub fn enqueue(&mut self, track: Track) {
        self.queue.push(track);
    }

    /// Starts playing `queue[idx]`.  The duration is resolved synchronously
    /// from the file headers on the UI thread (fast; just reads a few bytes).
    pub fn play_queue_index(&mut self, idx: usize) {
        if idx >= self.queue.len() {
            return;
        }
        let path = self.queue[idx].path.clone();
        self.player.play(path.clone());
        self.now_playing = Some(idx);
        self.track_start = Some(Instant::now());
        self.seek_offset = Duration::ZERO;
        self.track_duration = resolve_duration(&path);
        if self.show_lyrics {
            self.lyrics_scroll = 0;
            self.refresh_lyrics_cache();
        }
    }

    pub fn play_next(&mut self) {
        match self.loop_mode {
            LoopMode::Track => {
                // Replay the same index; if now_playing is somehow None,
                // fall back to starting from 0.
                let idx = self.now_playing.unwrap_or(0);
                self.play_queue_index(idx);
            }
            LoopMode::Queue => {
                let next = self.now_playing.map_or(0, |i| i + 1);
                // Wrap around to the first track instead of halting.
                let idx = if next < self.queue.len() { next } else { 0 };
                if !self.queue.is_empty() {
                    self.play_queue_index(idx);
                }
            }
            LoopMode::Off => {
                let next = self.now_playing.map_or(0, |i| i + 1);
                if next < self.queue.len() {
                    self.play_queue_index(next);
                } else {
                    self.halt_playback();
                }
            }
        }
    }

    pub fn play_prev(&mut self) {
        match self.loop_mode {
            LoopMode::Track => {
                let idx = self.now_playing.unwrap_or(0);
                self.play_queue_index(idx);
            }
            LoopMode::Queue => {
                if !self.queue.is_empty() {
                    let idx = match self.now_playing {
                        None | Some(0) => self.queue.len() - 1,
                        Some(i) => i - 1,
                    };
                    self.play_queue_index(idx);
                }
            }
            LoopMode::Off => {
                if let Some(cur) = self.now_playing
                    && cur > 0
                {
                    self.play_queue_index(cur - 1);
                }
            }
        }
    }

    /// Stops playback and resets all progress state to idle.
    /// Use this everywhere a track is forcibly interrupted (removal, queue
    /// exhaustion, etc.) so the player bar always shows a clean 0:00 / -:--.
    fn halt_playback(&mut self) {
        self.now_playing = None;
        self.track_start = None;
        self.seek_offset = Duration::ZERO;
        self.track_duration = None;
        self.player.stop();
    }

    // ── Active playlist helpers ──────────────────────────────────────────

    /// Rebuilds the filtered track ID cache. Call after any change to the
    /// active playlist, filter string, or library track list.
    pub fn rebuild_filter_cache(&mut self) {
        let all = self.library.playlist_tracks(self.active_playlist);
        if self.tracklist_filter.is_empty() {
            self.filtered_ids = all.iter().map(|t| t.id).collect();
            return;
        }
        let q = self.tracklist_filter.to_lowercase();
        self.filtered_ids = all
            .iter()
            .filter(|t| {
                t.name.to_lowercase().contains(&q)
                    || t.artist
                        .as_deref()
                        .is_some_and(|s| s.to_lowercase().contains(&q))
                    || t.album
                        .as_deref()
                        .is_some_and(|s| s.to_lowercase().contains(&q))
                    || t.genre
                        .as_deref()
                        .is_some_and(|s| s.to_lowercase().contains(&q))
                    || t.year.is_some_and(|y| y.to_string().contains(&*q))
            })
            .map(|t| t.id)
            .collect();
    }

    /// Returns the filtered tracks for the active playlist from the cached ID list.
    pub fn active_tracks(&self) -> Vec<&Track> {
        self.filtered_ids
            .iter()
            .filter_map(|&id| self.library.track(id))
            .collect()
    }

    fn selected_track(&self) -> Option<Track> {
        self.active_tracks()
            .get(self.tracklist_cursor)
            .map(|t| (*t).clone())
    }

    // ── Tick ─────────────────────────────────────────────────────────────

    /// Processes player events and auto-advances on natural track end.
    pub fn tick(&mut self) {
        for event in self.player.drain_events() {
            match event {
                PlayerEvent::TrackFinished => self.play_next(),
                PlayerEvent::Error(msg) => {
                    self.modal = Some(Modal::Notify { message: msg });
                }
            }
        }
    }

    // ── Input ─────────────────────────────────────────────────────────────

    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if self.handle_quit_shortcut(code, modifiers) {
            return;
        }
        if self.handle_file_picker_key(code) {
            return;
        }
        if self.handle_modal_key(code, modifiers) {
            return;
        }
        if self.handle_lyrics_overlay_key(code) {
            return;
        }
        if self.handle_filter_key(code) {
            return;
        }
        self.handle_global_key(code);
    }

    // ── Ctrl-C: same as 'q' (ask to quit, confirm on a second press) ──────
    fn handle_quit_shortcut(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        if code != KeyCode::Char('c') || !modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        if matches!(self.modal, Some(Modal::ConfirmQuit)) {
            self.modal = None;
            self.apply_modal_confirm(ModalConfirm::Quit);
        } else {
            self.file_picker = None;
            self.modal = Some(Modal::ConfirmQuit);
        }
        true
    }

    // ── File picker takes priority over everything else ───────────────────
    fn handle_file_picker_key(&mut self, code: KeyCode) -> bool {
        let Some(picker) = &mut self.file_picker else {
            return false;
        };
        match picker.handle_key(code) {
            FilePickerOutcome::Continue => {}
            FilePickerOutcome::Dismissed => self.file_picker = None,
            FilePickerOutcome::Selected(path) => {
                self.file_picker = None;
                self.import_file(&path);
            }
        }
        true
    }

    // ── Modal intercepts next ──────────────────────────────────────────────
    fn handle_modal_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        let Some(modal) = &mut self.modal else {
            return false;
        };
        match modal.handle_key(code, modifiers) {
            ModalOutcome::Consumed => {}
            ModalOutcome::Dismissed => self.modal = None,
            ModalOutcome::Confirm(c) => {
                if !matches!(c, ModalConfirm::PreviewTheme { .. }) {
                    self.modal = None;
                }
                self.apply_modal_confirm(c);
            }
        }
        true
    }

    // ── Lyrics overlay intercepts when visible, no modal open ─────────────
    const fn handle_lyrics_overlay_key(&mut self, code: KeyCode) -> bool {
        if !self.show_lyrics || self.modal.is_some() || self.file_picker.is_some() {
            return false;
        }
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.lyrics_scroll = self.lyrics_scroll.saturating_add(1);
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.lyrics_scroll = self.lyrics_scroll.saturating_sub(1);
                true
            }
            KeyCode::Esc | KeyCode::Char('y') => {
                self.show_lyrics = false;
                true
            }
            _ => false,
        }
    }

    // ── Filter input (captures printable chars when active) ───────────────
    fn handle_filter_key(&mut self, code: KeyCode) -> bool {
        if !self.filter_active {
            return false;
        }
        match code {
            KeyCode::Esc => {
                self.tracklist_filter.clear();
                self.filter_active = false;
                self.tracklist_cursor = 0;
                self.rebuild_filter_cache();
                true
            }
            KeyCode::Backspace => {
                self.tracklist_filter.pop();
                self.tracklist_cursor = 0;
                self.rebuild_filter_cache();
                true
            }
            KeyCode::Char(c) => {
                self.tracklist_filter.push(c);
                self.tracklist_cursor = 0;
                self.rebuild_filter_cache();
                true
            }
            // Enter exits typing mode; falls through to action_enter below.
            KeyCode::Enter => {
                self.filter_active = false;
                false
            }
            // All other keys (navigation, playback shortcuts) fall through.
            _ => false,
        }
    }

    // ── Global keybindings ─────────────────────────────────────────────────
    fn handle_global_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') => self.modal = Some(Modal::ConfirmQuit),
            KeyCode::Char('?') => self.modal = Some(Modal::Help),

            // Playback
            KeyCode::Char(' ') => self.action_toggle_play(),
            KeyCode::Char('n') => self.play_next(),
            KeyCode::Char('N') => self.play_prev(),
            KeyCode::Left => self.action_seek(-self.settings.seek_step_secs.cast_signed()),
            KeyCode::Right => self.action_seek(self.settings.seek_step_secs.cast_signed()),
            KeyCode::Char('+' | '=') => self.player.volume_up(),
            KeyCode::Char('-') => self.player.volume_down(),

            // Navigation
            KeyCode::Tab => {
                self.focus = self.focus.cycle();
                self.filter_active = false;
                self.tracklist_filter.clear();
                self.tracklist_cursor = 0;
                self.rebuild_filter_cache();
            }
            KeyCode::Char('j') | KeyCode::Down => self.cursor_down(),
            KeyCode::Char('k') | KeyCode::Up => self.cursor_up(),

            // Context actions
            KeyCode::Enter => self.action_enter(),
            KeyCode::Char('a') => self.action_add_to_queue(),
            KeyCode::Char('p') => self.action_add_to_playlist(),
            KeyCode::Char('d') => self.action_remove(),
            KeyCode::Char('D') => self.action_clear_queue(),
            KeyCode::Char('r') => self.action_rename(),
            KeyCode::Char('c') => self.action_new_playlist(),
            KeyCode::Char('f') => self.action_open_filepicker(),
            KeyCode::Char('m') => self.action_open_menu(),
            KeyCode::Char('l') => self.loop_mode = self.loop_mode.cycle(),
            KeyCode::Char('z') => self.action_shuffle_playlist(),
            KeyCode::Char('[') => self.action_speed_down(),
            KeyCode::Char(']') => self.action_speed_up(),
            KeyCode::Char('e') => self.action_edit_metadata(),
            KeyCode::Char('y') => self.action_toggle_lyrics(),
            KeyCode::Char('/') if self.focus == Focus::TrackList => {
                self.filter_active = true;
            }

            _ => {}
        }
    }

    // ── Cursor movement ───────────────────────────────────────────────────

    fn cursor_down(&mut self) {
        match self.focus {
            Focus::Sidebar => {
                let len = self.library.playlists.len();
                if len > 0 {
                    self.sidebar_cursor = (self.sidebar_cursor + 1).min(len - 1);
                    self.sync_active_playlist();
                }
            }
            Focus::TrackList => {
                let len = self.active_tracks().len();
                if len > 0 {
                    self.tracklist_cursor = (self.tracklist_cursor + 1).min(len - 1);
                }
            }
            Focus::Queue => {
                let len = self.queue.len();
                if len > 0 {
                    self.queue_cursor = (self.queue_cursor + 1).min(len - 1);
                }
            }
        }
    }

    fn cursor_up(&mut self) {
        match self.focus {
            Focus::Sidebar => {
                self.sidebar_cursor = self.sidebar_cursor.saturating_sub(1);
                self.sync_active_playlist();
            }
            Focus::TrackList => {
                self.tracklist_cursor = self.tracklist_cursor.saturating_sub(1);
            }
            Focus::Queue => {
                self.queue_cursor = self.queue_cursor.saturating_sub(1);
            }
        }
    }

    /// Keeps `active_playlist` in sync with `sidebar_cursor`.
    fn sync_active_playlist(&mut self) {
        if let Some(pl) = self.library.playlists.get(self.sidebar_cursor) {
            self.active_playlist = pl.id;
            self.tracklist_cursor = 0;
            self.tracklist_filter.clear();
            self.filter_active = false;
            self.rebuild_filter_cache();
        }
    }

    // ── Actions ───────────────────────────────────────────────────────────

    fn action_toggle_play(&mut self) {
        if self.now_playing.is_none() {
            if !self.queue.is_empty() {
                self.play_queue_index(0);
            }
        } else if self.player.is_paused {
            // Currently paused → resume.
            self.track_start = Some(Instant::now());
            self.player.resume();
        } else {
            // Currently playing → pause.
            // Snapshot elapsed before setting is_paused so elapsed() still
            // accumulates track_start.elapsed() during this call.
            self.seek_offset = self.elapsed();
            self.track_start = None;
            self.player.pause();
        }
    }

    /// Seek by `delta_secs` seconds (negative = rewind, positive = forward).
    /// No-op if nothing is playing or the track path is unavailable.
    fn action_seek(&mut self, delta_secs: i64) {
        let Some(idx) = self.now_playing else {
            return;
        };
        let Some(track) = self.queue.get(idx) else {
            return;
        };
        let path = track.path.clone();

        // Compute new position, clamped to [0, duration].
        let current = self.elapsed().as_secs().cast_signed();
        let max_secs = self
            .track_duration
            .map_or(i64::MAX, |d| d.as_secs().saturating_sub(1).cast_signed());
        let target_secs = (current + delta_secs).clamp(0, max_secs).cast_unsigned();
        let target = Duration::from_secs(target_secs);

        // Update UI-side clock immediately so the bar moves on the next frame.
        self.seek_offset = target;
        self.track_start = if self.player.is_paused {
            None
        } else {
            Some(Instant::now())
        };

        // Tell the audio thread to reopen, seek, and continue (or stay paused).
        self.player.seek(path, target, self.player.is_paused);
    }

    /// Enter on tracklist  →  play immediately (inserts after current).
    /// Enter on queue      →  play that queue entry immediately.
    fn action_enter(&mut self) {
        match self.focus {
            Focus::Sidebar => {
                self.sync_active_playlist();
                self.focus = Focus::TrackList;
            }
            Focus::TrackList => {
                if let Some(track) = self.selected_track() {
                    let insert_at = self.now_playing.map_or(0, |i| i + 1);
                    self.queue.insert(insert_at, track);
                    self.play_queue_index(insert_at);
                }
            }
            Focus::Queue => {
                let idx = self.queue_cursor;
                self.play_queue_index(idx);
            }
        }
    }

    fn action_add_to_queue(&mut self) {
        if let Some(track) = self.selected_track() {
            self.queue.push(track);
        }
    }

    fn action_add_to_playlist(&mut self) {
        if let Some(track) = self.selected_track() {
            let choices: Vec<(u64, String)> = self
                .library
                .playlists
                .iter()
                .filter(|p| p.id != ALL_TRACKS_ID)
                .map(|p| (p.id, p.name.clone()))
                .collect();

            self.modal = Some(Modal::AddToPlaylist {
                track_id: track.id,
                track_name: track.name,
                choices,
                cursor: 0,
            });
        }
    }

    fn action_remove(&mut self) {
        match self.focus {
            Focus::Sidebar => {
                if let Some(pl) = self.library.playlists.get(self.sidebar_cursor) {
                    if pl.id == ALL_TRACKS_ID {
                        return; // cannot delete "All Tracks"
                    }
                    self.modal = Some(Modal::ConfirmRemove {
                        description: format!("Delete playlist \"{}\"?", pl.name),
                        target: RemoveTarget::Playlist { playlist_id: pl.id },
                    });
                }
            }
            Focus::TrackList => {
                if let Some(track) = self.selected_track() {
                    self.modal = Some(Modal::ConfirmRemove {
                        description: format!("Remove \"{}\" from library?", track.name),
                        target: RemoveTarget::TrackFromLibrary { track_id: track.id },
                    });
                }
            }
            Focus::Queue => {
                if self.queue_cursor < self.queue.len() {
                    self.modal = Some(Modal::ConfirmRemove {
                        description: "Remove this track from the queue?".into(),
                        target: RemoveTarget::TrackFromQueue {
                            queue_idx: self.queue_cursor,
                        },
                    });
                }
            }
        }
    }

    fn action_clear_queue(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        self.modal = Some(Modal::ConfirmRemove {
            description: "Clear the entire queue?".into(),
            target: RemoveTarget::Queue,
        });
    }

    fn action_rename(&mut self) {
        match self.focus {
            Focus::Sidebar => {
                if let Some(pl) = self.library.playlists.get(self.sidebar_cursor) {
                    if pl.id == ALL_TRACKS_ID {
                        return;
                    }
                    self.modal = Some(Modal::Rename {
                        kind: "Playlist".into(),
                        id: pl.id,
                        input: TextInput::with_value(&pl.name),
                    });
                }
            }
            Focus::TrackList => {
                if let Some(t) = self.selected_track() {
                    self.modal = Some(Modal::Rename {
                        kind: "Track".into(),
                        id: t.id,
                        input: TextInput::with_value(&t.name),
                    });
                }
            }
            Focus::Queue => {
                if let Some(t) = self.queue.get(self.queue_cursor).cloned() {
                    self.modal = Some(Modal::Rename {
                        kind: "Track".into(),
                        id: t.id,
                        input: TextInput::with_value(&t.name),
                    });
                }
            }
        }
    }

    fn action_new_playlist(&mut self) {
        self.modal = Some(Modal::NewPlaylist {
            input: TextInput::default(),
        });
    }

    fn action_open_filepicker(&mut self) {
        let start = std::env::var_os("HOME").map_or_else(|| "/".into(), std::path::PathBuf::from);
        self.file_picker = Some(FilePicker::new(start));
    }

    fn action_open_menu(&mut self) {
        self.modal = Some(Modal::Menu { cursor: 0 });
    }

    fn open_settings(&mut self) {
        let vol_pct = numeric::ratio_to_whole_percent(self.settings.default_volume);
        let preview_theme_idx = crate::ui::layout::themes()
            .iter()
            .position(|t| t.name == self.settings.theme_name.as_str())
            .unwrap_or(0);
        self.modal = Some(Modal::Settings(SettingsState {
            cursor: 0,
            volume_pct: vol_pct,
            seek_secs: self.settings.seek_step_secs,
            preview_theme_idx,
            transparent: self.settings.transparent,
            color_mode: self.settings.color_mode,
            detected_truecolor: detect_truecolor(),
        }));
    }

    fn open_about(&mut self) {
        self.modal = Some(Modal::About);
    }

    /// `z`: prompt to shuffle the active playlist into the queue.
    fn action_shuffle_playlist(&mut self) {
        if let Some(pl) = self.library.playlist(self.active_playlist) {
            if pl.tracks.is_empty() {
                return;
            }
            self.modal = Some(Modal::ShufflePlaylist {
                playlist_id: pl.id,
                playlist_name: pl.name.clone(),
            });
        }
    }

    // ── Playback speed ────────────────────────────────────────────────────

    fn action_speed_up(&mut self) {
        let new = ((self.player.playback_speed + SPEED_STEP) * 100.0).round() / 100.0;
        self.change_speed(new.min(SPEED_MAX));
    }

    fn action_speed_down(&mut self) {
        let new = ((self.player.playback_speed - SPEED_STEP) * 100.0).round() / 100.0;
        self.change_speed(new.max(SPEED_MIN));
    }

    fn change_speed(&mut self, new_speed: f32) {
        if (new_speed - self.player.playback_speed).abs() < 0.001 {
            return;
        }
        // Snapshot track position with the OLD speed before changing.
        let current_pos = if self.now_playing.is_some() {
            Some(self.elapsed())
        } else {
            None
        };
        self.player.playback_speed = new_speed;
        // Re-seek to the same position so the new speed takes effect immediately.
        if let Some(pos) = current_pos
            && let Some(np) = self.now_playing
            && let Some(track) = self.queue.get(np)
        {
            let path = track.path.clone();
            let paused = self.player.is_paused;
            self.seek_offset = pos;
            self.track_start = if paused { None } else { Some(Instant::now()) };
            self.player.seek(path, pos, paused);
        }
    }

    // ── Metadata / lyrics actions ─────────────────────────────────────────

    fn selected_track_for_edit(&self) -> Option<Track> {
        match self.focus {
            Focus::TrackList => self.selected_track(),
            Focus::Queue => self.queue.get(self.queue_cursor).cloned(),
            Focus::Sidebar => None,
        }
    }

    fn action_edit_metadata(&mut self) {
        if let Some(t) = self.selected_track_for_edit() {
            self.modal = Some(Modal::EditMetadata {
                track_id: t.id,
                fields: [
                    TextInput::with_value(&t.name),
                    TextInput::with_value(t.artist.as_deref().unwrap_or("")),
                    TextInput::with_value(t.album.as_deref().unwrap_or("")),
                    TextInput::with_value(t.year.map(|y| y.to_string()).unwrap_or_default()),
                    TextInput::with_value(t.genre.as_deref().unwrap_or("")),
                ],
                active_field: 0,
                year_error: false,
            });
        }
    }

    fn refresh_lyrics_cache(&mut self) {
        self.lyrics_lines = self
            .now_playing
            .and_then(|i| self.queue.get(i))
            .and_then(|t| self.library.track(t.id))
            .and_then(|t| t.lyrics.as_ref())
            .map(|raw| lyrics::parse_lrc(raw))
            .unwrap_or_default();
    }

    fn sync_queue_name(&mut self, track_id: TrackId, name: &str) {
        for t in &mut self.queue {
            if t.id == track_id {
                t.name = name.to_string();
            }
        }
    }

    fn sync_queue_metadata(
        &mut self,
        track_id: TrackId,
        name: &str,
        artist: Option<&String>,
        album: Option<&String>,
        year: Option<u32>,
        genre: Option<&String>,
    ) {
        for t in &mut self.queue {
            if t.id == track_id {
                t.name = name.to_string();
                t.artist = artist.cloned();
                t.album = album.cloned();
                t.year = year;
                t.genre = genre.cloned();
            }
        }
    }

    fn action_edit_lyrics(&mut self, track_id: TrackId) {
        let raw = self
            .library
            .track(track_id)
            .and_then(|t| t.lyrics.as_deref())
            .unwrap_or("")
            .to_string();
        self.modal = Some(Modal::EditLyrics {
            track_id,
            textarea: make_lyrics_textarea(&raw),
        });
    }

    fn action_toggle_lyrics(&mut self) {
        if let Some(np) = self.now_playing.and_then(|i| self.queue.get(i)) {
            let has_lyrics = self
                .library
                .track(np.id)
                .and_then(|t| t.lyrics.as_ref())
                .is_some();
            if has_lyrics {
                self.show_lyrics = !self.show_lyrics;
                if self.show_lyrics {
                    self.lyrics_scroll = 0;
                    self.refresh_lyrics_cache();
                }
            } else {
                self.show_lyrics = false;
                self.modal = Some(Modal::Notify {
                    message: "No lyrics for this track. Press e to edit metadata and add them."
                        .into(),
                });
            }
        } else {
            self.show_lyrics = false;
            self.modal = Some(Modal::Notify {
                message: "Nothing is playing.".into(),
            });
        }
    }

    // ── Modal confirm handler ─────────────────────────────────────────────

    fn apply_remove(&mut self, target: RemoveTarget) {
        match target {
            RemoveTarget::TrackFromQueue { queue_idx } => {
                if queue_idx < self.queue.len() {
                    self.queue.remove(queue_idx);
                    if let Some(np) = self.now_playing {
                        if queue_idx < np {
                            self.now_playing = Some(np - 1);
                        } else if queue_idx == np {
                            self.halt_playback();
                        }
                    }

                    self.queue_cursor = self.queue_cursor.min(self.queue.len().saturating_sub(1));
                }
            }

            RemoveTarget::TrackFromLibrary { track_id } => {
                let _ = self.library.remove_track(track_id);

                let playing_removed = self
                    .now_playing
                    .and_then(|np| self.queue.get(np))
                    .is_some_and(|t| t.id == track_id);
                let removed_before_np = self.now_playing.map_or(0, |np| {
                    self.queue[..np].iter().filter(|t| t.id == track_id).count()
                });

                self.queue.retain(|t| t.id != track_id);

                if playing_removed {
                    self.halt_playback();
                } else if removed_before_np > 0
                    && let Some(np) = self.now_playing.as_mut()
                {
                    *np -= removed_before_np;
                }

                self.queue_cursor = self.queue_cursor.min(self.queue.len().saturating_sub(1));
                self.rebuild_filter_cache();
                self.tracklist_cursor = self
                    .tracklist_cursor
                    .min(self.active_tracks().len().saturating_sub(1));
            }

            RemoveTarget::Playlist { playlist_id } => {
                let _ = self.library.delete_playlist(playlist_id);
                self.sidebar_cursor = self
                    .sidebar_cursor
                    .min(self.library.playlists.len().saturating_sub(1));
                self.sync_active_playlist();
            }

            RemoveTarget::Queue => {
                self.queue.clear();
                self.queue_cursor = 0;
                self.halt_playback();
            }
        }
    }

    /// Persists edited metadata to the library and syncs it into the live
    /// queue. Shared by `SaveMetadata` and `SaveMetadataAndEditLyrics`.
    fn save_metadata(
        &mut self,
        track_id: TrackId,
        name: &str,
        artist: Option<&String>,
        album: Option<&String>,
        year: Option<u32>,
        genre: Option<&String>,
    ) {
        let _ = self.library.update_track_metadata(
            track_id,
            name.to_string(),
            artist.cloned(),
            album.cloned(),
            year,
            genre.cloned(),
        );
        self.sync_queue_metadata(track_id, name, artist, album, year, genre);
        self.rebuild_filter_cache();
    }

    fn apply_modal_confirm(&mut self, confirm: ModalConfirm) {
        match confirm {
            ModalConfirm::Remove(target) => self.apply_remove(target),

            ModalConfirm::Rename { kind, id, new_name } => self.apply_rename(&kind, id, new_name),

            ModalConfirm::NewPlaylist { name } => {
                let _ = self.library.create_playlist(name);
            }

            ModalConfirm::AddToPlaylist {
                track_id,
                playlist_id,
            } => {
                let _ = self.library.playlist_add_track(playlist_id, track_id);
                self.rebuild_filter_cache();
            }

            ModalConfirm::OpenSettings => {
                self.open_settings();
            }

            ModalConfirm::OpenAbout => {
                self.open_about();
            }

            ModalConfirm::Quit => {
                self.should_quit = true;
            }

            ModalConfirm::SaveSettings {
                volume_pct,
                seek_secs,
                theme_name,
                transparent,
                color_mode,
            } => self.apply_save_settings(
                volume_pct,
                seek_secs,
                &theme_name,
                transparent,
                color_mode,
            ),

            ModalConfirm::PreviewTheme {
                theme_name,
                transparent,
                color_mode,
            } => {
                let truecolor = color_mode.truecolor(detect_truecolor());
                self.theme = resolve_theme(&theme_name, transparent, truecolor);
            }

            ModalConfirm::ShufflePlaylist { playlist_id } => {
                self.apply_shuffle_playlist(playlist_id);
            }

            ModalConfirm::SaveMetadata {
                track_id,
                name,
                artist,
                album,
                year,
                genre,
            } => {
                self.save_metadata(
                    track_id,
                    &name,
                    artist.as_ref(),
                    album.as_ref(),
                    year,
                    genre.as_ref(),
                );
            }

            ModalConfirm::SaveLyrics { track_id, lyrics } => {
                let _ = self.library.set_track_lyrics(track_id, lyrics);
                if self.show_lyrics {
                    self.refresh_lyrics_cache();
                }
            }

            ModalConfirm::SaveMetadataAndEditLyrics {
                track_id,
                name,
                artist,
                album,
                year,
                genre,
            } => {
                self.save_metadata(
                    track_id,
                    &name,
                    artist.as_ref(),
                    album.as_ref(),
                    year,
                    genre.as_ref(),
                );
                self.action_edit_lyrics(track_id);
            }
        }
    }

    // ── File import ───────────────────────────────────────────────────────

    fn import_file(&mut self, path: &std::path::Path) {
        match self.library.add_file(path) {
            Ok((track, is_new)) => {
                self.rebuild_filter_cache();
                self.modal = Some(Modal::Notify {
                    message: if is_new {
                        format!("\"{}\" added to library.", track.name)
                    } else {
                        format!("\"{}\" is already in the library.", track.name)
                    },
                });
            }
            Err(e) => {
                self.modal = Some(Modal::Notify {
                    message: format!("Error importing file: {e}"),
                });
            }
        }
    }

    fn apply_rename(&mut self, kind: &str, id: u64, new_name: String) {
        if kind == "Track" {
            let _ = self.library.rename_track(id, &new_name);
            self.sync_queue_name(id, &new_name);
            self.rebuild_filter_cache();
        } else {
            let _ = self.library.rename_playlist(id, new_name);
        }
    }

    fn apply_save_settings(
        &mut self,
        volume_pct: u32,
        seek_secs: u64,
        theme_name: &str,
        transparent: bool,
        color_mode: ColorMode,
    ) {
        self.settings
            .set_default_volume(numeric::whole_percent_to_ratio(volume_pct));
        self.settings.set_seek_step_secs(seek_secs);
        self.settings.set_theme(theme_name);
        self.settings.transparent = transparent;
        self.settings.color_mode = color_mode;
        // Apply the resolved theme live (console fallback when not truecolor).
        self.theme = resolve_theme(
            theme_name,
            transparent,
            color_mode.truecolor(detect_truecolor()),
        );
        let _ = self.settings.save();
    }

    fn apply_shuffle_playlist(&mut self, playlist_id: PlaylistId) {
        // Resolve tracks, shuffle in place, replace queue, start playing.
        let mut tracks: Vec<Track> = self
            .library
            .playlist_tracks(playlist_id)
            .into_iter()
            .cloned()
            .collect();

        if tracks.is_empty() {
            return;
        }

        tracks.shuffle(&mut rand::rng());

        self.halt_playback();
        self.queue = tracks;
        self.queue_cursor = 0;
        self.play_queue_index(0);
    }
}
// ── Entry point ────────────────────────────────────────────────────────────

pub fn run(cli: Cli) -> Result<()> {
    let mut library = Library::load()?;
    let settings = Settings::load();

    let initial_track: Option<(Track, bool)> = if let Some(file) = cli.file {
        let result = library.add_file(&file)?;
        Some(result)
    } else {
        None
    };

    let player = spawn_audio_thread(settings.default_volume)?;

    let mut state = AppState::new(library, player, settings);

    if let Some((track, is_new)) = initial_track {
        let msg = if is_new {
            format!("\"{}\" added to library.", track.name)
        } else {
            format!("\"{}\" is already in the library.", track.name)
        };
        state.enqueue(track);
        state.play_queue_index(0);
        state.modal = Some(Modal::Notify { message: msg });
    }

    let terminal = ratatui::init();
    let result = event_loop(terminal, &mut state);
    ratatui::restore();
    result
}

fn event_loop(mut terminal: DefaultTerminal, state: &mut AppState) -> Result<()> {
    loop {
        state.tick();
        terminal.draw(|frame| ui::render(frame, state))?;

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            state.handle_key(key.code, key.modifiers);
        }

        if state.should_quit {
            break;
        }
    }
    Ok(())
}
