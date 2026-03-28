pub mod layout;
pub mod overlay;
pub mod player_bar;
pub mod queue;
pub mod sidebar;
pub mod tracklist;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::app::AppState;

pub use layout::Colors;

/// Top-bar height in rows.
const TOP_BAR_HEIGHT: u16 = 1;

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Background fill.
    frame.render_widget(
        Block::default().style(Style::default().bg(Colors::BG)),
        area,
    );

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(TOP_BAR_HEIGHT), // top bar
            Constraint::Min(0),                 // body
            Constraint::Length(5),              // player bar
        ])
        .split(area);

    render_top_bar(frame, vertical[0]);

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

    // Overlay layer (modal or file picker) — rendered last so it sits on top.
    overlay::render_overlay(frame, state);
}

fn render_top_bar(frame: &mut Frame, area: ratatui::layout::Rect) {
    let spans = vec![
        Span::styled(
            " audium ",
            Style::default()
                .fg(Colors::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  —  terminal music player  ",
            Style::default().fg(Colors::TEXT_DIM),
        ),
        Span::styled(
            " [?] help  [f] file picker  [c] new playlist  [q] quit ",
            Style::default().fg(Colors::SUBTLE),
        ),
    ];
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Rgb(14, 14, 14))),
        area,
    );
}
