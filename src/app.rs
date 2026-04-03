use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use rand::seq::SliceRandom;
use ratatui::DefaultTerminal;
use std::time::{Duration, Instant};

use crate::{
    cli::Cli,
    filepicker::{FilePicker, FilePickerOutcome},
    library::{ALL_TRACKS_ID, Library, PlaylistId, Track},
    modal::{Modal, ModalConfirm, ModalOutcome, RemoveTarget, TextInput},
    player::{PlayerEvent, PlayerHandle, resolve_duration, spawn_audio_thread},
    settings::Settings,
    ui,
};

// ── Focus ──────────────────────────────────────────────────────────────────

/// Which panel currently owns keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    TrackList,
    Queue,
}

impl Focus {
    pub fn cycle(self) -> Self {
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

    pub settings: Settings,
    pub should_quit: bool,
}

impl AppState {
    pub fn new(library: Library, player: PlayerHandle, settings: Settings) -> Self {
        Self {
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
            settings,
            should_quit: false,
        }
    }

    // ── Progress ─────────────────────────────────────────────────────────

    pub fn elapsed(&self) -> Duration {
        if self.player.is_paused {
            return self.seek_offset;
        }
        self.seek_offset
            + self
                .track_start
                .map(|s| s.elapsed())
                .unwrap_or(Duration::ZERO)
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
    }

    pub fn play_next(&mut self) {
        let next = self.now_playing.map(|i| i + 1).unwrap_or(0);
        if next < self.queue.len() {
            self.play_queue_index(next);
        } else {
            self.halt_playback();
        }
    }

    pub fn play_prev(&mut self) {
        if let Some(cur) = self.now_playing
            && cur > 0
        {
            self.play_queue_index(cur - 1);
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

    /// Returns the tracks of the currently displayed playlist.
    pub fn active_tracks(&self) -> Vec<&Track> {
        self.library.playlist_tracks(self.active_playlist)
    }

    // ── Tick ─────────────────────────────────────────────────────────────

    /// Processes player events and auto-advances on natural track end.
    pub fn tick(&mut self) {
        for event in self.player.drain_events() {
            match event {
                PlayerEvent::TrackFinished => self.play_next(),
                PlayerEvent::DurationResolved(d) => {
                    self.track_duration = Some(d);
                }
                PlayerEvent::Error(msg) => {
                    // Surface as a transient notification modal.
                    self.modal = Some(Modal::TrackAdded {
                        name: format!("Error: {}", msg),
                    });
                }
            }
        }
    }

    // ── Input ─────────────────────────────────────────────────────────────

    pub fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) {
        // ── File picker takes priority ────────────────────────────────
        if let Some(picker) = &mut self.file_picker {
            match picker.handle_key(code) {
                FilePickerOutcome::Continue => return,
                FilePickerOutcome::Dismissed => {
                    self.file_picker = None;
                    return;
                }
                FilePickerOutcome::Selected(path) => {
                    let path_clone = path.clone();
                    self.file_picker = None;
                    self.import_file(&path_clone);
                    return;
                }
            }
        }

        // ── Modal intercepts next ─────────────────────────────────────
        if let Some(modal) = &mut self.modal {
            match modal.handle_key(code) {
                ModalOutcome::Consumed => return,
                ModalOutcome::Dismissed => {
                    self.modal = None;
                    return;
                }
                ModalOutcome::Confirm(c) => {
                    self.modal = None;
                    self.apply_modal_confirm(c);
                    return;
                }
            }
        }

        // ── Global keybindings ─────────────────────────────────────────
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.modal = Some(Modal::Help),

            // Playback
            KeyCode::Char(' ') => self.action_toggle_play(),
            KeyCode::Char('n') => self.play_next(),
            KeyCode::Char('N') => self.play_prev(),
            KeyCode::Left => self.action_seek(-(self.settings.seek_step_secs as i64)),
            KeyCode::Right => self.action_seek(self.settings.seek_step_secs as i64),
            KeyCode::Char('+') | KeyCode::Char('=') => self.player.volume_up(),
            KeyCode::Char('-') => self.player.volume_down(),

            // Navigation
            KeyCode::Tab => self.focus = self.focus.cycle(),
            KeyCode::Char('j') | KeyCode::Down => self.cursor_down(),
            KeyCode::Char('k') | KeyCode::Up => self.cursor_up(),

            // Context actions
            KeyCode::Enter => self.action_enter(),
            KeyCode::Char('a') => self.action_add_to_queue(),
            KeyCode::Char('p') => self.action_add_to_playlist(),
            KeyCode::Char('d') => self.action_remove(),
            KeyCode::Char('r') => self.action_rename(),
            KeyCode::Char('x') => self.action_remove_from_queue(),
            KeyCode::Char('c') => self.action_new_playlist(),
            KeyCode::Char('f') => self.action_open_filepicker(),
            KeyCode::Char('s') => self.action_open_settings(),
            KeyCode::Char('z') => self.action_shuffle_playlist(),

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
        let idx = match self.now_playing {
            Some(i) => i,
            None => return,
        };
        let path = match self.queue.get(idx) {
            Some(t) => t.path.clone(),
            None => return,
        };

        // Compute new position, clamped to [0, duration].
        let current = self.elapsed().as_secs() as i64;
        let max_secs = self
            .track_duration
            .map(|d| d.as_secs().saturating_sub(1) as i64)
            .unwrap_or(i64::MAX);
        let target_secs = (current + delta_secs).clamp(0, max_secs) as u64;
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
                let tracks = self
                    .active_tracks()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                if let Some(track) = tracks.get(self.tracklist_cursor).cloned() {
                    let insert_at = self.now_playing.map(|i| i + 1).unwrap_or(0);
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
        let tracks = self
            .active_tracks()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        if let Some(track) = tracks.get(self.tracklist_cursor).cloned() {
            self.queue.push(track);
        }
    }

    fn action_add_to_playlist(&mut self) {
        let tracks = self
            .active_tracks()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        if let Some(track) = tracks.get(self.tracklist_cursor) {
            let choices: Vec<(u64, String)> = self
                .library
                .playlists
                .iter()
                .filter(|p| p.id != ALL_TRACKS_ID)
                .map(|p| (p.id, p.name.clone()))
                .collect();

            self.modal = Some(Modal::AddToPlaylist {
                track_id: track.id,
                track_name: track.name.clone(),
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
                let tracks = self
                    .active_tracks()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                if let Some(track) = tracks.get(self.tracklist_cursor) {
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
            Focus::TrackList | Focus::Queue => {
                let track: Option<Track> = match self.focus {
                    Focus::TrackList => self
                        .active_tracks()
                        .get(self.tracklist_cursor)
                        .map(|t| (*t).clone()),
                    Focus::Queue => self.queue.get(self.queue_cursor).cloned(),
                    _ => None,
                };
                if let Some(t) = track {
                    self.modal = Some(Modal::Rename {
                        kind: "Track".into(),
                        id: t.id,
                        input: TextInput::with_value(&t.name),
                    });
                }
            }
        }
    }

    fn action_remove_from_queue(&mut self) {
        if self.queue_cursor < self.queue.len() {
            self.modal = Some(Modal::ConfirmRemove {
                description: "Remove this track from the queue?".into(),
                target: RemoveTarget::TrackFromQueue {
                    queue_idx: self.queue_cursor,
                },
            });
        }
    }

    fn action_new_playlist(&mut self) {
        self.modal = Some(Modal::NewPlaylist {
            input: TextInput::default(),
        });
    }

    fn action_open_filepicker(&mut self) {
        let start = dirs_next::home_dir().unwrap_or_else(|| "/".into());
        self.file_picker = Some(FilePicker::new(start));
    }

    fn action_open_settings(&mut self) {
        let vol_pct = (self.settings.default_volume * 100.0).round() as u32;
        self.modal = Some(Modal::Settings {
            cursor: 0,
            volume_pct: vol_pct,
            seek_secs: self.settings.seek_step_secs,
        });
    }

    /// `z` — prompt to shuffle the active playlist into the queue.
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

    // ── Modal confirm handler ─────────────────────────────────────────────

    fn apply_modal_confirm(&mut self, confirm: ModalConfirm) {
        match confirm {
            ModalConfirm::None => {}

            ModalConfirm::Remove(target) => match target {
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
                        self.queue_cursor =
                            self.queue_cursor.min(self.queue.len().saturating_sub(1));
                    }
                }
                RemoveTarget::TrackFromLibrary { track_id } => {
                    let _ = self.library.remove_track(track_id);
                    let before_len = self.queue.len();
                    self.queue.retain(|t| t.id != track_id);
                    if self.queue.len() != before_len {
                        self.halt_playback();
                    }
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
            },

            ModalConfirm::Rename { kind, id, new_name } => {
                if kind == "Track" {
                    let _ = self.library.rename_track(id, &new_name);
                    // Update name in queue as well.
                    for t in &mut self.queue {
                        if t.id == id {
                            t.name = new_name.clone();
                        }
                    }
                } else {
                    let _ = self.library.rename_playlist(id, new_name);
                }
            }

            ModalConfirm::NewPlaylist { name } => {
                let _ = self.library.create_playlist(name);
            }

            ModalConfirm::AddToPlaylist {
                track_id,
                playlist_id,
            } => {
                let _ = self.library.playlist_add_track(playlist_id, track_id);
            }

            ModalConfirm::SaveSettings {
                volume_pct,
                seek_secs,
            } => {
                self.settings.set_default_volume(volume_pct as f32 / 100.0);
                self.settings.set_seek_step_secs(seek_secs);
                // seek_step_secs is read at seek-time so it takes effect
                // immediately. default_volume is startup-only; the live player
                // volume is intentionally left unchanged.
                let _ = self.settings.save();
            }

            ModalConfirm::ShufflePlaylist { playlist_id } => {
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
    }

    // ── File import ───────────────────────────────────────────────────────

    fn import_file(&mut self, path: &std::path::Path) {
        match self.library.add_file(path) {
            Ok((track, is_new)) => {
                if is_new {
                    self.modal = Some(Modal::TrackAdded {
                        name: track.name.clone(),
                    });
                }
            }
            Err(e) => {
                self.modal = Some(Modal::TrackAdded {
                    name: format!("Error importing file: {e}"),
                });
            }
        }
    }
}

// ── Entry point ────────────────────────────────────────────────────────────

pub fn run(cli: Cli) -> Result<()> {
    let mut library = Library::load()?;
    let settings = Settings::load();

    let initial_track: Option<Track> = if let Some(file) = cli.file {
        let (track, _) = library.add_file(&file)?;
        Some(track)
    } else {
        None
    };

    let player = spawn_audio_thread(settings.default_volume)?; // no need to be mutable by now

    let mut state = AppState::new(library, player, settings);

    if let Some(track) = initial_track {
        state.enqueue(track);
        state.play_queue_index(0);
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
