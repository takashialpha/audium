use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::library::{PlaylistId, TrackId};

// ── Colour palette (kept in sync with ui/) ─────────────────────────────────
const BG: Color = Color::Rgb(18, 18, 18);
const ACCENT: Color = Color::Rgb(100, 180, 255);
const SUBTLE: Color = Color::Rgb(80, 80, 80);
const TEXT: Color = Color::White;
const TEXT_DIM: Color = Color::Rgb(179, 179, 179);
const DANGER: Color = Color::Rgb(255, 80, 80);

// ── Text-input widget (shared by rename / new-playlist modals) ─────────────

#[derive(Debug, Default, Clone)]
pub struct TextInput {
    pub value: String,
    pub cursor: usize, // byte offset
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
        // Step back one char boundary.
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

// ── What action to take on a confirmed removal ────────────────────────────

#[derive(Debug, Clone)]
pub enum RemoveTarget {
    TrackFromQueue { queue_idx: usize },
    TrackFromLibrary { track_id: TrackId },
    Playlist { playlist_id: PlaylistId },
}

// ── Modal variants ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Modal {
    /// Brief notification: a track was added to the library.
    TrackAdded { name: String },

    /// Confirm before a destructive removal.
    ConfirmRemove {
        /// Human-readable description shown in the prompt.
        description: String,
        target: RemoveTarget,
    },

    /// Rename a track (library) or a playlist.
    Rename {
        /// "Track" | "Playlist"
        kind: String,
        id: u64,
        input: TextInput,
    },

    /// Create a new playlist.
    NewPlaylist { input: TextInput },

    /// Choose which playlist to add the selected track into.
    AddToPlaylist {
        track_id: TrackId,
        track_name: String,
        /// (id, name) pairs for every user playlist.
        choices: Vec<(PlaylistId, String)>,
        cursor: usize,
    },

    /// Full keybinding reference.
    Help,

    /// Confirm before shuffling a playlist into the queue.
    ShufflePlaylist {
        playlist_id: PlaylistId,
        playlist_name: String,
    },

    /// Settings menu.
    Settings {
        /// Which row the cursor is on (0 = volume, 1 = seek step).
        cursor: usize,
        /// Editing state for the currently selected field, if active.
        editing: bool,
        input: TextInput,
        /// Live copy of current values for display.
        volume_pct: u32, // 0–100
        seek_secs: u64,
    },
}

/// Outcome returned from `Modal::handle_key`.
pub enum ModalOutcome {
    /// Modal handled the key; no further processing needed.
    Consumed,
    /// User confirmed.  Inner value carries semantic data for `AppState`.
    Confirm(ModalConfirm),
    /// User dismissed (Esc / q).
    Dismissed,
}

/// What `AppState` needs to act on when a modal is confirmed.
#[derive(Debug)]
#[allow(dead_code)]
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
    },
    ShufflePlaylist {
        playlist_id: PlaylistId,
    },
    /// Nothing to act on (e.g. TrackAdded notification just dismissed).
    None,
}

// ── Input handling ─────────────────────────────────────────────────────────

impl Modal {
    /// Returns the outcome of handling a keypress for this modal.
    pub fn handle_key(&mut self, code: KeyCode) -> ModalOutcome {
        match self {
            // ── Informational: any key dismisses ─────────────────────────
            Modal::TrackAdded { .. } | Modal::Help => ModalOutcome::Dismissed,

            // ── Shuffle confirmation ───────────────────────────────────────
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

            // ── Settings ──────────────────────────────────────────────────
            Modal::Settings {
                cursor,
                editing,
                input,
                volume_pct,
                seek_secs,
            } => {
                if *editing {
                    match code {
                        KeyCode::Enter => {
                            // Commit the edited value.
                            if let Ok(n) = input.value.trim().parse::<u64>() {
                                if *cursor == 0 {
                                    *volume_pct = (n as u32).clamp(0, 100);
                                } else {
                                    *seek_secs = n.clamp(1, 120);
                                }
                            }
                            *editing = false;
                            *input = TextInput::default();
                            ModalOutcome::Consumed
                        }
                        KeyCode::Esc => {
                            *editing = false;
                            *input = TextInput::default();
                            ModalOutcome::Consumed
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            input.push(c);
                            ModalOutcome::Consumed
                        }
                        KeyCode::Backspace => {
                            input.backspace();
                            ModalOutcome::Consumed
                        }
                        _ => ModalOutcome::Consumed,
                    }
                } else {
                    match code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            *cursor = (*cursor + 1).min(1);
                            ModalOutcome::Consumed
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            *cursor = cursor.saturating_sub(1);
                            ModalOutcome::Consumed
                        }
                        KeyCode::Enter => {
                            // Start editing the selected field.
                            let prefill = if *cursor == 0 {
                                volume_pct.to_string()
                            } else {
                                seek_secs.to_string()
                            };
                            *input = TextInput::with_value(prefill);
                            *editing = true;
                            ModalOutcome::Consumed
                        }
                        KeyCode::Esc | KeyCode::Char('q') => {
                            ModalOutcome::Confirm(ModalConfirm::SaveSettings {
                                volume_pct: *volume_pct,
                                seek_secs: *seek_secs,
                            })
                        }
                        _ => ModalOutcome::Consumed,
                    }
                }
            }

            // ── Confirm/cancel ────────────────────────────────────────────
            Modal::ConfirmRemove { target, .. } => match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    ModalOutcome::Confirm(ModalConfirm::Remove(target.clone()))
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') => {
                    ModalOutcome::Dismissed
                }
                _ => ModalOutcome::Consumed,
            },

            // ── Text inputs ───────────────────────────────────────────────
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

            // ── List selection ────────────────────────────────────────────
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

/// Renders `modal` as a centered overlay on top of whatever was drawn already.
pub fn render_modal(frame: &mut Frame, modal: &Modal) {
    match modal {
        Modal::TrackAdded { name } => {
            render_notification(
                frame,
                "Track Added",
                &format!("\"{}\" added to library.", name),
            );
        }
        Modal::Help => render_help(frame),
        Modal::ConfirmRemove { description, .. } => {
            render_confirm(frame, description);
        }
        Modal::Rename { kind, input, .. } => {
            render_text_input(frame, &format!("Rename {}", kind), input);
        }
        Modal::NewPlaylist { input } => {
            render_text_input(frame, "New Playlist", input);
        }
        Modal::AddToPlaylist {
            track_name,
            choices,
            cursor,
            ..
        } => {
            render_playlist_picker(frame, track_name, choices, *cursor);
        }
        Modal::Settings {
            cursor,
            editing,
            input,
            volume_pct,
            seek_secs,
        } => {
            render_settings(frame, *cursor, *editing, input, *volume_pct, *seek_secs);
        }
        Modal::ShufflePlaylist { playlist_name, .. } => {
            render_confirm(
                frame,
                &format!(
                    "Shuffle \"{}\"? This will clear the current queue.",
                    playlist_name
                ),
            );
        }
    }
}

// ── Overlay helpers ────────────────────────────────────────────────────────

/// Returns a centred `Rect` that is `width` × `height` within `area`.
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

fn modal_block(title: &str) -> Block<'_> {
    Block::default()
        .title(format!(" {} ", title))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG))
}

fn render_notification(frame: &mut Frame, title: &str, message: &str) {
    let area = frame.area();
    let rect = centered_rect(50, 5, area);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(message, Style::default().fg(TEXT))),
            Line::from(Span::styled(
                "Press any key to dismiss",
                Style::default().fg(TEXT_DIM),
            )),
        ])
        .alignment(Alignment::Center)
        .block(modal_block(title)),
        rect,
    );
}

fn render_confirm(frame: &mut Frame, description: &str) {
    let area = frame.area();
    let rect = centered_rect(52, 7, area);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(description, Style::default().fg(TEXT))),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "[Y]",
                    Style::default().fg(DANGER).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" confirm    ", Style::default().fg(TEXT_DIM)),
                Span::styled("[N / Esc]", Style::default().fg(ACCENT)),
                Span::styled(" cancel", Style::default().fg(TEXT_DIM)),
            ]),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(modal_block("Confirm")),
        rect,
    );
}

fn render_text_input(frame: &mut Frame, title: &str, input: &TextInput) {
    let area = frame.area();
    let rect = centered_rect(52, 7, area);
    frame.render_widget(Clear, rect);

    let block = modal_block(title);
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
        Paragraph::new(Span::styled("Enter name:", Style::default().fg(TEXT_DIM))),
        rows[0],
    );

    // Input field with a fake cursor drawn as a block character.
    let before = &input.value[..input.cursor];
    let after = &input.value[input.cursor..];
    let spans = vec![
        Span::styled(before.to_string(), Style::default().fg(TEXT)),
        Span::styled("█", Style::default().fg(ACCENT)),
        Span::styled(after.to_string(), Style::default().fg(TEXT)),
    ];
    frame.render_widget(Paragraph::new(Line::from(spans)), rows[1]);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "[Enter] confirm  [Esc] cancel",
            Style::default().fg(SUBTLE),
        )),
        rows[2],
    );
}

fn render_playlist_picker(
    frame: &mut Frame,
    track_name: &str,
    choices: &[(PlaylistId, String)],
    cursor: usize,
) {
    let area = frame.area();
    let height = (choices.len() as u16 + 6).min(area.height.saturating_sub(4));
    let rect = centered_rect(52, height, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Add to Playlist");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // subtitle
            Constraint::Length(1), // spacer
            Constraint::Min(0),    // list
            Constraint::Length(1), // hint
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("Track: {}", track_name),
            Style::default().fg(TEXT_DIM),
        )),
        rows[0],
    );

    if choices.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "No playlists yet.  Press N to create one.",
                Style::default().fg(SUBTLE),
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
                        .fg(TEXT)
                        .bg(Color::Rgb(40, 40, 40))
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
            Style::default().fg(SUBTLE),
        )),
        rows[3],
    );
}

fn render_help(frame: &mut Frame) {
    let area = frame.area();
    let rect = centered_rect(60, 34, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Help — Keybindings");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let bindings: &[(&str, &str)] = &[
        // Global
        ("q", "Quit"),
        ("Tab", "Cycle panel focus"),
        ("?", "Toggle this help"),
        ("", ""),
        // Navigation
        ("j / ↓", "Move cursor down"),
        ("k / ↑", "Move cursor up"),
        ("", ""),
        // Playback
        ("Space", "Play / Pause"),
        ("n", "Next track"),
        ("N", "Previous track"),
        ("← / →", "Seek backward / forward"),
        ("+ / =", "Volume up"),
        ("-", "Volume down"),
        ("", ""),
        // Library actions
        ("Enter", "Play selected track"),
        ("a", "Add track to queue"),
        ("p", "Add track to playlist"),
        ("d", "Remove track from library"),
        ("r", "Rename track / playlist"),
        ("", ""),
        // Playlist / queue
        ("c", "Create new playlist"),
        ("z", "Shuffle playlist into queue"),
        ("x", "Remove item from queue"),
        ("", ""),
        // File picker
        ("f", "Open file picker"),
        ("", ""),
        // Settings
        ("s", "Open settings"),
    ];

    let items: Vec<Line> = bindings
        .iter()
        .map(|(key, desc)| {
            if key.is_empty() {
                Line::from("")
            } else {
                Line::from(vec![
                    Span::styled(format!("  {:>10}  ", key), Style::default().fg(ACCENT)),
                    Span::styled(*desc, Style::default().fg(TEXT_DIM)),
                ])
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(items), inner);
}

fn render_settings(
    frame: &mut Frame,
    cursor: usize,
    editing: bool,
    input: &TextInput,
    volume_pct: u32,
    seek_secs: u64,
) {
    let area = frame.area();
    let rect = centered_rect(52, 14, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Settings");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // hint top
            Constraint::Length(1), // spacer
            Constraint::Length(2), // volume row
            Constraint::Length(1), // spacer
            Constraint::Length(2), // seek row
            Constraint::Min(0),    // padding
            Constraint::Length(2), // hint bottom (wraps to 2)
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "j/k select  Enter edit/confirm  Esc save & close",
            Style::default().fg(SUBTLE),
        )),
        rows[0],
    );

    render_settings_row(
        frame,
        rows[2],
        "Default volume",
        &format!("{}%", volume_pct),
        cursor == 0,
        editing,
        input,
    );

    render_settings_row(
        frame,
        rows[4],
        "Seek step (seconds)",
        &format!("{}s", seek_secs),
        cursor == 1,
        editing,
        input,
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            "Volume applies on next launch. Seek step applies immediately.",
            Style::default().fg(SUBTLE),
        ))
        .wrap(ratatui::widgets::Wrap { trim: true }),
        rows[6],
    );
}

fn render_settings_row(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    selected: bool,
    active_edit: bool,
    edit_input: &TextInput,
) {
    let border_col = if selected { ACCENT } else { SUBTLE };
    let label_col = if selected { TEXT } else { TEXT_DIM };

    let row_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_col));
    let row_inner = row_block.inner(area);
    frame.render_widget(row_block, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(18)])
        .split(row_inner);

    frame.render_widget(
        Paragraph::new(Span::styled(label, Style::default().fg(label_col))),
        cols[0],
    );

    if active_edit && selected {
        let before = &edit_input.value[..edit_input.cursor];
        let after = &edit_input.value[edit_input.cursor..];
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(before.to_string(), Style::default().fg(TEXT)),
                Span::styled("█", Style::default().fg(ACCENT)),
                Span::styled(after.to_string(), Style::default().fg(TEXT)),
            ])),
            cols[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new(Span::styled(
                value,
                Style::default().fg(if selected { ACCENT } else { TEXT_DIM }),
            )),
            cols[1],
        );
    }
}
