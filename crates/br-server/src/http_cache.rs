//! Port of `utils/http-cache.ts`: strong ETags and `If-None-Match` matching.

use serde_json::Value;
use sha1::{Digest, Sha1};

/// `"<sha1 of JSON.stringify(parts)>"`. Parts are pre-rendered JSON fragments, so numbers keep
/// JS formatting (`mtimeMs` is a fractional double).
pub fn strong_etag(parts: &[String]) -> String {
    let json = format!("[{}]", parts.join(","));
    let hex: String = Sha1::digest(json.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    format!("\"{hex}\"")
}

/// JSON string literal, as `JSON.stringify(str)` writes it.
pub fn json_str(s: &str) -> String {
    Value::String(s.to_string()).to_string()
}

/// JS number formatting for the integral/fractional doubles we hash (`Display` is shortest
/// round-trip and never prints an exponent below 1e21, like JS).
pub fn json_num(n: f64) -> String {
    format!("{n}")
}

/// True when `If-None-Match` matches `etag`: comma-separated list, `W/` prefix, or `*`.
pub fn if_none_match_satisfied(header: Option<&str>, etag: &str) -> bool {
    let Some(header) = header.map(str::trim).filter(|h| !h.is_empty()) else { return false };
    if header == "*" {
        return true;
    }
    let weak = format!("W/{etag}");
    header.split(',').map(str::trim).any(|c| c == etag || c == weak)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etag_is_sha1_of_the_json_array() {
        // sha1('["a",1]')
        let e = strong_etag(&[json_str("a"), json_num(1.0)]);
        assert_eq!(e.len(), 42);
        assert!(e.starts_with('"') && e.ends_with('"'));
        assert_eq!(e, strong_etag(&[json_str("a"), json_num(1.0)]));
        assert_ne!(e, strong_etag(&[json_str("a"), json_num(2.0)]));
    }

    #[test]
    fn matching_rules() {
        let e = "\"abc\"";
        assert!(if_none_match_satisfied(Some("*"), e));
        assert!(if_none_match_satisfied(Some("\"x\", \"abc\""), e));
        assert!(if_none_match_satisfied(Some("W/\"abc\""), e));
        assert!(!if_none_match_satisfied(Some("\"x\""), e));
        assert!(!if_none_match_satisfied(None, e));
        assert!(!if_none_match_satisfied(Some(""), e));
    }

    #[test]
    fn js_number_formatting() {
        assert_eq!(json_num(1759500000123.0), "1759500000123");
        assert_eq!(json_num(1759500000123.4568), "1759500000123.4568");
    }
}
