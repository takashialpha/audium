//! Numeric conversions that are provably safe at their call sites but that
//! Rust's standard library has no infallible API for (float <-> int, or a
//! widening conversion clippy can't prove is lossless). Each function
//! documents exactly why its cast is sound for the inputs it's used with
//! elsewhere in this codebase, so the `#[allow]`s stay in one audited place
//! instead of scattered throughout call sites.

/// Saturating `usize -> u16` conversion for terminal-cell counts (label
/// lengths, list sizes, ...) that are for all practical purposes always
/// well under `u16::MAX`, but aren't provably bounded at compile time.
pub fn usize_to_u16_saturating(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/// Converts a ratio into a rounded whole percent.
///
/// `ratio` is clamped to `0.0..=1.0` here, so `ratio * 100.0` rounds to a
/// value in `0.0..=100.0`, which fits a `u32` exactly with no truncation
/// or sign loss possible.
pub fn ratio_to_whole_percent(ratio: f32) -> u32 {
    let pct = (ratio.clamp(0.0, 1.0) * 100.0).round();
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let pct = pct as u32;
    pct
}

/// Converts a whole percent back into a `0.0..=1.0` ratio. Inverse of
/// [`ratio_to_whole_percent`].
///
/// Every call site clamps `pct` to `0..=100` before calling this (it's a
/// UI slider value), and any `u32` in that range converts to `f32` exactly
/// (`f32` represents all integers up to 2^24 without loss).
pub fn whole_percent_to_ratio(pct: u32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let pct = pct as f32;
    pct / 100.0
}

/// Rounds `ratio` (clamped to `0.0..=1.0`) times `total_units` into a count
/// of units, capped at `total_units`. Used for volume/progress bar
/// rendering, where `total_units` is a terminal-cell count and therefore
/// always far below 2^53 — the point at which `usize -> f64` could start
/// losing precision.
pub fn ratio_to_unit_count(ratio: f64, total_units: usize) -> usize {
    let ratio = ratio.clamp(0.0, 1.0);
    #[allow(clippy::cast_precision_loss)]
    let total_f = total_units as f64;
    let count = (ratio * total_f).round();
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let count = count as usize;
    count.min(total_units)
}
