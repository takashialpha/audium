use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
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
    /// When set, render UI glyphs as ASCII (for limited terminals / a tty).
    /// Only the console fallback theme enables this.
    pub ascii: bool,
}

// ── Glyphs ──────────────────────────────────────────────────────────────────

/// Semantic UI glyphs, chosen per-terminal so a tty / limited console gets
/// ASCII fallbacks instead of unrenderable Unicode.  One table is selected via
/// [`Theme::glyphs`]; every render site reads from it rather than hard-coding
/// glyph literals.
#[derive(Debug)]
pub struct Glyphs {
    /// Player status: playback in progress (shown while paused-capable).
    pub play: &'static str,
    /// Player status: paused-capable (shown while playing).
    pub pause: &'static str,
    /// 3-column list prefix marking the selected / active row.
    pub marker: &'static str,
    /// Left / right value-cycle arrows in the settings rows.
    pub arrow_left: &'static str,
    pub arrow_right: &'static str,
    /// Horizontal bar fill / empty (progress, thumb, settings volume).
    pub bar_fill: &'static str,
    pub bar_empty: &'static str,
    /// 5-column vertical volume cells (padded), filled / empty.
    pub vol_fill: &'static str,
    pub vol_empty: &'static str,
    /// Padded metadata separator (e.g. `album · year`).
    pub sep: &'static str,
    /// Music note (lyrics title, file-picker audio entries).
    pub note: &'static str,
    /// Directory marker in the file picker's title.
    pub folder: &'static str,
    /// Terminal-capability banner bullet.
    pub bullet: &'static str,
    /// Playback-speed multiplier sign.
    pub times: &'static str,
}

static UNICODE_GLYPHS: Glyphs = Glyphs {
    play: "▶",
    pause: "⏸",
    marker: "▶  ",
    arrow_left: "◀",
    arrow_right: "▶",
    bar_fill: "█",
    bar_empty: "░",
    vol_fill: " ▓▓▓ ",
    vol_empty: " ░░░ ",
    sep: "  ·  ",
    note: "♪",
    folder: "📁",
    bullet: "●",
    times: "×",
};

static ASCII_GLYPHS: Glyphs = Glyphs {
    play: "|>",
    pause: "||",
    marker: ">  ",
    arrow_left: "<",
    arrow_right: ">",
    bar_fill: "#",
    bar_empty: "-",
    vol_fill: " ### ",
    vol_empty: " --- ",
    sep: "  -  ",
    note: "*",
    folder: "[/]",
    bullet: "*",
    times: "x",
};

impl Theme {
    /// The glyph table to render with: ASCII on limited terminals.
    pub const fn glyphs(&self) -> &'static Glyphs {
        if self.ascii {
            &ASCII_GLYPHS
        } else {
            &UNICODE_GLYPHS
        }
    }

    fn maybe_color(&self, color: Color) -> Option<Color> {
        (!self.transparent).then_some(color)
    }

    fn apply_color(&self, style: Style, color: Color) -> Style {
        self.maybe_color(color).map_or(style, |c| style.bg(c))
    }

    pub fn apply_bg(&self, style: Style) -> Style {
        self.apply_color(style, self.bg)
    }

    pub fn apply_panel_bg(&self, style: Style) -> Style {
        self.apply_color(style, self.panel_bg)
    }

    pub fn apply_sidebar_bg(&self, style: Style) -> Style {
        self.apply_color(style, self.sidebar_bg)
    }
}

// ── Built-in themes ────────────────────────────────────────────────────────

pub fn themes() -> &'static [Theme] {
    &THEMES
}

pub fn theme_by_name(name: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.name == name).unwrap_or(&THEMES[0])
}

/// The 16-color fallback used when truecolor is not in effect.
///
/// Built from named ANSI colors so it renders correctly on a real tty and
/// inherits whatever palette the user has themed their console with.  All
/// backgrounds are `Reset` (terminal default); selection contrast comes from
/// bold/foreground, matching how the RGB themes highlight rows.  Kept out of
/// `THEMES` so it never appears in the theme cycler.
pub fn console_theme() -> &'static Theme {
    &CONSOLE_THEME
}

static CONSOLE_THEME: Theme = Theme {
    name: "console",
    bg: Color::Reset,
    panel_bg: Color::Reset,
    sidebar_bg: Color::Reset,
    // Bright ANSI variants for higher contrast against a typical dark console.
    accent: Color::LightCyan,
    subtle: Color::Gray,
    text: Color::White,
    text_dim: Color::Gray,
    now_playing: Color::LightGreen,
    danger: Color::LightRed,
    dir_col: Color::LightYellow,
    vol_empty: Color::DarkGray,
    transparent: false,
    ascii: true,
};

static THEMES: [Theme; 15] = [
    // 1: dark (default)
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
        ascii: false,
    },
    // 2: light
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
        ascii: false,
    },
    // 3: nord
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
        ascii: false,
    },
    // 4: gruvbox dark
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
        ascii: false,
    },
    // 5: gruvbox light
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
        ascii: false,
    },
    // 6: rosé pine
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
        ascii: false,
    },
    // 7: rosé pine dawn
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
        ascii: false,
    },
    // 8: catppuccin mocha
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
        ascii: false,
    },
    // 9: catppuccin latte
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
        ascii: false,
    },
    // 10: dracula
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
        ascii: false,
    },
    // 11: tokyo night
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
        ascii: false,
    },
    // 12: solarized dark
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
        ascii: false,
    },
    // 13: solarized light
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
        ascii: false,
    },
    // 14: everforest dark
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
        ascii: false,
    },
    // 15: kanagawa
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
        ascii: false,
    },
];

// ── Shared widget helpers ──────────────────────────────────────────────────

/// Renders `value` with a vi-style block cursor: reverse-video *covering* the
/// character at byte offset `cursor` (a trailing block at end-of-line) rather
/// than an extra glyph inserted between characters.  Works in every color mode
/// (`REVERSED` is a plain terminal attribute, so it renders as a real block on
/// a tty too).
pub fn cursor_spans(value: &str, cursor: usize, theme: &Theme) -> Vec<Span<'static>> {
    let text = Style::default().fg(theme.text);
    let block = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::REVERSED);
    let before = &value[..cursor];
    let after = &value[cursor..];
    let (under, rest) = after.chars().next().map_or_else(
        || (" ".to_string(), ""),
        |c| (c.to_string(), &after[c.len_utf8()..]),
    );
    vec![
        Span::styled(before.to_string(), text),
        Span::styled(under, block),
        Span::styled(rest.to_string(), text),
    ]
}

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
