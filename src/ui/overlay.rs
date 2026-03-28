use ratatui::Frame;

use crate::{app::AppState, filepicker::render_filepicker, modal::render_modal};

/// Renders whichever overlay is active (file picker takes priority over modals).
pub fn render_overlay(frame: &mut Frame, state: &AppState) {
    if let Some(picker) = &state.file_picker {
        render_filepicker(frame, picker);
    } else if let Some(modal) = &state.modal {
        render_modal(frame, modal);
    }
}
