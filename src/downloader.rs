use crate::library::Library;
use crate::modal::{TextInput, centered_rect, render_text_input};
use crate::ui::Colors;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::prelude::Style;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Gauge, Paragraph, Wrap};
use rusty_ytdl;
use rusty_ytdl::{VideoQuality, VideoSearchOptions};
use std::path::PathBuf;
use std::sync::mpsc;

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Initializing,
    Downloading {
        title: String,
        progress: f64,
    },
    Finished {
        title: String,
        path_buf: Option<PathBuf>,
    },
    Error(String),
}

pub struct DownloadHandler {
    pub url: TextInput,
    pub title: Option<String>,
    pub rx: Option<mpsc::Receiver<DownloadEvent>>,
    pub event: DownloadEvent,
    pub should_close: bool,
}

impl DownloadHandler {
    pub fn new() -> DownloadHandler {
        Self {
            url: TextInput::default(),
            title: None,
            rx: None,
            event: DownloadEvent::Initializing,
            should_close: false,
        }
    }

    pub fn init_event(&mut self, rx: mpsc::Receiver<DownloadEvent>) {
        self.rx = Some(rx);
        self.event = DownloadEvent::Downloading {
            title: "Starting...".to_string(),
            progress: 0.0,
        }
    }

    pub fn add_event(&mut self, event: DownloadEvent) {
        self.event = event;
    }

    pub fn handle_key(&mut self, code: KeyCode) -> DownloaderOutcome {
        match code {
            KeyCode::Esc => DownloaderOutcome::Dismissed,
            KeyCode::Backspace => {
                self.url.backspace();
                DownloaderOutcome::Continue
            }
            KeyCode::Enter => DownloaderOutcome::StartDownload(self.url.value.clone()),
            _ => match code.as_char() {
                None => DownloaderOutcome::Continue,
                Some(key) => {
                    self.url.push(key);
                    DownloaderOutcome::Continue
                }
            },
        }
    }
}

pub enum DownloaderOutcome {
    Continue,
    StartDownload(String),
    Dismissed,
}

pub fn render_downloader(frame: &mut Frame, download_handler: &DownloadHandler) {
    let area = frame.area();
    let rect = centered_rect(52, 7, area); // Slightly wider for long titles

    // 1. Punch the hole
    frame.render_widget(Clear, rect);

    // 2. Define the outer block
    let block = Block::default()
        .title(Span::styled(
            " 📥 YouTube Downloader ",
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Colors::ACCENT))
        .style(Style::default().bg(Colors::PANEL_BG));

    let inner_area = block.inner(rect);
    frame.render_widget(block, rect);

    match &download_handler.event {
        DownloadEvent::Initializing => {
            // Your existing input helper
            render_text_input(
                frame,
                "Download Handler",
                &download_handler.url,
                "Enter YouTube Url: ",
            );
        }

        DownloadEvent::Downloading { title, progress } => {
            // Split inner area into two rows: one for title, one for gauge
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1) // Padding inside the block
                .constraints([
                    Constraint::Length(1), // Title row
                    Constraint::Min(1),    // Gauge row
                ])
                .split(inner_area);

            let title_text = format!("Fetching: {}", title);
            frame.render_widget(
                Paragraph::new(title_text).style(Style::default().fg(Colors::TEXT)),
                chunks[0],
            );

            // Row 2: The Gauge
            let gauge = Gauge::default()
                .block(Block::default())
                .gauge_style(
                    Style::default()
                        .fg(Colors::ACCENT)
                        .bg(Color::Rgb(30, 30, 30)),
                )
                .percent(*progress as u16)
                .label(format!("{:.1}%", progress));

            frame.render_widget(gauge, chunks[1]);
        }

        DownloadEvent::Finished { title, .. } => {
            let msg = format!("Successfully saved: {}", title);
            let p = Paragraph::new(msg)
                .style(Style::default().fg(Color::Green))
                .alignment(Alignment::Center);
            frame.render_widget(p, inner_area);
        }

        DownloadEvent::Error(err) => {
            let p = Paragraph::new(format!("Error: {}", err))
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: true });
            frame.render_widget(p, inner_area);
        }
    }
}
pub fn manifest_audio(url: String, tx: mpsc::Sender<DownloadEvent>) {
    // Determine where the music goes
    let music_dir = match Library::music_dir() {
        Ok(p) => p,
        Err(err) => {
            let _ = tx.send(DownloadEvent::Error(err.to_string()));
            return;
        }
    };

    // Configure for high-quality audio only
    let options = rusty_ytdl::VideoOptions {
        quality: rusty_ytdl::VideoQuality::HighestAudio,
        filter: rusty_ytdl::VideoSearchOptions::Audio,
        ..Default::default()
    };

    // Initialize the blocking downloader
    let video = match rusty_ytdl::blocking::Video::new_with_options(&url, options) {
        Ok(v) => v,
        Err(e) => {
            let _ = tx.send(DownloadEvent::Error(e.to_string()));
            return;
        }
    };

    // Fetch video title and signal start to UI
    let title = match video.get_info() {
        Ok(i) => {
            let t = i.video_details.title.clone();
            let _ = tx.send(DownloadEvent::Downloading {
                title: t.clone(),
                progress: 0.0,
            });
            t
        }
        Err(err) => {
            let _ = tx.send(DownloadEvent::Error(err.to_string()));
            return;
        }
    };

    // Sanitize title and build final filesystem path
    let safe_title = title.replace(['/', '\\', '?'], "_");
    let mut file_path = music_dir.join(&safe_title);
    file_path.set_extension("mp3");

    // Execute blocking download in background thread
    match video.download(&file_path) {
        Ok(_) => {
            let _ = tx.send(DownloadEvent::Finished {
                title,
                path_buf: Some(file_path),
            });
        }
        Err(err) => {
            let _ = tx.send(DownloadEvent::Error(format!("Download failed: {}", err)));
        }
    }
}
