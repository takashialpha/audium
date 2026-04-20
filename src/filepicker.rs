use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

const BG: Color = Color::Rgb(18, 18, 18);
const ACCENT: Color = Color::Rgb(100, 180, 255);
const SUBTLE: Color = Color::Rgb(80, 80, 80);
const TEXT: Color = Color::White;
const TEXT_DIM: Color = Color::Rgb(179, 179, 179);
const DIR_COL: Color = Color::Rgb(255, 210, 100);

/// A file extension is considered audio if it is one of these.
const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "ogg", "wav", "aac", "m4a", "opus", "wma", "aiff",
];

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

// ── Entry ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

// ── State ──────────────────────────────────────────────────────────────────

pub struct FilePicker {
    pub current_dir: PathBuf,
    pub entries: Vec<DirEntry>,
    pub cursor: usize,
}

impl FilePicker {
    /// Opens the picker at `start_dir` (falls back to home, then `/`).
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

    /// Reloads directory entries for `current_dir`.
    pub fn refresh(&mut self) {
        self.entries.clear();
        self.cursor = 0;

        // ".." parent entry (unless we are at filesystem root).
        if self.current_dir.parent().is_some() {
            self.entries.push(DirEntry {
                name: "..".into(),
                path: self.current_dir.parent().unwrap().to_path_buf(),
                is_dir: true,
            });
        }

        let read = match fs::read_dir(&self.current_dir) {
            Ok(r) => r,
            Err(_) => return,
        };

        let mut dirs: Vec<DirEntry> = Vec::new();
        let mut files: Vec<DirEntry> = Vec::new();

        for entry in read.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = path.is_dir();

            if !is_dir && !is_audio(&path) {
                continue; // hide non-audio files
            }

            let bucket = if is_dir { &mut dirs } else { &mut files };
            bucket.push(DirEntry { name, path, is_dir });
        }

        dirs.sort_by_key(|a| a.name.to_lowercase());
        files.sort_by_key(|a| a.name.to_lowercase());

        self.entries.extend(dirs);
        self.entries.extend(files);
    }

    // ── Cursor movement ──────────────────────────────────────────────────

    pub fn move_down(&mut self) {
        if !self.entries.is_empty() {
            self.cursor = (self.cursor + 1).min(self.entries.len() - 1);
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    // ── Selection ────────────────────────────────────────────────────────

    /// Returns the selected entry, if any.
    pub fn selected(&self) -> Option<&DirEntry> {
        self.entries.get(self.cursor)
    }

    /// Navigates into the selected directory.  Returns `true` if successful.
    pub fn enter_dir(&mut self) -> bool {
        if let Some(entry) = self.selected()
            && entry.is_dir
        {
            self.current_dir = entry.path.clone();
            self.refresh();
            return true;
        }
        false
    }

    // ── Key handling ─────────────────────────────────────────────────────

    pub fn handle_key(&mut self, code: KeyCode) -> FilePickerOutcome {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_down();
                FilePickerOutcome::Continue
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_up();
                FilePickerOutcome::Continue
            }
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

// ── Rendering ──────────────────────────────────────────────────────────────

pub fn render_filepicker(frame: &mut Frame, picker: &FilePicker) {
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

    let title = format!(" 📁 {} ", picker.current_dir.to_string_lossy());

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));

    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    // Reserve the last row for the hint line.
    let list_height = inner.height.saturating_sub(1);
    let list_rect = Rect {
        height: list_height,
        ..inner
    };
    let hint_rect = Rect {
        y: inner.y + list_height,
        height: 1,
        ..inner
    };

    let items: Vec<ListItem> = picker
        .entries
        .iter()
        .map(|e| {
            let (icon, style) = if e.is_dir {
                (
                    "▶ ",
                    Style::default().fg(DIR_COL).add_modifier(Modifier::BOLD),
                )
            } else {
                ("♪ ", Style::default().fg(TEXT_DIM))
            };
            ListItem::new(Line::from(vec![
                Span::styled(icon, style),
                Span::styled(e.name.clone(), style),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(picker.cursor));

    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(
                Style::default()
                    .fg(TEXT)
                    .bg(Color::Rgb(40, 40, 40))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(""),
        list_rect,
        &mut list_state,
    );

    frame.render_widget(
        ratatui::widgets::Paragraph::new(Span::styled(
            "[Enter] open/select  [j/k] navigate  [Esc] cancel",
            Style::default().fg(SUBTLE),
        )),
        hint_rect,
    );
}
