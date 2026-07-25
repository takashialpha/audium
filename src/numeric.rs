//! Numeric conversions that are safe at their call sites but have no
//! infallible std API. Each documents why its cast is sound for the inputs it
//! is given, keeping the `#[allow]`s in one audited place.

/// Saturating `usize -> u16` for terminal-cell counts, which are always far
/// below `u16::MAX` but not provably so at compile time.
pub fn usize_to_u16_saturating(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/// Converts a ratio into a rounded whole percent. Clamped to `0.0..=1.0`
/// first, so the result lands in `0..=100` with no truncation or sign loss.
pub fn ratio_to_whole_percent(ratio: f32) -> u32 {
    let pct = (ratio.clamp(0.0, 1.0) * 100.0).round();
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let pct = pct as u32;
    pct
}

/// Inverse of [`ratio_to_whole_percent`]. Callers pass a UI slider value
/// clamped to `0..=100`, which `f32` represents exactly.
pub fn whole_percent_to_ratio(pct: u32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let pct = pct as f32;
    pct / 100.0
}

/// Rounds `ratio` (clamped to `0.0..=1.0`) times `total_units` into a count,
/// capped at `total_units`. Used for bar rendering, where `total_units` is a
/// cell count and so far below 2^53, where `usize -> f64` starts losing bits.
pub fn ratio_to_unit_count(ratio: f64, total_units: usize) -> usize {
    let ratio = ratio.clamp(0.0, 1.0);
    #[allow(clippy::cast_precision_loss)]
    let total_f = total_units as f64;
    let count = (ratio * total_f).round();
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let count = count as usize;
    count.min(total_units)
}
