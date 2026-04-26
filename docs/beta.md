# Audium Beta: Feature Roadmap & Workarounds

This document tracks experimental features, temporary technical debt, and setup instructions for the current development build of **Audium**.

---

## YouTube Audio Integration

### Current Status: System Binary Wrapper
Due to a **yanked dependency** (`lofty v0.23.2`) in the primary Rust `yt-dlp` crate ecosystem, we have implemented a pragmatic workaround using `std::process::Command`.

This ensures that development on the TUI and Library logic can continue while the upstream Rust ecosystem stabilizes.

---

## Required Setup

To use the download feature, you must have the following dependencies installed on your system.

### **For Arch Linux Users:**
```bash
sudo pacman -S yt-dlp ffmpeg
```

### **Verification**

Check your installation by running:

```bash
yt-dlp --version
ffmpeg -version
```

This setup will be used until  yt-dlp fix the bug or I find a way around.....

---

## Implementation Logic

The application currently calls the system binary with optimized flags for high-fidelity audio extraction:

| Flag | Description                                                       |
|------|-------------------------------------------------------------------|
| `-x` | Extract Audio: Skips video stream downloading.                    |
| `--audio-format mp3` | Transcode: Converts raw DASH streams to MP3 for compatibility.    |
| `--audio-quality 0` | High Fidelity: Forces the highest VBR (Variable Bitrate).         |
| `-o` | Output Template: Saves to `/home/(user)/Music/%(title)s.%(ext)s`. |

For now these are just not so fine I would say things to do but it will be fixed asap.........