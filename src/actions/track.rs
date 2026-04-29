use tokio::process::Command;
use std::process::Stdio;
use tokio::io::{BufReader, AsyncBufReadExt};
use tokio::sync::mpsc::UnboundedSender;
use rusty_ytdl::Video;
use crate::library::Library;
use crate::modal::DownloadEvent;

pub async fn download_audio_with_binary(
    url: &str,
    tx: UnboundedSender<DownloadEvent>
) {
    // Fetch video metadata to update UI with the song title
    let video = Video::new(url).map_err(|e| e.to_string())?;
    let info = video.get_info().await.map_err(|e| e.to_string())?;
    let video_title = info.video_details.title;

    let _ = tx.send(DownloadEvent::Title(video_title.clone()));

    // Construct the file path in the local music directory
    let mut output_template = Library::music_dir().expect("Music directory not found");
    output_template.push(&video_title);
    output_template.set_extension("mp3");

    // Spawn yt-dlp as an async child process
    let mut child = Command::new("yt-dlp")
        .args([
            "-x",                       // Extract audio only
            "--audio-format", "mp3",    // Convert to mp3
            "--newline",                // Force progress updates on new lines
            "-o", output_template.to_str().unwrap(),
            url,
        ])
        .stdout(Stdio::piped())         // Capture output to parse progress
        .spawn()
        .map_err(|e| format!("Process start failed: {}", e))?;

    let stdout = child.stdout.take().ok_or("Stdout capture failed")?;
    let mut reader = BufReader::new(stdout).lines();

    // Line-by-line parser for yt-dlp terminal output
    while let Ok(Some(line)) = reader.next_line().await {
        if line.contains('%') {
            // Locate the percentage string in the output line
            if let Some(pct_str) = line.split_whitespace().find(|s| s.contains('%')) {
                if let Ok(p) = pct_str.replace('%', "").parse::<f64>() {
                    let _ = tx.send(DownloadEvent::Progress(p));
                }
            }
        }
    }

    // Await process completion without blocking the TUI event loop
    let status = child.wait().await.map_err(|e| e.to_string())?;
}