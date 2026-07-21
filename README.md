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
- **It's your library:** your tracks are stored as plain JSON at `$XDG_DATA_HOME/audium/audium.json` (typically `~/.local/share/audium/audium.json`). Edit it by hand, back it up, move it anywhere. audium doesn't rename your files and never phones home.
- **Themes:** 15 built-in truecolor themes (nord, gruvbox, catppuccin, rosé pine, dracula, tokyo night, and more). Switch live with instant preview. Optional background transparency for composited terminals.
- **Adapts to your terminal:** detects truecolor support and, on a bare Linux console (tty) or any terminal without it, automatically falls back to a crisp 16-color theme with ASCII-only glyphs so the UI stays readable everywhere. The settings menu shows what it detected and lets you force the color mode if the guess is wrong.
- **Library and playlists:** your whole collection and your playlists are separate things, in their own panels. Create, rename and delete playlists, queue or shuffle either one, and pick a loop mode.
- **Playback control:** filter the tracklist in real time, adjust playback speed and seek freely.
- **Threaded audio:** playback runs on its own thread; the UI never stutters your music.
- **System audio output:** audium plays through your default system output. Change the output device in your OS and audium follows, no in-app device switching, no surprises.
- **Format agnostic:** MP3, MP2, FLAC, OGG/Vorbis, WAV, AAC, M4A and AIFF via [Symphonia](https://github.com/pdeljanov/Symphonia). No FFmpeg required.
- **Tiny binary:** ~4 MB stripped release build.
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
$XDG_DATA_HOME/audium/     # typically ~/.local/share/audium/
├── audium.json    # track registry + playlists
└── music/         # copies of all imported audio files

$XDG_CONFIG_HOME/audium/   # typically ~/.config/audium/
└── settings.json  # user preferences (volume, seek step, theme, transparency, color mode)
```

`audium.json` is human-readable and editable by hand. audium re-validates it on next launch, so feel free to reorganise playlists, fix track names, or move the file to another machine.

It carries a `version` field, and audium **never migrates an index it cannot read**. Anything unrecognised is renamed to `audium.v<n>.json` and left in place; the collection is then rebuilt by re-scanning `music/`. Nothing is ever deleted.

### Upgrading

The same procedure applies to every upgrade, from any version, and to downgrades:

1. Make sure your audio files are in `$XDG_DATA_HOME/audium/music/`. If you are coming from a version that stored them elsewhere (very early releases used `~/.audium/music/`), copy them there first.
2. Start audium. Every file in `music/` is re-imported, with its name and metadata read back from the file's own tags.
3. Recreate your playlists, and anything you had edited in-app.

What is lost is whatever lived *only* in the index: your playlists, plus any track name, metadata or lyrics you edited inside audium rather than in the file's tags. Your old index file is left on disk untouched: open it to see what it held, then delete it once you're done.

Preferences are not carried over either: `settings.json` moved from the data directory to `$XDG_CONFIG_HOME/audium/` in this release.

Old files audium no longer reads, safe to remove by hand: `$XDG_DATA_HOME/audium/settings.json`, `library.json`, `~/.audium/`, and any `audium.v<n>.json`.

2.0 is a clean break in every one of these: the index filename, its schema, where preferences live, and several keybindings. That is what the major version marks.

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

### Hashing

Use `rustc_hash::FxHashMap` / `FxHashSet` throughout; there are no
`std::collections` hash containers left in the tree. FxHash is materially
faster than std's SipHash for the small keys audium hashes (`TrackId` is a
`u64`), and every key is locally generated — track ids and paths under our own
music directory — so std's DoS resistance buys nothing here. Keeping one
hasher project-wide also means no one has to wonder why two exist.

### UI conventions

**Modal spacing is centralised, never hand-rolled.** `modal_block()` applies a
fixed inset — `MODAL_PAD_X` columns (2) and `MODAL_PAD_Y` rows (1) — on all four
sides of every dialog. A renderer lays out *content only*: no leading blank
line, no trailing `Constraint::Min(0)` standing in for a bottom margin, no
`format!("  {label}")` gutters. This is the rule that keeps the gap identical
between the border and the first line of text in every popup.

It follows that a modal's height is `content rows + MODAL_CHROME_H`, and its
usable width is `width - 2 - 2 * MODAL_PAD_X`. Size dialogs from their content
using that constant rather than a literal; a hardcoded height silently drifts
into a lopsided gap the moment a row is added or removed.

Blank rows *between* content groups are still the renderer's business — the
rule governs margins, not internal separators.

A gap row must be guaranteed, never left over. Use `Min(1)` when the content
above it is fixed-height and `Length(1)` when the content above it is the
flexible region; `Min(0)` promises nothing and silently collapses to zero the
moment the dialog is sized correctly from its content.

**Key hints go at the bottom, as `[key] action` pairs.** Build them with
`hint()` / `danger_hint()` and render with `render_hints()`; never hand-write
a hint string. The brackets are what make the pairing legible — spacing alone
cannot distinguish a gap *inside* a pair from a gap *between* two, and the
color difference vanishes on a monochrome tty. `danger_hint()` marks a key
that destroys something.

`hint_lines()` wraps at whole pairs, so a footer that outgrows its dialog gains
a row instead of being cut off mid-word. Ask `hint_height()` for the row count
*before* sizing the dialog and reserve that many rows at the bottom.

Only the keybindings dialog has no footer: the whole dialog is one.

Both rules cover *every* overlay, not just the modals in `modal.rs` — the file
picker and the lyrics overlay go through `modal_block()` and `render_hints()`
too. The file picker is the one dialog that overrides anything, left-aligning
its title because a long path reads better anchored to the left.
