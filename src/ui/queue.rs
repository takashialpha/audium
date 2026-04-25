use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List, ListItem, ListState, Paragraph},
};

use super::layout::{styled_block, truncate};
use crate::app::{AppState, Focus};

pub fn render_queue(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::Queue;
    let t = &state.theme;
    let block = styled_block(" Queue ", focused, t).style(t.apply_panel_bg(Style::default()));

    if state.queue.is_empty() {
        frame.render_widget(
            Paragraph::new("  No tracks in queue.  Enter to play, 'a' to add.")
                .style(Style::default().fg(t.subtle))
                .block(block),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = state
        .queue
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_current = state.now_playing == Some(i);
            let style = if is_current {
                Style::default()
                    .fg(t.now_playing)
                    .add_modifier(Modifier::BOLD)
            } else if i == state.queue_cursor && focused {
                Style::default().fg(t.text).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_dim)
            };

            let prefix = if is_current { "> " } else { "  " };
            let label = format!(
                "{}{:<3} {}",
                prefix,
                i + 1,
                truncate(&track.name, area.width as usize - 8)
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
                .fg(t.text)
                .bg(t.panel_bg)
                .add_modifier(Modifier::BOLD),
        ),
        area,
        &mut list_state,
    );
}
