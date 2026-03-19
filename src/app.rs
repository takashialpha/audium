use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use std::time::{Duration, Instant};

use crate::{
    cli::Cli,
    library::{Library, Track},
    player::AudiumPlayer,
    ui,
};

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

/// Top-level application state threaded through every render and event cycle.
pub struct AppState {
    pub library: Library,
    pub player: AudiumPlayer,

    // ---- UI state --------------------------------------------------------
    pub focus: Focus,

    /// Index into `library.tracks` (sidebar / track-list cursor).
    pub library_cursor: usize,
    /// Index into `queue` (queue-panel cursor).
    pub queue_cursor: usize,

    // ---- Playback queue --------------------------------------------------
    /// Ordered list of tracks waiting to be played.
    pub queue: Vec<Track>,
    /// Index of the currently playing track inside `queue`, if any.
    pub now_playing: Option<usize>,

    // ---- Progress tracking -----------------------------------------------
    /// Wall-clock instant when the current track started (or was last seeked).
    pub track_start: Option<Instant>,
    /// Accumulated elapsed time before the last seek / pause.
    pub seek_offset: Duration,
    /// Whether playback was paused (used to freeze the progress clock).
    pub was_paused: bool,
    /// Total duration of the currently playing track, if known.
    pub track_duration: Option<Duration>,

    pub should_quit: bool,
}

impl AppState {
    pub fn new(library: Library, player: AudiumPlayer) -> Self {
        Self {
            library,
            player,
            focus: Focus::Sidebar,
            library_cursor: 0,
            queue_cursor: 0,
            queue: Vec::new(),
            now_playing: None,
            track_start: None,
            seek_offset: Duration::ZERO,
            was_paused: true,
            track_duration: None,
            should_quit: false,
        }
    }

    // ------------------------------------------------------------------ //
    //  Progress                                                            //
    // ------------------------------------------------------------------ //

    /// Returns elapsed playback time for the current track.
    pub fn elapsed(&self) -> Duration {
        if self.was_paused {
            return self.seek_offset;
        }
        self.seek_offset
            + self
                .track_start
                .map(|s| s.elapsed())
                .unwrap_or(Duration::ZERO)
    }

    // ------------------------------------------------------------------ //
    //  Queue helpers                                                       //
    // ------------------------------------------------------------------ //

    /// Enqueue a track (does not start playback).
    pub fn enqueue(&mut self, track: Track) {
        self.queue.push(track);
    }

    /// Start playing the track at `queue[idx]`.
    fn play_queue_index(&mut self, idx: usize) {
        if idx >= self.queue.len() {
            return;
        }
        let path = self.queue[idx].path.clone();
        if let Ok(duration) = self.player.play_file(&path) {
            self.now_playing = Some(idx);
            self.track_start = Some(Instant::now());
            self.seek_offset = Duration::ZERO;
            self.was_paused = false;
            self.track_duration = duration;
        }
    }

    /// Returns a 0.0–1.0 ratio of elapsed / total for the progress bar.
    /// Falls back to 0.0 if duration is unknown.
    pub fn progress_ratio(&self) -> f64 {
        let elapsed = self.elapsed().as_secs_f64();
        match self.track_duration {
            Some(d) if d.as_secs_f64() > 0.0 => (elapsed / d.as_secs_f64()).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// Start playing the next track in the queue, if available.
    pub fn play_next(&mut self) {
        let next = self.now_playing.map(|i| i + 1).unwrap_or(0);
        self.play_queue_index(next);
    }

    /// Start playing the previous track in the queue, if available.
    pub fn play_prev(&mut self) {
        if let Some(cur) = self.now_playing
            && cur > 0
        {
            self.play_queue_index(cur - 1);
        }
    }

    // ------------------------------------------------------------------ //
    //  Tick — called every event-loop cycle                               //
    // ------------------------------------------------------------------ //

    /// Auto-advance to the next track when the current one finishes.
    pub fn tick(&mut self) {
        if self.now_playing.is_some() && self.player.is_finished() {
            self.play_next();
        }
    }

    // ------------------------------------------------------------------ //
    //  Input                                                               //
    // ------------------------------------------------------------------ //

    pub fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) {
        match code {
            // ---- Global ------------------------------------------------
            KeyCode::Char('q') => self.should_quit = true,

            // Space  →  play/pause (only if something is actually queued)
            KeyCode::Char(' ') => {
                if self.now_playing.is_none() {
                    // Nothing has ever started — start the queue if possible
                    if !self.queue.is_empty() {
                        self.play_queue_index(0);
                    }
                    // Otherwise do nothing at all
                } else {
                    self.player.toggle_pause();
                    if self.player.is_paused() {
                        self.seek_offset = self.elapsed();
                        self.track_start = None;
                        self.was_paused = true;
                    } else {
                        self.track_start = Some(Instant::now());
                        self.was_paused = false;
                    }
                }
            }

            // n / N  →  next / prev track
            KeyCode::Char('n') => self.play_next(),
            KeyCode::Char('N') => self.play_prev(),

            // Volume  +/- (0.1 steps)
            KeyCode::Char('+') | KeyCode::Char('=') => self.player.volume_up(),
            KeyCode::Char('-') => self.player.volume_down(),

            // Tab  →  cycle panel focus
            KeyCode::Tab => self.focus = self.focus.cycle(),

            // j/k + up/down arrows  →  navigate the focused list
            KeyCode::Char('j') | KeyCode::Down => self.cursor_down(),
            KeyCode::Char('k') | KeyCode::Up => self.cursor_up(),

            // Enter  →  context action on focused panel
            KeyCode::Enter => self.action_enter(),

            // a  →  append focused library track to end of queue
            KeyCode::Char('a') => self.action_add_to_queue(),

            _ => {}
        }
    }

    fn cursor_down(&mut self) {
        match self.focus {
            Focus::Sidebar | Focus::TrackList => {
                let len = self.library.tracks.len();
                if len > 0 {
                    self.library_cursor = (self.library_cursor + 1).min(len - 1);
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
            Focus::Sidebar | Focus::TrackList => {
                self.library_cursor = self.library_cursor.saturating_sub(1);
            }
            Focus::Queue => {
                self.queue_cursor = self.queue_cursor.saturating_sub(1);
            }
        }
    }

    /// Enter on library  →  play immediately (enqueue first if not present).
    /// Enter on queue    →  play that queue entry immediately.
    fn action_enter(&mut self) {
        match self.focus {
            Focus::Sidebar | Focus::TrackList => {
                if let Some(track) = self.library.tracks.get(self.library_cursor).cloned() {
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

    /// `a`  →  append focused library track to the end of the queue.
    fn action_add_to_queue(&mut self) {
        if let Some(track) = self.library.tracks.get(self.library_cursor).cloned() {
            self.queue.push(track);
        }
    }
}

// -------------------------------------------------------------------------- //
//  Entry point                                                                //
// -------------------------------------------------------------------------- //

/// Initialises the terminal, runs the event loop, then restores the terminal.
pub fn run(cli: Cli) -> Result<()> {
    let mut library = Library::load()?;

    let initial_track: Option<Track> = if let Some(file) = cli.file {
        let track = library.add_file(&file)?;
        Some(track)
    } else {
        None
    };

    let player = AudiumPlayer::new()?;
    let mut state = AppState::new(library, player);

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

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press {
                state.handle_key(key.code, key.modifiers);
            }

        if state.should_quit {
            break;
        }
    }
    Ok(())
}
