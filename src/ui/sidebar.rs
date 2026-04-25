use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List, ListItem, ListState},
};

use super::layout::{styled_block, truncate};
use crate::app::{AppState, Focus};

pub fn render_sidebar(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::Sidebar;
    let t = &state.theme;

    let block = styled_block(" Playlists ", focused, t).style(t.apply_sidebar_bg(Style::default()));

    let items: Vec<ListItem> = state
        .library
        .playlists
        .iter()
        .enumerate()
        .map(|(i, pl)| {
            let is_active = pl.id == state.active_playlist;

            let style = if is_active && focused {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else if i == state.sidebar_cursor && focused {
                Style::default().fg(t.text).add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(t.accent)
            } else {
                Style::default().fg(t.text_dim)
            };

            let count = pl.tracks.len();
            let label = format!("{} ({})", truncate(&pl.name, 18), count);
            ListItem::new(label).style(style)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.sidebar_cursor));

    frame.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .fg(t.text)
                    .bg(t.panel_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> "),
        area,
        &mut list_state,
    );
}
