use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use super::layout::{Theme, format_duration};
use crate::app::{AppState, LoopMode};

pub fn render_player_bar(frame: &mut Frame, state: &AppState, area: Rect) {
    let t = &state.theme;

    let outer = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(t.subtle))
        .style(t.apply_bg(Style::default()));

    let inner_area = outer.inner(area);
    frame.render_widget(outer, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(6)])
        .split(inner_area);

    let main_area = cols[0];
    let vol_area = cols[1];

    // ── Title + progress ─────────────────────────────────────────────────
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(main_area);

    let is_paused = state.player.is_paused;
    let has_track = state.now_playing.is_some();
    let status = if has_track && !is_paused {
        "⏸ "
    } else {
        "▶ "
    };

    let title = state
        .now_playing
        .and_then(|i| state.queue.get(i))
        .map(|tr| tr.name.as_str())
        .unwrap_or("-- Nothing playing --");

    let loop_label = match state.loop_mode {
        LoopMode::Off => "",
        LoopMode::Queue => " loop queue ",
        LoopMode::Track => " loop track ",
    };
    let loop_width = loop_label.chars().count() as u16;

    let title_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(loop_width),
        ])
        .split(rows[0]);

    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(t.accent).add_modifier(Modifier::BOLD)),
        title_cols[0],
    );
    frame.render_widget(
        Paragraph::new(title).style(Style::default().fg(t.text).add_modifier(Modifier::BOLD)),
        title_cols[1],
    );
    if !loop_label.is_empty() {
        frame.render_widget(
            Paragraph::new(loop_label).style(Style::default().fg(t.subtle)),
            title_cols[2],
        );
    }

    // Progress bar + time label.
    let (elapsed_str, total_str) = if state.now_playing.is_some() {
        let e = format_duration(state.elapsed().as_secs());
        let d = state
            .track_duration
            .map(|d| format_duration(d.as_secs()))
            .unwrap_or_else(|| "-:--".to_string());
        (e, d)
    } else {
        ("0:00".to_string(), "-:--".to_string())
    };
    let time_label = format!(" {elapsed_str} / {total_str} ");
    let time_width = time_label.chars().count() as u16;

    let progress_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(time_width)])
        .split(rows[1]);

    frame.render_widget(
        thumb_bar(progress_cols[0].width as usize, state.progress_ratio(), t),
        progress_cols[0],
    );
    frame.render_widget(
        Paragraph::new(time_label).style(Style::default().fg(t.text_dim)),
        progress_cols[1],
    );

    // ── Vertical volume bar ───────────────────────────────────────────────
    let vol = state.player.volume;
    let vol_pct = (vol * 100.0).round() as u8;
    let bar_height = vol_area.height.saturating_sub(1);
    let filled = ((vol * bar_height as f32).round() as u16).min(bar_height);
    let empty = bar_height - filled;

    let mut vol_lines: Vec<Line> = Vec::new();
    for _ in 0..empty {
        vol_lines.push(Line::from(Span::styled(
            " ░░░ ",
            Style::default().fg(t.vol_empty),
        )));
    }
    for _ in 0..filled {
        vol_lines.push(Line::from(Span::styled(
            " ▓▓▓ ",
            Style::default().fg(t.accent),
        )));
    }
    vol_lines.push(Line::from(Span::styled(
        format!("{:>4}%", vol_pct),
        Style::default().fg(t.text_dim),
    )));

    frame.render_widget(
        Paragraph::new(vol_lines)
            .block(
                Block::default()
                    .borders(Borders::LEFT)
                    .border_style(Style::default().fg(t.subtle)),
            )
            .style(t.apply_bg(Style::default())),
        vol_area,
    );
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn thumb_bar(width: usize, ratio: f64, t: &Theme) -> Paragraph<'static> {
    if width == 0 {
        return Paragraph::new("");
    }
    let ratio = ratio.clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    let filled = filled.min(width);
    let remaining = width.saturating_sub(filled);

    let accent = t.accent;
    let text = t.text;
    let subtle = t.subtle;

    let mut spans: Vec<Span> = Vec::with_capacity(width + 1);
    if filled > 0 {
        spans.push(Span::styled(
            "█".repeat(filled.saturating_sub(1)),
            Style::default().fg(accent),
        ));
        spans.push(Span::styled(
            "█",
            Style::default().fg(text).add_modifier(Modifier::BOLD),
        ));
    }
    if remaining > 0 {
        spans.push(Span::styled(
            "░".repeat(remaining),
            Style::default().fg(subtle),
        ));
    }
    Paragraph::new(Line::from(spans))
}
