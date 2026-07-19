use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::library::{PlaylistId, TrackId};
use crate::numeric::usize_to_u16_saturating;
use crate::settings::ColorMode;
use crate::ui::layout::{Theme, cursor_spans, format_duration, themes};

// ── Text-input widget ──────────────────────────────────────────────────────

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

// ── Multi-line text editor ─────────────────────────────────────────────────

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

// ── RemoveTarget ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum RemoveTarget {
    TrackFromQueue { queue_idx: usize },
    TrackFromLibrary { track_id: TrackId },
    Playlist { playlist_id: PlaylistId },
    Queue,
}

// ── Settings modal state ───────────────────────────────────────────────────

/// Live state of the settings modal, mutated in place while it is open.
#[derive(Debug, Clone)]
pub struct SettingsState {
    pub cursor: usize,
    pub volume_pct: u32,
    pub seek_secs: u64,
    pub preview_theme_idx: usize,
    pub transparent: bool,
    /// Editable color-mode preference (Auto / Truecolor / 16-color).
    pub color_mode: ColorMode,
    /// What truecolor auto-detection found; drives the banner and whether the
    /// theme / transparency rows are interactive.
    pub detected_truecolor: bool,
}

// ── Modal variants ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Modal {
    Notify {
        message: String,
    },
    ConfirmRemove {
        description: String,
        target: RemoveTarget,
    },
    Rename {
        kind: String,
        id: u64,
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
    ShufflePlaylist {
        playlist_id: PlaylistId,
        playlist_name: String,
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

// ── ModalOutcome ───────────────────────────────────────────────────────────

pub enum ModalOutcome {
    Consumed,
    Confirm(ModalConfirm),
    Dismissed,
}

#[derive(Debug)]
pub enum ModalConfirm {
    Remove(RemoveTarget),
    Rename {
        kind: String,
        id: u64,
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
        transparent: bool,
        color_mode: ColorMode,
    },
    /// Apply a theme live during settings preview without closing the modal.
    PreviewTheme {
        theme_name: String,
        transparent: bool,
        color_mode: ColorMode,
    },
    ShufflePlaylist {
        playlist_id: PlaylistId,
    },
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

// ── Shared helpers ─────────────────────────────────────────────────────────

fn nonempty_opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

// ── Text-input key helper ──────────────────────────────────────────────────

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

// ── Input handling ─────────────────────────────────────────────────────────

// Settings rows, in display order.
const SET_VOLUME: usize = 0;
const SET_SEEK: usize = 1;
const SET_COLOR_MODE: usize = 2;
const SET_THEME: usize = 3;
const SET_TRANSPARENCY: usize = 4;
const SET_ROWS: usize = 5;

/// Which rows accept input.  Theme and transparency are locked (and skipped by
/// the cursor) whenever truecolor is not in effect, since the console fallback
/// ignores both.
const fn settings_enabled(color_mode: ColorMode, detected: bool) -> [bool; SET_ROWS] {
    let tc = color_mode.truecolor(detected);
    [true, true, true, tc, tc]
}

fn handle_settings_key(code: KeyCode, s: &mut SettingsState) -> ModalOutcome {
    let enabled = settings_enabled(s.color_mode, s.detected_truecolor);
    let preview = |s: &SettingsState| {
        ModalOutcome::Confirm(ModalConfirm::PreviewTheme {
            theme_name: themes()[s.preview_theme_idx].name.to_string(),
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
                    s.color_mode = if left {
                        s.color_mode.prev()
                    } else {
                        s.color_mode.next()
                    };
                    preview(s)
                }
                SET_THEME => {
                    if left {
                        s.preview_theme_idx = s
                            .preview_theme_idx
                            .checked_sub(1)
                            .unwrap_or_else(|| themes().len() - 1);
                    } else {
                        s.preview_theme_idx = (s.preview_theme_idx + 1) % themes().len();
                    }
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
    // Rows 0-4 are text inputs; row 5 is the "Edit Lyrics →" button.
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

            Self::ShufflePlaylist { playlist_id, .. } => match confirm_key(code) {
                Confirm::Yes => ModalOutcome::Confirm(ModalConfirm::ShufflePlaylist {
                    playlist_id: *playlist_id,
                }),
                Confirm::No => ModalOutcome::Dismissed,
                Confirm::Ignore => ModalOutcome::Consumed,
            },

            Self::Settings(state) => handle_settings_key(code, state),

            Self::ConfirmRemove { target, .. } => match confirm_key(code) {
                Confirm::Yes => ModalOutcome::Confirm(ModalConfirm::Remove(*target)),
                Confirm::No => ModalOutcome::Dismissed,
                Confirm::Ignore => ModalOutcome::Consumed,
            },

            // Renaming edits existing data, so Enter and Esc both write the
            // change; a blank name keeps the original (nothing is lost).
            Self::Rename { kind, id, input } => match handle_text_key(input, code) {
                TextInputResult::Submitted(name) => ModalOutcome::Confirm(ModalConfirm::Rename {
                    kind: kind.clone(),
                    id: *id,
                    new_name: name,
                }),
                TextInputResult::Dismissed => {
                    let name = input.value.trim();
                    if name.is_empty() {
                        ModalOutcome::Dismissed
                    } else {
                        ModalOutcome::Confirm(ModalConfirm::Rename {
                            kind: kind.clone(),
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

// ── Rendering ──────────────────────────────────────────────────────────────

pub fn render_modal(frame: &mut Frame<'_>, modal: &Modal, theme: &Theme) {
    match modal {
        Modal::Notify { message } => render_notification(frame, "Notice", message, theme),
        Modal::Help => render_help(frame, theme),
        Modal::About => render_about(frame, theme),
        Modal::ConfirmQuit => render_confirm(frame, "Quit audium?", theme),
        Modal::Menu { cursor } => render_menu(frame, *cursor, theme),
        Modal::ConfirmRemove { description, .. } => render_confirm(frame, description, theme),
        Modal::Rename { kind, input, .. } => {
            render_text_input(
                frame,
                &format!("Rename {kind}"),
                input,
                "[Enter / Esc] rename",
                theme,
            );
        }
        Modal::NewPlaylist { input, add_track } => {
            let (title, hint) = if add_track.is_some() {
                ("Add to New Playlist", "[Enter] create & add   [Esc] cancel")
            } else {
                ("New Playlist", "[Enter] create   [Esc] cancel")
            };
            render_text_input(frame, title, input, hint, theme);
        }
        Modal::AddToPlaylist {
            track_name,
            choices,
            cursor,
            ..
        } => render_playlist_picker(frame, track_name, choices, *cursor, theme),
        Modal::Settings(state) => render_settings(frame, state, theme),
        Modal::ShufflePlaylist { playlist_name, .. } => render_confirm(
            frame,
            &format!("Shuffle \"{playlist_name}\"? This will clear the current queue."),
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

// ── Overlay helpers ────────────────────────────────────────────────────────

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

fn modal_block<'a>(title: &'a str, theme: &Theme) -> Block<'a> {
    Block::default()
        .title(format!(" {title} "))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .style(theme.apply_bg(Style::default()))
}

// ── Individual renderers ───────────────────────────────────────────────────

fn render_notification(frame: &mut Frame<'_>, title: &str, message: &str, theme: &Theme) {
    let area = frame.area();
    let rect = centered_rect(50, 7, area);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(message, Style::default().fg(theme.text))),
            Line::from(""),
            Line::from(Span::styled(
                "Press any key to dismiss",
                Style::default().fg(theme.text_dim),
            )),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false })
        .block(modal_block(title, theme)),
        rect,
    );
}

fn render_confirm(frame: &mut Frame<'_>, description: &str, theme: &Theme) {
    let area = frame.area();
    let rect = centered_rect(52, 7, area);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(description, Style::default().fg(theme.text))),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "[Y / Enter]",
                    Style::default()
                        .fg(theme.danger)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" confirm    ", Style::default().fg(theme.text_dim)),
                Span::styled("[N / Esc]", Style::default().fg(theme.accent)),
                Span::styled(" cancel", Style::default().fg(theme.text_dim)),
            ]),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(modal_block("Confirm", theme)),
        rect,
    );
}

fn render_text_input(
    frame: &mut Frame<'_>,
    title: &str,
    input: &TextInput,
    hint: &str,
    theme: &Theme,
) {
    let area = frame.area();
    let rect = centered_rect(52, 7, area);
    frame.render_widget(Clear, rect);

    let block = modal_block(title, theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
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
        Paragraph::new(Line::from(cursor_spans(&input.value, input.cursor, theme))),
        rows[1],
    );

    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(theme.subtle))),
        rows[2],
    );
}

fn render_playlist_picker(
    frame: &mut Frame<'_>,
    track_name: &str,
    choices: &[(PlaylistId, String)],
    cursor: usize,
    theme: &Theme,
) {
    let area = frame.area();
    let height = (usize_to_u16_saturating(choices.len()) + 6).min(area.height.saturating_sub(4));
    let rect = centered_rect(52, height, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Add to Playlist", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
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
                .highlight_style(
                    Style::default()
                        .fg(theme.text)
                        .bg(theme.panel_bg)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> "),
            rows[2],
            &mut list_state,
        );
    }

    frame.render_widget(
        Paragraph::new(Span::styled(
            "[Enter] add  [j/k] navigate  [Esc] cancel",
            Style::default().fg(theme.subtle),
        )),
        rows[3],
    );
}

fn render_help(frame: &mut Frame<'_>, theme: &Theme) {
    let area = frame.area();
    let rect = centered_rect(60, 38, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Help: Keybindings", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let bindings: &[(&str, &str)] = &[
        ("q", "Quit"),
        ("Tab", "Cycle panel focus"),
        ("?", "Toggle this help"),
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
        ("a", "Add track to queue"),
        ("p", "Add track to playlist"),
        ("d", "Remove selected item"),
        ("D", "Clear queue"),
        ("r", "Rename track / playlist"),
        ("e", "Edit track metadata & lyrics"),
        ("y", "Toggle lyrics overlay"),
        ("", ""),
        ("c", "Create new playlist"),
        ("z", "Shuffle playlist into queue"),
        ("", ""),
        ("f", "Open file picker"),
        ("/", "Filter tracklist"),
        ("", ""),
        ("m", "Open menu"),
    ];

    let items: Vec<Line<'_>> = bindings
        .iter()
        .map(|(key, desc)| {
            if key.is_empty() {
                Line::from("")
            } else {
                Line::from(vec![
                    Span::styled(format!("  {key:>10}  "), Style::default().fg(theme.accent)),
                    Span::styled(*desc, Style::default().fg(theme.text_dim)),
                ])
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(items), inner);
}

fn render_menu(frame: &mut Frame<'_>, cursor: usize, theme: &Theme) {
    let area = frame.area();
    let rect = centered_rect(32, 9, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Menu", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let entries = ["Settings", "About", "Quit"];

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // spacer
            Constraint::Length(1), // Settings
            Constraint::Length(1), // About
            Constraint::Length(1), // Quit
            Constraint::Min(0),    // padding
            Constraint::Length(1), // hint
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
            rows[1 + i],
        );
    }

    frame.render_widget(
        Paragraph::new(Span::styled(
            "j/k  navigate   Enter  select   Esc  close",
            Style::default().fg(theme.subtle),
        ))
        .alignment(Alignment::Center),
        rows[5],
    );
}

fn render_about(frame: &mut Frame<'_>, theme: &Theme) {
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

    let area = frame.area();
    let rect = centered_rect(64, 17, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("About", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // top margin
            Constraint::Length(7), // logo
            Constraint::Length(1), // spacer
            Constraint::Length(1), // version
            Constraint::Length(1), // author
            Constraint::Length(1), // license
            Constraint::Length(1), // repo
            Constraint::Min(0),    // bottom gap
            Constraint::Length(1), // hint
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
    frame.render_widget(Paragraph::new(logo).alignment(Alignment::Center), rows[1]);

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
                Span::styled(format!("  {label:>8}  "), Style::default().fg(theme.subtle)),
                Span::styled(*value, Style::default().fg(theme.text)),
            ])),
            rows[3 + i],
        );
    }

    frame.render_widget(
        Paragraph::new(Span::styled(
            "Press any key to close",
            Style::default().fg(theme.subtle),
        ))
        .alignment(Alignment::Center),
        rows[8],
    );
}

fn render_settings(frame: &mut Frame<'_>, view: &SettingsState, theme: &Theme) {
    let truecolor = view.color_mode.truecolor(view.detected_truecolor);
    let area = frame.area();
    // border(2) + banner(1) + hint(1) + spacer(1) + 5×row(3) + footnote(2) = 20 inner → 22
    let rect = centered_rect(60, 22, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Settings", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // banner
            Constraint::Length(1), // hint
            Constraint::Length(1), // spacer
            Constraint::Length(3), // volume
            Constraint::Length(3), // seek
            Constraint::Length(3), // color mode
            Constraint::Length(3), // theme
            Constraint::Length(3), // transparency
            Constraint::Length(2), // footnote (wraps to 2 lines)
        ])
        .split(inner);

    render_settings_header(frame, rows[0], rows[1], view, theme);

    render_settings_row(
        frame,
        rows[3],
        "Default volume",
        view.cursor == SET_VOLUME,
        volume_bar(view.volume_pct, theme),
        theme,
    );
    render_settings_row(
        frame,
        rows[4],
        "Seek step",
        view.cursor == SET_SEEK,
        seek_display(view.seek_secs, theme),
        theme,
    );
    render_settings_row(
        frame,
        rows[5],
        "Color mode",
        view.cursor == SET_COLOR_MODE,
        color_mode_display(view.color_mode, theme),
        theme,
    );

    // Theme and transparency are locked when the console fallback is active.
    let theme_name = themes()[view.preview_theme_idx].name;
    let theme_value = if truecolor {
        theme_cycle_display(theme_name, theme)
    } else {
        locked_display("console", theme)
    };
    render_settings_row(
        frame,
        rows[6],
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
        rows[7],
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
        rows[8],
    );
}

/// One-line description of a settings row, shown in the footnote when selected.
const fn settings_row_description(cursor: usize, truecolor: bool) -> &'static str {
    match cursor {
        SET_VOLUME => "Volume applied each time audium starts.",
        SET_SEEK => "How far the seek keys jump, in seconds.",
        SET_COLOR_MODE if truecolor => {
            "Auto detects truecolor support; force a mode if detection is wrong."
        }
        SET_COLOR_MODE => "Theme & transparency need truecolor. Set this to Truecolor to override.",
        SET_THEME => "Color scheme for the interface.",
        // SET_TRANSPARENCY
        _ => {
            "Shows the terminal background through the UI. Best with a transparent or blurred terminal."
        }
    }
}

/// Renders the settings modal header: capability banner plus key hint.
fn render_settings_header(
    frame: &mut Frame<'_>,
    banner_area: Rect,
    hint_area: Rect,
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
    frame.render_widget(
        Paragraph::new(Span::styled(
            "j/k select   h/l adjust   Enter/Esc close",
            Style::default().fg(theme.text_dim),
        ))
        .alignment(Alignment::Center),
        hint_area,
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

// ── EditMetadata renderer ──────────────────────────────────────────────────

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
            Paragraph::new(Line::from(cursor_spans(&input.value, input.cursor, theme))),
            cols[1],
        );
    } else {
        let (text, style) = if input.value.is_empty() {
            ("-", Style::default().fg(theme.subtle))
        } else {
            (input.value.as_str(), Style::default().fg(theme.text_dim))
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
    let area = frame.area();
    let rect = centered_rect(62, 14, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Edit Track", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // nav hint
            Constraint::Length(1), // spacer
            Constraint::Length(1), // Name
            Constraint::Length(1), // Artist
            Constraint::Length(1), // Album
            Constraint::Length(1), // Year
            Constraint::Length(1), // Genre
            Constraint::Length(1), // Edit Lyrics → button
            Constraint::Min(0),    // padding
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "Tab  next field    arrows  move cursor    Esc/Enter  close",
            Style::default().fg(theme.subtle),
        ))
        .alignment(Alignment::Center),
        rows[0],
    );

    // Text input fields (rows 0-4)
    for (i, (label, input)) in META_LABELS.iter().zip(fields.iter()).enumerate() {
        render_metadata_field(frame, rows[2 + i], label, input, active_field == i, theme);
    }

    render_edit_lyrics_button(frame, rows[7], active_field == 5, theme);
}

// ── EditLyrics / tui-textarea renderer ────────────────────────────────────

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

    let splits = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "arrows  navigate    Home/End  line start/end",
            Style::default().fg(theme.subtle),
        ))
        .alignment(Alignment::Center),
        splits[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Enter  new line   Backspace  delete   LRC: [mm:ss.xx] lyric   Esc  save",
            Style::default().fg(theme.subtle),
        ))
        .alignment(Alignment::Center),
        splits[2],
    );

    let visible = usize::from(splits[1].height);
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

    frame.render_widget(Paragraph::new(items), splits[1]);
}
