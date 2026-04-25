pub mod layout;
pub mod overlay;
pub mod player_bar;
pub mod queue;
pub mod sidebar;
pub mod tracklist;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::app::AppState;

const TOP_BAR_HEIGHT: u16 = 1;

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    frame.render_widget(
        Block::default().style(state.theme.apply_bg(Style::default())),
        area,
    );

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(TOP_BAR_HEIGHT),
            Constraint::Min(0),
            Constraint::Length(5),
        ])
        .split(area);

    render_top_bar(frame, vertical[0], state);

    let body_area = vertical[1];
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(0)])
        .split(body_area);

    let content_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(horizontal[1]);

    sidebar::render_sidebar(frame, state, horizontal[0]);
    tracklist::render_tracklist(frame, state, content_split[0]);
    queue::render_queue(frame, state, content_split[1]);
    player_bar::render_player_bar(frame, state, vertical[2]);
    overlay::render_overlay(frame, state);
}

fn render_top_bar(frame: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let t = &state.theme;
    let spans = vec![
        Span::styled(
            " audium ",
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  —  terminal music player  ",
            Style::default().fg(t.text_dim),
        ),
        Span::styled(
            " [?] help  [f] file picker  [c] new playlist  [m] menu  [q] quit ",
            Style::default().fg(t.subtle),
        ),
    ];
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(t.apply_bg(Style::default())),
        area,
    );
}
