use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List, ListItem, ListState},
};

use super::layout::{Colors, styled_block, truncate};
use crate::app::{AppState, Focus};

pub fn render_sidebar(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::Sidebar;

    let block = styled_block(" Playlists ", focused).style(Style::default().bg(Colors::SIDEBAR_BG));

    let items: Vec<ListItem> = state
        .library
        .playlists
        .iter()
        .enumerate()
        .map(|(i, pl)| {
            let is_active = pl.id == state.active_playlist;

            let style = if is_active && focused {
                Style::default()
                    .fg(Colors::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else if i == state.sidebar_cursor && focused {
                Style::default()
                    .fg(Colors::TEXT)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(Colors::ACCENT)
            } else {
                Style::default().fg(Colors::TEXT_DIM)
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
                    .fg(Colors::TEXT)
                    .bg(Colors::PANEL_BG)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> "),
        area,
        &mut list_state,
    );
}
