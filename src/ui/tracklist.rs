use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};

use super::layout::{Colors, styled_block, truncate};
use crate::app::{AppState, Focus};

pub fn render_tracklist(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::TrackList;

    let pl_name = state
        .library
        .playlist(state.active_playlist)
        .map(|p| p.name.as_str())
        .unwrap_or("Tracks");

    let title = format!(" {} ", pl_name);
    let block = styled_block(&title, focused).style(Style::default().bg(Colors::PANEL_BG));

    let header = ListItem::new(Line::from(Span::styled(
        format!(" {:<4}  {:<}", "#", "Title"),
        Style::default()
            .fg(Colors::SUBTLE)
            .add_modifier(Modifier::BOLD),
    )));

    let tracks = state.active_tracks();

    let items: Vec<ListItem> = tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_playing = state
                .now_playing
                .and_then(|np| state.queue.get(np))
                .map(|qt| qt.id == t.id)
                .unwrap_or(false);

            let num = Span::styled(
                format!(" {:>3}  ", i + 1),
                Style::default().fg(if is_playing {
                    Colors::ACCENT
                } else {
                    Colors::SUBTLE
                }),
            );
            let title_span = Span::styled(
                truncate(&t.name, area.width as usize - 8),
                if is_playing {
                    Style::default()
                        .fg(Colors::NOW_PLAYING)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Colors::TEXT_DIM)
                },
            );
            ListItem::new(Line::from(vec![num, title_span]))
        })
        .collect();

    let all_items: Vec<ListItem> = std::iter::once(header).chain(items).collect();

    let mut list_state = ListState::default();
    if focused {
        list_state.select(Some(state.tracklist_cursor + 1)); // +1 for header row
    }

    frame.render_stateful_widget(
        List::new(all_items).block(block).highlight_style(
            Style::default()
                .fg(Colors::TEXT)
                .bg(Colors::PANEL_BG)
                .add_modifier(Modifier::BOLD),
        ),
        area,
        &mut list_state,
    );
}
