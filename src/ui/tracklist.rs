use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};

use super::layout::{styled_block, truncate};
use crate::app::{AppState, Focus};

pub fn render_tracklist(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::TrackList;
    let t = &state.theme;

    let pl_name = state
        .library
        .playlist(state.active_playlist)
        .map(|p| p.name.as_str())
        .unwrap_or("Tracks");

    let title = format!(" {} ", pl_name);
    let block = styled_block(&title, focused, t).style(t.apply_panel_bg(Style::default()));

    let header = ListItem::new(Line::from(Span::styled(
        format!(" {:<4}  {:<}", "#", "Title"),
        Style::default().fg(t.subtle).add_modifier(Modifier::BOLD),
    )));

    let tracks = state.active_tracks();

    let items: Vec<ListItem> = tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_playing = state
                .now_playing
                .and_then(|np| state.queue.get(np))
                .map(|qt| qt.id == track.id)
                .unwrap_or(false);

            let num = Span::styled(
                format!(" {:>3}  ", i + 1),
                Style::default().fg(if is_playing { t.accent } else { t.subtle }),
            );
            let title_span = Span::styled(
                truncate(&track.name, area.width as usize - 8),
                if is_playing {
                    Style::default()
                        .fg(t.now_playing)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(t.text_dim)
                },
            );
            ListItem::new(Line::from(vec![num, title_span]))
        })
        .collect();

    let all_items: Vec<ListItem> = std::iter::once(header).chain(items).collect();

    let mut list_state = ListState::default();
    if focused {
        list_state.select(Some(state.tracklist_cursor + 1));
    }

    frame.render_stateful_widget(
        List::new(all_items).block(block).highlight_style(
            Style::default()
                .fg(t.text)
                .bg(t.panel_bg)
                .add_modifier(Modifier::BOLD),
        ),
        area,
        &mut list_state,
    );
}
