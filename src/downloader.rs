use crate::library::Library;
use crate::modal::{TextInput, render_text_input, centered_rect};
use crossterm::event::KeyCode;
use ratatui::Frame;
use rusty_ytdl::Video;
use std::process::Stdio;
use ratatui::widgets::Paragraph;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Initializing,
    Downloading { title: String, progress: f64 },
    Finished { title: String },
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

    pub fn handle_key(&mut self, code: KeyCode) -> DownloaderOutcome {
        match code {
            KeyCode::Esc => DownloaderOutcome::Dismissed,
            KeyCode::Backspace => {
                self.url.backspace();
                DownloaderOutcome::Continue
            }
            KeyCode::Enter => {
                DownloaderOutcome::StartDownload(self.url.value.clone())
            }
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
    StartDownload(String), // The signal to start the worker thread
    Dismissed,
}

pub fn render_downloader(frame: &mut Frame, download_handler: &DownloadHandler) {
    let rect = centered_rect(52, 7, frame.area());
    match download_handler.event {
        DownloadEvent::Initializing => {
            render_text_input(
                frame,
                "Download Handler",
                &download_handler.url,
                "Enter YouTube Url: ",
            );
        }
        _ => {
            let paragraph = Paragraph::default().centered();
            frame.render_widget(paragraph, rect);
        }
    }
}

pub async fn spawn_manifest_audio_thread(url: String, tx: UnboundedSender<DownloadEvent>) {
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
            let _ = tx.send(DownloadEvent::Finished { title: video_title });
        }
        _ => {
            let _ = tx.send(DownloadEvent::Error(
                "yt-dlp finished with errors".to_string(),
            ));
        }
    }
}
