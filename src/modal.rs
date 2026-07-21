use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap,
    },
};

use crate::library::{PlaylistId, TrackId};
use crate::numeric::usize_to_u16_saturating;
use crate::settings::ColorMode;
use crate::ui::layout::{
    Theme, console_themes, cursor_spans, cursor_spans_windowed, format_duration, themes, truncate,
};

// -- Text-input widget ------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct TextInput {
    pub value: String,
    pub cursor: usize,
}

impl TextInput {
    pub fn with_value(v: impl Into<String>) -> Self {
        let value = v.into();
        let cursor = value.len();
        Self { value, cursor }
    }

    pub fn push(&mut self, c: char) {
        self.value.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut new_cursor = self.cursor - 1;
        while !self.value.is_char_boundary(new_cursor) {
            new_cursor -= 1;
        }
        self.value.remove(new_cursor);
        self.cursor = new_cursor;
    }

    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut c = self.cursor - 1;
        while !self.value.is_char_boundary(c) {
            c -= 1;
        }
        self.cursor = c;
    }

    pub fn move_right(&mut self) {
        if self.cursor >= self.value.len() {
            return;
        }
        let mut c = self.cursor + 1;
        while !self.value.is_char_boundary(c) {
            c += 1;
        }
        self.cursor = c;
    }
}

// -- Multi-line text editor -------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct TextArea {
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
}

impl TextArea {
    pub fn from_text(text: &str) -> Self {
        let lines: Vec<String> = text
            .lines()
            .map(|l| l.trim_end_matches('\r').to_string())
            .collect();
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        let cursor_row = lines.len() - 1;
        let cursor_col = lines[cursor_row].len();
        Self {
            lines,
            cursor_row,
            cursor_col,
        }
    }

    pub fn as_string(&self) -> String {
        self.lines.join("\n")
    }

    pub fn insert_char(&mut self, c: char) {
        self.lines[self.cursor_row].insert(self.cursor_col, c);
        self.cursor_col += c.len_utf8();
    }

    pub fn delete_char(&mut self) {
        if self.cursor_col > 0 {
            let line = &self.lines[self.cursor_row];
            let mut col = self.cursor_col - 1;
            while !line.is_char_boundary(col) {
                col -= 1;
            }
            self.lines[self.cursor_row].remove(col);
            self.cursor_col = col;
        } else if self.cursor_row > 0 {
            let current = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_len = self.lines[self.cursor_row].len();
            self.lines[self.cursor_row].push_str(&current);
            self.cursor_col = prev_len;
        }
    }

    pub fn delete_next_char(&mut self) {
        let line_len = self.lines[self.cursor_row].len();
        if self.cursor_col < line_len {
            self.lines[self.cursor_row].remove(self.cursor_col);
        } else if self.cursor_row + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next);
        }
    }

    pub fn insert_newline(&mut self) {
        let rest = self.lines[self.cursor_row][self.cursor_col..].to_string();
        self.lines[self.cursor_row].truncate(self.cursor_col);
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, rest);
        self.cursor_col = 0;
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            let line = &self.lines[self.cursor_row];
            let mut col = self.cursor_col - 1;
            while !line.is_char_boundary(col) {
                col -= 1;
            }
            self.cursor_col = col;
        }
    }

    pub fn move_right(&mut self) {
        let line = &self.lines[self.cursor_row];
        if self.cursor_col < line.len()
            && let Some(c) = line[self.cursor_col..].chars().next()
        {
            self.cursor_col += c.len_utf8();
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = clamp_col(&self.lines[self.cursor_row], self.cursor_col);
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = clamp_col(&self.lines[self.cursor_row], self.cursor_col);
        }
    }

    pub const fn move_line_start(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_line_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_row].len();
    }
}

fn clamp_col(line: &str, col: usize) -> usize {
    let mut c = col.min(line.len());
    while c > 0 && !line.is_char_boundary(c) {
        c -= 1;
    }
    c
}

/// A `TextArea` wrapped with a name so app.rs can import it as `LyricsTextArea`.
pub type LyricsTextArea = TextArea;

// -- RemoveTarget -----------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum RemoveTarget {
    TrackFromQueue {
        queue_idx: usize,
    },
    TrackFromLibrary {
        track_id: TrackId,
    },
    TrackFromPlaylist {
        playlist_id: PlaylistId,
        track_id: TrackId,
    },
    Playlist {
        playlist_id: PlaylistId,
    },
    Queue,
}

// -- Settings modal state ---------------------------------------------------

/// Live state of the settings modal, mutated in place while it is open.
#[derive(Debug, Clone)]
pub struct SettingsState {
    pub cursor: usize,
    pub volume_pct: u32,
    pub seek_secs: u64,
    pub preview_theme_idx: usize,
    pub transparent: bool,
    /// Index into the 16-color console themes, previewed independently of
    /// `preview_theme_idx` so neither choice clobbers the other.
    pub preview_console_idx: usize,
    /// Editable color-mode preference (Auto / Truecolor / 16-color).
    pub color_mode: ColorMode,
    /// What truecolor auto-detection found; drives the banner and whether the
    /// theme / transparency rows are interactive.
    pub detected_truecolor: bool,
}

// -- Modal variants ---------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Modal {
    Notify {
        message: String,
    },
    ConfirmRemove {
        description: String,
        target: RemoveTarget,
    },
    EditPlaylist {
        id: PlaylistId,
        input: TextInput,
    },
    NewPlaylist {
        input: TextInput,
        /// If set, the track to drop into the playlist right after it's created.
        add_track: Option<TrackId>,
    },
    AddToPlaylist {
        track_id: TrackId,
        track_name: String,
        choices: Vec<(PlaylistId, String)>,
        cursor: usize,
    },
    Help,
    /// Confirms shuffling the active view into the queue; the app resolves
    /// which tracks that is when the confirmation comes back.
    ShuffleView {
        view_name: String,
    },
    /// Top-level menu: Settings / About / Quit.
    Menu {
        cursor: usize,
    },
    About,
    ConfirmQuit,
    Settings(SettingsState),
    /// In-app editor for track metadata (name, artist, album, year, genre).
    EditMetadata {
        track_id: TrackId,
        /// [0]=name [1]=artist [2]=album [3]=year [4]=genre
        fields: [TextInput; 5],
        active_field: usize,
        /// Restored if the name field is cleared, so closing always writes a
        /// valid name (the editor never traps you).
        original_name: String,
    },
    /// Multi-line editor for raw LRC (or plain) lyrics text.
    EditLyrics {
        track_id: TrackId,
        textarea: LyricsTextArea,
    },
}

const MENU_ENTRIES: usize = 3;

// -- ModalOutcome -----------------------------------------------------------

pub enum ModalOutcome {
    Consumed,
    Confirm(ModalConfirm),
    Dismissed,
}

#[derive(Debug)]
pub enum ModalConfirm {
    Remove(RemoveTarget),
    RenamePlaylist {
        id: PlaylistId,
        new_name: String,
    },
    NewPlaylist {
        name: String,
        add_track: Option<TrackId>,
    },
    AddToPlaylist {
        track_id: TrackId,
        playlist_id: PlaylistId,
    },
    SaveSettings {
        volume_pct: u32,
        seek_secs: u64,
        theme_name: String,
        console_theme_name: String,
        transparent: bool,
        color_mode: ColorMode,
    },
    /// Apply a theme live during settings preview without closing the modal.
    PreviewTheme {
        theme_name: String,
        console_theme_name: String,
        transparent: bool,
        color_mode: ColorMode,
    },
    ShuffleView,
    OpenSettings,
    OpenAbout,
    Quit,
    SaveMetadata {
        track_id: TrackId,
        name: String,
        artist: Option<String>,
        album: Option<String>,
        year: Option<u32>,
        genre: Option<String>,
    },
    SaveLyrics {
        track_id: TrackId,
        /// `None` means the user cleared the lyrics field entirely.
        lyrics: Option<String>,
    },
    /// Saves all metadata fields then immediately opens the lyrics editor.
    SaveMetadataAndEditLyrics {
        track_id: TrackId,
        name: String,
        artist: Option<String>,
        album: Option<String>,
        year: Option<u32>,
        genre: Option<String>,
    },
}

// -- Shared helpers ---------------------------------------------------------

fn nonempty_opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

// -- Text-input key helper --------------------------------------------------

enum TextInputResult {
    Consumed,
    Dismissed,
    Submitted(String),
}

fn handle_text_key(input: &mut TextInput, code: KeyCode) -> TextInputResult {
    match code {
        KeyCode::Enter => {
            let name = input.value.trim().to_string();
            if name.is_empty() {
                TextInputResult::Consumed
            } else {
                TextInputResult::Submitted(name)
            }
        }
        KeyCode::Esc => TextInputResult::Dismissed,
        KeyCode::Char(c) => {
            input.push(c);
            TextInputResult::Consumed
        }
        KeyCode::Backspace => {
            input.backspace();
            TextInputResult::Consumed
        }
        KeyCode::Left => {
            input.move_left();
            TextInputResult::Consumed
        }
        KeyCode::Right => {
            input.move_right();
            TextInputResult::Consumed
        }
        _ => TextInputResult::Consumed,
    }
}

// -- Input handling ---------------------------------------------------------

// Settings rows, in display order.
const SET_VOLUME: usize = 0;
const SET_SEEK: usize = 1;
const SET_COLOR_MODE: usize = 2;
const SET_THEME: usize = 3;
const SET_TRANSPARENCY: usize = 4;
const SET_ROWS: usize = 5;

/// Which rows accept input.  Every mode has themes to choose from, so only
/// transparency is locked (and skipped by the cursor) without truecolor: the
/// console themes leave the background at the terminal default, which is
/// already whatever the terminal is.
const fn settings_enabled(color_mode: ColorMode, detected: bool) -> [bool; SET_ROWS] {
    let tc = color_mode.truecolor(detected);
    [true, true, true, true, tc]
}

fn handle_settings_key(code: KeyCode, s: &mut SettingsState) -> ModalOutcome {
    let enabled = settings_enabled(s.color_mode, s.detected_truecolor);
    let preview = |s: &SettingsState| {
        ModalOutcome::Confirm(ModalConfirm::PreviewTheme {
            theme_name: themes()[s.preview_theme_idx].name.to_string(),
            console_theme_name: console_themes()[s.preview_console_idx].name.to_string(),
            transparent: s.transparent,
            color_mode: s.color_mode,
        })
    };
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            let mut n = s.cursor;
            while n + 1 < SET_ROWS {
                n += 1;
                if enabled[n] {
                    s.cursor = n;
                    break;
                }
            }
            ModalOutcome::Consumed
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let mut n = s.cursor;
            while n > 0 {
                n -= 1;
                if enabled[n] {
                    s.cursor = n;
                    break;
                }
            }
            ModalOutcome::Consumed
        }
        KeyCode::Char('g') | KeyCode::Home | KeyCode::PageUp => {
            s.cursor = 0; // first row is always enabled
            ModalOutcome::Consumed
        }
        KeyCode::Char('G') | KeyCode::End | KeyCode::PageDown => {
            s.cursor = enabled.iter().rposition(|&e| e).unwrap_or(0);
            ModalOutcome::Consumed
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') => {
            let left = matches!(code, KeyCode::Left | KeyCode::Char('h'));
            match s.cursor {
                SET_VOLUME => {
                    if left {
                        s.volume_pct = s.volume_pct.saturating_sub(1);
                    } else {
                        s.volume_pct = (s.volume_pct + 1).min(100);
                    }
                    ModalOutcome::Consumed
                }
                SET_SEEK => {
                    if left {
                        s.seek_secs = s.seek_secs.saturating_sub(1).max(1);
                    } else {
                        s.seek_secs = (s.seek_secs + 1).min(120);
                    }
                    ModalOutcome::Consumed
                }
                SET_COLOR_MODE => {
                    // Two states, so both directions are the same toggle.
                    s.color_mode = s.color_mode.toggle(s.detected_truecolor);
                    preview(s)
                }
                SET_THEME => {
                    // Cycle whichever palette is actually on screen.
                    let (idx, len) = if s.color_mode.truecolor(s.detected_truecolor) {
                        (&mut s.preview_theme_idx, themes().len())
                    } else {
                        (&mut s.preview_console_idx, console_themes().len())
                    };
                    *idx = if left {
                        idx.checked_sub(1).unwrap_or(len - 1)
                    } else {
                        (*idx + 1) % len
                    };
                    preview(s)
                }
                _ => {
                    s.transparent = !s.transparent;
                    preview(s)
                }
            }
        }
        // Enter, Esc and q all save and close.
        KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => {
            ModalOutcome::Confirm(ModalConfirm::SaveSettings {
                volume_pct: s.volume_pct,
                seek_secs: s.seek_secs,
                theme_name: themes()[s.preview_theme_idx].name.to_string(),
                console_theme_name: console_themes()[s.preview_console_idx].name.to_string(),
                transparent: s.transparent,
                color_mode: s.color_mode,
            })
        }
        _ => ModalOutcome::Consumed,
    }
}

const META_YEAR: usize = 3;

fn handle_edit_metadata_key(
    code: KeyCode,
    track_id: TrackId,
    fields: &mut [TextInput; 5],
    active_field: &mut usize,
    original_name: &str,
) -> ModalOutcome {
    // Rows 0-4 are text inputs; row 5 is the "Edit Lyrics ->" button.
    const ROWS: usize = 6;
    match code {
        // Esc/Enter close and write.  Every field is already valid (year takes
        // digits only), and a cleared name falls back to the original, so the
        // editor always commits and never traps.
        KeyCode::Esc | KeyCode::Enter => {
            let typed = fields[0].value.trim();
            let name = if typed.is_empty() {
                original_name.to_string()
            } else {
                typed.to_string()
            };
            let artist = nonempty_opt(&fields[1].value);
            let album = nonempty_opt(&fields[2].value);
            let year = fields[META_YEAR].value.trim().parse().ok();
            let genre = nonempty_opt(&fields[4].value);
            if matches!(code, KeyCode::Enter) && *active_field == 5 {
                ModalOutcome::Confirm(ModalConfirm::SaveMetadataAndEditLyrics {
                    track_id,
                    name,
                    artist,
                    album,
                    year,
                    genre,
                })
            } else {
                ModalOutcome::Confirm(ModalConfirm::SaveMetadata {
                    track_id,
                    name,
                    artist,
                    album,
                    year,
                    genre,
                })
            }
        }
        KeyCode::Tab | KeyCode::Down => {
            *active_field = (*active_field + 1) % ROWS;
            ModalOutcome::Consumed
        }
        KeyCode::BackTab | KeyCode::Up => {
            *active_field = (*active_field + ROWS - 1) % ROWS;
            ModalOutcome::Consumed
        }
        // Text-input keys only apply to the 5 text fields (rows 0-4).
        // The year field accepts digits only, so it is never invalid.
        KeyCode::Char(c) if *active_field < 5 => {
            if *active_field != META_YEAR || c.is_ascii_digit() {
                fields[*active_field].push(c);
            }
            ModalOutcome::Consumed
        }
        KeyCode::Backspace if *active_field < 5 => {
            fields[*active_field].backspace();
            ModalOutcome::Consumed
        }
        KeyCode::Left if *active_field < 5 => {
            fields[*active_field].move_left();
            ModalOutcome::Consumed
        }
        KeyCode::Right if *active_field < 5 => {
            fields[*active_field].move_right();
            ModalOutcome::Consumed
        }
        _ => ModalOutcome::Consumed,
    }
}

fn handle_edit_lyrics_key(
    code: KeyCode,
    track_id: TrackId,
    textarea: &mut TextArea,
) -> ModalOutcome {
    match code {
        KeyCode::Esc => {
            let s = textarea.as_string();
            let lyrics = if s.trim().is_empty() { None } else { Some(s) };
            return ModalOutcome::Confirm(ModalConfirm::SaveLyrics { track_id, lyrics });
        }
        KeyCode::Enter => textarea.insert_newline(),
        KeyCode::Backspace => textarea.delete_char(),
        KeyCode::Delete => textarea.delete_next_char(),
        KeyCode::Up => textarea.move_up(),
        KeyCode::Down => textarea.move_down(),
        KeyCode::Left => textarea.move_left(),
        KeyCode::Right => textarea.move_right(),
        KeyCode::Home => textarea.move_line_start(),
        KeyCode::End => textarea.move_line_end(),
        KeyCode::Char(c) => textarea.insert_char(c),
        _ => {}
    }
    ModalOutcome::Consumed
}

fn handle_add_to_playlist_key(
    code: KeyCode,
    choices: &[(PlaylistId, String)],
    cursor: &mut usize,
    track_id: TrackId,
) -> ModalOutcome {
    if let Some(new) = crate::nav::list_move(code, *cursor, choices.len()) {
        *cursor = new;
        return ModalOutcome::Consumed;
    }
    match code {
        KeyCode::Enter => {
            if let Some((playlist_id, _)) = choices.get(*cursor) {
                ModalOutcome::Confirm(ModalConfirm::AddToPlaylist {
                    track_id,
                    playlist_id: *playlist_id,
                })
            } else {
                ModalOutcome::Dismissed
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => ModalOutcome::Dismissed,
        _ => ModalOutcome::Consumed,
    }
}

/// Uniform yes/no decision shared by every confirmation dialog.
enum Confirm {
    Yes,
    No,
    Ignore,
}

const fn confirm_key(code: KeyCode) -> Confirm {
    match code {
        KeyCode::Char('y' | 'Y') | KeyCode::Enter => Confirm::Yes,
        KeyCode::Char('n' | 'N' | 'q') | KeyCode::Esc => Confirm::No,
        _ => Confirm::Ignore,
    }
}

impl Modal {
    pub fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> ModalOutcome {
        match self {
            Self::Notify { .. } | Self::Help | Self::About => ModalOutcome::Dismissed,

            Self::ConfirmQuit => match confirm_key(code) {
                Confirm::Yes => ModalOutcome::Confirm(ModalConfirm::Quit),
                Confirm::No => ModalOutcome::Dismissed,
                Confirm::Ignore => ModalOutcome::Consumed,
            },

            Self::Menu { cursor } => {
                if let Some(new) = crate::nav::list_move(code, *cursor, MENU_ENTRIES) {
                    *cursor = new;
                    return ModalOutcome::Consumed;
                }
                match code {
                    KeyCode::Enter => match *cursor {
                        0 => ModalOutcome::Confirm(ModalConfirm::OpenSettings),
                        1 => ModalOutcome::Confirm(ModalConfirm::OpenAbout),
                        _ => ModalOutcome::Confirm(ModalConfirm::Quit),
                    },
                    KeyCode::Esc | KeyCode::Char('q') => ModalOutcome::Dismissed,
                    _ => ModalOutcome::Consumed,
                }
            }

            Self::ShuffleView { .. } => match confirm_key(code) {
                Confirm::Yes => ModalOutcome::Confirm(ModalConfirm::ShuffleView),
                Confirm::No => ModalOutcome::Dismissed,
                Confirm::Ignore => ModalOutcome::Consumed,
            },

            Self::Settings(state) => handle_settings_key(code, state),

            Self::ConfirmRemove { target, .. } => match confirm_key(code) {
                Confirm::Yes => ModalOutcome::Confirm(ModalConfirm::Remove(*target)),
                Confirm::No => ModalOutcome::Dismissed,
                Confirm::Ignore => ModalOutcome::Consumed,
            },

            // Editing existing data, so Enter and Esc both write the change;
            // a blank name keeps the original (nothing is lost).
            Self::EditPlaylist { id, input } => match handle_text_key(input, code) {
                TextInputResult::Submitted(name) => {
                    ModalOutcome::Confirm(ModalConfirm::RenamePlaylist {
                        id: *id,
                        new_name: name,
                    })
                }
                TextInputResult::Dismissed => {
                    let name = input.value.trim();
                    if name.is_empty() {
                        ModalOutcome::Dismissed
                    } else {
                        ModalOutcome::Confirm(ModalConfirm::RenamePlaylist {
                            id: *id,
                            new_name: name.to_string(),
                        })
                    }
                }
                TextInputResult::Consumed => ModalOutcome::Consumed,
            },

            Self::NewPlaylist { input, add_track } => match handle_text_key(input, code) {
                TextInputResult::Submitted(name) => {
                    ModalOutcome::Confirm(ModalConfirm::NewPlaylist {
                        name,
                        add_track: *add_track,
                    })
                }
                TextInputResult::Dismissed => ModalOutcome::Dismissed,
                TextInputResult::Consumed => ModalOutcome::Consumed,
            },

            Self::AddToPlaylist {
                choices,
                cursor,
                track_id,
                ..
            } => handle_add_to_playlist_key(code, choices, cursor, *track_id),

            Self::EditMetadata {
                track_id,
                fields,
                active_field,
                original_name,
            } => handle_edit_metadata_key(code, *track_id, fields, active_field, original_name),

            Self::EditLyrics { track_id, textarea } => {
                handle_edit_lyrics_key(code, *track_id, textarea)
            }
        }
    }
}

// -- Rendering --------------------------------------------------------------

pub fn render_modal(frame: &mut Frame<'_>, modal: &Modal, theme: &Theme) {
    match modal {
        Modal::Notify { message } => render_notification(frame, "Notice", message, theme),
        Modal::Help => render_help(frame, theme),
        Modal::About => render_about(frame, theme),
        Modal::ConfirmQuit => render_confirm(frame, "Quit audium?", theme),
        Modal::Menu { cursor } => render_menu(frame, *cursor, theme),
        Modal::ConfirmRemove { description, .. } => render_confirm(frame, description, theme),
        Modal::EditPlaylist { input, .. } => {
            render_text_input(
                frame,
                "Edit Playlist",
                input,
                &[hint("Enter / Esc", "save")],
                theme,
            );
        }
        Modal::NewPlaylist { input, add_track } => {
            let (title, create) = if add_track.is_some() {
                ("Add to New Playlist", "create & add")
            } else {
                ("New Playlist", "create")
            };
            render_text_input(
                frame,
                title,
                input,
                &[hint("Enter", create), hint("Esc", "cancel")],
                theme,
            );
        }
        Modal::AddToPlaylist {
            track_name,
            choices,
            cursor,
            ..
        } => render_playlist_picker(frame, track_name, choices, *cursor, theme),
        Modal::Settings(state) => render_settings(frame, state, theme),
        Modal::ShuffleView { view_name } => render_confirm(
            frame,
            &format!("Shuffle \"{view_name}\"? This will clear the current queue."),
            theme,
        ),
        Modal::EditMetadata {
            fields,
            active_field,
            ..
        } => render_edit_metadata(frame, fields, *active_field, theme),
        Modal::EditLyrics { textarea, .. } => render_edit_lyrics(frame, textarea, theme),
    }
}

// -- Overlay helpers --------------------------------------------------------

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

/// Every modal is inset from its border by [`MODAL_PAD_X`] columns and
/// [`MODAL_PAD_Y`] rows on all four sides.  Applying it here rather than in
/// each renderer is what keeps the gap identical across dialogs: a renderer
/// only ever lays out content, never its own margins.
///
/// Consequence for sizing: a modal's height is `content rows + 2 borders +
/// 2 * MODAL_PAD_Y`, and its usable width is `width - 2 - 2 * MODAL_PAD_X`.
pub const MODAL_PAD_X: u16 = 2;
pub const MODAL_PAD_Y: u16 = 1;

/// Rows and columns a modal spends on its border plus the inset above.
pub const MODAL_CHROME_H: u16 = 2 + 2 * MODAL_PAD_Y;

// -- Dialog hint footer -----------------------------------------------------

/// One key hint in a dialog footer.
#[derive(Clone, Copy)]
pub struct Hint<'a> {
    key: &'a str,
    action: &'a str,
    /// Rendered in the danger color: this key destroys something.
    danger: bool,
}

pub const fn hint<'a>(key: &'a str, action: &'a str) -> Hint<'a> {
    Hint {
        key,
        action,
        danger: false,
    }
}

const fn danger_hint<'a>(key: &'a str, action: &'a str) -> Hint<'a> {
    Hint {
        key,
        action,
        danger: true,
    }
}

/// Renders a dialog's hints as `[key] action` pairs joined by the theme
/// separator, e.g. `[Tab] next field  -  [Esc] close`.
///
/// The brackets carry the key/action boundary. Spacing alone cannot: a reader
/// has no way to tell a wide gap *within* a pair from the gap *between* two,
/// and the color difference disappears on a monochrome tty.
///
/// Wraps at whole pairs, never mid-pair, so a hint gains a row instead of
/// being cut off. Size the dialog with [`hint_height`] first.
fn hint_lines<'a>(hints: &[Hint<'a>], width: usize, theme: &Theme) -> Vec<Line<'a>> {
    let sep = theme.glyphs().sep;
    let sep_w = sep.chars().count();
    let action_style = Style::default().fg(theme.subtle);

    let mut lines: Vec<Line<'a>> = Vec::new();
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut used = 0usize;

    for h in hints {
        let pair_w = h.key.chars().count() + h.action.chars().count() + 3; // "[k] a"
        let needed = if spans.is_empty() {
            pair_w
        } else {
            pair_w + sep_w
        };

        if !spans.is_empty() && used + needed > width {
            lines.push(Line::from(std::mem::take(&mut spans)));
            used = 0;
        }
        if !spans.is_empty() {
            spans.push(Span::styled(sep, action_style));
            used += sep_w;
        }

        let key_style = if h.danger {
            Style::default()
                .fg(theme.danger)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.accent)
        };
        spans.push(Span::styled(format!("[{}]", h.key), key_style));
        spans.push(Span::styled(format!(" {}", h.action), action_style));
        used += pair_w;
    }

    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }
    lines
}

/// Rows [`hint_lines`] will occupy at `width`.
pub fn hint_height(hints: &[Hint<'_>], width: usize, theme: &Theme) -> u16 {
    usize_to_u16_saturating(hint_lines(hints, width, theme).len())
}

/// Draws the hint footer centred in `area`.  Every dialog puts it at the
/// bottom; see the UI conventions in the README.
pub fn render_hints(frame: &mut Frame<'_>, area: Rect, hints: &[Hint<'_>], theme: &Theme) {
    frame.render_widget(
        Paragraph::new(hint_lines(hints, area.width as usize, theme)).alignment(Alignment::Center),
        area,
    );
}

/// Rows `text` occupies once wrapped to `width`, so a dialog can be sized
/// from its message instead of reserving a fixed guess that clips anything
/// longer.  Greedy whitespace wrapping, matching `Paragraph`'s.
fn wrapped_height(text: &str, width: usize) -> u16 {
    if width == 0 {
        return 1;
    }
    let mut rows = 1usize;
    let mut used = 0usize;
    for word in text.split_whitespace() {
        let w = word.chars().count();
        if used == 0 {
            used = w;
        } else if used + 1 + w <= width {
            used += 1 + w;
        } else {
            rows += 1;
            used = w;
        }
    }
    usize_to_u16_saturating(rows)
}

/// Usable content width inside a modal of total width `w`.
const fn modal_inner_width(w: u16) -> usize {
    (w.saturating_sub(2 + 2 * MODAL_PAD_X)) as usize
}

pub fn modal_block<'a>(title: &'a str, theme: &Theme) -> Block<'a> {
    Block::default()
        .padding(Padding::new(
            MODAL_PAD_X,
            MODAL_PAD_X,
            MODAL_PAD_Y,
            MODAL_PAD_Y,
        ))
        .title(format!(" {title} "))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .style(theme.apply_bg(Style::default()))
}

// -- Individual renderers ---------------------------------------------------

fn render_notification(frame: &mut Frame<'_>, title: &str, message: &str, theme: &Theme) {
    const WIDTH: u16 = 50;
    let hints = [hint("any key", "dismiss")];
    let hint_h = hint_height(&hints, modal_inner_width(WIDTH), theme);
    let msg_h = wrapped_height(message, modal_inner_width(WIDTH));

    let area = frame.area();
    let rect = centered_rect(WIDTH, msg_h + 1 + hint_h + MODAL_CHROME_H, area);
    frame.render_widget(Clear, rect);

    let block = modal_block(title, theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(msg_h),  // message
            Constraint::Length(1),      // gap above the hints
            Constraint::Length(hint_h), // hints
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(message, Style::default().fg(theme.text)))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        rows[0],
    );
    render_hints(frame, rows[2], &hints, theme);
}

fn render_confirm(frame: &mut Frame<'_>, description: &str, theme: &Theme) {
    const WIDTH: u16 = 52;
    let hints = [
        danger_hint("Y / Enter", "confirm"),
        hint("N / Esc", "cancel"),
    ];
    let hint_h = hint_height(&hints, modal_inner_width(WIDTH), theme);
    let msg_h = wrapped_height(description, modal_inner_width(WIDTH));

    let area = frame.area();
    let rect = centered_rect(WIDTH, msg_h + 1 + hint_h + MODAL_CHROME_H, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Confirm", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(msg_h),  // message
            Constraint::Length(1),      // gap above the hints
            Constraint::Length(hint_h), // hints
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(description, Style::default().fg(theme.text)))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        rows[0],
    );
    render_hints(frame, rows[2], &hints, theme);
}

fn render_text_input(
    frame: &mut Frame<'_>,
    title: &str,
    input: &TextInput,
    hints: &[Hint<'_>],
    theme: &Theme,
) {
    const WIDTH: u16 = 52;
    let hint_h = hint_height(hints, modal_inner_width(WIDTH), theme);

    let area = frame.area();
    let rect = centered_rect(WIDTH, 3 + hint_h + MODAL_CHROME_H, area);
    frame.render_widget(Clear, rect);

    let block = modal_block(title, theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // label
            Constraint::Length(1),      // input
            Constraint::Length(1),      // spacer
            Constraint::Length(hint_h), // hints
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "Enter name:",
            Style::default().fg(theme.text_dim),
        )),
        rows[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(cursor_spans_windowed(
            &input.value,
            input.cursor,
            rows[1].width as usize,
            theme,
        ))),
        rows[1],
    );

    render_hints(frame, rows[3], hints, theme);
}

fn render_playlist_picker(
    frame: &mut Frame<'_>,
    track_name: &str,
    choices: &[(PlaylistId, String)],
    cursor: usize,
    theme: &Theme,
) {
    const WIDTH: u16 = 52;
    let area = frame.area();
    // track name + spacer + one row per choice + gap + hints, plus the chrome.
    let hints = [
        hint("Enter", "add"),
        hint("j/k", "navigate"),
        hint("Esc", "cancel"),
    ];
    let hint_h = hint_height(&hints, modal_inner_width(WIDTH), theme);
    let height = (usize_to_u16_saturating(choices.len()) + 3 + hint_h + MODAL_CHROME_H)
        .min(area.height.saturating_sub(4));
    let rect = centered_rect(WIDTH, height, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Add to Playlist", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // track name
            Constraint::Length(1),      // spacer
            Constraint::Min(0),         // choices
            Constraint::Length(1),      // gap above the hints
            Constraint::Length(hint_h), // hints
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("Track: {track_name}"),
            Style::default().fg(theme.text_dim),
        )),
        rows[0],
    );

    if choices.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "No playlists yet.",
                Style::default().fg(theme.subtle),
            )),
            rows[2],
        );
    } else {
        let items: Vec<ListItem<'_>> = choices
            .iter()
            .map(|(_, name)| ListItem::new(name.clone()))
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(cursor));

        frame.render_stateful_widget(
            List::new(items)
                .highlight_style(theme.selection_style())
                .highlight_symbol("> "),
            rows[2],
            &mut list_state,
        );
    }

    render_hints(frame, rows[4], &hints, theme);
}

fn render_help(frame: &mut Frame<'_>, theme: &Theme) {
    let bindings: &[(&str, &str)] = &[
        ("q", "Quit"),
        ("Tab / S-Tab", "Next / previous panel"),
        ("?", "Toggle this list"),
        ("", ""),
        ("j / Down", "Move cursor down"),
        ("k / Up", "Move cursor up"),
        ("g / G", "Jump to top / bottom"),
        ("PgUp / PgDn", "Page up / down"),
        ("", ""),
        ("Space", "Play / Pause"),
        ("n", "Next track"),
        ("N", "Previous track"),
        ("Left / Right", "Seek backward / forward"),
        ("+ / =", "Volume up"),
        ("-", "Volume down"),
        ("l", "Cycle loop mode"),
        ("[  /  ]", "Speed down / up"),
        ("", ""),
        ("Enter", "Play selected track"),
        ("a", "Add selected track / list to queue"),
        ("p", "Add track to playlist"),
        ("d", "Remove selected item"),
        ("D", "Clear queue"),
        ("e", "Edit selected track / playlist"),
        ("y", "Toggle lyrics overlay"),
        ("", ""),
        ("c", "Create new playlist"),
        ("z", "Shuffle current view into queue"),
        ("", ""),
        ("f", "Open file picker"),
        ("/", "Filter tracklist"),
        ("", ""),
        ("m", "Open menu"),
    ];

    // Height follows the table, so it cannot drift when a key is added or
    // removed and leave a lopsided gap at the bottom.
    let area = frame.area();
    let height = usize_to_u16_saturating(bindings.len()).saturating_add(MODAL_CHROME_H);
    let rect = centered_rect(60, height, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Keybindings", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    // Size the key column to the widest key so no row can push its description
    // out of alignment.
    let key_w = bindings.iter().map(|(key, _)| key.len()).max().unwrap_or(0);

    let items: Vec<Line<'_>> = bindings
        .iter()
        .map(|(key, desc)| {
            if key.is_empty() {
                Line::from("")
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("{key:>key_w$}  "),
                        Style::default().fg(theme.accent),
                    ),
                    Span::styled(*desc, Style::default().fg(theme.text_dim)),
                ])
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(items), inner);
}

fn render_menu(frame: &mut Frame<'_>, cursor: usize, theme: &Theme) {
    // Wide enough that the hint footer wraps to two rows, not three, which
    // would leave this small dialog bottom-heavy.
    const WIDTH: u16 = 40;
    let area = frame.area();
    let hints = [
        hint("j/k", "navigate"),
        hint("Enter", "select"),
        hint("Esc", "close"),
    ];
    let hint_h = hint_height(&hints, modal_inner_width(WIDTH), theme);
    let rect = centered_rect(WIDTH, 4 + hint_h + MODAL_CHROME_H, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Menu", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let entries = ["Settings", "About", "Quit"];

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // Settings
            Constraint::Length(1),      // About
            Constraint::Length(1),      // Quit
            Constraint::Min(1),         // gap above the hints
            Constraint::Length(hint_h), // hints
        ])
        .split(inner);

    for (i, label) in entries.iter().enumerate() {
        let selected = cursor == i;
        let prefix = if selected {
            theme.glyphs().marker
        } else {
            "   "
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(theme.accent)),
                Span::styled(
                    *label,
                    if selected {
                        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_dim)
                    },
                ),
            ])),
            rows[i],
        );
    }

    render_hints(frame, rows[4], &hints, theme);
}

fn render_about(frame: &mut Frame<'_>, theme: &Theme) {
    const WIDTH: u16 = 64;
    // Single pure-ASCII logo, rendered the same on every terminal.
    const LOGO: [&str; 7] = [
        "                          mm     ##                       ",
        "                          ##     \"\"                       ",
        " m#####m  ##    ##   m###m##   ####     ##    ##  ####m##m",
        " \" mmm##  ##    ##  ##\"  \"##     ##     ##    ##  ## ## ##",
        "m##\"\"\"##  ##    ##  ##    ##     ##     ##    ##  ## ## ##",
        "##mmm###  ##mmm###  \"##mm###  mmm##mmm  ##mmm###  ## ## ##",
        " \"\"\"\" \"\"   \"\"\"\" \"\"    \"\"\" \"\"  \"\"\"\"\"\"\"\"   \"\"\"\" \"\"  \"\" \"\" \"\"",
    ];

    let hints = [hint("any key", "close")];
    let hint_h = hint_height(&hints, modal_inner_width(WIDTH), theme);

    let area = frame.area();
    // logo(7) + spacer + 4 meta rows + spacer, then the hints.
    let rect = centered_rect(WIDTH, 13 + hint_h + MODAL_CHROME_H, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("About", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),      // logo
            Constraint::Length(1),      // spacer
            Constraint::Length(1),      // version
            Constraint::Length(1),      // author
            Constraint::Length(1),      // license
            Constraint::Length(1),      // repo
            Constraint::Min(1),         // gap above the hints
            Constraint::Length(hint_h), // hints
        ])
        .split(inner);

    let logo: Vec<Line<'_>> = LOGO
        .iter()
        .map(|l| {
            Line::from(Span::styled(
                *l,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    frame.render_widget(Paragraph::new(logo).alignment(Alignment::Center), rows[0]);

    let version = env!("CARGO_PKG_VERSION");
    let meta: [(&str, &str); 4] = [
        ("version", version),
        ("author", "takashialpha"),
        ("license", "GPL-3.0-or-later"),
        ("source", "github.com/takashialpha/audium"),
    ];

    for (i, (label, value)) in meta.iter().enumerate() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("{label:>8}  "), Style::default().fg(theme.subtle)),
                Span::styled(*value, Style::default().fg(theme.text)),
            ])),
            rows[2 + i],
        );
    }

    render_hints(frame, rows[7], &hints, theme);
}

fn render_settings(frame: &mut Frame<'_>, view: &SettingsState, theme: &Theme) {
    const WIDTH: u16 = 60;
    let truecolor = view.color_mode.truecolor(view.detected_truecolor);
    let area = frame.area();
    let hints = [
        hint("j/k", "select"),
        hint("h/l", "adjust"),
        hint("Enter / Esc", "close"),
    ];
    let hint_h = hint_height(&hints, modal_inner_width(WIDTH), theme);
    // banner(1) + spacer(1) + 5xrow(3) + footnote(2) + spacer(1), then hints.
    let rect = centered_rect(WIDTH, 20 + hint_h + MODAL_CHROME_H, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Settings", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // banner
            Constraint::Length(1),      // spacer
            Constraint::Length(3),      // volume
            Constraint::Length(3),      // seek
            Constraint::Length(3),      // color mode
            Constraint::Length(3),      // theme
            Constraint::Length(3),      // transparency
            Constraint::Length(2),      // footnote (wraps to 2 lines)
            Constraint::Min(1),         // gap above the hints
            Constraint::Length(hint_h), // hints
        ])
        .split(inner);

    render_settings_header(frame, rows[0], view, theme);

    render_settings_row(
        frame,
        rows[2],
        "Default volume",
        view.cursor == SET_VOLUME,
        volume_bar(view.volume_pct, theme),
        theme,
    );
    render_settings_row(
        frame,
        rows[3],
        "Seek step",
        view.cursor == SET_SEEK,
        seek_display(view.seek_secs, theme),
        theme,
    );
    render_settings_row(
        frame,
        rows[4],
        "Color mode",
        view.cursor == SET_COLOR_MODE,
        color_mode_display(view.color_mode, theme),
        theme,
    );

    // Both modes have themes; only transparency depends on truecolor.
    let theme_name = if truecolor {
        themes()[view.preview_theme_idx].name
    } else {
        console_themes()[view.preview_console_idx].name
    };
    let theme_value = theme_cycle_display(theme_name, theme);
    render_settings_row(
        frame,
        rows[5],
        "Theme",
        view.cursor == SET_THEME,
        theme_value,
        theme,
    );

    let transparency_value = if truecolor {
        toggle_display(if view.transparent { "on" } else { "off" }, theme)
    } else {
        locked_display("unavailable", theme)
    };
    render_settings_row(
        frame,
        rows[6],
        "Terminal transparency",
        view.cursor == SET_TRANSPARENCY,
        transparency_value,
        theme,
    );

    // Footnote follows the selected row in both modes.  Locked rows can't be
    // selected, so the color-mode row's description carries the override hint.
    frame.render_widget(
        Paragraph::new(Span::styled(
            settings_row_description(view.cursor, truecolor),
            Style::default().fg(theme.text_dim),
        ))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        rows[7],
    );

    render_hints(frame, rows[9], &hints, theme);
}

/// One-line description of a settings row, shown in the footnote when selected.
const fn settings_row_description(cursor: usize, truecolor: bool) -> &'static str {
    match cursor {
        SET_VOLUME => "Volume applied each time audium starts.",
        SET_SEEK => "How far the seek keys jump, in seconds.",
        SET_COLOR_MODE if truecolor => {
            "Auto follows what your terminal reports. Switch to 16-color if the colors look wrong."
        }
        SET_COLOR_MODE => {
            "Auto follows what your terminal reports. Switch to Truecolor only if it does support it."
        }
        SET_THEME if truecolor => "Color scheme for the interface.",
        SET_THEME => {
            "Console themes track your terminal's own palette. Pick the one matching its background."
        }
        SET_TRANSPARENCY if truecolor => {
            "Shows the terminal background through the UI. Best with a transparent or blurred terminal."
        }
        // SET_TRANSPARENCY, 16-color
        _ => "Console themes already leave the background untouched, so there is nothing to blend.",
    }
}

/// Renders the settings modal header: capability banner plus key hint.
fn render_settings_header(
    frame: &mut Frame<'_>,
    banner_area: Rect,
    view: &SettingsState,
    theme: &Theme,
) {
    frame.render_widget(
        Paragraph::new(terminal_banner(
            view.color_mode,
            view.detected_truecolor,
            theme,
        ))
        .alignment(Alignment::Center),
        banner_area,
    );
}

/// The terminal-capability banner shown at the top of the settings modal.
fn terminal_banner(color_mode: ColorMode, detected: bool, theme: &Theme) -> Line<'static> {
    let truecolor = color_mode.truecolor(detected);
    let (label, color) = if truecolor {
        ("truecolor", theme.now_playing)
    } else {
        ("16-color console", theme.dir_col)
    };
    let source = if matches!(color_mode, ColorMode::Auto) {
        "auto-detected"
    } else {
        "forced"
    };
    Line::from(vec![
        Span::styled(theme.glyphs().bullet, Style::default().fg(color)),
        Span::raw(" "),
        Span::styled("Terminal: ", Style::default().fg(theme.text_dim)),
        Span::styled(
            label,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  ({source})"), Style::default().fg(theme.subtle)),
    ])
}

fn render_settings_row<'a>(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &'a str,
    selected: bool,
    value_line: Line<'a>,
    theme: &Theme,
) {
    let row_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(if selected { theme.accent } else { theme.subtle }));
    let row_inner = row_block.inner(area);
    frame.render_widget(row_block, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(24)])
        .split(row_inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            label,
            Style::default().fg(if selected { theme.text } else { theme.text_dim }),
        )),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(value_line).alignment(Alignment::Right),
        cols[1],
    );
}

/// Wraps a row's value span(s) between the left/right cycle arrows.
fn cycle_line(middle: Vec<Span<'static>>, theme: &Theme) -> Line<'static> {
    let g = theme.glyphs();
    let arrow = Style::default().fg(theme.subtle);
    let mut spans = Vec::with_capacity(middle.len() + 4);
    spans.push(Span::styled(g.arrow_left, arrow));
    spans.push(Span::raw("  "));
    spans.extend(middle);
    spans.push(Span::raw("  "));
    spans.push(Span::styled(g.arrow_right, arrow));
    Line::from(spans)
}

/// A bold value styled with `color`, the common middle of most cycle rows.
fn cycle_value(value: &str, color: Color, theme: &Theme) -> Line<'static> {
    cycle_line(
        vec![Span::styled(
            value.to_owned(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )],
        theme,
    )
}

fn volume_bar(pct: u32, theme: &Theme) -> Line<'static> {
    let filled = (pct / 10) as usize;
    let empty = 10usize.saturating_sub(filled);
    let g = theme.glyphs();
    let bar = g.bar_fill.repeat(filled) + &g.bar_empty.repeat(empty);
    cycle_line(
        vec![
            Span::styled(bar, Style::default().fg(theme.accent)),
            Span::styled(
                format!(" {pct:>3}%"),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
        ],
        theme,
    )
}

fn seek_display(secs: u64, theme: &Theme) -> Line<'static> {
    cycle_value(&format_duration(secs), theme.text, theme)
}

fn theme_cycle_display(name: &str, theme: &Theme) -> Line<'static> {
    cycle_value(name, theme.accent, theme)
}

fn color_mode_display(mode: ColorMode, theme: &Theme) -> Line<'static> {
    cycle_value(mode.label(), theme.accent, theme)
}

fn toggle_display(value: &str, theme: &Theme) -> Line<'static> {
    let col = if value == "on" {
        theme.now_playing
    } else {
        theme.text_dim
    };
    cycle_value(value, col, theme)
}

/// A dimmed, non-interactive value shown for rows locked by the console fallback.
fn locked_display(value: &'static str, theme: &Theme) -> Line<'static> {
    Line::from(Span::styled(value, Style::default().fg(theme.subtle)))
}

// -- EditMetadata renderer --------------------------------------------------

const META_LABELS: [&str; 5] = ["Name", "Artist", "Album", "Year", "Genre"];

fn render_metadata_field(
    frame: &mut Frame<'_>,
    row: Rect,
    label: &str,
    input: &TextInput,
    is_active: bool,
    theme: &Theme,
) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(row);

    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("{label:>7}  "),
            Style::default().fg(if is_active {
                theme.accent
            } else {
                theme.text_dim
            }),
        )),
        cols[0],
    );

    if is_active {
        frame.render_widget(
            Paragraph::new(Line::from(cursor_spans_windowed(
                &input.value,
                input.cursor,
                cols[1].width as usize,
                theme,
            ))),
            cols[1],
        );
    } else {
        // Inactive fields have no cursor to follow, so a long value is marked
        // as clipped rather than silently cut off at the border.
        let (text, style) = if input.value.is_empty() {
            ("-".to_string(), Style::default().fg(theme.subtle))
        } else {
            (
                truncate(&input.value, cols[1].width as usize),
                Style::default().fg(theme.text_dim),
            )
        };
        frame.render_widget(Paragraph::new(Span::styled(text, style)), cols[1]);
    }
}

fn render_edit_lyrics_button(frame: &mut Frame<'_>, row: Rect, active: bool, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                if active { theme.glyphs().marker } else { "   " },
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!("Edit Lyrics {}", theme.glyphs().arrow_right),
                Style::default()
                    .fg(if active { theme.text } else { theme.text_dim })
                    .add_modifier(if active {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ])),
        row,
    );
}

fn render_edit_metadata(
    frame: &mut Frame<'_>,
    fields: &[TextInput; 5],
    active_field: usize,
    theme: &Theme,
) {
    const WIDTH: u16 = 62;
    let area = frame.area();
    let hints = [
        hint("Tab", "next field"),
        hint("arrows", "move cursor"),
        hint("Esc / Enter", "close"),
    ];
    let hint_h = hint_height(&hints, modal_inner_width(WIDTH), theme);
    let rect = centered_rect(WIDTH, 8 + hint_h + MODAL_CHROME_H, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Edit Track", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // Name
            Constraint::Length(1),      // Artist
            Constraint::Length(1),      // Album
            Constraint::Length(1),      // Year
            Constraint::Length(1),      // Genre
            Constraint::Length(1),      // spacer
            Constraint::Length(1),      // Edit Lyrics -> button
            Constraint::Min(1),         // gap above the hints
            Constraint::Length(hint_h), // hints
        ])
        .split(inner);

    for (i, (label, input)) in META_LABELS.iter().zip(fields.iter()).enumerate() {
        render_metadata_field(frame, rows[i], label, input, active_field == i, theme);
    }

    render_edit_lyrics_button(frame, rows[6], active_field == 5, theme);
    render_hints(frame, rows[8], &hints, theme);
}

// -- EditLyrics / tui-textarea renderer ------------------------------------

/// Creates a `TextArea` (lyrics editor) pre-populated with `raw`.
pub fn make_lyrics_textarea(raw: &str) -> LyricsTextArea {
    TextArea::from_text(raw)
}

fn render_edit_lyrics(frame: &mut Frame<'_>, textarea: &TextArea, theme: &Theme) {
    let area = frame.area();
    let width = area.width.saturating_sub(8).max(40);
    let height = area.height.saturating_sub(4).max(12);
    let rect = centered_rect(width, height, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Edit Lyrics", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let hints = [
        hint("arrows", "navigate"),
        hint("Home/End", "line start/end"),
        hint("Enter", "new line"),
        hint("Backspace", "delete"),
        hint("Esc", "save"),
    ];
    let hint_h = hint_height(&hints, inner.width as usize, theme);

    let splits = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),         // editor
            Constraint::Length(1),      // gap below the editor
            Constraint::Length(1),      // LRC format note
            Constraint::Length(hint_h), // hints
        ])
        .split(inner);

    // Not a keybinding, so it sits above the footer rather than inside it.
    frame.render_widget(
        Paragraph::new(Span::styled(
            "synced lyrics: prefix a line with [mm:ss.xx]",
            Style::default().fg(theme.subtle),
        ))
        .alignment(Alignment::Center),
        splits[2],
    );
    render_hints(frame, splits[3], &hints, theme);

    let visible = usize::from(splits[0].height);
    let scroll = textarea
        .cursor_row
        .saturating_sub(visible.saturating_sub(1))
        .min(textarea.lines.len().saturating_sub(visible));

    let items: Vec<Line<'_>> = textarea
        .lines
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(row, line)| {
            if row == textarea.cursor_row {
                Line::from(cursor_spans(line, textarea.cursor_col, theme))
            } else {
                Line::from(Span::styled(
                    line.as_str(),
                    Style::default().fg(theme.text_dim),
                ))
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(items), splits[0]);
}
