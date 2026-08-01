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
[![License](https://img.shields.io/crates/l/audium?style=flat-square&color=2E8B57&label=license&labelColor=161616&v=2)](LICENSE)
[![Built With Ratatui](https://img.shields.io/badge/Built_With_Ratatui-000?logo=ratatui&logoColor=fff)](https://ratatui.rs/)

[Features](#features) | [Installation](#installation) | [Usage](#usage) | [Building](#building-from-source) | [Library layout](#library-layout) | [Why audium?](#why-audium) | [TODO](#todo) | [Contributing](#contributing)

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
<video src="https://github.com/user-attachments/assets/47378664-1c18-427f-8b80-ec59f9238b11" autoplay loop muted playsinline></video>

## Features

- **Keyboard-driven:** built to be driven entirely from the keyboard, for people who live in the terminal and never reach for the mouse. Press `?` in-app for grouped help covering every keybinding.
- **Library & metadata:** import through the built-in file picker; title, artist, album and track length are read from the files' own tags, and edits are written straight back to them.
- **Lyrics:** store plain text or LRC synced lyrics per track. An overlay auto-scrolls synced lyrics to the current line, with a built-in editor.
- **It's your library:** all metadata lives in the files' own tags, edits included, so it travels with them and any other player can read it. `$XDG_DATA_HOME/audium/audium.json` is a small plain-JSON index that only records which files exist and your playlists; delete it and audium rebuilds it from the tags. audium never phones home.
- **Themes:** 15 built-in truecolor themes plus 2 console themes (nord, gruvbox, catppuccin, rose pine, dracula, tokyo night, and more). Switch live with instant preview. Optional background transparency for composited terminals.
- **Adapts to your terminal:** detects truecolor support and, on a bare Linux console (tty) or any terminal without it, automatically falls back to a 16-color theme with ASCII-only glyphs so the UI stays readable everywhere. Two console themes are built from named ANSI colors, one for a dark background and one for a light one, and each color mode remembers its own theme. The settings menu shows what it detected and lets you override it if the guess is wrong.
- **Playlists:** your whole collection and your playlists are separate things, in their own panels. Create, rename and delete playlists, queue or shuffle either one, and pick a loop mode.
- **Playback control:** tracks are listed as a table of title, artist, album and length; filter it in real time, adjust playback speed and seek freely.
- **Resumes where you left off:** the queue, current track, position and playback modes are restored on the next launch, paused, so nothing plays until you press Space.
- **Threaded audio:** playback runs on its own thread; the UI never stutters your music.
- **System audio output:** audium plays through your default system output. Change the output device in your OS and audium follows, no in-app device switching, no surprises.
- **Wide format support:** MP1, MP2, MP3, FLAC, Ogg Vorbis, WAV, AAC, M4A, M4B and AIFF via [Symphonia](https://github.com/pdeljanov/Symphonia). No FFmpeg required.
- **Tiny binary:** ~5 MB stripped release build.
- **100% safe Rust:** zero `unsafe` blocks in the codebase; it's forbidden.

---

## Installation

> **Linux only:** Audium is developed and tested exclusively on Linux. It may work on macOS or other POSIX-compatible systems, but support for those platforms is not provided.

### Cargo

```sh
cargo install audium --locked
```

Installs the `audium` binary to `~/.cargo/bin/`. Build with the latest stable Rust: audium tracks the stable channel rather than supporting a minimum version, and uses new language features as they land. If cargo reports your toolchain is too old, run `rustup update stable`.

Audio goes through ALSA, the standard on Linux, and its development headers are needed to build. See [Building from source](#building-from-source) for the package names on your distro.

### AUR (Arch Linux)

```sh
paru -S audium
# or: yay -S audium
```

---

## Usage

```sh
# Launch audium
audium

# Open a specific file immediately (imports it to your library, plays it)
audium path/to/song.flac
```

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

---

## Library layout

```
$XDG_DATA_HOME/audium/     # typically ~/.local/share/audium/
  audium.json              # index: track filenames and playlists
  music/                   # copies of all imported audio files

$XDG_CONFIG_HOME/audium/   # typically ~/.config/audium/
  settings.json            # user preferences (volume, seek step, theme, transparency, color mode, resume)
                           # reads from $XDG_CONFIG_DIRS too, so the system can ship defaults

$XDG_STATE_HOME/audium/    # typically ~/.local/state/audium/
  state.json               # last session: queue, position, loop/speed/volume (safe to delete)
```

`music/` holds audium's own copies of your imported files, so importing never moves or renames your originals and removing a track deletes only audium's copy. Track metadata is read from the files' tags on every launch.

The index lists tracks and playlist entries by filename, so it is genuinely hand-editable (add a song to a playlist by typing its filename) and identical across machines. audium re-validates it on the next launch, so feel free to reorganise playlists in a text editor.

`audium.json` carries a `version` field, and audium **never migrates an index it cannot read**. Anything unrecognised is renamed to `audium.v<n>.json` and left in place; the collection is then rebuilt by re-scanning `music/`. Nothing is ever deleted.

Every file audium writes, index and tags alike, is written to a temporary file and renamed into place, so a crash or a power cut leaves the previous version intact rather than a half-written one.

### Upgrading

The same procedure applies to every upgrade, from any version, and to downgrades:

1. Make sure your audio files are in `$XDG_DATA_HOME/audium/music/`. If you are coming from a version that stored them elsewhere (very early releases used `~/.audium/music/`), copy them there first.
2. Start audium. Every file in `music/` is re-imported, with its name and metadata read back from the file's own tags.
3. Recreate your playlists.

Playlists are the only thing lost, because they are the only thing that exists solely in the index. Titles, artists, albums and lyrics all live in the files' own tags, that includes the ones you edit inside audium, so they come back with the tracks. Your old index file is left on disk untouched: open it to see what your playlists held, then delete it once you're done.

Preferences are not carried over either: `settings.json` moved from the data directory to `$XDG_CONFIG_HOME/audium/`; it's recommended to set them inside audium.

Old files audium no longer reads, safe to remove by hand: `$XDG_DATA_HOME/audium/settings.json`, `library.json`, `~/.audium/`, and any `audium.v<n>.json`.

---

## Why audium?

Alternatives like termusic and cmus are solid, but they come with tradeoffs: heavy dependency trees, FFmpeg requirements, daemon processes, or configuration formats that take longer to learn than the app itself. audium is different in a few concrete ways:

- **No FFmpeg, no daemon:** one binary, zero background processes.
- **Smaller and faster to build:** fewer dependencies means shorter compile times and a ~5 MB release binary.
- **Cleaner UI:** built on ratatui with a layout designed for actual daily use, not just feature completeness.
- **More modern codebase:** written in current Rust with edition 2024, Symphonia for decoding, and rodio for playback.
- **Plain JSON library:** your data is always readable, portable, and yours.

## TODO

- MPRIS, so desktop media keys and status bars can see and drive playback
- Safe concurrent instances (state; the index is last-writer-wins; needs a lock or reload-before-write)
- Pitch-preserving speed control (time-stretch DSP), so faster playback does not raise the pitch
- Enhance lyric modal
- YouTube/Web audio import (no external binary deps)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).
