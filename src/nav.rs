//! Shared list-navigation keymap.
//!
//! Every scrollable list in the app (library panels, file picker, menus,
//! playlist picker, lyrics scroll) routes cursor movement through
//! [`list_move`] so the keys behave identically everywhere.

use ratatui::crossterm::event::KeyCode;

/// Rows moved by `PageUp` / `PageDown`.
pub const PAGE: usize = 10;

/// Maps a navigation key to the new cursor index over a list of `len` items.
///
/// Returns `None` for keys that are not list navigation, so callers can fall
/// through to their own bindings.  Supported keys (all clamped to range):
/// `j`/`Down`, `k`/`Up`, `g`/`Home` (top), `G`/`End` (bottom),
/// `PageDown`, `PageUp`.
pub fn list_move(code: KeyCode, cursor: usize, len: usize) -> Option<usize> {
    let last = len.saturating_sub(1);
    let target = match code {
        KeyCode::Char('j') | KeyCode::Down => (cursor + 1).min(last),
        KeyCode::Char('k') | KeyCode::Up => cursor.saturating_sub(1),
        KeyCode::Char('g') | KeyCode::Home => 0,
        KeyCode::Char('G') | KeyCode::End => last,
        KeyCode::PageDown => (cursor + PAGE).min(last),
        KeyCode::PageUp => cursor.saturating_sub(PAGE),
        _ => return None,
    };
    Some(target)
}
