use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::library::{PlaylistId, TrackId};
use crate::ui::layout::{Theme, format_duration, themes};

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

// ── RemoveTarget ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum RemoveTarget {
    TrackFromQueue { queue_idx: usize },
    TrackFromLibrary { track_id: TrackId },
    Playlist { playlist_id: PlaylistId },
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
    Settings {
        cursor: usize,
        volume_pct: u32,
        seek_secs: u64,
        preview_theme_idx: usize,
        transparent: bool,
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
    },
    /// Apply a theme live during settings preview without closing the modal.
    PreviewTheme {
        theme_name: String,
        transparent: bool,
    },
    ShufflePlaylist {
        playlist_id: PlaylistId,
    },
    OpenSettings,
    OpenAbout,
    Quit,
}

// ── Input handling ─────────────────────────────────────────────────────────

impl Modal {
    pub fn handle_key(&mut self, code: KeyCode) -> ModalOutcome {
        match self {
            Modal::Notify { .. } | Modal::Help | Modal::About => ModalOutcome::Dismissed,

            Modal::Menu { cursor } => match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    *cursor = (*cursor + 1).min(MENU_ENTRIES - 1);
                    ModalOutcome::Consumed
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    *cursor = cursor.saturating_sub(1);
                    ModalOutcome::Consumed
                }
                KeyCode::Enter => match *cursor {
                    0 => ModalOutcome::Confirm(ModalConfirm::OpenSettings),
                    1 => ModalOutcome::Confirm(ModalConfirm::OpenAbout),
                    _ => ModalOutcome::Confirm(ModalConfirm::Quit),
                },
                KeyCode::Esc | KeyCode::Char('q') => ModalOutcome::Dismissed,
                _ => ModalOutcome::Consumed,
            },

            Modal::ShufflePlaylist { playlist_id, .. } => match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    ModalOutcome::Confirm(ModalConfirm::ShufflePlaylist {
                        playlist_id: *playlist_id,
                    })
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') => {
                    ModalOutcome::Dismissed
                }
                _ => ModalOutcome::Consumed,
            },

            Modal::Settings {
                cursor,
                volume_pct,
                seek_secs,
                preview_theme_idx,
                transparent,
            } => {
                const ROWS: usize = 4; // volume, seek, theme, transparency
                match code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        *cursor = (*cursor + 1).min(ROWS - 1);
                        ModalOutcome::Consumed
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        *cursor = cursor.saturating_sub(1);
                        ModalOutcome::Consumed
                    }
                    KeyCode::Left => {
                        match *cursor {
                            0 => *volume_pct = volume_pct.saturating_sub(1),
                            1 => *seek_secs = seek_secs.saturating_sub(1).max(1),
                            2 => {
                                *preview_theme_idx = preview_theme_idx
                                    .checked_sub(1)
                                    .unwrap_or(themes().len() - 1);
                                // Apply live preview without closing the modal.
                                return ModalOutcome::Confirm(ModalConfirm::PreviewTheme {
                                    theme_name: themes()[*preview_theme_idx].name.to_string(),
                                    transparent: *transparent,
                                });
                            }
                            _ => *transparent = !*transparent,
                        }
                        ModalOutcome::Consumed
                    }
                    KeyCode::Right => {
                        match *cursor {
                            0 => *volume_pct = (*volume_pct + 1).min(100),
                            1 => *seek_secs = (*seek_secs + 1).min(120),
                            2 => {
                                *preview_theme_idx = (*preview_theme_idx + 1) % themes().len();
                                // Apply live preview without closing the modal.
                                return ModalOutcome::Confirm(ModalConfirm::PreviewTheme {
                                    theme_name: themes()[*preview_theme_idx].name.to_string(),
                                    transparent: *transparent,
                                });
                            }
                            _ => *transparent = !*transparent,
                        }
                        ModalOutcome::Consumed
                    }
                    // Esc and q both save and close.
                    KeyCode::Esc | KeyCode::Char('q') => {
                        ModalOutcome::Confirm(ModalConfirm::SaveSettings {
                            volume_pct: *volume_pct,
                            seek_secs: *seek_secs,
                            theme_name: themes()[*preview_theme_idx].name.to_string(),
                            transparent: *transparent,
                        })
                    }
                    _ => ModalOutcome::Consumed,
                }
            }

            Modal::ConfirmRemove { target, .. } => match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    ModalOutcome::Confirm(ModalConfirm::Remove(target.clone()))
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') => {
                    ModalOutcome::Dismissed
                }
                _ => ModalOutcome::Consumed,
            },

            Modal::Rename { kind, id, input } => match code {
                KeyCode::Enter => {
                    let name = input.value.trim().to_string();
                    if name.is_empty() {
                        return ModalOutcome::Consumed;
                    }
                    ModalOutcome::Confirm(ModalConfirm::Rename {
                        kind: kind.clone(),
                        id: *id,
                        new_name: name,
                    })
                }
                KeyCode::Esc => ModalOutcome::Dismissed,
                KeyCode::Char(c) => {
                    input.push(c);
                    ModalOutcome::Consumed
                }
                KeyCode::Backspace => {
                    input.backspace();
                    ModalOutcome::Consumed
                }
                KeyCode::Left => {
                    input.move_left();
                    ModalOutcome::Consumed
                }
                KeyCode::Right => {
                    input.move_right();
                    ModalOutcome::Consumed
                }
                _ => ModalOutcome::Consumed,
            },

            Modal::NewPlaylist { input } => match code {
                KeyCode::Enter => {
                    let name = input.value.trim().to_string();
                    if name.is_empty() {
                        return ModalOutcome::Consumed;
                    }
                    ModalOutcome::Confirm(ModalConfirm::NewPlaylist { name })
                }
                KeyCode::Esc => ModalOutcome::Dismissed,
                KeyCode::Char(c) => {
                    input.push(c);
                    ModalOutcome::Consumed
                }
                KeyCode::Backspace => {
                    input.backspace();
                    ModalOutcome::Consumed
                }
                KeyCode::Left => {
                    input.move_left();
                    ModalOutcome::Consumed
                }
                KeyCode::Right => {
                    input.move_right();
                    ModalOutcome::Consumed
                }
                _ => ModalOutcome::Consumed,
            },

            Modal::AddToPlaylist {
                choices,
                cursor,
                track_id,
                ..
            } => match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if !choices.is_empty() {
                        *cursor = (*cursor + 1).min(choices.len() - 1);
                    }
                    ModalOutcome::Consumed
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    *cursor = cursor.saturating_sub(1);
                    ModalOutcome::Consumed
                }
                KeyCode::Enter => {
                    if let Some((playlist_id, _)) = choices.get(*cursor) {
                        ModalOutcome::Confirm(ModalConfirm::AddToPlaylist {
                            track_id: *track_id,
                            playlist_id: *playlist_id,
                        })
                    } else {
                        ModalOutcome::Dismissed
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => ModalOutcome::Dismissed,
                _ => ModalOutcome::Consumed,
            },
        }
    }
}

// ── Rendering ──────────────────────────────────────────────────────────────

pub fn render_modal(frame: &mut Frame, modal: &Modal, theme: &Theme) {
    match modal {
        Modal::Notify { message } => render_notification(frame, "Notice", message, theme),
        Modal::Help => render_help(frame, theme),
        Modal::About => render_about(frame, theme),
        Modal::Menu { cursor } => render_menu(frame, *cursor, theme),
        Modal::ConfirmRemove { description, .. } => render_confirm(frame, description, theme),
        Modal::Rename { kind, input, .. } => {
            render_text_input(frame, &format!("Rename {}", kind), input, theme)
        }
        Modal::NewPlaylist { input } => render_text_input(frame, "New Playlist", input, theme),
        Modal::AddToPlaylist {
            track_name,
            choices,
            cursor,
            ..
        } => render_playlist_picker(frame, track_name, choices, *cursor, theme),
        Modal::Settings {
            cursor,
            volume_pct,
            seek_secs,
            preview_theme_idx,
            transparent,
            ..
        } => render_settings(
            frame,
            *cursor,
            *volume_pct,
            *seek_secs,
            *preview_theme_idx,
            *transparent,
            theme,
        ),
        Modal::ShufflePlaylist { playlist_name, .. } => render_confirm(
            frame,
            &format!(
                "Shuffle \"{}\"? This will clear the current queue.",
                playlist_name
            ),
            theme,
        ),
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
        .title(format!(" {} ", title))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .style(theme.apply_bg(Style::default()))
}

// ── Individual renderers ───────────────────────────────────────────────────

fn render_notification(frame: &mut Frame, title: &str, message: &str, theme: &Theme) {
    let area = frame.area();
    let rect = centered_rect(50, 5, area);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(message, Style::default().fg(theme.text))),
            Line::from(Span::styled(
                "Press any key to dismiss",
                Style::default().fg(theme.text_dim),
            )),
        ])
        .alignment(Alignment::Center)
        .block(modal_block(title, theme)),
        rect,
    );
}

fn render_confirm(frame: &mut Frame, description: &str, theme: &Theme) {
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
                    "[Y]",
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

fn render_text_input(frame: &mut Frame, title: &str, input: &TextInput, theme: &Theme) {
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

    let before = &input.value[..input.cursor];
    let after = &input.value[input.cursor..];
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(before.to_string(), Style::default().fg(theme.text)),
            Span::styled("█", Style::default().fg(theme.accent)),
            Span::styled(after.to_string(), Style::default().fg(theme.text)),
        ])),
        rows[1],
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            "[Enter] confirm  [Esc] cancel",
            Style::default().fg(theme.subtle),
        )),
        rows[2],
    );
}

fn render_playlist_picker(
    frame: &mut Frame,
    track_name: &str,
    choices: &[(PlaylistId, String)],
    cursor: usize,
    theme: &Theme,
) {
    let area = frame.area();
    let height = (choices.len() as u16 + 6).min(area.height.saturating_sub(4));
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
            format!("Track: {}", track_name),
            Style::default().fg(theme.text_dim),
        )),
        rows[0],
    );

    if choices.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "No playlists yet.  Press c to create one.",
                Style::default().fg(theme.subtle),
            )),
            rows[2],
        );
    } else {
        let items: Vec<ListItem> = choices
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

fn render_help(frame: &mut Frame, theme: &Theme) {
    let area = frame.area();
    let rect = centered_rect(60, 34, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Help — Keybindings", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let bindings: &[(&str, &str)] = &[
        ("q", "Quit"),
        ("Tab", "Cycle panel focus"),
        ("?", "Toggle this help"),
        ("", ""),
        ("j / ↓", "Move cursor down"),
        ("k / ↑", "Move cursor up"),
        ("", ""),
        ("Space", "Play / Pause"),
        ("n", "Next track"),
        ("N", "Previous track"),
        ("← / →", "Seek backward / forward"),
        ("+ / =", "Volume up"),
        ("-", "Volume down"),
        ("l", "Cycle loop mode"),
        ("", ""),
        ("Enter", "Play selected track"),
        ("a", "Add track to queue"),
        ("p", "Add track to playlist"),
        ("d", "Remove selected item"),
        ("r", "Rename track / playlist"),
        ("", ""),
        ("c", "Create new playlist"),
        ("z", "Shuffle playlist into queue"),
        ("", ""),
        ("f", "Open file picker"),
        ("", ""),
        ("m", "Open menu"),
    ];

    let items: Vec<Line> = bindings
        .iter()
        .map(|(key, desc)| {
            if key.is_empty() {
                Line::from("")
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("  {:>10}  ", key),
                        Style::default().fg(theme.accent),
                    ),
                    Span::styled(*desc, Style::default().fg(theme.text_dim)),
                ])
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(items), inner);
}

fn render_menu(frame: &mut Frame, cursor: usize, theme: &Theme) {
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
        let prefix = if selected { "▶  " } else { "   " };
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

fn render_about(frame: &mut Frame, theme: &Theme) {
    let area = frame.area();
    let rect = centered_rect(62, 18, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("About", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let logo_lines = [
        " █████╗ ██╗   ██╗██████╗ ██╗██╗   ██╗███╗   ███╗",
        "██╔══██╗██║   ██║██╔══██╗██║██║   ██║████╗ ████║",
        "███████║██║   ██║██║  ██║██║██║   ██║██╔████╔██║",
        "██╔══██║██║   ██║██║  ██║██║██║   ██║██║╚██╔╝██║",
        "██║  ██║╚██████╔╝██████╔╝██║╚██████╔╝██║ ╚═╝ ██║",
        "╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚═╝ ╚═════╝ ╚═╝     ╚═╝",
    ];

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // logo
            Constraint::Length(1), // spacer
            Constraint::Length(1), // version
            Constraint::Length(1), // author
            Constraint::Length(1), // license
            Constraint::Length(1), // repo
            Constraint::Min(0),    // padding
            Constraint::Length(1), // hint
        ])
        .split(inner);

    let logo: Vec<Line> = logo_lines
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
        ("license", "Apache-2.0"),
        ("source", "github.com/takashialpha/audium"),
    ];

    for (i, (label, value)) in meta.iter().enumerate() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("  {:>8}  ", label),
                    Style::default().fg(theme.subtle),
                ),
                Span::styled(*value, Style::default().fg(theme.text)),
            ])),
            rows[2 + i],
        );
    }

    frame.render_widget(
        Paragraph::new(Span::styled(
            "Press any key to close",
            Style::default().fg(theme.subtle),
        ))
        .alignment(Alignment::Center),
        rows[7],
    );
}

fn render_settings(
    frame: &mut Frame,
    cursor: usize,
    volume_pct: u32,
    seek_secs: u64,
    preview_theme_idx: usize,
    transparent: bool,
    theme: &Theme,
) {
    let area = frame.area();
    // border(2) + hint(1) + spacer(1) + 4×row(3) + 3×spacer(1) = 17 inner → 19 with border
    let rect = centered_rect(56, 19, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Settings", theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // hint
            Constraint::Length(1), // spacer
            Constraint::Length(3), // volume
            Constraint::Length(1), // spacer
            Constraint::Length(3), // seek
            Constraint::Length(1), // spacer
            Constraint::Length(3), // theme
            Constraint::Length(1), // spacer
            Constraint::Length(3), // transparency
            Constraint::Min(0),    // padding
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "j/k select   ← → adjust   Esc/q save & close",
            Style::default().fg(theme.subtle),
        ))
        .alignment(Alignment::Center),
        rows[0],
    );

    render_settings_row(
        frame,
        rows[2],
        "Default volume",
        cursor == 0,
        volume_bar(volume_pct, theme),
        theme,
    );
    render_settings_row(
        frame,
        rows[4],
        "Seek step",
        cursor == 1,
        seek_display(seek_secs, theme),
        theme,
    );

    let theme_name = themes()[preview_theme_idx].name;
    render_settings_row(
        frame,
        rows[6],
        "Theme",
        cursor == 2,
        theme_cycle_display(theme_name, theme),
        theme,
    );

    render_settings_row(
        frame,
        rows[8],
        "Transparency",
        cursor == 3,
        toggle_display(if transparent { "on" } else { "off" }, theme),
        theme,
    );
}

fn render_settings_row<'a>(
    frame: &mut Frame,
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

fn volume_bar(pct: u32, theme: &Theme) -> Line<'static> {
    let filled = (pct / 10) as usize;
    let empty = 10usize.saturating_sub(filled);
    let bar: String = "█".repeat(filled) + &"░".repeat(empty);
    Line::from(vec![
        Span::styled("◀ ", Style::default().fg(theme.subtle)),
        Span::styled(bar, Style::default().fg(theme.accent)),
        Span::styled(
            format!(" {:>3}%", pct),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ▶", Style::default().fg(theme.subtle)),
    ])
}

fn seek_display(secs: u64, theme: &Theme) -> Line<'static> {
    let label = format_duration(secs);
    Line::from(vec![
        Span::styled("◀  ", Style::default().fg(theme.subtle)),
        Span::styled(
            label,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ▶", Style::default().fg(theme.subtle)),
    ])
}

fn theme_cycle_display(name: &'static str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled("◀  ", Style::default().fg(theme.subtle)),
        Span::styled(
            name,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ▶", Style::default().fg(theme.subtle)),
    ])
}

fn toggle_display(value: &'static str, theme: &Theme) -> Line<'static> {
    let col = if value == "on" {
        theme.now_playing
    } else {
        theme.text_dim
    };
    Line::from(vec![
        Span::styled("◀  ", Style::default().fg(theme.subtle)),
        Span::styled(value, Style::default().fg(col).add_modifier(Modifier::BOLD)),
        Span::styled("  ▶", Style::default().fg(theme.subtle)),
    ])
}
