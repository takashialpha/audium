use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};

use super::layout::{Columns, GAP_S, NUM_W, Theme, cursor_spans, format_duration, styled_block};
use crate::app::{AppState, Focus};

pub fn render_tracklist(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let focused = state.focus == Focus::TrackList;
    let t = &state.theme;
    let has_filter = state.filter_active || !state.tracklist_filter.is_empty();

    let title = format!(" {} ", state.active_view_name());
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

    if let Some(fr) = filter_rect {
        render_filter_bar(frame, fr, state, t);
    }

    // -- Track list ------------------------------------------------------
    // The header and its rule sit outside the list: as list items they scroll
    // away with the content, losing the column labels exactly when a long
    // library makes them useful.
    let table = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(1), // rule
            Constraint::Min(0),    // rows
        ])
        .split(list_rect);
    let (header_rect, rule_rect, rows_rect) = (table[0], table[1], table[2]);

    let cols = Columns::for_width(usize::from(list_rect.width));
    render_table_header(frame, header_rect, rule_rect, &cols, t);

    let tracks = state.active_tracks();

    let items: Vec<ListItem<'_>> = tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_playing = state
                .now_playing
                .and_then(|np| state.queue.get(np))
                .is_some_and(|qt| qt.id == track.id);

            // The playing row trades its index for the play glyph, which reads
            // faster than a recoloured number.
            let marker = if is_playing {
                t.glyphs().play.to_string()
            } else {
                (i + 1).to_string()
            };
            let num = Span::styled(
                format!("{marker:>NUM_W$}{GAP_S}"),
                Style::default().fg(if is_playing { t.accent } else { t.subtle }),
            );

            let title_style = if is_playing {
                Style::default()
                    .fg(t.now_playing)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text)
            };
            // Three levels of depth: the title carries the row, the artist
            // supports it, the album and length recede furthest.
            let artist_style = Style::default().fg(t.text_dim);
            let meta_style = Style::default().fg(t.subtle);

            let cells = cols.cells(
                &track.name,
                track.artist.as_deref().unwrap_or(""),
                track.album.as_deref().unwrap_or(""),
                &track
                    .duration_secs
                    .map_or_else(String::new, format_duration),
            );
            let mut spans = vec![num];
            for (n, cell) in cells.into_iter().enumerate() {
                spans.push(cell.style(match n {
                    0 => title_style,
                    1 => artist_style,
                    _ => meta_style,
                }));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut list_state = ListState::default();
    if focused {
        list_state.select(Some(state.tracklist_cursor));
    }

    frame.render_stateful_widget(
        List::new(items).highlight_style(t.selection_style()),
        rows_rect,
        &mut list_state,
    );
}

/// The `/` filter row, shown under the table while a filter is active.
fn render_filter_bar(frame: &mut Frame<'_>, area: Rect, state: &AppState, t: &Theme) {
    let prefix_style = if state.filter_active {
        Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.text_dim)
    };
    let mut spans = vec![Span::styled("/ ", prefix_style)];
    if state.filter_active {
        // Cursor sits at the end of the filter text (no in-place editing).
        spans.extend(cursor_spans(
            &state.tracklist_filter,
            state.tracklist_filter.len(),
            t,
        ));
    } else {
        spans.push(Span::styled(
            state.tracklist_filter.as_str(),
            Style::default().fg(t.text),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Column labels and the rule under them.
fn render_table_header(frame: &mut Frame<'_>, header: Rect, rule: Rect, cols: &Columns, t: &Theme) {
    let mut spans = vec![Span::raw(format!("{:>NUM_W$}{GAP_S}", "#"))];
    spans.extend(cols.cells("Title", "Artist", "Album", "Time"));
    frame.render_widget(
        Paragraph::new(Line::from(
            spans
                .into_iter()
                .map(|sp| sp.style(Style::default().fg(t.subtle).add_modifier(Modifier::BOLD)))
                .collect::<Vec<_>>(),
        )),
        header,
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            t.glyphs().rule.repeat(usize::from(rule.width)),
            Style::default().fg(t.subtle),
        )),
        rule,
    );
}
