//! The slice of Fuse.js 7.4.2 that `better-wiki` uses to pick the best title match:
//! `new Fuse(candidates, { keys: ['page.title'], threshold: 0.4, includeScore: true }).search(q)`.
//!
//! Bitap works on UTF-16 code units in JS, so the text and pattern are `Vec<u16>` here and
//! the 32-bit shifts are `i32` ones. Checked against the real library in `tests/golden.rs`.

use std::collections::HashMap;

const THRESHOLD: f64 = 0.4;
const DISTANCE: f64 = 100.0;
const MAX_BITS: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// Index into the searched list.
    pub idx: usize,
    /// `None` only for a blank query, where Fuse returns every item unscored.
    pub score: Option<f64>,
}

/// `fuse.search(query)` over a list of titles: matches only, best (lowest score) first.
pub fn search<S: AsRef<str>>(titles: &[S], query: &str) -> Vec<Hit> {
    if js_trim(query).is_empty() {
        return (0..titles.len()).map(|idx| Hit { idx, score: None }).collect();
    }
    let searcher = Bitap::new(query);
    let mut hits = Vec::new();
    for (idx, title) in titles.iter().enumerate() {
        let title = title.as_ref();
        if js_trim(title).is_empty() {
            continue; // blank values are not indexed
        }
        let (is_match, score) = searcher.search_in(title);
        if is_match {
            let exponent = field_norm(title);
            let base = if score == 0.0 { f64::EPSILON } else { score };
            hits.push(Hit { idx, score: Some(base.powf(exponent)) });
        }
    }
    hits.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal).then(a.idx.cmp(&b.idx)));
    hits
}

/// JS `String.prototype.trim` (U+FEFF counts as whitespace, U+0085 does not).
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}')
}

/// Fuse's field-length norm: `1 / sqrt(tokens)` rounded to 3 decimals, tokens = space-separated runs.
fn field_norm(value: &str) -> f64 {
    let mut tokens = 1u32;
    let mut in_space = false;
    for c in value.chars() {
        if c == ' ' {
            if !in_space {
                tokens += 1;
                in_space = true;
            }
        } else {
            in_space = false;
        }
    }
    let m = 1000.0;
    (m / (tokens as f64).powf(0.5) + 0.5).floor() / m // Math.round
}

struct Chunk {
    pattern: Vec<u16>,
    alphabet: HashMap<u16, i32>,
    start_index: usize,
}

struct Bitap {
    pattern: Vec<u16>,
    chunks: Vec<Chunk>,
}

impl Bitap {
    fn new(pattern: &str) -> Self {
        let pattern: Vec<u16> = pattern.to_lowercase().encode_utf16().collect();
        let len = pattern.len();
        let mut chunks = Vec::new();
        let mut add = |start: usize, end: usize| {
            let part = pattern[start..end].to_vec();
            chunks.push(Chunk { alphabet: pattern_alphabet(&part), pattern: part, start_index: start });
        };
        if len > MAX_BITS {
            let remainder = len % MAX_BITS;
            let end = len - remainder;
            let mut i = 0;
            while i < end {
                add(i, i + MAX_BITS);
                i += MAX_BITS;
            }
            if remainder != 0 {
                add(len - MAX_BITS, len);
            }
        } else if len > 0 {
            add(0, len);
        }
        Self { pattern, chunks }
    }

    fn search_in(&self, text: &str) -> (bool, f64) {
        let text: Vec<u16> = text.to_lowercase().encode_utf16().collect();
        if self.pattern == text {
            return (true, 0.0);
        }
        let mut total = 0.0;
        let mut has_matches = false;
        for chunk in &self.chunks {
            let (is_match, score) = bitap(&text, &chunk.pattern, &chunk.alphabet, chunk.start_index);
            has_matches |= is_match;
            total += score;
        }
        (has_matches, if has_matches { total / self.chunks.len() as f64 } else { 1.0 })
    }
}

fn pattern_alphabet(pattern: &[u16]) -> HashMap<u16, i32> {
    let mut mask: HashMap<u16, i32> = HashMap::new();
    let len = pattern.len();
    for (i, c) in pattern.iter().enumerate() {
        *mask.entry(*c).or_insert(0) |= 1i32.wrapping_shl((len - i - 1) as u32);
    }
    mask
}

fn index_of(text: &[u16], pattern: &[u16], from: usize) -> Option<usize> {
    if pattern.is_empty() || from + pattern.len() > text.len() {
        return None;
    }
    (from..=text.len() - pattern.len()).find(|&i| &text[i..i + pattern.len()] == pattern)
}

/// Fuse's `bitapSearch` with the default options (`ignoreLocation: false`, `distance: 100`,
/// `findAllMatches: false`, no match indices). Returns `(isMatch, score)`.
fn bitap(text: &[u16], pattern: &[u16], alphabet: &HashMap<u16, i32>, location: usize) -> (bool, f64) {
    let pattern_len = pattern.len();
    let text_len = text.len();
    let expected = location.min(text_len) as i64;
    let mut threshold = THRESHOLD;

    let calc = |errors: usize, at: i64| -> f64 { errors as f64 / pattern_len as f64 + (expected - at).abs() as f64 / DISTANCE };

    let mut best_location = expected;
    while let Some(index) = usize::try_from(best_location).ok().and_then(|from| index_of(text, pattern, from)) {
        threshold = threshold.min(calc(0, index as i64));
        best_location = (index + pattern_len) as i64;
    }

    best_location = -1;
    let mut last_bits: Vec<i32> = Vec::new();
    let mut final_score = 1.0;
    let mut bin_max = (pattern_len + text_len) as i64;
    let mask = 1i32.wrapping_shl((pattern_len - 1) as u32);

    for i in 0..pattern_len {
        let mut bin_min = 0i64;
        let mut bin_mid = bin_max;
        while bin_min < bin_mid {
            if calc(i, expected + bin_mid) <= threshold {
                bin_min = bin_mid;
            } else {
                bin_max = bin_mid;
            }
            bin_mid = (bin_max - bin_min) / 2 + bin_min;
        }
        bin_max = bin_mid;

        let mut start = 1.max(expected - bin_mid + 1);
        let finish = (expected + bin_mid).min(text_len as i64) + pattern_len as i64;
        let mut bits = vec![0i32; finish as usize + 2];
        bits[finish as usize + 1] = 1i32.wrapping_shl(i as u32).wrapping_sub(1);

        let last = |idx: i64| -> i32 { usize::try_from(idx).ok().and_then(|u| last_bits.get(u)).copied().unwrap_or(0) };

        let mut j = finish;
        while j >= start {
            let at = j - 1;
            let char_match = text.get(at as usize).and_then(|c| alphabet.get(c)).copied().unwrap_or(0);
            let ju = j as usize;
            bits[ju] = (bits[ju + 1].wrapping_shl(1) | 1) & char_match;
            if i > 0 {
                bits[ju] |= ((last(j + 1) | last(j)).wrapping_shl(1)) | 1 | last(j + 1);
            }
            if bits[ju] & mask != 0 {
                final_score = calc(i, at);
                if final_score <= threshold {
                    threshold = final_score;
                    best_location = at;
                    if best_location <= expected {
                        break;
                    }
                    start = 1.max(2 * expected - best_location);
                }
            }
            j -= 1;
        }

        if calc(i + 1, expected) > threshold {
            break;
        }
        last_bits = bits;
    }

    (best_location >= 0, final_score.max(0.001))
}
