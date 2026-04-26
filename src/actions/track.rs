use std::process::Command;

// Use the Command tool to call "yt-dlp" (Until the rust crate is fixed..... )
// Rust "yt-dlp" is having an error so a temporary fix..... lol
// Need to have installed "yt-dlp"
// Run this command........... "sudo pacman -S yt-dlp ffmpeg"
pub fn download_audio(url: &str) {
    let output = Command::new("yt-dlp")
        .args([
            "-x",
            "--audio-format", "mp3",
            "--audio-quality", "0",
            "-o", "/home/aeon/Music/%(title)s.%(ext)s",
            url
        ])
        .output(); // .spawn() is better for async, .output() blocks

    match output {
        Ok(o) if o.status.success() => println!("Success! Check ~/Music"),
        _ => eprintln!("Binary download failed. Is yt-dlp installed?"),
    }
}