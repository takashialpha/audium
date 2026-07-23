use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

// -- Theme ------------------------------------------------------------------

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

// -- Glyphs ------------------------------------------------------------------

/// Semantic UI glyphs, chosen per-terminal so a tty / limited console gets
/// ASCII fallbacks instead of unrenderable Unicode.  One table is selected via
/// [`Theme::glyphs`]; every render site reads from it rather than hard-coding
/// glyph literals.
#[derive(Debug)]
pub struct Glyphs {
    /// Vertical rule separating items in a status cluster.
    pub divider: &'static str,
    /// 3-column list prefix marking the selected / active row.
    pub marker: &'static str,
    /// Left / right value-cycle arrows in the settings rows.
    pub arrow_left: &'static str,
    pub arrow_right: &'static str,
    /// Horizontal bar fill / empty (progress, thumb, settings volume).
    pub bar_fill: &'static str,
    pub bar_empty: &'static str,
    /// Padded metadata separator (e.g. `album - year`).
    pub sep: &'static str,
    /// Music note (lyrics title, file-picker audio entries).
    pub note: &'static str,
    /// Directory marker in the file picker's title.
    pub folder: &'static str,
    /// Terminal-capability banner bullet.
    pub bullet: &'static str,
    /// Playback-speed multiplier sign.
    pub times: &'static str,
    /// Horizontal rule fill, used under the track table's header row.
    pub rule: &'static str,
    /// Scrubber: the track line, and the head riding along it.
    pub track: &'static str,
    pub thumb: &'static str,
    /// Frames of the small equaliser shown against the playing row. Cycled by
    /// elapsed time, so the list shows playback is live without the cost of a
    /// real animation.
    pub eq: [&'static str; 4],
    /// The same equaliser at rest, for the track that is loaded but paused.
    pub eq_idle: &'static str,
}

static UNICODE_GLYPHS: Glyphs = Glyphs {
    divider: "│",
    marker: "▶  ",
    arrow_left: "◀",
    arrow_right: "▶",
    bar_fill: "█",
    bar_empty: "░",
    sep: "  ·  ",
    note: "♪",
    folder: "📁",
    bullet: "●",
    times: "×",
    rule: "─",
    track: "━",
    thumb: "●",
    eq: ["▁▃▅", "▃▅▂", "▅▂▆", "▂▆▃"],
    eq_idle: "▁▁▁",
};

static ASCII_GLYPHS: Glyphs = Glyphs {
    divider: "|",
    marker: ">  ",
    arrow_left: "<",
    arrow_right: ">",
    bar_fill: "#",
    bar_empty: "-",
    sep: "  -  ",
    note: "*",
    folder: "[/]",
    bullet: "*",
    times: "x",
    rule: "-",
    track: "=",
    thumb: "O",
    eq: [".oO", "oO.", "O.o", ".Oo"],
    eq_idle: "...",
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

    /// Style marking the selected row of a list.
    ///
    /// The RGB themes tint the row's background, which is legible because
    /// `panel_bg` sits a shade off `bg`.  The console themes have no such
    /// shade to spend: at 16 colors a mid-tone selection background wrecks
    /// contrast from both sides, and leaving the row to a brighter foreground
    /// alone gives only 2.3:1 against an unselected one, which on a real tty
    /// is hard to pick out.  Reverse video swaps foreground and background
    /// instead, so the row reads as a solid block whatever the palette is.
    pub fn selection_style(&self) -> Style {
        if self.ascii {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
                .fg(self.text)
                .bg(self.panel_bg)
                .add_modifier(Modifier::BOLD)
        }
    }
}

// -- Built-in themes --------------------------------------------------------

pub fn themes() -> &'static [Theme] {
    &THEMES
}

pub fn theme_by_name(name: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.name == name).unwrap_or(&THEMES[0])
}

/// The 16-color fallbacks used when truecolor is not in effect.
///
/// Tuned against the Linux console's actual default palette, which the kernel
/// exposes at `/sys/module/vt/parameters/default_{red,grn,blu}`:
///
/// ```text
/// 0 black    (  0,  0,  0)    8  darkgray     ( 85, 85, 85)
/// 1 red      (170,  0,  0)    9  lightred     (255, 85, 85)
/// 2 green    (  0,170,  0)    10 lightgreen   ( 85,255, 85)
/// 3 yellow   (170, 85,  0)    11 lightyellow  (255,255, 85)
/// 4 blue     (  0,  0,170)    12 lightblue    ( 85, 85,255)
/// 5 magenta  (170,  0,170)    13 lightmagenta (255, 85,255)
/// 6 cyan     (  0,170,170)    14 lightcyan    ( 85,255,255)
/// 7 gray     (170,170,170)    15 white        (255,255,255)
/// ```
///
/// Those values decide which color can appear on which background.  On black,
/// blue is 1.6:1 and darkgray 2.8:1 -- both effectively unreadable.  On white,
/// every bright variant collapses (lightyellow is 1.1:1, lightcyan 1.2:1), so
/// the two themes share almost no colors.
///
/// Both themes keep `panel_bg` equal to `bg`, so panels are flat and the
/// selected row is marked by foreground and weight rather than a background
/// band.  A mid-tone selection background is tempting but wrecks contrast from
/// both sides at this palette size: lightred on darkgray is 2.4:1, yellow on
/// gray is 2.3:1.  The `>` marker plus a bold, brighter foreground carries the
/// selection instead, which is what console programs generally do.
///
/// Kept out of `THEMES` so they never appear in the truecolor theme cycler.
pub fn console_themes() -> &'static [Theme] {
    &CONSOLE_THEMES
}

/// Looks up a console theme by name, falling back to the dark one.
pub fn console_theme_by_name(name: &str) -> &'static Theme {
    CONSOLE_THEMES
        .iter()
        .find(|t| t.name == name)
        .unwrap_or(&CONSOLE_THEMES[0])
}

static CONSOLE_THEMES: [Theme; 2] = [
    // Default: a tty is black out of the box, so `bg` stays at the terminal's
    // own color and every foreground is picked for contrast against black.
    // Sixteen colors do not offer three legible neutral steps above black:
    // darkgray is 2.8:1 and reads as black on a real tty, so it is not used at
    // all here.  Dim text and hints therefore share one step below white, and
    // the bar's empty remainder is told apart by its glyph (`-` against `#`)
    // rather than by being dimmer.
    Theme {
        name: "console_dark",
        bg: Color::Reset,
        panel_bg: Color::Reset,
        sidebar_bg: Color::Reset,
        accent: Color::LightCyan,       // 17.1:1
        subtle: Color::Gray,            //  9.0:1
        text: Color::White,             // 21.0:1
        text_dim: Color::Gray,          //  9.0:1
        now_playing: Color::LightGreen, // 15.8:1
        danger: Color::LightRed,        //  6.7:1
        dir_col: Color::LightYellow,    // 19.7:1
        vol_empty: Color::Gray,         //  9.0:1; darkgray reads as black on a tty
        transparent: false,
        ascii: true,
    },
    // For a light console.  `bg` is set explicitly rather than inherited:
    // "light" has to paint the background white, or on a tty it renders black
    // text on the console's black.  Only the dim ANSI variants survive here.
    Theme {
        name: "console_light",
        bg: Color::White,
        panel_bg: Color::White,
        sidebar_bg: Color::White,
        accent: Color::Blue,         // 13.3:1
        subtle: Color::DarkGray,     //  7.5:1
        text: Color::Black,          // 21.0:1
        text_dim: Color::DarkGray,   //  7.5:1
        now_playing: Color::Magenta, //  6.4:1 (green would be 3.1:1 here)
        danger: Color::Red,          //  7.8:1
        dir_col: Color::Yellow,      //  5.2:1 (renders as brown)
        vol_empty: Color::DarkGray,  //  7.5:1; gray on white is only 2.3:1
        transparent: false,
        ascii: true,
    },
];

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
    // 6: rose pine
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
    // 7: rose pine dawn
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

// -- Shared widget helpers --------------------------------------------------

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

/// Like [`cursor_spans`], but keeps the cursor visible in values too long for
/// the field by scrolling a `width`-cell window over them, the way a shell
/// prompt does.  Without this a long name simply runs past the right edge and
/// you end up typing blind.
pub fn cursor_spans_windowed(
    value: &str,
    cursor: usize,
    width: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let start = h_window_start(value, cursor, width);
    if start == 0 && value.chars().count() < width.max(1) {
        return cursor_spans(value, cursor, theme);
    }

    let window: String = value.chars().skip(start).take(width).collect();
    let cur = value[..cursor].chars().count();
    let window_cursor = window
        .char_indices()
        .nth(cur.saturating_sub(start))
        .map_or(window.len(), |(i, _)| i);

    cursor_spans(&window, window_cursor, theme)
}

/// First visible character of a `width`-wide window that keeps `cursor` in
/// view.
///
/// Exposed so a multi-line editor can offset every row by the same amount:
/// scrolling only the cursor's row would slide it out of alignment with the
/// lines above and below it.
pub fn h_window_start(value: &str, cursor: usize, width: usize) -> usize {
    let len = value.chars().count();
    // The block cursor needs one cell past the final character, so a value
    // only fits untouched when it is strictly shorter than the field.
    if width == 0 || len < width {
        return 0;
    }
    let cur = value[..cursor].chars().count();
    cur.saturating_sub(width - 1).min(len + 1 - width)
}

/// A `width`-wide slice of `line` starting at `start` characters in.
pub fn h_window(line: &str, start: usize, width: usize) -> String {
    line.chars().skip(start).take(width).collect()
}

/// Centred two-line prompt for an empty panel: a statement of what is missing,
/// then the key that fixes it.  Shared so every empty panel reads the same
/// way instead of each inventing its own one-liner.
pub fn render_empty_state(
    frame: &mut Frame<'_>,
    area: Rect,
    headline: &str,
    key: &str,
    action: &str,
    t: &Theme,
) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1), // headline
            Constraint::Length(1), // "press <key> to ..."
            Constraint::Min(0),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Span::styled(
            headline.to_string(),
            Style::default().fg(t.text_dim),
        ))
        .alignment(Alignment::Center),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("press ", Style::default().fg(t.subtle)),
            Span::styled(key.to_string(), Style::default().fg(t.accent)),
            Span::styled(format!(" {action}"), Style::default().fg(t.subtle)),
        ]))
        .alignment(Alignment::Center),
        rows[2],
    );
}

/// Width of the index column, and the gap between every column.
pub const NUM_W: usize = 3;
const GAP: usize = 2;
pub const GAP_S: &str = "  ";

/// Column widths for one tracklist row.
///
/// A library reads as a table, not as `artist - title` run together. Narrow
/// panels drop columns from the right rather than squeezing all three into
/// illegibility, so the title always keeps a usable width.
pub struct Columns {
    title: usize,
    artist: usize,
    album: usize,
    time: usize,
}

impl Columns {
    const MIN_TITLE: usize = 16;
    const MIN_META: usize = 10;
    /// Enough for `mm:ss` up to 99 minutes; longer runs simply widen the cell.
    const TIME_W: usize = 5;

    pub const fn for_width(width: usize) -> Self {
        let body = width.saturating_sub(NUM_W + GAP);
        let meta2 = (GAP + Self::MIN_META) * 2;
        let time = GAP + Self::TIME_W;

        if body >= Self::MIN_TITLE + meta2 + time {
            // Roughly 4:3:3. Giving the title every spare column instead left
            // it sprawling on a wide terminal, with the metadata stranded far
            // to the right; keeping the split proportional holds the columns
            // together at any width.
            let rest = body - time - GAP * 2;
            let artist = rest * 3 / 10;
            let album = rest * 3 / 10;
            Self {
                title: rest - artist - album,
                artist,
                album,
                time: Self::TIME_W,
            }
        } else if body >= Self::MIN_TITLE + GAP + Self::MIN_META + time {
            let rest = body - time;
            let title = rest * 3 / 5;
            Self {
                title,
                artist: rest - title - GAP,
                album: 0,
                time: Self::TIME_W,
            }
        } else if body >= Self::MIN_TITLE + GAP + Self::MIN_META {
            let title = body * 3 / 5;
            Self {
                title,
                artist: body - title - GAP,
                album: 0,
                time: 0,
            }
        } else {
            Self {
                title: body,
                artist: 0,
                album: 0,
                time: 0,
            }
        }
    }

    /// One span per visible column, each padded to its width so the columns
    /// line up whatever the contents are.
    pub fn cells(&self, title: &str, artist: &str, album: &str, time: &str) -> Vec<Span<'static>> {
        let mut out = vec![Span::raw(pad(title, self.title))];
        if self.artist > 0 {
            out.push(Span::raw(format!("{GAP_S}{}", pad(artist, self.artist))));
        }
        if self.album > 0 {
            out.push(Span::raw(format!("{GAP_S}{}", pad(album, self.album))));
        }
        if self.time > 0 {
            // Right-aligned: durations line up on the colon.
            out.push(Span::raw(format!(
                "{GAP_S}{time:>w$}",
                w = self.time.max(time.chars().count())
            )));
        }
        out
    }
}

/// Truncates to `width` and pads back out to it, so a short value still
/// occupies its whole column.
fn pad(s: &str, width: usize) -> String {
    let s = truncate(s, width);
    let fill = width.saturating_sub(s.chars().count());
    format!("{s}{blank:fill$}", blank = "")
}

/// The gutter cell for a track row: a live equaliser on the playing track,
/// blank on every other.  Shared so the library and the queue mark the current
/// track the same way.
pub fn row_marker(is_playing: bool, paused: bool, elapsed: Duration, t: &Theme) -> String {
    if !is_playing {
        return String::new();
    }
    let g = t.glyphs();
    if paused {
        return g.eq_idle.to_string();
    }
    let frame = (elapsed.as_millis() / 250) as usize % g.eq.len();
    g.eq[frame].to_string()
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
        return s.to_string();
    }
    // At max == 0 there is not even room for the ellipsis, and returning "~"
    // would overflow the caller's field by one cell.
    if max == 0 {
        return String::new();
    }
    format!("{}~", s.chars().take(max - 1).collect::<String>())
}

pub fn format_duration(secs: u64) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{m}:{s:02}")
}
