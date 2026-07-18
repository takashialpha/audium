use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

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
    let block = Block::default()
        .title(format!(" {} {} ", t.glyphs().note, track.display()))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent))
        .style(t.apply_bg(Style::default()));

    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    if inner.height == 0 {
        return;
    }

    // Reserve the last row for the close hint.
    let hint_row = Rect {
        y: inner.y + inner.height.saturating_sub(1),
        height: 1,
        ..inner
    };
    let text_rect = Rect {
        height: inner.height.saturating_sub(1),
        ..inner
    };

    frame.render_widget(
        Paragraph::new(Span::styled("[y] close", Style::default().fg(t.subtle)))
            .alignment(Alignment::Center),
        hint_row,
    );

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
