use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};

use super::layout::{
    Columns, GAP_S, NUM_W, format_duration, render_empty_state, row_marker, styled_block,
};
use crate::app::{AppState, Focus};

pub fn render_queue(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::Queue;
    let t = &state.theme;
    let block = styled_block(" Queue ", focused, t).style(t.apply_panel_bg(Style::default()));

    if state.queue.is_empty() {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        render_empty_state(
            frame,
            inner,
            "Nothing queued",
            "a",
            "to add a track or list",
            t,
        );
        return;
    }

    // Same table shape as the tracklist, so a track reads the same wherever
    // it appears rather than as `artist - title` in one place and columns in
    // the other.
    let cols = Columns::for_width(usize::from(area.width).saturating_sub(2));

    let items: Vec<ListItem<'_>> = state
        .queue
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_current = state.now_playing == Some(i);

            let marker = row_marker(is_current, state.player.is_paused, state.elapsed(), t);
            let num = Span::styled(
                format!("{marker:>NUM_W$}{GAP_S}"),
                Style::default().fg(if is_current { t.accent } else { t.subtle }),
            );

            let title_style = if is_current {
                Style::default()
                    .fg(t.now_playing)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text)
            };
            let meta_style = Style::default().fg(t.text_dim);

            let mut spans = vec![num];
            for (n, cell) in cols
                .cells(
                    &track.name,
                    track.artist.as_deref().unwrap_or(""),
                    track.album.as_deref().unwrap_or(""),
                    &track
                        .duration_secs
                        .map_or_else(String::new, format_duration),
                )
                .into_iter()
                .enumerate()
            {
                spans.push(cell.style(if n == 0 { title_style } else { meta_style }));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut list_state = ListState::default();
    if focused {
        list_state.select(Some(state.queue_cursor));
    } else if let Some(np) = state.now_playing {
        list_state.select(Some(np));
    }

    frame.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(t.selection_style()),
        area,
        &mut list_state,
    );
}
