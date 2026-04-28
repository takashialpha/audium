use std::fmt::format;
use std::process::Command;
use rusty_ytdl::Video;
use crate::library::Library;

pub async fn download_audio_with_binary(url: &str) -> Result<(), String> {
    let video = Video::new(url).expect("Error creating video instance");
    let video_title = video.get_info().await.expect("Error getting video title").video_details.title;
    let mut output_template = Library::music_dir()
        .expect("music_dir should always be present in data directory");
    output_template.push(video_title);
    output_template.set_extension("mp3");

    // --- Build the command USING: Command TO access "yt-dlp" Needs to be installed --
    // RUN: sudo pacman -S yt-dlp ffmpeg -- to isntall these 2 tools.........
    let mut child = Command::new("yt-dlp")
        .args([
            "-x", // "-x" to Extract audio
            "--audio-format", "mp3",
            "--audio-quality", "0",      // 0 is the best (VBR)
            "-o", output_template.to_str().unwrap(),
            url,
        ])
        .spawn()                         // .spawn() runs it in the background!
        .map_err(|e| format!("Failed to start yt-dlp: {}", e))?;

    // 3. Wait for it to finish (or just let it run if you want async)
    let status = child.wait()
        .map_err(|e| format!("Error waiting for yt-dlp: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err("yt-dlp exited with an error. Check the URL or your internet.".to_string())
    }
}