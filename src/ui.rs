use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};

use crate::app::{AppState, Focus};

// ── Colour palette ─────────────────────────────────────────────────────────

const BG: Color = Color::Rgb(18, 18, 18);
const PANEL_BG: Color = Color::Rgb(24, 24, 24);
const SIDEBAR_BG: Color = Color::Rgb(18, 18, 18);
const ACCENT: Color = Color::Rgb(100, 180, 255);
const SUBTLE: Color = Color::Rgb(80, 80, 80);
const TEXT: Color = Color::White;
const TEXT_DIM: Color = Color::Rgb(179, 179, 179);
const NOW_PLAYING: Color = Color::Rgb(100, 180, 255);

// ── Public entry point ─────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    frame.render_widget(Block::default().style(Style::default().bg(BG)), area);

    // ┌─────────────────────────────────────────┐
    // │  sidebar  │        main content         │
    // │           │  track list                 │
    // │           │  queue                      │
    // ├───────────┴─────────────────────────────┤
    // │  |>  Track name                         │  <- row 1: status + title
    // │  [progress bar]          0:32 / 3:47    │  <- row 2: progress
    // │  VOL [====------]  70%                  │  <- row 3: volume gauge
    // └─────────────────────────────────────────┘

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
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

    render_sidebar(frame, state, sidebar_area);
    render_tracklist(frame, state, content_split[0]);
    render_queue(frame, state, content_split[1]);
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

            ListItem::new(format!("{}{}", prefix, truncate(&t.name, 22))).style(style)
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

    let header = Line::from(Span::styled(
        format!(" {:<4}  {:<}", "#", "Title"),
        Style::default().fg(SUBTLE).add_modifier(Modifier::BOLD),
    ));

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

            let num = Span::styled(
                format!(" {:>3}  ", i + 1),
                Style::default().fg(if is_playing { ACCENT } else { SUBTLE }),
            );
            let title = Span::styled(
                truncate(&t.name, area.width as usize - 8),
                if is_playing {
                    Style::default()
                        .fg(NOW_PLAYING)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(TEXT_DIM)
                },
            );
            ListItem::new(Line::from(vec![num, title]))
        })
        .collect();

    let items_with_header: Vec<ListItem> = std::iter::once(ListItem::new(header))
        .chain(items)
        .collect();

    let mut list_state = ListState::default();
    if focused {
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
        frame.render_widget(
            Paragraph::new("  No tracks in queue. Press 'a' to add, Enter to play.")
                .style(Style::default().fg(SUBTLE))
                .block(block),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = state
        .queue
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_current = state.now_playing == Some(i);
            let style = if is_current {
                Style::default()
                    .fg(NOW_PLAYING)
                    .add_modifier(Modifier::BOLD)
            } else if i == state.queue_cursor && focused {
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT_DIM)
            };
            let prefix = if is_current { "> " } else { "  " };
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
    let outer = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(SUBTLE))
        .style(Style::default().bg(BG));

    let inner_area = outer.inner(area);
    frame.render_widget(outer, area);

    // Split: main content (title + progress) | vertical volume bar (6 cols)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(6)])
        .split(inner_area);

    let main_area = cols[0];
    let vol_area = cols[1];

    // ── Left: title row + progress row ─────────────────────────────────
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // row 0: status icon + track name
            Constraint::Length(1), // row 1: progress bar + time
            Constraint::Min(0),    // row 2: padding
        ])
        .split(main_area);

    // ▶ when playing or idle, ⏸ only when explicitly paused mid-track
    let is_playing = state.now_playing.is_some() && !state.player.is_paused();
    let is_paused = state.now_playing.is_some() && state.player.is_paused();
    let status = if is_playing {
        "▶ "
    } else if is_paused {
        "⏸ "
    } else {
        "▶ "
    };
    let title = state
        .now_playing
        .and_then(|i| state.queue.get(i))
        .map(|t| t.name.as_str())
        .unwrap_or("-- Nothing playing --");

    let title_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(rows[0]);

    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        title_cols[0],
    );
    frame.render_widget(
        Paragraph::new(title).style(Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
        title_cols[1],
    );

    // Row 1 — progress bar + elapsed / total
    let elapsed_secs = state.elapsed().as_secs();
    let elapsed_str = format_duration(elapsed_secs);
    let total_str = state
        .track_duration
        .map(|d| format_duration(d.as_secs()))
        .unwrap_or_else(|| "-:--".to_string());
    let time_label = format!(" {elapsed_str} / {total_str} ");
    let time_width = time_label.chars().count() as u16;

    let progress_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(time_width)])
        .split(rows[1]);

    frame.render_widget(
        thumb_bar(progress_cols[0].width as usize, state.progress_ratio()),
        progress_cols[0],
    );
    frame.render_widget(
        Paragraph::new(time_label).style(Style::default().fg(TEXT_DIM)),
        progress_cols[1],
    );

    // ── Right: vertical volume bar ──────────────────────────────────────
    // Filled from bottom up using ▓ (filled) and ░ (empty).
    // Bottom row is always the % label. A left border visually separates it.
    let vol = state.player.volume();
    let vol_pct = (vol * 100.0).round() as u8;
    let bar_height = vol_area.height.saturating_sub(1); // one row reserved for label
    let filled = ((vol * bar_height as f32).round() as u16).min(bar_height);
    let empty = bar_height - filled;

    let mut vol_lines: Vec<Line> = Vec::new();
    for _ in 0..empty {
        vol_lines.push(Line::from(Span::styled(
            " ░░░ ",
            Style::default().fg(Color::Rgb(50, 50, 50)),
        )));
    }
    for _ in 0..filled {
        vol_lines.push(Line::from(Span::styled(
            " ▓▓▓ ",
            Style::default().fg(ACCENT),
        )));
    }
    // Percentage label, right-aligned in the 6-char column
    vol_lines.push(Line::from(Span::styled(
        format!("{:>4}%", vol_pct),
        Style::default().fg(TEXT_DIM),
    )));

    frame.render_widget(
        Paragraph::new(vol_lines)
            .block(
                Block::default()
                    .borders(Borders::LEFT)
                    .border_style(Style::default().fg(SUBTLE)),
            )
            .style(Style::default().bg(BG)),
        vol_area,
    );
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn styled_block(title: &str, focused: bool) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(if focused { ACCENT } else { SUBTLE }))
        .title_style(
            Style::default()
                .fg(if focused { ACCENT } else { TEXT_DIM })
                .add_modifier(Modifier::BOLD),
        )
}

/// Progress bar: ████████░░░░░░  played portion in accent, rest dimmed.
/// A single bright block at the boundary acts as the thumb.
fn thumb_bar(width: usize, ratio: f64) -> Paragraph<'static> {
    if width == 0 {
        return Paragraph::new("");
    }
    let ratio = ratio.clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    let filled = filled.min(width);
    let remaining = width.saturating_sub(filled);

    let mut spans: Vec<Span> = Vec::with_capacity(width + 1);
    if filled > 0 {
        // All played blocks in accent colour
        spans.push(Span::styled(
            "█".repeat(filled.saturating_sub(1)),
            Style::default().fg(ACCENT),
        ));
        // Thumb — bright white to stand out
        spans.push(Span::styled(
            "█",
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ));
    }
    if remaining > 0 {
        spans.push(Span::styled(
            "░".repeat(remaining),
            Style::default().fg(SUBTLE),
        ));
    }
    Paragraph::new(Line::from(spans))
}

fn format_duration(secs: u64) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{m}:{s:02}")
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}~",
            s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}
