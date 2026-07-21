pub mod layout;
pub mod lyrics_overlay;
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

/// Top-bar segments: name flush left, tagline centred, hint flush right.
const BAR_NAME: &str = " audium";
const BAR_TAGLINE: &str = "terminal music app";
const BAR_HINT: &str = "[?] keybindings ";

pub fn render(frame: &mut Frame<'_>, state: &AppState) {
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

/// Name flush left, tagline centred against the *whole* bar, one hint flush
/// right.  `?` is the only shortcut advertised here: it lists every binding
/// there is, so repeating a handful of them alongside it just adds noise.
fn render_top_bar(frame: &mut Frame<'_>, area: ratatui::layout::Rect, state: &AppState) {
    let t = &state.theme;
    let width = area.width as usize;
    // Centre the tagline on the bar, not on the space left over after the
    // name -- otherwise it drifts whenever either end changes length.
    let tagline_start = width.saturating_sub(BAR_TAGLINE.len()) / 2;
    let lead = tagline_start.saturating_sub(BAR_NAME.len());
    let trail = width.saturating_sub(BAR_NAME.len() + lead + BAR_TAGLINE.len() + BAR_HINT.len());

    // Too narrow to lay all three out without collision: keep the ends, which
    // carry the useful information, and drop the tagline.
    let spans = if lead == 0 || trail == 0 {
        let gap = width.saturating_sub(BAR_NAME.len() + BAR_HINT.len());
        vec![
            Span::styled(
                BAR_NAME,
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(gap)),
            Span::styled(BAR_HINT, Style::default().fg(t.subtle)),
        ]
    } else {
        vec![
            Span::styled(
                BAR_NAME,
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(lead)),
            Span::styled(BAR_TAGLINE, Style::default().fg(t.text_dim)),
            Span::raw(" ".repeat(trail)),
            Span::styled(BAR_HINT, Style::default().fg(t.subtle)),
        ]
    };

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(t.apply_bg(Style::default())),
        area,
    );
}
