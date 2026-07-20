<div align="center">

<pre>
                          mm     ##                       
                          ##     ""                       
 m#####m  ##    ##   m###m##   ####     ##    ##  ####m##m
 " mmm##  ##    ##  ##"  "##     ##     ##    ##  ## ## ##
m##"""##  ##    ##  ##    ##     ##     ##    ##  ## ## ##
##mmm###  ##mmm###  "##mm###  mmm##mmm  ##mmm###  ## ## ##
 """" ""   """" ""    """ ""  """"""""   """" ""  "" "" ""
</pre>

**A terminal music app.**

[![Website](https://img.shields.io/badge/website-takashialpha.com%2Faudium-64b4ff?style=flat-square&labelColor=161616)](https://takashialpha.com/audium)
[![crates.io](https://img.shields.io/crates/v/audium?style=flat-square&color=64b4ff&labelColor=161616)](https://crates.io/crates/audium)
[![AUR](https://img.shields.io/aur/version/audium?style=flat-square&color=64b4ff&labelColor=161616)](https://aur.archlinux.org/packages/audium)
[![License](https://img.shields.io/crates/l/audium?style=flat-square&color=64b4ff&labelColor=161616)](LICENSE)
[![Built With Ratatui](https://img.shields.io/badge/Built_With_Ratatui-000?logo=ratatui&logoColor=fff)](https://ratatui.rs/)

[Installation](#installation) · [Building](#building-from-source)

</div>

<!--
  Demo video - see issue #6 for context on why this is set up the way it is.

  GitHub strips autoplay/loop from <video> tags unless the src points to a
  GitHub-hosted asset (uploaded via the issue/PR editor). A repo-relative path
  like ./audium-demo.mp4 renders as a plain download link on GitHub.com and
  does not play inline at all. Animated WebP is not rendered by GitHub either.
  GIF works everywhere but tends to be larger for equivalent quality.

  Solution: the src below points to a GitHub CDN asset (autoplay + loop on
  GitHub.com). audium-demo.mp4 is kept in the repo root as a fallback for
  local/offline viewers who clone the repo.
-->
<video src="https://github.com/user-attachments/assets/38cd89c3-42d6-4133-8feb-08bd45095649" autoplay loop muted playsinline></video>

## Features

- **Keyboard-driven:** built to be driven entirely from the keyboard, for people who live in the terminal and never reach for the mouse. Press `?` in-app to see every keybinding.
- **Library & metadata:** import through the built-in file picker; artist, album, year and genre are read from tags automatically and editable in-app.
- **Lyrics:** store plain text or LRC synced lyrics per track. An overlay auto-scrolls synced lyrics to the current line, with a built-in editor.
- **It's your library:** your tracks are stored as plain JSON at `$XDG_DATA_HOME/audium/library.json` (typically `~/.local/share/audium/library.json`). Edit it by hand, back it up, move it anywhere. audium doesn't rename your files and never phones home.
- **Themes:** 15 built-in truecolor themes (nord, gruvbox, catppuccin, rosé pine, dracula, tokyo night, and more). Switch live with instant preview. Optional background transparency for composited terminals.
- **Adapts to your terminal:** detects truecolor support and, on a bare Linux console (tty) or any terminal without it, automatically falls back to a crisp 16-color theme with ASCII-only glyphs so the UI stays readable everywhere. The settings menu shows what it detected and lets you force the color mode if the guess is wrong.
- **Playlists & queue:** create, rename and delete playlists, shuffle them into the queue, and pick a loop mode.
- **Playback control:** filter the tracklist in real time, adjust playback speed and seek freely.
- **Threaded audio:** playback runs on its own thread; the UI never stutters your music.
- **System audio output:** audium plays through your default system output. Change the output device in your OS and audium follows, no in-app device switching, no surprises.
- **Format agnostic:** MP3, FLAC, OGG, WAV, AAC, M4A, Opus, AIFF and more via [Symphonia](https://github.com/pdeljanov/Symphonia). No FFmpeg required.
- **Tiny binary:** ~3 MB stripped release build.
- **100% safe Rust:** zero `unsafe` blocks in the codebase.

---

## Installation

### Cargo

```sh
cargo install audium
```

Requires a Rust toolchain with edition 2024 support. Installs the `audium` binary to `~/.cargo/bin/`.

audium uses ALSA for audio, the standard on Linux, its development headers are needed to build, see [Building from source](#building-from-source) for distro-specific instructions.

### AUR (Arch Linux)

```sh
paru -S audium
# or: yay -S audium
```

---

## Usage

```sh
# Launch with your library
audium

# Open a specific file immediately (imports it to your library)
audium path/to/song.flac
```

audium stores your library at `$XDG_DATA_HOME/audium/library.json` and your music at `$XDG_DATA_HOME/audium/music/` (typically under `~/.local/share/audium/`).

---

## Building from source

```sh
git clone https://github.com/takashialpha/audium
cd audium
cargo build --release
# binary is at ./target/release/audium
```

Uses ALSA, the standard Linux audio API. Install its development headers:

```sh
# Debian / Ubuntu
sudo apt install alsa-base alsa-utils libasound2-dev

# Arch
sudo pacman -S alsa-utils alsa-lib

# Fedora
sudo dnf install alsa-utils alsa-lib-devel
```

> **Linux only:** audium targets Linux exclusively; other platforms are not supported.

---

## Library layout

```
$XDG_DATA_HOME/audium/   # typically ~/.local/share/audium/
├── library.json   # track registry + playlists
├── settings.json  # user preferences (volume, seek step, theme, transparency, color mode)
└── music/         # copies of all imported audio files
```

`library.json` is human-readable and editable by hand. audium re-validates it on next launch, so feel free to reorganise playlists, fix track names, or move the file to another machine.

---

## Why audium?

Alternatives like termusic and cmus are solid, but they come with tradeoffs: heavy dependency trees, FFmpeg requirements, daemon processes, or configuration formats that take longer to learn than the app itself. audium is different in a few concrete ways:

- **No FFmpeg, no daemon:** one binary, zero background processes.
- **Smaller and faster to build:** fewer dependencies means shorter compile times and a ~3 MB release binary.
- **Cleaner UI:** built on ratatui with a layout designed for actual daily use, not just feature completeness.
- **More modern codebase:** written in current Rust with edition 2024, Symphonia for decoding, and rodio for playback.
- **Plain JSON library:** your data is always readable, portable, and yours.

## TODO

- YouTube audio import (no external binary deps)

## Contributing

Issues and pull requests are welcome.
Please open an issue before starting work on a large change.
