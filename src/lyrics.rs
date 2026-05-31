use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub time_ms: Option<u64>,
    pub text: String,
}

/// Parses raw LRC text or plain text into lyric lines.
///
/// If the input contains any `[mm:ss.xx]` timestamp tags those lines are
/// extracted and returned sorted by time; metadata tags like `[ti:…]` are
/// skipped.  If no timed lines are found every non-empty input line is
/// returned with `time_ms: None`.
pub fn parse_lrc(raw: &str) -> Vec<LyricLine> {
    let mut timed: Vec<LyricLine> = Vec::new();
    let mut plain: Vec<LyricLine> = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if let Some(ll) = try_timed(line) {
            timed.push(ll);
        } else if !line.starts_with('[') {
            plain.push(LyricLine { time_ms: None, text: line.to_string() });
        }
    }

    if !timed.is_empty() {
        timed.sort_by_key(|l| l.time_ms);
        timed
    } else {
        plain
    }
}

fn try_timed(line: &str) -> Option<LyricLine> {
    if !line.starts_with('[') { return None; }
    let close = line.find(']')?;
    let tag = &line[1..close];
    let text = line[close + 1..].trim().to_string();

    // Reject metadata tags whose first char is not a digit (e.g. [ti:Title]).
    if !tag.starts_with(|c: char| c.is_ascii_digit()) { return None; }

    let colon = tag.find(':')?;
    let mm: u64 = tag[..colon].parse().ok()?;
    let frac = &tag[colon + 1..];

    let ms = if let Some(dot) = frac.find('.') {
        let ss: u64 = frac[..dot].parse().ok()?;
        let sub = &frac[dot + 1..];
        let sub_ms: u64 = match sub.len() {
            1 => sub.parse::<u64>().ok()? * 100,
            2 => sub.parse::<u64>().ok()? * 10,
            _ => sub.parse::<u64>().ok()?,
        };
        mm * 60_000 + ss * 1_000 + sub_ms
    } else {
        let ss: u64 = frac.parse().ok()?;
        mm * 60_000 + ss * 1_000
    };

    Some(LyricLine { time_ms: Some(ms), text })
}

/// Returns the index of the last timed line whose timestamp ≤ `elapsed`.
pub fn active_idx(lines: &[LyricLine], elapsed: Duration) -> Option<usize> {
    let elapsed_ms = elapsed.as_millis() as u64;
    lines.iter()
        .enumerate()
        .filter_map(|(i, l)| l.time_ms.map(|t| (i, t)))
        .rfind(|(_, t)| *t <= elapsed_ms)
        .map(|(i, _)| i)
}
