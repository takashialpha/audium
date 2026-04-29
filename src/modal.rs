use crate::actions::download_audio_with_binary;
use crate::library::{PlaylistId, TrackId};
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use tokio::macros::support::maybe_done;
use tokio::spawn;
use tokio::sync::mpsc::{channel, unbounded_channel, UnboundedReceiver};

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
    /// Generic transient notification (track added, error surfaced, etc.).
    Notify { message: String },

    /// Confirm before a destructive removal.
    ConfirmRemove {
        description: String,
        target: RemoveTarget,
    },

    /// Rename a track (library) or a playlist.
    Rename {
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
    /// Left/Right adjust the focused field; Esc/q saves and closes.
    Settings {
        /// Which row the cursor is on (0 = volume, 1 = seek step).
        cursor: usize,
        /// Live copy of current values, mutated in place as the user adjusts.
        volume_pct: u32, // 0–100
        seek_secs: u64,
    },

    // Downloader for YouTube
    Downloader {
        url: TextInput,
        download_event: Option<DownloadEvent>,
    },
}

/// Outcome returned from `Modal::handle_key`.
pub enum ModalOutcome {
    Consumed,
    Confirm(ModalConfirm),
    Dismissed,
}

/// What `AppState` needs to act on when a modal is confirmed.
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
    },
    ShufflePlaylist {
        playlist_id: PlaylistId,
    },
    DownloadYoutube {
        rx: UnboundedReceiver<DownloadEvent>,
    },
}

// -- Download Event to communicate between the download thread adn ui........
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Title(String),
    Progress(f64),
    Finished,
    Error(String),
}

// ── Input handling ─────────────────────────────────────────────────────────

impl Modal {
    pub fn handle_key(&mut self, code: KeyCode) -> ModalOutcome {
        match self {
            // ── Informational: any key dismisses ─────────────────────────
            Modal::Notify { .. } | Modal::Help => ModalOutcome::Dismissed,

            // ── Shuffle confirmation ──────────────────────────────────────
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
            // Left/Right increment or decrement the focused field in place.
            // Esc/q saves immediately — no separate confirm step.
            Modal::Settings {
                cursor,
                volume_pct,
                seek_secs,
            } => match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    *cursor = (*cursor + 1).min(1);
                    ModalOutcome::Consumed
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    *cursor = cursor.saturating_sub(1);
                    ModalOutcome::Consumed
                }
                KeyCode::Left => {
                    if *cursor == 0 {
                        *volume_pct = volume_pct.saturating_sub(1);
                    } else {
                        *seek_secs = seek_secs.saturating_sub(1).max(1);
                    }
                    ModalOutcome::Consumed
                }
                KeyCode::Right => {
                    if *cursor == 0 {
                        *volume_pct = (*volume_pct + 1).min(100);
                    } else {
                        *seek_secs = (*seek_secs + 1).min(120);
                    }
                    ModalOutcome::Consumed
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    ModalOutcome::Confirm(ModalConfirm::SaveSettings {
                        volume_pct: *volume_pct,
                        seek_secs: *seek_secs,
                    })
                }
                _ => ModalOutcome::Consumed,
            },

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

            Modal::Downloader {
                url,
                download_event,
                ..
            } => {
                // Handle Active/Finished Download Events first
                if let Some(event) = download_event {
                    match event {
                        // If it's an Error or Finished, any key (except maybe arrows) should dismiss
                        DownloadEvent::Error(_) | DownloadEvent::Finished => {
                            return ModalOutcome::Dismissed;
                        }
                        // While downloading (Progress), ignore all keys so the user can't mess up the URL
                        DownloadEvent::Progress(_) => {
                            // If they hit Esc, maybe let them cancel? Otherwise, consume.
                            // if code == KeyCode::Esc { return ModalOutcome::Dismissed; }
                            return ModalOutcome::Consumed;
                        }
                        _ => {}
                    }
                }

                // 2. Normal Typing Logic (only reached if download_event is None)
                match code {
                    KeyCode::Esc => ModalOutcome::Dismissed,
                    KeyCode::Enter => {
                        let url = url.value.trim().to_string().as_str();
                        if url.is_empty() {
                            return ModalOutcome::Dismissed;
                        }
                        let (tx, rx) = unbounded_channel();
                        spawn(async {
                            download_audio_with_binary(url, tx).await;
                        });
                        ModalOutcome::Confirm(ModalConfirm::DownloadYoutube {
                            rx,
                        })
                    }
                    KeyCode::Backspace => {
                        url.backspace();
                        ModalOutcome::Consumed
                    }
                    KeyCode::Left => {
                        url.move_left();
                        ModalOutcome::Consumed
                    }
                    KeyCode::Right => {
                        url.move_right();
                        ModalOutcome::Consumed
                    }
                    KeyCode::Char(c) => {
                        url.push(c);
                        ModalOutcome::Consumed
                    }
                    _ => ModalOutcome::Consumed,
                }
            }
        }
    }
}

// ── Rendering ──────────────────────────────────────────────────────────────

pub fn render_modal(frame: &mut Frame, modal: &Modal) {
    match modal {
        Modal::Notify { message } => {
            render_notification(frame, "Notice", message);
        }
        Modal::Help => render_help(frame),
        Modal::ConfirmRemove { description, .. } => {
            render_confirm(frame, description);
        }
        Modal::Rename { kind, input, .. } => {
            render_text_input(frame, &format!("Rename {}", kind), input, "Enter Name: ");
        }
        Modal::NewPlaylist { input } => {
            render_text_input(frame, "New Playlist", input, "Enter Name: ");
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
            volume_pct,
            seek_secs,
        } => {
            render_settings(frame, *cursor, *volume_pct, *seek_secs);
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
        Modal::Downloader {
            url,
            download_event,
        } => render_downloader(frame, url, download_event),
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

fn render_text_input(frame: &mut Frame, title: &str, input: &TextInput, label: &str) {
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
        Paragraph::new(Span::styled(label, Style::default().fg(TEXT_DIM))),
        rows[0],
    );

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
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
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
    let rect = centered_rect(60, 36, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Help — Keybindings");
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
        ("d", "Remove track from library"),
        ("r", "Rename track / playlist"),
        ("", ""),
        ("c", "Create new playlist"),
        ("z", "Shuffle playlist into queue"),
        ("x", "Remove item from queue"),
        ("", ""),
        ("f", "Open file picker"),
        ("o", "Open YouTube Downloader"),
        ("", ""),
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

// ── Settings rendering ─────────────────────────────────────────────────────

fn render_settings(frame: &mut Frame, cursor: usize, volume_pct: u32, seek_secs: u64) {
    let area = frame.area();
    // Height: border(2) + hint(1) + gap(1) + vol_row(3) + gap(1) + seek_row(3) = 11
    let rect = centered_rect(54, 11, area);
    frame.render_widget(Clear, rect);

    let block = modal_block("Settings");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // top hint
            Constraint::Length(1), // spacer
            Constraint::Length(3), // volume row
            Constraint::Length(1), // spacer
            Constraint::Length(3), // seek row
            Constraint::Min(0),    // padding
        ])
        .split(inner);

    // Top hint
    frame.render_widget(
        Paragraph::new(Span::styled(
            "j/k  select     ← →  adjust     Esc  close",
            Style::default().fg(SUBTLE),
        ))
            .alignment(Alignment::Center),
        rows[0],
    );

    // Volume row
    render_settings_row(
        frame,
        rows[2],
        "Default volume",
        cursor == 0,
        volume_bar(volume_pct),
    );

    // Seek row
    render_settings_row(
        frame,
        rows[4],
        "Seek step",
        cursor == 1,
        seek_display(seek_secs),
    );
}

// --- Downloader renderer ----------------------------------------
fn render_downloader(frame: &mut Frame, input: &TextInput, download_event: &Option<DownloadEvent>) {
    let area = frame.area();
    let rect = centered_rect(52, 7, area);

    match download_event {
        Some(event) => {
            frame.render_widget(Clear, rect); // Clear the area behind the modal
            let block = modal_block("YouTube Downloader");
            let inner = block.inner(rect);
            frame.render_widget(block, rect);

            match event {
                DownloadEvent::Title(title) => {
                    let text = vec![
                        Line::from(Span::styled("Fetching Metadata...", Style::default().fg(TEXT_DIM))),
                        Line::from(Span::styled(title, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))),
                    ];
                    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), inner);
                }
                DownloadEvent::Progress(pct) => {
                    // Ratatui Gauge for the download percentage
                    let gauge = ratatui::widgets::Gauge::default()
                        .block(Block::default().title(" Downloading... ").title_alignment(Alignment::Center))
                        .gauge_style(Style::default().fg(ACCENT).bg(BG))
                        .percent(*pct as u16)
                        .label(format!("{:.1}%", pct));
                    frame.render_widget(gauge, inner);
                }
                DownloadEvent::Finished => {
                    let text = vec![
                        Line::from(""),
                        Line::from(Span::styled("SUCCESS", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))),
                        Line::from(Span::styled("Press any key to dismiss", Style::default().fg(TEXT_DIM))),
                    ];
                    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), inner);
                }
                DownloadEvent::Error(err) => {
                    let text = vec![
                        Line::from(Span::styled("FAILED", Style::default().fg(DANGER).add_modifier(Modifier::BOLD))),
                        Line::from(Span::styled(err, Style::default().fg(TEXT))),
                        Line::from(Span::styled("Press any key to try again", Style::default().fg(TEXT_DIM))),
                    ];
                    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center).wrap(Wrap { trim: true }), inner);
                }
            }
        }
        None => {
            // No event yet? Show the standard URL input box
            render_text_input(
                frame,
                "YouTube Downloader",
                input,
                "Enter YouTube URL:",
            );
        }
    }
}

/// Renders a single settings row inside a plain border.
/// `value_line` is the pre-built `Line` shown on the right side.
fn render_settings_row<'a>(
    frame: &mut Frame,
    area: Rect,
    label: &'a str,
    selected: bool,
    value_line: Line<'a>,
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
        .constraints([Constraint::Min(0), Constraint::Length(22)])
        .split(row_inner);

    frame.render_widget(
        Paragraph::new(Span::styled(label, Style::default().fg(label_col))),
        cols[0],
    );

    frame.render_widget(
        Paragraph::new(value_line).alignment(Alignment::Right),
        cols[1],
    );
}

/// Builds a compact volume bar: `◀ ████████░░ 80% ▶`
/// Total width fits comfortably in the 22-char value column.
fn volume_bar(pct: u32) -> Line<'static> {
    let filled = (pct / 10) as usize; // 0–10 blocks
    let empty = 10usize.saturating_sub(filled);
    let bar: String = "█".repeat(filled) + &"░".repeat(empty);
    Line::from(vec![
        Span::styled("◀ ", Style::default().fg(SUBTLE)),
        Span::styled(bar, Style::default().fg(ACCENT)),
        Span::styled(
            format!(" {:>3}%", pct),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ▶", Style::default().fg(SUBTLE)),
    ])
}

/// Builds the seek display: `◀  5s ▶`
fn seek_display(secs: u64) -> Line<'static> {
    Line::from(vec![
        Span::styled("◀  ", Style::default().fg(SUBTLE)),
        Span::styled(
            format!("{}s", secs),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ▶", Style::default().fg(SUBTLE)),
    ])
}
