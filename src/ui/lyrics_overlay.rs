use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::modal::{hint, hint_height, modal_block, render_hints};
use crate::{app::AppState, library::TrackId, lyrics};

pub fn render_lyrics_overlay(frame: &mut Frame<'_>, state: &AppState, track_id: TrackId) {
    let Some(track) = state.library.track(track_id) else {
        return;
    };
    let lines = &state.lyrics_lines;
    if lines.is_empty() {
        return;
    }

    let area = frame.area();
    let width = (area.width * 2 / 3)
        .max(40)
        .min(area.width.saturating_sub(4));
    let height = (area.height / 2).max(8).min(area.height.saturating_sub(4));
    let rect = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };

    frame.render_widget(Clear, rect);

    let t = &state.theme;
    let title = format!("{} {}", t.glyphs().note, track.display());
    let block = modal_block(&title, t);

    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    if inner.height == 0 {
        return;
    }

    let hints = [hint("y", "close")];
    let hint_h = hint_height(&hints, inner.width as usize, t);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),         // lyrics
            Constraint::Length(1),      // gap above the hints
            Constraint::Length(hint_h), // hints
        ])
        .split(inner);
    let text_rect = rows[0];

    render_hints(frame, rows[2], &hints, t);

    let is_synced = lines.iter().any(|l| l.time_ms.is_some());
    let current = if is_synced {
        lyrics::active_idx(lines, state.elapsed())
    } else {
        None
    };

    let visible = usize::from(text_rect.height);
    let total = lines.len();

    let scroll = if is_synced {
        // Auto-scroll: keep the current line centred.
        current
            .map_or(0, |cur| cur.saturating_sub(visible / 2))
            .min(total.saturating_sub(visible))
    } else {
        // Manual scroll via j/k; clamp to content length.
        state.lyrics_scroll.min(total.saturating_sub(visible))
    };

    let items: Vec<Line<'_>> = lines
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(i, l)| {
            let is_current = current == Some(i);
            let style = if is_current {
                Style::default()
                    .fg(t.now_playing)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_dim)
            };
            Line::from(Span::styled(l.text.clone(), style))
        })
        .collect();

    frame.render_widget(
        Paragraph::new(items).alignment(Alignment::Center),
        text_rect,
    );
}
