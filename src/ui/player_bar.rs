use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use super::layout::{Theme, format_duration};
use crate::app::{AppState, LoopMode};
use crate::library::Track;
use crate::numeric::{ratio_to_unit_count, ratio_to_whole_percent, usize_to_u16_saturating};

pub fn render_player_bar(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
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

    let current_track = state.now_playing.and_then(|i| state.queue.get(i));
    render_title_row(frame, rows[0], state, current_track, t);
    render_progress_row(frame, rows[1], state, t);

    // ── Metadata line (album · year · genre) ─────────────────────────────
    if let Some(tr) = current_track {
        let parts: Vec<String> = [
            tr.album.clone(),
            tr.year.map(|y| y.to_string()),
            tr.genre.clone(),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !parts.is_empty() {
            frame.render_widget(
                Paragraph::new(parts.join(t.glyphs().sep)).style(Style::default().fg(t.text_dim)),
                rows[2],
            );
        }
    }

    render_volume_bar(frame, vol_area, state.player.volume, t);
}

fn render_title_row(
    frame: &mut Frame<'_>,
    row: Rect,
    state: &AppState,
    current_track: Option<&Track>,
    t: &Theme,
) {
    let is_paused = state.player.is_paused;
    let has_track = state.now_playing.is_some();
    let status = if has_track && !is_paused {
        t.glyphs().pause
    } else {
        t.glyphs().play
    };

    let title_owned;
    // Deferred-init pattern needs `if let` block scoping; `map_or` can't express
    // a closure that both assigns an outer binding and returns a ref into it.
    #[allow(clippy::option_if_let_else)]
    let title: &str = if let Some(tr) = current_track {
        title_owned = tr.display();
        &title_owned
    } else {
        "-- Nothing playing --"
    };

    let loop_label = match state.loop_mode {
        LoopMode::Off => "",
        LoopMode::Queue => " loop queue ",
        LoopMode::Track => " loop track ",
    };
    let loop_width = usize_to_u16_saturating(loop_label.chars().count());

    let speed = state.player.playback_speed;
    let speed_label = if (speed - 1.0).abs() > 0.001 {
        // Round to 2dp for display to avoid float-representation noise.
        let s = (speed * 100.0).round() / 100.0;
        format!(" {s}{} ", t.glyphs().times)
    } else {
        String::new()
    };
    let speed_width = usize_to_u16_saturating(speed_label.chars().count());

    let title_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(speed_width),
            Constraint::Length(loop_width),
        ])
        .split(row);

    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(t.accent).add_modifier(Modifier::BOLD)),
        title_cols[0],
    );
    frame.render_widget(
        Paragraph::new(title).style(Style::default().fg(t.text).add_modifier(Modifier::BOLD)),
        title_cols[1],
    );
    if speed_width > 0 {
        frame.render_widget(
            Paragraph::new(speed_label.as_str())
                .style(Style::default().fg(t.accent).add_modifier(Modifier::BOLD)),
            title_cols[2],
        );
    }
    if !loop_label.is_empty() {
        frame.render_widget(
            Paragraph::new(loop_label).style(Style::default().fg(t.subtle)),
            title_cols[3],
        );
    }
}

fn render_progress_row(frame: &mut Frame<'_>, row: Rect, state: &AppState, t: &Theme) {
    let (elapsed_str, total_str) = if state.now_playing.is_some() {
        let e = format_duration(state.elapsed().as_secs());
        let d = state
            .track_duration
            .map_or_else(|| "-:--".to_string(), |d| format_duration(d.as_secs()));
        (e, d)
    } else {
        ("0:00".to_string(), "-:--".to_string())
    };
    let time_label = format!(" {elapsed_str} / {total_str} ");
    let time_width = usize_to_u16_saturating(time_label.chars().count());

    let progress_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(time_width)])
        .split(row);

    frame.render_widget(
        thumb_bar(
            usize::from(progress_cols[0].width),
            state.progress_ratio(),
            t,
        ),
        progress_cols[0],
    );
    frame.render_widget(
        Paragraph::new(time_label).style(Style::default().fg(t.text_dim)),
        progress_cols[1],
    );
}

fn render_volume_bar(frame: &mut Frame<'_>, vol_area: Rect, vol: f32, t: &Theme) {
    let vol_pct = ratio_to_whole_percent(vol);
    let bar_height = vol_area.height.saturating_sub(1);
    let filled =
        usize_to_u16_saturating(ratio_to_unit_count(f64::from(vol), usize::from(bar_height)));
    let empty = bar_height - filled;

    let g = t.glyphs();
    let mut vol_lines: Vec<Line<'_>> = Vec::new();
    for _ in 0..empty {
        vol_lines.push(Line::from(Span::styled(
            g.vol_empty,
            Style::default().fg(t.vol_empty),
        )));
    }
    for _ in 0..filled {
        vol_lines.push(Line::from(Span::styled(
            g.vol_fill,
            Style::default().fg(t.accent),
        )));
    }
    vol_lines.push(Line::from(Span::styled(
        format!("{vol_pct:>4}%"),
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
    let filled = ratio_to_unit_count(ratio, width);
    let remaining = width.saturating_sub(filled);
    let g = t.glyphs();

    let mut spans: Vec<Span<'_>> = Vec::with_capacity(width + 1);
    if filled > 0 {
        if filled > 1 {
            spans.push(Span::styled(
                g.bar_fill.repeat(filled - 1),
                Style::default().fg(t.accent),
            ));
        }
        spans.push(Span::styled(
            g.bar_fill,
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        ));
    }
    if remaining > 0 {
        spans.push(Span::styled(
            g.bar_empty.repeat(remaining),
            Style::default().fg(t.subtle),
        ));
    }
    Paragraph::new(Line::from(spans))
}
