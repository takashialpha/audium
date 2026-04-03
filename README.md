<div align="center">

```
 █████╗ ██╗   ██╗██████╗ ██╗██╗   ██╗███╗   ███╗
██╔══██╗██║   ██║██╔══██╗██║██║   ██║████╗ ████║
███████║██║   ██║██║  ██║██║██║   ██║██╔████╔██║
██╔══██║██║   ██║██║  ██║██║██║   ██║██║╚██╔╝██║
██║  ██║╚██████╔╝██████╔╝██║╚██████╔╝██║ ╚═╝ ██║
╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚═╝ ╚═════╝ ╚═╝     ╚═╝
```

**A terminal music player.**

[![crates.io](https://img.shields.io/crates/v/audium?style=flat-square&color=64b4ff&labelColor=161616)](https://crates.io/crates/audium)
[![AUR](https://img.shields.io/aur/version/audium?style=flat-square&color=64b4ff&labelColor=161616)](https://aur.archlinux.org/packages/audium)
[![License](https://img.shields.io/crates/l/audium?style=flat-square&color=64b4ff&labelColor=161616)](LICENSE)

[Website](https://takashialpha.github.io/audium) · [Installation](#installation) · [Keybindings](#keybindings) · [Building](#building-from-source)

</div>

---

```
 audium  —  terminal music player      [?] help  [f] file picker  [c] playlist  [q] quit
┌─────────────────────────┬──────────────────────────────────────────────────────┐
│ Playlists               │ All Tracks                                           │
│                         │  #    Title                                          │
│ > All Tracks (4)        │     1  04 - Neon Arpeggio                            │
│   Late Night (2)        │     2  Drift                                         │
│   Focus (2)             │  >  3  Weightless (feat. Macarena)                   │
│                         │     4  The River Calls                               │
│                         │                                                      │
│                         ├──────────────────────────────────────────────────────┤
│                         │ Queue                                                │
│                         │     1  04 - Neon Arpeggio                            │
│                         │     2  Drift                                         │
│                         │  >  3  Weightless (feat. Macarena)                   │
└─────────────────────────┴──────────────────────────────────────────────────────┘
⏸  Weightless (feat. Macarena)
████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  2:14 / 5:02
```

---

## Features

- **Keyboard-driven** — every action is one key. Mouse optional.
- **Playlists** — create, rename, delete. *All Tracks* is always there.
- **Built-in file picker** — import audio files without leaving the app.
- **Threaded audio** — playback on its own thread; UI interaction never stutters playback.
- **Persistent library** — stored at `~/.audium/library.json`. Plain JSON, easy to back up.
- **Format agnostic** — MP3, FLAC, OGG, WAV, AAC, M4A, Opus, AIFF and more via [Symphonia](https://github.com/pdeljanov/Symphonia). No FFmpeg required.
- **Tiny binary** — `~3 MB` stripped release build, no runtime dependencies on macOS.

---

## Installation

### Cargo

```sh
cargo install audium
```
On Linux, Requires ALSA development headers to build and ALSA to run.
Requires Rust 1.85+ (MSRV). Installs the `audium` binary to `~/.cargo/bin/`.

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

audium stores your library at `~/.audium/library.json` and your music at `~/.audium/music/`.

---

## Keybindings

### Global

| Key          | Action                   |
|--------------|--------------------------|
| `q`          | Quit                     |
| `Tab`        | Cycle panel focus        |
| `?`          | Toggle help overlay      |

### Playback

| Key          | Action                   |
|--------------|--------------------------|
| `Space`      | Play / Pause             |
| `n`          | Next track               |
| `N`          | Previous track           |
| `←` / `→`   | Seek backward / forward  |
| `+` / `=`   | Volume up                |
| `-`          | Volume down              |

### Navigation

| Key          | Action                   |
|--------------|--------------------------|
| `j` / `↓`   | Move cursor down         |
| `k` / `↑`   | Move cursor up           |
| `Enter`      | Play selected track      |

### Library & Queue

| Key  | Action                              |
|------|-------------------------------------|
| `f`  | Open file picker                    |
| `a`  | Add selected track to queue         |
| `p`  | Add selected track to a playlist    |
| `c`  | Create new playlist                 |
| `z`  | Shuffle playlist into queue         |
| `d`  | Remove selected item                |
| `r`  | Rename selected track or playlist   |
| `x`  | Remove selected item from queue     |
| `s`  | Open settings                       |

---

## Building from source

```sh
git clone https://github.com/takashialpha/audium
cd audium
cargo build --release
# binary is at ./target/release/audium
```

**Linux** requires ALSA development headers:

```sh
# Debian / Ubuntu
sudo apt install libasound2-dev

# Arch
sudo pacman -S alsa-lib

# Fedora
sudo dnf install alsa-lib-devel
```

**macOS** has no extra dependencies.

> **Windows:** audium compiles on Windows but is not an officially supported platform and has not been tested.

---

## Library layout

```
~/.audium/
├── library.json   # track registry + playlists
└── music/         # copies of all imported audio files
```

`library.json` is human-readable. You can edit it directly if needed, though audium will re-validate it on next launch.

---

## Contributing

Issues and pull requests are welcome.
Please open an issue before starting work on a large change.

---

## License

[Apache-2.0](LICENSE) © takashialpha
