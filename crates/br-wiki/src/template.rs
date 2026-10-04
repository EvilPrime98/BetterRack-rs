//! Wikitext helpers shared by the DC and Marvel builders. Everything here mirrors a function of
//! `better-wiki` (`parseMediaWikiTemplate` in `better-wiki.js`, the helpers in the plugins).

use crate::fuse::js_trim;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Infobox fields of a page, `key -> value`. A repeated key keeps the last value.
pub type Content = HashMap<String, String>;

static HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{[^|{}\n]+\n").unwrap());
static LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").unwrap());
static NOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{([^{}|]+)\}\}").unwrap());
static APPEARING_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^'''\s*(.+?):?\s*'''$").unwrap());
static REF_BLOCK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<ref[^>]*>.*?</ref>").unwrap());
static REF_SELF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<ref[^>]*/>").unwrap());
static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->").unwrap());
static QUOTES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"'''?").unwrap());
static LEADING_BULLETS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\*+\s*").unwrap());

/// `parseMediaWikiTemplate`: the first multi-line `{{Template` block of the page, split into
/// `|key = value` pairs. Brace depth is tracked to find the real closing `}}`.
pub fn parse_media_wiki_template(page: &str) -> Content {
    let mut result = Content::new();
    let Some(header) = HEADER.find(page) else {
        return result;
    };
    let bytes = page.as_bytes();
    let (mut depth, mut end, mut i) = (0i32, None, header.start());
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            depth += 1;
            i += 1;
        } else if bytes[i] == b'}' && bytes[i + 1] == b'}' {
            depth -= 1;
            if depth == 0 {
                end = Some(i);
                break;
            }
            i += 1;
        }
        i += 1;
    }
    let Some(end) = end else { return result };
    let inner = &page[header.start() + 2..end];
    for part in inner.split("\n|").skip(1) {
        let Some(eq) = part.find('=') else { continue };
        let key = js_trim(&part[..eq]);
        if !key.is_empty() {
            result.insert(key.to_string(), js_trim(&part[eq + 1..]).to_string());
        }
    }
    result
}

/// `content[key]` when the field exists (even if empty).
pub fn get<'a>(content: &'a Content, key: &str) -> Option<&'a str> {
    content.get(key).map(String::as_str)
}

/// `content[key] || ''`.
pub fn text(content: &Content, key: &str) -> String {
    content.get(key).cloned().unwrap_or_default()
}

pub fn collect_sequential(content: &Content, key: impl Fn(usize) -> String) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 1;
    while let Some(v) = content.get(&key(i)) {
        let v = js_trim(v);
        if !v.is_empty() {
            out.push(v.to_string());
        }
        i += 1;
    }
    out
}

fn collect_credits(content: &Content, role: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut s = 1;
    while content.contains_key(&format!("{role}{s}_1")) {
        let mut p = 1;
        while let Some(v) = content.get(&format!("{role}{s}_{p}")) {
            let v = js_trim(v);
            if !v.is_empty() {
                out.push(v.to_string());
            }
            p += 1;
        }
        s += 1;
    }
    out
}

pub fn build_credits(content: &Content) -> Value {
    let executive = content
        .get("Executive Editor")
        .map(|v| js_trim(v))
        .filter(|v| !v.is_empty());
    json!({
        "writers": collect_credits(content, "Writer"),
        "artists": collect_credits(content, "Penciler"),
        "inkers": collect_credits(content, "Inker"),
        "colorists": collect_credits(content, "Colorist"),
        "letterers": collect_credits(content, "Letterer"),
        "editors": collect_credits(content, "Editor"),
        "executiveEditors": executive.map(|v| vec![v]).unwrap_or_default(),
    })
}

/// Lines starting with `*`, bullet stripped.
pub fn parse_bullets(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw.filter(|r| !r.is_empty()) else {
        return vec![];
    };
    raw.split('\n')
        .map(js_trim)
        .filter(|l| l.starts_with('*'))
        .map(|l| js_trim(&LEADING_BULLETS.replace(l, "")).to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

const APPEARING_SECTIONS: [(&str, &str); 7] = [
    ("featured characters", "featuredCharacters"),
    ("supporting characters", "supportingCharacters"),
    ("antagonists", "antagonists"),
    ("other characters", "otherCharacters"),
    ("locations", "locations"),
    ("items", "items"),
    ("concepts", "concepts"),
];

/// The `Appearing1` field: `'''Featured Characters:'''` headers followed by `* [[Link|Name]] {{note}}`.
pub fn parse_appearing(raw: Option<&str>) -> Value {
    let mut sections: Vec<(&str, Vec<Value>)> = APPEARING_SECTIONS
        .iter()
        .map(|(_, key)| (*key, Vec::new()))
        .collect();
    if let Some(raw) = raw.filter(|r| !r.is_empty()) {
        let mut current: Option<usize> = None;
        for line in raw.split('\n') {
            let line = js_trim(line);
            if let Some(header) = APPEARING_HEADER.captures(line) {
                let name = js_trim(&header[1]).to_lowercase();
                current = APPEARING_SECTIONS.iter().position(|(k, _)| *k == name);
                continue;
            }
            let Some(section) = current.filter(|_| line.starts_with('*')) else {
                continue;
            };
            let Some(link) = LINK.captures(line) else {
                continue;
            };
            let page_title = js_trim(&link[1]);
            let name = js_trim(link.get(2).map_or(&link[1], |m| m.as_str()));
            let notes: Vec<&str> = NOTE
                .captures_iter(line)
                .map(|c| js_trim(c.get(1).unwrap().as_str()))
                .collect();
            let mut entry = Map::new();
            entry.insert("name".into(), name.into());
            entry.insert("pageTitle".into(), page_title.into());
            if !notes.is_empty() {
                entry.insert("statusNote".into(), notes.join(", ").into());
            }
            sections[section].1.push(Value::Object(entry));
        }
    }
    Value::Object(
        sections
            .into_iter()
            .map(|(k, v)| (k.to_string(), Value::Array(v)))
            .collect(),
    )
}

/// `stripWiki`: drop refs and comments, keep link labels, drop bold/italic marks.
pub fn strip_wiki(s: &str) -> String {
    let s = REF_BLOCK.replace_all(s, "");
    let s = REF_SELF.replace_all(&s, "");
    let s = COMMENT.replace_all(&s, "");
    let s = LINK.replace_all(&s, |c: &regex::Captures| {
        js_trim(c.get(2).map_or(&c[1], |m| m.as_str())).to_string()
    });
    let s = QUOTES.replace_all(&s, "");
    js_trim(&s).to_string()
}

/// A `[[Target|Label]]` label, else the text without wiki markup (Marvel's `Speaker`).
pub fn extract_speaker(raw: Option<&str>) -> Option<String> {
    let raw = raw.filter(|r| !r.is_empty())?;
    if let Some(link) = LINK.captures(raw) {
        return Some(js_trim(link.get(2).map_or(&link[1], |m| m.as_str())).to_string());
    }
    Some(strip_wiki(raw)).filter(|s| !s.is_empty())
}

/// JS `parseInt(s, 10)`: leading whitespace, sign, digits; `NaN` when there are none.
pub fn js_parse_int(s: &str) -> f64 {
    let s = js_trim(s);
    let (sign, digits) = match s.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, s.strip_prefix('+').unwrap_or(s)),
    };
    let end = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());
    if end == 0 {
        return f64::NAN;
    }
    sign * digits[..end].parse::<f64>().unwrap_or(f64::NAN)
}

/// JS `Number(s)` for the values that occur in infobox date parts.
pub fn js_number(s: &str) -> f64 {
    let t = js_trim(s);
    if t.is_empty() {
        return 0.0;
    }
    if t.chars()
        .any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
    {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// JS `padStart(2, '0')`.
pub fn pad2(s: &str) -> String {
    let units = s.encode_utf16().count();
    if units >= 2 {
        s.to_string()
    } else {
        format!("{}{s}", "0".repeat(2 - units))
    }
}

/// How JS prints a whole-number `Number` (`120`, not `120.0`); `NaN`/`Infinity` as JS does.
pub fn js_number_string(n: f64) -> String {
    if n.is_nan() {
        "NaN".into()
    } else if n.is_infinite() {
        if n > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        }
    } else if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_skips_inline_templates_and_nested_braces() {
        let page = "{{DISPLAYTITLE:x}}\n{{TOC}}\n{{Comic Infobox\n| Volume = 2\n| Writer1_1 = Jane {{a|b}} Doe\n|Synopsis1 = Line one\nwith a break\n| Image = Cover.jpg\n}}\nbody {{x}}";
        let c = parse_media_wiki_template(page);
        assert_eq!(get(&c, "Volume"), Some("2"));
        assert_eq!(get(&c, "Writer1_1"), Some("Jane {{a|b}} Doe"));
        assert_eq!(get(&c, "Synopsis1"), Some("Line one\nwith a break"));
        assert_eq!(get(&c, "Image"), Some("Cover.jpg"));
        assert_eq!(c.len(), 4);
    }

    #[test]
    fn template_without_a_multiline_block_or_unclosed_is_empty() {
        assert!(parse_media_wiki_template("plain text {{inline}} only").is_empty());
        assert!(parse_media_wiki_template("{{Box\n| A = 1\n").is_empty());
    }

    #[test]
    fn template_value_may_contain_equals_and_empty_keys_are_dropped() {
        let c = parse_media_wiki_template("{{Box\n| A = x=y\n| = nokey\n| B\n}}");
        assert_eq!(get(&c, "A"), Some("x=y"));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn credits_walk_sections_and_parts() {
        let page = "{{Box\n| Writer1_1 = A\n| Writer1_2 = B\n| Writer2_1 = C\n| Penciler1_1 = P\n| Executive Editor = Boss \n}}";
        let credits = build_credits(&parse_media_wiki_template(page));
        assert_eq!(credits["writers"], json!(["A", "B", "C"]));
        assert_eq!(credits["artists"], json!(["P"]));
        assert_eq!(credits["executiveEditors"], json!(["Boss"]));
        assert_eq!(credits["inkers"], json!([]));
    }

    #[test]
    fn appearing_sections_links_and_notes() {
        let raw = "'''Featured Characters:'''\n* [[Batman]] {{1st}}\n* [[Robin|Dick Grayson]]\n'''Locations:'''\n* [[Gotham City]]\n'''Unknown:'''\n* [[Ignored]]\n* no link";
        let a = parse_appearing(Some(raw));
        assert_eq!(
            a["featuredCharacters"],
            json!([{"name": "Batman", "pageTitle": "Batman", "statusNote": "1st"}, {"name": "Dick Grayson", "pageTitle": "Robin"}])
        );
        assert_eq!(
            a["locations"],
            json!([{"name": "Gotham City", "pageTitle": "Gotham City"}])
        );
        assert_eq!(a["items"], json!([]));
        assert_eq!(parse_appearing(None)["concepts"], json!([]));
    }

    #[test]
    fn bullets_and_speaker_and_strip() {
        assert_eq!(
            parse_bullets(Some("* a\n** b \nplain\n*\n*  c")),
            vec!["a", "b", "c"]
        );
        assert_eq!(
            extract_speaker(Some("[[Bruce Wayne|Batman]] said")).as_deref(),
            Some("Batman")
        );
        assert_eq!(
            extract_speaker(Some("'''Joker'''<ref>x</ref>")).as_deref(),
            Some("Joker")
        );
        assert_eq!(extract_speaker(Some("<!-- c -->")), None);
        assert_eq!(strip_wiki("a [[B|c]] <ref name=\"x\"/>d"), "a c d");
    }

    #[test]
    fn js_number_helpers() {
        assert_eq!(js_parse_int("March").is_nan(), true);
        assert_eq!(js_parse_int(" 12abc"), 12.0);
        assert_eq!(js_parse_int("-3"), -3.0);
        assert_eq!(js_number(""), 0.0);
        assert_eq!(js_number("7"), 7.0);
        assert!(js_number("x").is_nan());
        assert_eq!(pad2("6"), "06");
        assert_eq!(pad2("12"), "12");
        assert_eq!(js_number_string(120.0), "120");
        assert_eq!(js_number_string(f64::NAN), "NaN");
    }
}
