use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::modal::{hint, hint_height, modal_block, render_hints};
use crate::ui::layout::{Theme, truncate};

/// A file extension is considered audio if it is one of these.
/// Extensions the bundled symphonia decoders can actually play.  Anything not
/// listed here is hidden from the picker rather than offered and then rejected
/// by `validate_decodable` at import.
///
/// Deliberately absent: `opus` (symphonia 0.5 demuxes Ogg but ships no Opus
/// decoder) and `wma` (no ASF demuxer at all).
const AUDIO_EXTS: &[&str] = &[
    "mp3", "mp2", "mp1", "flac", "ogg", "oga", "wav", "wave", "aac", "m4a", "m4b", "aiff", "aif",
    "aifc",
];

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

// -- Entry ------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

// -- State ------------------------------------------------------------------

pub struct FilePicker {
    pub current_dir: PathBuf,
    pub entries: Vec<DirEntry>,
    pub cursor: usize,
}

impl FilePicker {
    pub fn new(start_dir: impl Into<PathBuf>) -> Self {
        let dir = start_dir.into();
        let mut picker = Self {
            current_dir: dir,
            entries: Vec::new(),
            cursor: 0,
        };
        picker.refresh();
        picker
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        self.cursor = 0;

        if let Some(parent) = self.current_dir.parent() {
            self.entries.push(DirEntry {
                name: "..".into(),
                path: parent.to_path_buf(),
                is_dir: true,
            });
        }

        let Ok(read) = fs::read_dir(&self.current_dir) else {
            return;
        };

        let mut dirs: Vec<DirEntry> = Vec::new();
        let mut files: Vec<DirEntry> = Vec::new();

        for entry in read.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = path.is_dir();

            if !is_dir && !is_audio(&path) {
                continue;
            }

            let bucket = if is_dir { &mut dirs } else { &mut files };
            bucket.push(DirEntry { name, path, is_dir });
        }

        dirs.sort_by_key(|a| a.name.to_lowercase());
        files.sort_by_key(|a| a.name.to_lowercase());

        self.entries.extend(dirs);
        self.entries.extend(files);
    }

    fn selected(&self) -> Option<&DirEntry> {
        self.entries.get(self.cursor)
    }

    fn enter_dir(&mut self) -> bool {
        if let Some(entry) = self.selected()
            && entry.is_dir
        {
            self.current_dir = entry.path.clone();
            self.refresh();
            return true;
        }
        false
    }

    pub fn handle_key(&mut self, code: KeyCode) -> FilePickerOutcome {
        if let Some(new) = crate::nav::list_move(code, self.cursor, self.entries.len()) {
            self.cursor = new;
            return FilePickerOutcome::Continue;
        }
        match code {
            KeyCode::Enter => {
                if let Some(entry) = self.selected().cloned() {
                    if entry.is_dir {
                        self.enter_dir();
                        FilePickerOutcome::Continue
                    } else {
                        FilePickerOutcome::Selected(entry.path)
                    }
                } else {
                    FilePickerOutcome::Continue
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => FilePickerOutcome::Dismissed,
            _ => FilePickerOutcome::Continue,
        }
    }
}

pub enum FilePickerOutcome {
    Continue,
    Selected(PathBuf),
    Dismissed,
}

// -- Rendering --------------------------------------------------------------

pub fn render_filepicker(frame: &mut Frame<'_>, picker: &FilePicker, theme: &Theme) {
    let area = frame.area();
    let width = area.width.min(70);
    let height = area.height.saturating_sub(4).min(30);
    let rect = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };

    frame.render_widget(Clear, rect);

    // icon (<=3 cols) + surrounding spaces (3) + corners (2) = 8 overhead
    let path_max = usize::from(width.saturating_sub(8));
    let path_str = picker.current_dir.to_string_lossy();
    let title = format!(
        " {} {} ",
        theme.glyphs().folder,
        truncate(&path_str, path_max)
    );

    // Same chrome as every other dialog; only the title alignment differs,
    // because a long path reads better anchored to the left.
    let block = modal_block(&title, theme).title_alignment(Alignment::Left);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let hints = [
        hint("Enter", "open/select"),
        hint("j/k", "navigate"),
        hint("Esc", "cancel"),
    ];
    let hint_h = hint_height(&hints, inner.width as usize, theme);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),         // entries
            Constraint::Length(1),      // gap above the hints
            Constraint::Length(hint_h), // hints
        ])
        .split(inner);
    let list_rect = rows[0];

    let name_max = usize::from(inner.width.saturating_sub(2)); // 2 cols for the icon
    let items: Vec<ListItem<'_>> = picker
        .entries
        .iter()
        .map(|e| {
            let g = theme.glyphs();
            let (icon, style) = if e.is_dir {
                (
                    format!("{} ", g.arrow_right),
                    Style::default()
                        .fg(theme.dir_col)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (format!("{} ", g.note), Style::default().fg(theme.text_dim))
            };
            ListItem::new(Line::from(vec![
                Span::styled(icon, style),
                Span::styled(truncate(&e.name, name_max), style),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(picker.cursor));

    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(theme.selection_style())
            .highlight_symbol(""),
        list_rect,
        &mut list_state,
    );

    render_hints(frame, rows[2], &hints, theme);
}
