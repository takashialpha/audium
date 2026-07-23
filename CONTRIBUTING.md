# Contributing to audium

Issues and pull requests are welcome. Please open an issue before starting work
on a large change.

Every change is expected to keep `cargo fmt --check`, `cargo clippy
--all-targets` and `cargo test` clean. The lint set in `Cargo.toml` is strict on
purpose: clippy runs with `pedantic`, `nursery` and a selection of `restriction`
lints, all denied.

## Text

`src/` is ASCII, including comments. The one exception is the `UNICODE_GLYPHS`
table, which *is* the text audium draws on screen; every entry there has an
`ASCII_GLYPHS` counterpart for terminals that cannot render it.

Documentation is ASCII too. `LICENSE` is the sole exception: it reproduces the
GPL-3.0 text verbatim, typographic quotes included, and must not be retyped.

Nowhere in the repository uses an en or em dash. Write `--`, or reach for a
colon or parentheses instead.

## Hashing

Use `rustc_hash::FxHashMap` / `FxHashSet` throughout; there are no
`std::collections` hash containers left in the tree. FxHash is materially
faster than std's SipHash for the small keys audium hashes (`TrackId` is a
`u64`), and every key is locally generated (track ids, and paths under our own
music directory), so std's DoS resistance buys nothing here. Keeping one
hasher project-wide also means no one has to wonder why two exist.

## UI conventions

**Modal spacing is centralised, never hand-rolled.** `modal_block()` applies a
fixed inset of `MODAL_PAD_X` columns (2) and `MODAL_PAD_Y` rows (1) on all four
sides of every dialog. A renderer lays out *content only*: no leading blank
line, no trailing `Constraint::Min(0)` standing in for a bottom margin, no
`format!("  {label}")` gutters. This is the rule that keeps the gap identical
between the border and the first line of text in every popup.

It follows that a modal's height is `content rows + MODAL_CHROME_H`, and its
usable width is `width - 2 - 2 * MODAL_PAD_X`. Size dialogs from their content
using that constant rather than a literal; a hardcoded height silently drifts
into a lopsided gap the moment a row is added or removed.

Blank rows *between* content groups are still the renderer's business: the
rule governs margins, not internal separators.

A gap row must be guaranteed, never left over. Use `Min(1)` when the content
above it is fixed-height and `Length(1)` when the content above it is the
flexible region; `Min(0)` promises nothing and silently collapses to zero the
moment the dialog is sized correctly from its content.

**Key hints go at the bottom, as `[key] action` pairs.** Build them with
`hint()` / `danger_hint()` and render with `render_hints()`; never hand-write
a hint string. The brackets are what make the pairing legible: spacing alone
cannot distinguish a gap *inside* a pair from a gap *between* two, and the
color difference vanishes on a monochrome tty. `danger_hint()` marks a key
that destroys something.

`hint_lines()` wraps at whole pairs, so a footer that outgrows its dialog gains
a row instead of being cut off mid-word. Ask `hint_height()` for the row count
*before* sizing the dialog and reserve that many rows at the bottom.

Only the help dialog departs from the rule: its own body is a list of keys, so its footer carries just the scroll and close hints.

Both rules cover *every* overlay, not just the modals in `modal.rs`. The file
picker and the lyrics overlay go through `modal_block()` and `render_hints()`
too. The file picker is the one dialog that overrides anything, left-aligning
its title because a long path reads better anchored to the left.
