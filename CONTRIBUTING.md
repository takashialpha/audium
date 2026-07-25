# Contributing to audium

Issues and pull requests are welcome. Please open an issue before starting work
on a large change.

## Before you push

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

These are what CI runs, so a green local run is a green CI run.

The lint set lives in `Cargo.toml` rather than in CI flags, so a plain
`cargo build` fails the same way the pipeline does. It is strict on purpose:
warnings denied, `unsafe_code` forbidden, and clippy with `pedantic`, `nursery`
and a hand-picked selection of `restriction`. Do not silence a lint to land a
change. An `#[allow]` needs a comment saying why the lint is wrong *here*, and
changing the lint set belongs in its own PR rather than inside a feature.

## Text

`src/` is ASCII, including comments. The one exception is the table of glyphs
audium draws on screen; every entry there has an ASCII counterpart for
terminals that cannot render it.

Documentation is ASCII too. `LICENSE` is the sole exception: it reproduces the
GPL-3.0-or-later text verbatim, typographic quotes included, and must not be
retyped.

Nowhere in the repository uses an en or em dash. Write `--`, or reach for a
colon or parentheses instead.

Comments explain *why*, not *what*: the reason a reader would otherwise have to
reconstruct, or the alternative that was tried and rejected. Every module opens
with a `//!` header and every public item carries a `///`. A comment that
restates the code is worse than none, because it is one edit away from lying.

## Rust

**Nothing panics.** `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!` and
`unimplemented!` are denied: releases abort on panic, and a panic behind a TUI
leaves the terminal in raw mode with the message scrolled off screen. Fallible
work returns `anyhow::Result` with `.with_context(|| ...)` naming the operation
that failed. Everything else uses the total API (`unwrap_or`, `map_or`, `get`,
`saturating_*`, `min`).

**Bad input is a fallback, not an error.** Tags, config and hand-edited files
come from users and are often wrong. An unrecognised value falls back to a
default and the app keeps going.

**Never print.** The TUI owns the terminal, so `println!`, `eprintln!` and
`dbg!` corrupt the frame and are denied. What the user must see goes in a
dialog.

**Casts are audited in one place.** Lossy or truncating conversions live in a
single module, each documenting why it is sound for the inputs it is given.
Scattering `#[allow(clippy::cast_*)]` across call sites turns an audited
decision into an unreviewed one.

**Arithmetic on sizes saturates.** Terminal dimensions are user-controlled: a
window dragged down to three columns must render badly, not underflow.

**Take the narrow type.** Prefer `&str` and `&[T]` in parameters, return owned
values only where ownership really transfers, and derive traits rather than
hand-writing them. Clone when it is genuinely simpler, not to escape a borrow
that wanted restructuring.

**Match our own enums exhaustively.** A catch-all `_ =>` arm on a type defined
in this crate silently swallows the next variant someone adds; list them and
let the compiler point at every site that needs updating.

**Name the constant.** A literal that appears twice, or that a reader would
have to measure to understand, is a `const` with a doc comment.

**Keep logic out of I/O and rendering.** Functions that take data and return
data are the only part of a TUI that is testable at all, so push decisions into
them and keep the edges thin.

**One hasher project-wide.** Use `rustc_hash::FxHashMap` / `FxHashSet`, never
the `std::collections` equivalents. Every key here is small and locally
generated, so SipHash's DoS resistance buys nothing and costs speed, and a
single choice means nobody has to wonder why two exist.

## Data on disk

**The tags are the record; the index is a cache.** Metadata lives in the audio
files' own tags, so a library survives audium being deleted and opens in any
other player. The index only records which files exist and what the playlists
hold: never add a field to it that could have been a tag.

**Identity is the filename.** That is what makes the index hand-editable and
portable between machines. Nothing may key off an absolute path or an inode.

**Every write is atomic:** write a temporary file, then rename it over the
target. Truncate-then-write loses the file outright if the process dies
mid-write. Name scratch files so a leftover one cannot be picked up as real
content.

**Nothing is destroyed to make room for a new format.** Files audium writes
carry a `version`. One it cannot read is renamed aside and rebuilt from
scratch, never migrated in place and never deleted. Serde defaults on every
field let an older file load with the new keys filled in and the old ones
untouched.

## Layout

A module with children is `foo.rs` alongside a `foo/` directory; there are no
`mod.rs` files, and clippy enforces it. Shared behaviour gets a small module
rather than a copy: list navigation, numeric casts and the modal helpers are
each defined once, which is why the same keys work in every list and every
dialog has the same margins. A new list or dialog goes through them too.

## UI conventions

**Never hardcode a color or a glyph.** Colors come off the active theme, and
drawing characters off its glyph table, which yields the ASCII set on terminals
that cannot do better. A literal RGB value or box-drawing character looks fine
on the machine it was written on and breaks silently on a tty.

**Modal spacing is centralised, never hand-rolled.** One helper insets every
dialog by a fixed padding on all four sides. A renderer lays out *content
only*: no leading blank line, no trailing `Constraint::Min(0)` standing in for
a bottom margin, no `format!("  {label}")` gutters. This is what keeps the gap
between the border and the first line of text identical in every popup.

It follows that a dialog is sized from its content plus the shared chrome
constant, never from a hardcoded height, which drifts into a lopsided gap the
moment a row is added or removed. Blank rows *between* content groups are still
the renderer's business: the rule governs margins, not internal separators, and
a gap row must be guaranteed rather than left over. Use `Min(1)` when the
content above it is fixed-height and `Length(1)` when it is the flexible
region; `Min(0)` promises nothing and collapses to zero the moment the dialog
is sized correctly.

**Key hints go at the bottom, as `[key] action` pairs**, built and rendered
with the shared helpers rather than hand-written strings. The brackets are what
make the pairing legible: spacing alone cannot distinguish a gap *inside* a
pair from a gap *between* two, and color does not survive a monochrome tty. Ask
the helper for the footer's height *before* sizing the dialog, since it wraps
at whole pairs and can take more than one row.

Both rules cover *every* overlay, not just the ones in `modal.rs`.

**A new keybinding is documented in the commit that adds it.** The help dialog
is the only place bindings are advertised, so one missing from it does not
exist as far as users are concerned.

## Tests

There is deliberately no test suite, and a PR is not expected to add one. Most
of what a test would assert here is already enforced by the compiler and the
lint set, and a test that needs scaffolding is a second thing to maintain that
breaks for reasons of its own.

A test earns its place only by proving something genuinely worth proving, and
only if it is as static as the code it covers: a pure function in, a value out.
No fixtures, no temp directories, no terminal, no timing, no dev-dependency. If
a bug comes down to a small assertion over a pure function, put it in a
`#[cfg(test)]` module in the same file. Otherwise leave it out.

## Commits and pull requests

Keep commits focused: a formatting sweep, a lint change and a behaviour change
are three commits, not one. Say what a user would notice, and name the terminal
you tested in. If it touches rendering, check a bare Linux console too: that is
the path the ASCII glyphs and console themes exist for, and the one most easily
broken without noticing.
