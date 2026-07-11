use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};

use super::layout::{styled_block, truncate};
use crate::app::{AppState, Focus};

pub fn render_tracklist(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::TrackList;
    let t = &state.theme;
    let has_filter = state.filter_active || !state.tracklist_filter.is_empty();

    let pl_name = state
        .library
        .playlist(state.active_playlist)
        .map_or("Tracks", |p| p.name.as_str());

    let title = format!(" {pl_name} ");
    let block = styled_block(&title, focused, t).style(t.apply_panel_bg(Style::default()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner area: list on top, filter bar at bottom when active.
    let (list_rect, filter_rect) = if has_filter {
        let s = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(inner);
        (s[0], Some(s[1]))
    } else {
        (inner, None)
    };

    // ── Filter bar ──────────────────────────────────────────────────────
    if let Some(fr) = filter_rect {
        let prefix_style = if state.filter_active {
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.text_dim)
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("/ ", prefix_style),
                Span::styled(state.tracklist_filter.as_str(), Style::default().fg(t.text)),
                Span::styled(
                    if state.filter_active { "█" } else { "" },
                    Style::default().fg(t.accent),
                ),
            ])),
            fr,
        );
    }

    // ── Track list ──────────────────────────────────────────────────────
    let header = ListItem::new(Line::from(Span::styled(
        format!(" {:<4}  {:<}", "#", "Track"),
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
                .is_some_and(|qt| qt.id == track.id);

            let num = Span::styled(
                format!(" {:>3}  ", i + 1),
                Style::default().fg(if is_playing { t.accent } else { t.subtle }),
            );
            let title_span = Span::styled(
                truncate(
                    &track.display(),
                    usize::from(list_rect.width).saturating_sub(8),
                ),
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
        List::new(all_items).highlight_style(
            Style::default()
                .fg(t.text)
                .bg(t.panel_bg)
                .add_modifier(Modifier::BOLD),
        ),
        list_rect,
        &mut list_state,
    );
}
