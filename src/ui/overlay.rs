use ratatui::Frame;

use crate::ui::lyrics_overlay::render_lyrics_overlay;
use crate::{app::AppState, filepicker::render_filepicker, modal::render_modal};

/// Renders whichever overlay is active.
/// Priority: file picker > modal > lyrics overlay.
pub fn render_overlay(frame: &mut Frame, state: &AppState) {
    if let Some(picker) = &state.file_picker {
        render_filepicker(frame, picker, &state.theme);
    } else if let Some(modal) = &state.modal {
        render_modal(frame, modal, &state.theme);
    } else if state.show_lyrics
        && let Some(track_id) = state
            .now_playing
            .and_then(|i| state.queue.get(i))
            .map(|t| t.id)
    {
        render_lyrics_overlay(frame, state, track_id);
    }
}
