use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};

use super::layout::{Theme, styled_block, truncate};
use crate::app::{AppState, Focus, SidebarItem};

/// Columns a list row spends on its selection marker.
const MARKER: &str = "> ";
const MARKER_W: usize = 2;

/// Borders plus the single library row.
const LIBRARY_H: u16 = 3;

pub fn render_sidebar(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    // The library and the playlists are different kinds of thing, so they get
    // separate frames rather than being flattened into one list under a shared
    // heading. Each frame is its own Tab stop.
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(LIBRARY_H), Constraint::Min(0)])
        .split(area);

    render_library(frame, state, sections[0]);
    render_playlists(frame, state, sections[1]);
}

fn render_library(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let t = &state.theme;
    let g = t.glyphs();
    let selected = state.focus == Focus::Library;
    let is_active = state.active_view == SidebarItem::Library;

    let block = styled_block(" Library ", selected, t).style(t.apply_sidebar_bg(Style::default()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // The marker is prepended below, so the label gets what is left after it.
    let label = entry(
        &format!("{} All tracks", g.note),
        state.library.tracks.len(),
        (inner.width as usize).saturating_sub(MARKER_W),
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("{}{label}", if selected { MARKER } else { "  " }),
            row_style(t, is_active, selected),
        ))),
        inner,
    );
}

fn render_playlists(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let t = &state.theme;
    let focused = state.focus == Focus::Playlists;
    let selected_row = focused.then_some(state.playlist_cursor);

    let block = styled_block(" Playlists ", selected_row.is_some(), t)
        .style(t.apply_sidebar_bg(Style::default()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.library.playlists.is_empty() {
        render_empty_hint(frame, t, inner);
        return;
    }

    let width = (inner.width as usize).saturating_sub(MARKER_W);
    let items: Vec<ListItem<'_>> = state
        .library
        .playlists
        .iter()
        .enumerate()
        .map(|(i, pl)| {
            ListItem::new(entry(&pl.name, pl.tracks.len(), width)).style(row_style(
                t,
                state.active_view == SidebarItem::Playlist(pl.id),
                selected_row == Some(i),
            ))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(selected_row);

    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(
                Style::default()
                    .fg(t.text)
                    .bg(t.panel_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(MARKER),
        inner,
        &mut list_state,
    );
}

/// Centred two-line prompt, so an empty sidebar reads as an invitation rather
/// than a stray list row.
fn render_empty_hint(frame: &mut Frame<'_>, t: &Theme, inner: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "No playlists yet",
            Style::default().fg(t.text_dim),
        ))
        .alignment(Alignment::Center),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("press ", Style::default().fg(t.subtle)),
            Span::styled("c", Style::default().fg(t.accent)),
            Span::styled(" to create one", Style::default().fg(t.subtle)),
        ]))
        .alignment(Alignment::Center),
        rows[2],
    );
}

fn row_style(t: &Theme, is_active: bool, is_cursor: bool) -> Style {
    if is_active && is_cursor {
        Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
    } else if is_cursor {
        Style::default().fg(t.text).add_modifier(Modifier::BOLD)
    } else if is_active {
        Style::default().fg(t.accent)
    } else {
        Style::default().fg(t.text_dim)
    }
}

/// A row with its name on the left and its track count flush right, so counts
/// line up in a column instead of trailing each name at a ragged offset.
///
/// The count is laid out first and never truncated, so as it grows it eats
/// into the name from the right rather than overflowing the row.
fn entry(name: &str, count: usize, width: usize) -> String {
    let count = count.to_string();
    let room = width.saturating_sub(count.chars().count() + 1);
    if room == 0 {
        // Narrower than the count plus a separator: the count is the more
        // useful of the two, and this keeps the row within `width`.
        return truncate(&count, width);
    }
    let name = truncate(name, room);
    let pad = width.saturating_sub(name.chars().count() + count.chars().count());
    format!("{name}{blank:pad$}{count}", blank = "")
}
