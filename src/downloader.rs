use crate::library::Library;
use crate::modal::{TextInput, centered_rect, render_text_input};
use crate::ui::Colors;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::prelude::Style;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Gauge, Paragraph, Wrap};
use rusty_ytdl::Video;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

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
    pub rx: Option<UnboundedReceiver<DownloadEvent>>,
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

    pub fn init_event(&mut self, rx: UnboundedReceiver<DownloadEvent>) {
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

pub async fn manifest_audio(url: String, tx: UnboundedSender<DownloadEvent>) {
    // Fetch metadata - Send error to UI if YouTube link is dead
    let video = match Video::new(url.clone()) {
        Ok(v) => v,
        Err(e) => {
            let _ = tx.send(DownloadEvent::Error(format!("Video init failed: {}", e)));
            return;
        }
    };

    let info = match video.get_info().await {
        Ok(i) => i,
        Err(e) => {
            let _ = tx.send(DownloadEvent::Error(format!(
                "Metadata fetch failed: {}",
                e
            )));
            return;
        }
    };

    let video_title = info.video_details.title;

    // Initial state: Title found, progress 0
    let _ = tx.send(DownloadEvent::Downloading {
        title: video_title.clone(),
        progress: 0.0,
    });

    // 2. Setup File Path
    let mut output_template = Library::music_dir().expect("Music directory not found");
    output_template.push(&video_title);
    output_template.set_extension("mp3");

    // 3. Spawn yt-dlp
    let mut child = match Command::new("yt-dlp")
        .args([
            "-x",
            "--audio-format",
            "mp3",
            "--newline",
            "-o",
            output_template.to_str().unwrap(),
            &url,
        ])
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(DownloadEvent::Error(format!("yt-dlp start failed: {}", e)));
            return;
        }
    };

    let stdout = child.stdout.take().expect("Stdout capture failed");
    let mut reader = BufReader::new(stdout).lines();

    // 4. Parse Progress
    while let Ok(Some(line)) = reader.next_line().await {
        if line.contains('%') {
            if let Some(pct_str) = line.split_whitespace().find(|s| s.contains('%')) {
                if let Ok(p) = pct_str.replace('%', "").parse::<f64>() {
                    let _ = tx.send(DownloadEvent::Downloading {
                        title: video_title.clone(),
                        progress: p,
                    });
                }
            }
        }
    }

    // 5. Finalize
    match child.wait().await {
        Ok(status) if status.success() => {
            let _ = tx.send(DownloadEvent::Finished {
                title: video_title,
                path_buf: Some(output_template),
            });
        }
        _ => {
            let _ = tx.send(DownloadEvent::Error(
                "yt-dlp finished with errors".to_string(),
            ));
        }
    }
}
