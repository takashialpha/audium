use ratatui::Frame;

use crate::{app::AppState, filepicker::render_filepicker, modal::render_modal};
use crate::downloader::render_downloader;

/// Renders whichever overlay is active (file picker takes priority over modals).
pub fn render_overlay(frame: &mut Frame, state: &AppState) {
    if let Some(picker) = &state.file_picker {
        render_filepicker(frame, picker);
    } else if let Some(modal) = &state.modal {
        render_modal(frame, modal);
    } else if let Some (downloader) = &state.downloader {
        render_downloader(frame, downloader);
    }
}