use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};

use crate::app::{AppState, Focus};

// ── Colour palette (Spotify-ish dark) ─────────────────────────────────────

const BG: Color = Color::Rgb(18, 18, 18); // near-black background
const PANEL_BG: Color = Color::Rgb(24, 24, 24); // slightly lighter panels
const SIDEBAR_BG: Color = Color::Rgb(18, 18, 18);
const ACCENT: Color = Color::Rgb(100, 180, 255); // soft sky-blue
const SUBTLE: Color = Color::Rgb(80, 80, 80); // dimmed borders / text
const TEXT: Color = Color::White;
const TEXT_DIM: Color = Color::Rgb(179, 179, 179);
const NOW_PLAYING: Color = Color::Rgb(100, 180, 255);

// ── Public entry point ─────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Fill background
    frame.render_widget(Block::default().style(Style::default().bg(BG)), area);

    // ┌────────────────────────────────────────┐
    // │  sidebar  │        main content        │
    // │           │  ─────────────────────     │
    // │           │  track list                │
    // │           │  ─────────────────────     │
    // │           │  queue                     │
    // ├───────────┴────────────────────────────┤
    // │           player bar                   │
    // └────────────────────────────────────────┘

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(4)])
        .split(area);

    let body_area = vertical[0];
    let player_area = vertical[1];

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(0)])
        .split(body_area);

    let sidebar_area = horizontal[0];
    let content_area = horizontal[1];

    let content_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(content_area);

    let tracklist_area = content_split[0];
    let queue_area = content_split[1];

    render_sidebar(frame, state, sidebar_area);
    render_tracklist(frame, state, tracklist_area);
    render_queue(frame, state, queue_area);
    render_player_bar(frame, state, player_area);
}

// ── Sidebar ────────────────────────────────────────────────────────────────

fn render_sidebar(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::Sidebar;
    let block = styled_block(" Library ", focused).style(Style::default().bg(SIDEBAR_BG));

    let items: Vec<ListItem> = state
        .library
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_playing = state
                .now_playing
                .and_then(|np| state.queue.get(np))
                .map(|qt| qt.path == t.path)
                .unwrap_or(false);

            let prefix = if is_playing { "> " } else { "  " };
            let style = if is_playing {
                Style::default()
                    .fg(NOW_PLAYING)
                    .add_modifier(Modifier::BOLD)
            } else if i == state.library_cursor && focused {
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT_DIM)
            };

            let label = format!("{}{}", prefix, truncate(&t.name, 22));
            ListItem::new(label).style(style)
        })
        .collect();

    let mut list_state = ListState::default();
    if focused || state.focus == Focus::TrackList {
        list_state.select(Some(state.library_cursor));
    }

    frame.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .fg(TEXT)
                    .bg(Color::Rgb(40, 40, 40))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> "),
        area,
        &mut list_state,
    );
}

// ── Track list ─────────────────────────────────────────────────────────────

fn render_tracklist(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::TrackList;
    let block = styled_block(" Tracks ", focused).style(Style::default().bg(PANEL_BG));

    // Column header
    let header = Line::from(vec![Span::styled(
        format!(" {:<4}  {:<}", "#", "Title"),
        Style::default().fg(SUBTLE).add_modifier(Modifier::BOLD),
    )]);

    let items: Vec<ListItem> = state
        .library
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_playing = state
                .now_playing
                .and_then(|np| state.queue.get(np))
                .map(|qt| qt.path == t.path)
                .unwrap_or(false);

            let num_style = if is_playing {
                Style::default().fg(ACCENT)
            } else {
                Style::default().fg(SUBTLE)
            };

            let num = Span::styled(format!(" {:>3}  ", i + 1), num_style);
            let title_style = if is_playing {
                Style::default()
                    .fg(NOW_PLAYING)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT_DIM)
            };
            let title = Span::styled(truncate(&t.name, area.width as usize - 8), title_style);

            ListItem::new(Line::from(vec![num, title]))
        })
        .collect();

    let items_with_header: Vec<ListItem> = std::iter::once(ListItem::new(header))
        .chain(items)
        .collect();

    let mut list_state = ListState::default();
    if focused {
        // Offset by 1 for the header row
        list_state.select(Some(state.library_cursor + 1));
    }

    frame.render_stateful_widget(
        List::new(items_with_header).block(block).highlight_style(
            Style::default()
                .fg(TEXT)
                .bg(Color::Rgb(40, 40, 40))
                .add_modifier(Modifier::BOLD),
        ),
        area,
        &mut list_state,
    );
}

// ── Queue ──────────────────────────────────────────────────────────────────

fn render_queue(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::Queue;
    let block = styled_block(" Queue ", focused).style(Style::default().bg(PANEL_BG));

    if state.queue.is_empty() {
        let hint = Paragraph::new("  No tracks in queue. Press 'a' to add, Enter to play.")
            .style(Style::default().fg(SUBTLE))
            .block(block);
        frame.render_widget(hint, area);
        return;
    }

    let items: Vec<ListItem> = state
        .queue
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_current = state.now_playing == Some(i);
            let prefix = if is_current { "> " } else { "  " };
            let style = if is_current {
                Style::default()
                    .fg(NOW_PLAYING)
                    .add_modifier(Modifier::BOLD)
            } else if i == state.queue_cursor && focused {
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT_DIM)
            };
            let label = format!(
                "{}{:<3} {}",
                prefix,
                i + 1,
                truncate(&t.name, area.width as usize - 8)
            );
            ListItem::new(label).style(style)
        })
        .collect();

    let mut list_state = ListState::default();
    if focused {
        list_state.select(Some(state.queue_cursor));
    } else if let Some(np) = state.now_playing {
        list_state.select(Some(np));
    }

    frame.render_stateful_widget(
        List::new(items).block(block).highlight_style(
            Style::default()
                .fg(TEXT)
                .bg(Color::Rgb(40, 40, 40))
                .add_modifier(Modifier::BOLD),
        ),
        area,
        &mut list_state,
    );
}

// ── Player bar ─────────────────────────────────────────────────────────────

fn render_player_bar(frame: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(SUBTLE))
        .style(Style::default().bg(Color::Rgb(18, 18, 18)));

    // Track name
    let now_playing_text = state
        .now_playing
        .and_then(|i| state.queue.get(i))
        .map(|t| t.name.as_str())
        .unwrap_or("-- Nothing playing --");

    let status_char = if state.player.is_paused() { "||" } else { "|>" };
    let volume_pct = (state.player.volume() * 100.0).round() as u8;
    let vol_bar = volume_bar(state.player.volume(), 10);

    // Layout: [ status  track name       vol bar ]
    let inner = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(4),  // status icon
            Constraint::Min(0),     // track name
            Constraint::Length(20), // volume
        ])
        .split(block.inner(area));

    frame.render_widget(block, area);

    // Status
    frame.render_widget(
        Paragraph::new(status_char).style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        inner[0],
    );

    // Track name
    frame.render_widget(
        Paragraph::new(now_playing_text)
            .style(Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
        inner[1],
    );

    // Volume
    let vol_text = format!("VOL {}  {}", vol_bar, volume_pct);
    frame.render_widget(
        Paragraph::new(vol_text).style(Style::default().fg(TEXT_DIM)),
        inner[2],
    );
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn styled_block(title: &str, focused: bool) -> Block<'_> {
    let border_color = if focused { ACCENT } else { SUBTLE };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color))
        .title_style(
            Style::default()
                .fg(if focused { ACCENT } else { TEXT_DIM })
                .add_modifier(Modifier::BOLD),
        )
}

/// Renders a volume bar like `[========  ]` of `width` inner chars.
fn volume_bar(volume: f32, width: usize) -> String {
    let filled = (volume * width as f32).round() as usize;
    let empty = width - filled.min(width);
    format!("[{}{}]", "=".repeat(filled.min(width)), " ".repeat(empty))
}

/// Truncates `s` to at most `max` characters (single-pass, naive — fine for
/// filenames that are already ASCII / basic Unicode).
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}~",
            &s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}
