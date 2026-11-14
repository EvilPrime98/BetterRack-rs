//! Wednesday-anchored weeks for the pull list, with hand-written civil-date math.
//!
//! A day is a count of days since 1970-01-01. A week runs from a Wednesday through the next
//! Tuesday and is identified by its Wednesday.

use std::time::{SystemTime, UNIX_EPOCH};

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[cfg(test)]
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
/// Day number of a leading `YYYY-MM-DD`, or `None` when the text has no valid date there.
pub fn parse_day(raw: &str) -> Option<i64> {
    let s = raw.get(..10)?;
    let b = s.as_bytes();
    if b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let t = &s[r];
        t.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| t.parse().ok())?
    };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&m) || d < 1 {
        return None;
    }
    let day = days_from_civil(y, m, d);
    (civil_from_days(day) == (y, m, d)).then_some(day)
}

/// The Wednesday on or before `day`.
pub fn week_start(day: i64) -> i64 {
    // 1970-01-01 was a Thursday, one day after a Wednesday.
    day - (day + 1).rem_euclid(7)
}

/// Today (UTC) as a day number.
pub fn today() -> i64 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs / 86_400) as i64
}

/// Midnight at the start of `day` as the full datetime the store's `after`/`before` filters need.
pub fn day_start(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}T00:00:00")
}

fn short(day: i64) -> String {
    let (_, m, d) = civil_from_days(day);
    format!("{} {}", MONTHS[(m - 1) as usize], d)
}

/// Range label of the week starting at `start`, such as `Nov 11 – Nov 17`.
pub fn range_label(start: i64) -> String {
    format!("{} – {}", short(start), short(start + 6))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> i64 {
        parse_day(s).unwrap()
    }

    #[test]
    fn parses_dates_and_rejects_bad_ones() {
        assert_eq!(day("1970-01-01"), 0);
        assert_eq!(day("2024-02-29T10:00:00"), day("2024-02-28") + 1);
        assert_eq!(parse_day("2023-02-29"), None);
        assert_eq!(parse_day("2023-13-01"), None);
        assert_eq!(parse_day("nope"), None);
        assert_eq!(parse_day("2023/02/01"), None);
    }

    #[test]
    fn wednesday_on_or_before() {
        // 2026-11-11 is a Wednesday.
        assert_eq!(week_start(day("2026-11-11")), day("2026-11-11"));
        assert_eq!(week_start(day("2026-11-14")), day("2026-11-11"));
        assert_eq!(week_start(day("2026-11-17")), day("2026-11-11"));
        assert_eq!(week_start(day("2026-11-18")), day("2026-11-18"));
        // Across a month and a year boundary.
        assert_eq!(week_start(day("2025-01-01")), day("2025-01-01"));
        assert_eq!(week_start(day("2025-01-02")), day("2025-01-01"));
        assert_eq!(week_start(day("2024-12-31")), day("2024-12-25"));
        // Leap day: 2024-02-29 is a Thursday.
        assert_eq!(week_start(day("2024-02-29")), day("2024-02-28"));
        assert_eq!(week_start(day("2024-03-05")), day("2024-02-28"));
    }

    #[test]
    fn day_starts_are_full_datetimes() {
        assert_eq!(day_start(day("2026-11-11")), "2026-11-11T00:00:00");
        assert_eq!(day_start(day("2026-11-11") + 7), "2026-11-18T00:00:00");
        assert_eq!(day_start(day("2024-02-29") + 1), "2024-03-01T00:00:00");
    }

    #[test]
    fn range_labels() {
        assert_eq!(range_label(day("2026-11-11")), "Nov 11 – Nov 17");
        assert_eq!(range_label(day("2024-12-25")), "Dec 25 – Dec 31");
        assert_eq!(range_label(day("2024-02-28")), "Feb 28 – Mar 5");
        assert_eq!(range_label(day("2024-12-31")), "Dec 31 – Jan 6");
    }
}
