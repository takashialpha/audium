use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

use super::layout::{Theme, format_duration};
use crate::app::{AppState, LoopMode};
use crate::library::Track;
use crate::numeric::{ratio_to_unit_count, ratio_to_whole_percent};

/// Rows the bar occupies: the top rule plus three content rows.
pub const PLAYER_BAR_H: u16 = 4;

/// Columns inset from each edge, matching the dialogs' gutter.
const PAD_X: u16 = 2;

/// Cells in the volume meter.  Wide enough that a step is visible rather than
/// implied: at eight cells a 1% change often moved nothing.
const VOL_CELLS: usize = 14;

/// Width below which the volume meter shrinks to a bare percentage.
const NARROW: u16 = 64;

/// Columns the title side keeps before a right-hand cluster is dropped
/// altogether: the track and its position matter more than loop state or
/// volume, so those yield first.
const MIN_LEFT: u16 = 24;

/// Renders the bar for the current track.  Only called while something is
/// playing; `ui::render` gives the rows back to the lists otherwise.
pub fn render_player_bar(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let t = &state.theme;

    let outer = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(t.subtle))
        .padding(Padding::horizontal(PAD_X))
        .style(t.apply_bg(Style::default()));

    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    if inner.width == 0 || inner.height < 3 {
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title + transport state
            Constraint::Length(1), // artist / album + volume
            Constraint::Length(1), // scrubber
        ])
        .split(inner);

    let track = state.now_playing.and_then(|i| state.queue.get(i));

    render_title_row(frame, rows[0], state, track, t);
    render_meta_row(frame, rows[1], state, track, t);
    render_scrubber_row(frame, rows[2], state, t);
}

/// The track title, with playback state and transport modes opposite it.
fn render_title_row(
    frame: &mut Frame<'_>,
    row: Rect,
    state: &AppState,
    track: Option<&Track>,
    t: &Theme,
) {
    let title = track.map_or_else(
        || Span::styled("Nothing playing".to_string(), Style::default().fg(t.subtle)),
        |tr| {
            Span::styled(
                tr.name.clone(),
                Style::default().fg(t.text).add_modifier(Modifier::BOLD),
            )
        },
    );

    // Playing or paused outranks the transport modes: if only one of them
    // fits, it is the one that says whether audio is coming out.
    let full = transport_spans(state, true, t);
    let (left, right) = if let (left, Some(area)) = split_row(row, span_width(&full)) {
        (left, Some((area, full)))
    } else {
        let brief = transport_spans(state, false, t);
        let (left, area) = split_row(row, span_width(&brief));
        (left, area.map(|a| (a, brief)))
    };

    frame.render_widget(Paragraph::new(Line::from(vec![title])), left);
    if let Some((area, spans)) = right {
        frame.render_widget(Paragraph::new(Line::from(spans)).right_aligned(), area);
    }
}

/// Artist and album, indented under the title, with the volume meter opposite.
fn render_meta_row(
    frame: &mut Frame<'_>,
    row: Rect,
    state: &AppState,
    track: Option<&Track>,
    t: &Theme,
) {
    let g = t.glyphs();
    let mut left: Vec<Span<'_>> = Vec::new();

    if let Some(tr) = track {
        if let Some(artist) = tr.artist.as_deref().filter(|a| !a.is_empty()) {
            left.push(Span::styled(
                artist.to_string(),
                Style::default().fg(t.text_dim),
            ));
        }
        if let Some(album) = tr.album.as_deref().filter(|a| !a.is_empty()) {
            if !left.is_empty() {
                left.push(Span::styled(g.sep, Style::default().fg(t.subtle)));
            }
            left.push(Span::styled(
                album.to_string(),
                Style::default().fg(t.subtle),
            ));
        }
    }

    let vol = volume_spans(state.player.volume, row.width, t);
    let (left_area, right) = split_row(row, span_width(&vol));

    frame.render_widget(Paragraph::new(Line::from(left)), left_area);
    if let Some(right) = right {
        frame.render_widget(Paragraph::new(Line::from(vol)).right_aligned(), right);
    }
}

/// Elapsed time, the scrubber, and the track length, in that order across the
/// full width: the arrangement every music player uses, and it lets the
/// scrubber be the widest thing on screen rather than sharing a row.
fn render_scrubber_row(frame: &mut Frame<'_>, row: Rect, state: &AppState, t: &Theme) {
    let playing = state.now_playing.is_some();
    let elapsed = if playing {
        format_duration(state.elapsed().as_secs())
    } else {
        "0:00".to_string()
    };
    let total = state
        .track_duration
        .filter(|_| playing)
        .map_or_else(|| "-:--".to_string(), |d| format_duration(d.as_secs()));

    // Both cells are sized to the longer label so the scrubber does not shift
    // when the clock rolls past ten minutes.
    let cell = elapsed.chars().count().max(total.chars().count()).max(4);
    let flank = cell + 2;
    let track_w = usize::from(row.width).saturating_sub(flank * 2);

    let mut spans = vec![Span::styled(
        format!("{elapsed:>cell$}  "),
        Style::default().fg(t.text_dim),
    )];
    spans.extend(scrubber_spans(track_w, state.progress_ratio(), t));
    spans.push(Span::styled(
        format!("  {total:<cell$}"),
        Style::default().fg(t.text_dim),
    ));

    frame.render_widget(Paragraph::new(Line::from(spans)), row);
}

// -- Pieces -----------------------------------------------------------------

/// A single unbroken line with a head at the play position.  The line keeps
/// one weight end to end and only its colour changes: a stroke that thins
/// after the head reads as though the rest of the track had faded out.
fn scrubber_spans(width: usize, ratio: f64, t: &Theme) -> Vec<Span<'static>> {
    let g = t.glyphs();
    if width == 0 {
        return Vec::new();
    }
    let filled = ratio_to_unit_count(ratio, width);
    let before = filled.saturating_sub(1);
    let after = width.saturating_sub(before + 1);

    let mut spans = Vec::with_capacity(3);
    if before > 0 {
        spans.push(Span::styled(
            g.track.repeat(before),
            Style::default().fg(t.accent),
        ));
    }
    spans.push(Span::styled(
        g.thumb.to_string(),
        Style::default().fg(t.text).add_modifier(Modifier::BOLD),
    ));
    if after > 0 {
        spans.push(Span::styled(
            g.track.repeat(after),
            Style::default().fg(t.subtle),
        ));
    }
    spans
}

/// Playback state and the transport modes, each as `label value`.
///
/// Every item is named. A bare glyph cannot say whether it reports the current
/// state or the action a button would take, and a bare `1.00x` does not say
/// what it measures. All three are always present, dim at their default value:
/// a label that appears and vanishes makes the row jump, and one that is never
/// shown is never discovered.
fn transport_spans(state: &AppState, with_modes: bool, t: &Theme) -> Vec<Span<'static>> {
    let g = t.glyphs();
    let on = Style::default().fg(t.accent).add_modifier(Modifier::BOLD);

    let playing = !state.player.is_paused;
    let status = Span::styled(
        if playing { "playing" } else { "paused" }.to_string(),
        if playing {
            on
        } else {
            // Paused is the state that explains why nothing is coming out of
            // the speakers, so it is the one worth noticing.
            Style::default().fg(t.danger).add_modifier(Modifier::BOLD)
        },
    );
    if !with_modes {
        return vec![status];
    }

    let speed = state.player.playback_speed;
    let speed_changed = (speed - 1.0).abs() > 0.001;
    // Two decimals: the step is 1%, so anything coarser would not move.
    let speed_value = format!("{speed:.2}{}", g.times);

    let (loop_value, loop_on) = match state.loop_mode {
        LoopMode::Off => ("off", false),
        LoopMode::Queue => ("queue", true),
        LoopMode::Track => ("track", true),
    };

    // Only non-default modes are shown. Carrying `speed 1.00x` and `loop off`
    // at all times filled the cluster with two items that say nothing is
    // happening; the keybinding list is where these are discovered, and
    // changing one is itself the moment you see it appear.
    let mut spans = vec![status];
    if speed_changed {
        push_item(&mut spans, "speed", speed_value, t);
    }
    if loop_on {
        push_item(&mut spans, "loop", loop_value.to_string(), t);
    }
    spans
}

/// Appends `label value` to a status cluster, preceded by a rule.
///
/// Items set apart only by whitespace run together into one long string of
/// words; a rule between them says where each one ends.
fn push_item(spans: &mut Vec<Span<'static>>, label: &str, value: String, t: &Theme) {
    spans.push(Span::styled(
        format!("  {}  ", t.glyphs().divider),
        Style::default().fg(t.subtle),
    ));
    spans.push(Span::styled(
        format!("{label} "),
        Style::default().fg(t.subtle),
    ));
    spans.push(Span::styled(
        value,
        Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
    ));
}

/// A labelled meter (`volume`, the bar, then the percentage), degrading to
/// just the percentage when space is tight.
fn volume_spans(vol: f32, row_width: u16, t: &Theme) -> Vec<Span<'static>> {
    let pct = ratio_to_whole_percent(vol);
    let label = Span::styled(format!("  {pct:>3}%"), Style::default().fg(t.text_dim));

    if row_width < NARROW {
        return vec![label];
    }

    let g = t.glyphs();
    let filled = ratio_to_unit_count(f64::from(vol), VOL_CELLS);
    vec![
        Span::styled("volume ".to_string(), Style::default().fg(t.subtle)),
        Span::styled(g.bar_fill.repeat(filled), Style::default().fg(t.accent)),
        Span::styled(
            g.bar_empty.repeat(VOL_CELLS - filled),
            Style::default().fg(t.vol_empty),
        ),
        label,
    ]
}

// -- Helpers ----------------------------------------------------------------

fn span_width(spans: &[Span<'_>]) -> u16 {
    u16::try_from(
        spans
            .iter()
            .map(|s| s.content.chars().count())
            .sum::<usize>(),
    )
    .unwrap_or(u16::MAX)
}

/// Splits a row into a flexible left side and a fixed right side, dropping the
/// right side entirely when the row cannot hold both.
fn split_row(row: Rect, right_w: u16) -> (Rect, Option<Rect>) {
    if right_w == 0 || row.width < right_w.saturating_add(MIN_LEFT) {
        return (row, None);
    }
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(right_w)])
        .split(row);
    (cols[0], Some(cols[1]))
}
