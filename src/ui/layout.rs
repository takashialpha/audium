use ratatui::{
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders},
};

/// Central colour palette.  Every UI sub-module imports from here.
pub struct Colors;

impl Colors {
    pub const BG: Color = Color::Rgb(18, 18, 18);
    pub const PANEL_BG: Color = Color::Rgb(24, 24, 24);
    pub const SIDEBAR_BG: Color = Color::Rgb(18, 18, 18);
    pub const ACCENT: Color = Color::Rgb(100, 180, 255);
    pub const SUBTLE: Color = Color::Rgb(80, 80, 80);
    pub const TEXT: Color = Color::White;
    pub const TEXT_DIM: Color = Color::Rgb(179, 179, 179);
    pub const NOW_PLAYING: Color = Color::Rgb(100, 180, 255);
}

/// Builds a consistently styled panel block.
pub fn styled_block(title: &str, focused: bool) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(if focused {
            Colors::ACCENT
        } else {
            Colors::SUBTLE
        }))
        .title_style(
            Style::default()
                .fg(if focused {
                    Colors::ACCENT
                } else {
                    Colors::TEXT_DIM
                })
                .add_modifier(Modifier::BOLD),
        )
}

/// Truncates a string to `max` chars, appending `~` if truncated.
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}~",
            s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}

pub fn format_duration(secs: u64) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{m}:{s:02}")
}
