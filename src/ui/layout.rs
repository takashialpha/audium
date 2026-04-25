use ratatui::{
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders},
};

// ── Theme ──────────────────────────────────────────────────────────────────

/// All semantic color roles used across the UI.
/// Passed by reference to every render function that needs colors.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub panel_bg: Color,
    pub sidebar_bg: Color,
    pub accent: Color,
    pub subtle: Color,
    pub text: Color,
    pub text_dim: Color,
    pub now_playing: Color,
    pub danger: Color,
    pub dir_col: Color,
    pub vol_empty: Color,
    pub transparent: bool,
}

impl Theme {
    /// Returns the background color to use, or `None` when transparency is
    /// enabled — callers must skip the `.bg(color)` style call entirely when
    /// `None` so ratatui emits no background escape and the compositor can
    /// show through.
    pub fn bg(&self) -> Option<Color> {
        if self.transparent {
            None
        } else {
            Some(self.bg)
        }
    }

    pub fn panel_bg(&self) -> Option<Color> {
        if self.transparent {
            None
        } else {
            Some(self.panel_bg)
        }
    }

    pub fn sidebar_bg(&self) -> Option<Color> {
        if self.transparent {
            None
        } else {
            Some(self.sidebar_bg)
        }
    }

    /// Applies the background color to a `Style` only if transparency is off.
    pub fn apply_bg(&self, style: Style) -> Style {
        match self.bg() {
            Some(c) => style.bg(c),
            None => style,
        }
    }

    pub fn apply_panel_bg(&self, style: Style) -> Style {
        match self.panel_bg() {
            Some(c) => style.bg(c),
            None => style,
        }
    }

    pub fn apply_sidebar_bg(&self, style: Style) -> Style {
        match self.sidebar_bg() {
            Some(c) => style.bg(c),
            None => style,
        }
    }
}

// ── Built-in themes ────────────────────────────────────────────────────────

pub fn themes() -> &'static [Theme] {
    &THEMES
}

pub fn theme_by_name(name: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.name == name).unwrap_or(&THEMES[0])
}

static THEMES: [Theme; 15] = [
    // 1 — dark (default)
    Theme {
        name: "dark",
        bg: Color::Rgb(18, 18, 18),
        panel_bg: Color::Rgb(24, 24, 24),
        sidebar_bg: Color::Rgb(18, 18, 18),
        accent: Color::Rgb(100, 180, 255),
        subtle: Color::Rgb(80, 80, 80),
        text: Color::White,
        text_dim: Color::Rgb(179, 179, 179),
        now_playing: Color::Rgb(100, 180, 255),
        danger: Color::Rgb(255, 80, 80),
        dir_col: Color::Rgb(255, 210, 100),
        vol_empty: Color::Rgb(50, 50, 50),
        transparent: false,
    },
    // 2 — light
    Theme {
        name: "light",
        bg: Color::Rgb(245, 245, 245),
        panel_bg: Color::Rgb(235, 235, 235),
        sidebar_bg: Color::Rgb(240, 240, 240),
        accent: Color::Rgb(0, 100, 210),
        subtle: Color::Rgb(160, 160, 160),
        text: Color::Rgb(20, 20, 20),
        text_dim: Color::Rgb(90, 90, 90),
        now_playing: Color::Rgb(0, 100, 210),
        danger: Color::Rgb(200, 30, 30),
        dir_col: Color::Rgb(180, 100, 0),
        vol_empty: Color::Rgb(200, 200, 200),
        transparent: false,
    },
    // 3 — nord
    Theme {
        name: "nord",
        bg: Color::Rgb(46, 52, 64),
        panel_bg: Color::Rgb(59, 66, 82),
        sidebar_bg: Color::Rgb(46, 52, 64),
        accent: Color::Rgb(136, 192, 208),
        subtle: Color::Rgb(76, 86, 106),
        text: Color::Rgb(236, 239, 244),
        text_dim: Color::Rgb(216, 222, 233),
        now_playing: Color::Rgb(163, 190, 140),
        danger: Color::Rgb(191, 97, 106),
        dir_col: Color::Rgb(235, 203, 139),
        vol_empty: Color::Rgb(67, 76, 94),
        transparent: false,
    },
    // 4 — gruvbox dark
    Theme {
        name: "gruvbox",
        bg: Color::Rgb(40, 40, 40),
        panel_bg: Color::Rgb(50, 48, 47),
        sidebar_bg: Color::Rgb(40, 40, 40),
        accent: Color::Rgb(250, 189, 47),
        subtle: Color::Rgb(102, 92, 84),
        text: Color::Rgb(235, 219, 178),
        text_dim: Color::Rgb(189, 174, 147),
        now_playing: Color::Rgb(184, 187, 38),
        danger: Color::Rgb(251, 73, 52),
        dir_col: Color::Rgb(214, 93, 14),
        vol_empty: Color::Rgb(60, 56, 54),
        transparent: false,
    },
    // 5 — gruvbox light
    Theme {
        name: "gruvbox_light",
        bg: Color::Rgb(251, 241, 199),
        panel_bg: Color::Rgb(242, 229, 188),
        sidebar_bg: Color::Rgb(251, 241, 199),
        accent: Color::Rgb(181, 118, 20),
        subtle: Color::Rgb(168, 153, 132),
        text: Color::Rgb(60, 56, 54),
        text_dim: Color::Rgb(102, 92, 84),
        now_playing: Color::Rgb(121, 116, 14),
        danger: Color::Rgb(204, 36, 29),
        dir_col: Color::Rgb(214, 93, 14),
        vol_empty: Color::Rgb(213, 196, 161),
        transparent: false,
    },
    // 6 — rosé pine
    Theme {
        name: "rosepine",
        bg: Color::Rgb(25, 23, 36),
        panel_bg: Color::Rgb(31, 29, 46),
        sidebar_bg: Color::Rgb(25, 23, 36),
        accent: Color::Rgb(196, 167, 231),
        subtle: Color::Rgb(110, 106, 134),
        text: Color::Rgb(224, 222, 244),
        text_dim: Color::Rgb(144, 140, 170),
        now_playing: Color::Rgb(235, 188, 186),
        danger: Color::Rgb(235, 111, 146),
        dir_col: Color::Rgb(246, 193, 119),
        vol_empty: Color::Rgb(38, 35, 58),
        transparent: false,
    },
    // 7 — rosé pine dawn
    Theme {
        name: "rosepine_dawn",
        bg: Color::Rgb(250, 244, 237),
        panel_bg: Color::Rgb(255, 250, 243),
        sidebar_bg: Color::Rgb(250, 244, 237),
        accent: Color::Rgb(144, 122, 169),
        subtle: Color::Rgb(152, 147, 165),
        text: Color::Rgb(87, 82, 121),
        text_dim: Color::Rgb(121, 117, 147),
        now_playing: Color::Rgb(180, 99, 122),
        danger: Color::Rgb(180, 99, 122),
        dir_col: Color::Rgb(234, 157, 52),
        vol_empty: Color::Rgb(223, 218, 217),
        transparent: false,
    },
    // 8 — catppuccin mocha
    Theme {
        name: "catppuccin",
        bg: Color::Rgb(30, 30, 46),
        panel_bg: Color::Rgb(36, 36, 54),
        sidebar_bg: Color::Rgb(30, 30, 46),
        accent: Color::Rgb(137, 180, 250),
        subtle: Color::Rgb(88, 91, 112),
        text: Color::Rgb(205, 214, 244),
        text_dim: Color::Rgb(166, 173, 200),
        now_playing: Color::Rgb(166, 227, 161),
        danger: Color::Rgb(243, 139, 168),
        dir_col: Color::Rgb(249, 226, 175),
        vol_empty: Color::Rgb(49, 50, 68),
        transparent: false,
    },
    // 9 — catppuccin latte
    Theme {
        name: "catppuccin_latte",
        bg: Color::Rgb(239, 241, 245),
        panel_bg: Color::Rgb(230, 233, 239),
        sidebar_bg: Color::Rgb(239, 241, 245),
        accent: Color::Rgb(30, 102, 245),
        subtle: Color::Rgb(172, 176, 190),
        text: Color::Rgb(76, 79, 105),
        text_dim: Color::Rgb(108, 111, 133),
        now_playing: Color::Rgb(64, 160, 43),
        danger: Color::Rgb(210, 15, 57),
        dir_col: Color::Rgb(223, 142, 29),
        vol_empty: Color::Rgb(204, 208, 218),
        transparent: false,
    },
    // 10 — dracula
    Theme {
        name: "dracula",
        bg: Color::Rgb(40, 42, 54),
        panel_bg: Color::Rgb(48, 50, 65),
        sidebar_bg: Color::Rgb(40, 42, 54),
        accent: Color::Rgb(189, 147, 249),
        subtle: Color::Rgb(98, 114, 164),
        text: Color::Rgb(248, 248, 242),
        text_dim: Color::Rgb(191, 192, 190),
        now_playing: Color::Rgb(80, 250, 123),
        danger: Color::Rgb(255, 85, 85),
        dir_col: Color::Rgb(255, 184, 108),
        vol_empty: Color::Rgb(55, 57, 72),
        transparent: false,
    },
    // 11 — tokyo night
    Theme {
        name: "tokyo_night",
        bg: Color::Rgb(26, 27, 38),
        panel_bg: Color::Rgb(31, 35, 53),
        sidebar_bg: Color::Rgb(26, 27, 38),
        accent: Color::Rgb(122, 162, 247),
        subtle: Color::Rgb(86, 95, 137),
        text: Color::Rgb(192, 202, 245),
        text_dim: Color::Rgb(169, 177, 214),
        now_playing: Color::Rgb(158, 206, 106),
        danger: Color::Rgb(247, 118, 142),
        dir_col: Color::Rgb(224, 175, 104),
        vol_empty: Color::Rgb(41, 46, 66),
        transparent: false,
    },
    // 12 — solarized dark
    Theme {
        name: "solarized_dark",
        bg: Color::Rgb(0, 43, 54),
        panel_bg: Color::Rgb(7, 54, 66),
        sidebar_bg: Color::Rgb(0, 43, 54),
        accent: Color::Rgb(38, 139, 210),
        subtle: Color::Rgb(88, 110, 117),
        text: Color::Rgb(253, 246, 227),
        text_dim: Color::Rgb(147, 161, 161),
        now_playing: Color::Rgb(133, 153, 0),
        danger: Color::Rgb(220, 50, 47),
        dir_col: Color::Rgb(203, 75, 22),
        vol_empty: Color::Rgb(0, 35, 43),
        transparent: false,
    },
    // 13 — solarized light
    Theme {
        name: "solarized_light",
        bg: Color::Rgb(253, 246, 227),
        panel_bg: Color::Rgb(238, 232, 213),
        sidebar_bg: Color::Rgb(253, 246, 227),
        accent: Color::Rgb(38, 139, 210),
        subtle: Color::Rgb(147, 161, 161),
        text: Color::Rgb(0, 43, 54),
        text_dim: Color::Rgb(88, 110, 117),
        now_playing: Color::Rgb(133, 153, 0),
        danger: Color::Rgb(220, 50, 47),
        dir_col: Color::Rgb(203, 75, 22),
        vol_empty: Color::Rgb(210, 203, 186),
        transparent: false,
    },
    // 14 — everforest dark
    Theme {
        name: "everforest",
        bg: Color::Rgb(35, 38, 33),
        panel_bg: Color::Rgb(43, 47, 40),
        sidebar_bg: Color::Rgb(35, 38, 33),
        accent: Color::Rgb(131, 192, 146),
        subtle: Color::Rgb(93, 98, 86),
        text: Color::Rgb(211, 198, 170),
        text_dim: Color::Rgb(157, 151, 132),
        now_playing: Color::Rgb(131, 192, 146),
        danger: Color::Rgb(230, 126, 128),
        dir_col: Color::Rgb(230, 192, 115),
        vol_empty: Color::Rgb(53, 57, 49),
        transparent: false,
    },
    // 15 — kanagawa
    Theme {
        name: "kanagawa",
        bg: Color::Rgb(22, 22, 29),
        panel_bg: Color::Rgb(31, 31, 40),
        sidebar_bg: Color::Rgb(22, 22, 29),
        accent: Color::Rgb(126, 156, 216),
        subtle: Color::Rgb(84, 84, 109),
        text: Color::Rgb(220, 215, 186),
        text_dim: Color::Rgb(150, 147, 125),
        now_playing: Color::Rgb(118, 148, 106),
        danger: Color::Rgb(195, 95, 95),
        dir_col: Color::Rgb(196, 154, 105),
        vol_empty: Color::Rgb(38, 38, 50),
        transparent: false,
    },
];

// ── Shared widget helpers ──────────────────────────────────────────────────

/// Builds a consistently styled panel block.
pub fn styled_block<'a>(title: &'a str, focused: bool, theme: &Theme) -> Block<'a> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(if focused { theme.accent } else { theme.subtle }))
        .title_style(
            Style::default()
                .fg(if focused {
                    theme.accent
                } else {
                    theme.text_dim
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
