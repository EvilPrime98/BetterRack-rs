//! `ComicInfo.xml` parsing and the `WikiComic` DTO.
//!
//! Only what the server reads is modelled: the direct text children of `<ComicInfo>` and the
//! `<Pages><Page .../></Pages>` attributes. Values are trimmed like fast-xml-parser does, and
//! parse failures yield `None` instead of an error.

use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComicInfo {
    /// Trimmed text of each direct child element of `<ComicInfo>` (first occurrence wins).
    pub fields: HashMap<String, String>,
    pub pages: Vec<ComicPage>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComicPage {
    /// `Image` attribute: the zero-based page index, as written.
    pub image: Option<String>,
    pub bookmark: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub page: u32,
    pub label: String,
}

#[derive(Default)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    text: String,
    children: Vec<Node>,
}

fn node_of(e: &quick_xml::events::BytesStart<'_>) -> Node {
    let attrs = e
        .attributes()
        .filter_map(|a| a.ok())
        .map(|a| {
            let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
            let val = a
                .unescape_value()
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned());
            (key, val)
        })
        .collect();
    Node {
        name: String::from_utf8_lossy(e.name().as_ref()).into_owned(),
        attrs,
        ..Node::default()
    }
}

fn build_tree(xml: &str) -> Option<Node> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().check_end_names = false;
    let mut stack: Vec<Node> = vec![Node::default()];
    loop {
        match reader.read_event().ok()? {
            Event::Start(e) => stack.push(node_of(&e)),
            Event::Empty(e) => stack.last_mut()?.children.push(node_of(&e)),
            Event::End(_) => {
                if stack.len() > 1 {
                    let node = stack.pop()?;
                    stack.last_mut()?.children.push(node);
                }
            }
            Event::Text(t) => {
                let s = t
                    .unescape()
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| String::from_utf8_lossy(&t).into_owned());
                stack.last_mut()?.text.push_str(&s);
            }
            Event::CData(c) => stack
                .last_mut()?
                .text
                .push_str(&String::from_utf8_lossy(&c)),
            Event::Eof => break,
            _ => {}
        }
    }
    // Unclosed elements: fold them into their parents.
    while stack.len() > 1 {
        let node = stack.pop()?;
        stack.last_mut()?.children.push(node);
    }
    stack.pop()
}

/// `None` when the document has no `<ComicInfo>` root or is not parseable.
pub fn parse(xml: &str) -> Option<ComicInfo> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let root = build_tree(xml)?;
    let info = root.children.into_iter().find(|n| n.name == "ComicInfo")?;
    let mut out = ComicInfo::default();
    for child in info.children {
        if child.name == "Pages" {
            if out.pages.is_empty() {
                out.pages = child
                    .children
                    .into_iter()
                    .filter(|p| p.name == "Page")
                    .map(|p| {
                        let attr =
                            |k: &str| p.attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
                        ComicPage {
                            image: attr("Image"),
                            bookmark: attr("Bookmark"),
                        }
                    })
                    .collect();
            }
        } else {
            out.fields
                .entry(child.name)
                .or_insert_with(|| child.text.trim().to_string());
        }
    }
    Some(out)
}

/// JS `Number(s)` for the string forms that matter here; NaN when not numeric.
pub fn js_number(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [
        ("0x", 16),
        ("0X", 16),
        ("0o", 8),
        ("0O", 8),
        ("0b", 2),
        ("0B", 2),
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return u64::from_str_radix(rest, radix).map_or(f64::NAN, |n| n as f64);
        }
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    if t.chars()
        .any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
    {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// `Number.isInteger(n) && n >= 1` -> the page.
pub fn js_positive_integer(n: f64) -> Option<u32> {
    (n.is_finite() && n.fract() == 0.0 && n >= 1.0 && n <= u32::MAX as f64).then_some(n as u32)
}

/// JS `Number.parseInt(s, 10)`: leading integer prefix, `None` for NaN.
fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start();
    let (sign, digits) = match t.strip_prefix('-') {
        Some(r) => (-1, r),
        None => (1, t.strip_prefix('+').unwrap_or(t)),
    };
    let end = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());
    (end > 0).then(|| sign * digits[..end].parse::<i64>().unwrap_or(i64::MAX))
}

/// JS `Number.parseFloat(s)`: longest numeric prefix, `None` for NaN.
fn parse_float(s: &str) -> Option<f64> {
    let t = s.trim_start();
    (1..=t.len())
        .rev()
        .filter(|&i| t.is_char_boundary(i))
        .map(|i| &t[..i])
        .find_map(|p| {
            p.parse::<f64>().ok().filter(|f| {
                f.is_finite()
                    && !p
                        .chars()
                        .any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
            })
        })
}

impl ComicInfo {
    pub fn text(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    /// `toInt(raw.X, fallback)`.
    pub fn int(&self, key: &str, fallback: i64) -> i64 {
        self.text(key).and_then(parse_int).unwrap_or(fallback)
    }

    fn float(&self, key: &str, fallback: f64) -> f64 {
        self.text(key).and_then(parse_float).unwrap_or(fallback)
    }

    /// Pages with a non-blank bookmark, 1-based, in document order.
    pub fn bookmarks(&self) -> Vec<Bookmark> {
        self.pages
            .iter()
            .filter_map(|p| {
                let label = p.bookmark.as_deref()?.trim();
                if label.is_empty() {
                    return None;
                }
                let image = p.image.as_deref().map_or(f64::NAN, js_number);
                let page = js_positive_integer(image + 1.0)?;
                Some(Bookmark {
                    page,
                    label: label.to_string(),
                })
            })
            .collect()
    }
}

fn split_list(value: Option<&str>) -> Vec<String> {
    value
        .map(|v| {
            v.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn appearances(value: Option<&str>) -> Vec<Value> {
    split_list(value)
        .into_iter()
        .map(|name| json!({ "name": name, "pageTitle": name }))
        .collect()
}

/// Count/Volume/Year/Month default to -1 when absent.
fn int_or_empty(v: i64) -> String {
    if v == -1 {
        String::new()
    } else {
        v.to_string()
    }
}

/// Key order is fixed, since the result is persisted as JSON.
pub fn to_wiki_comic(info: &ComicInfo) -> Value {
    let rating = info.float("CommunityRating", 0.0);
    let mut other = appearances(info.text("Characters"));
    other.extend(appearances(info.text("Teams")));
    let notes: Vec<Value> = ["Notes", "Review"]
        .iter()
        .filter_map(|k| info.text(k))
        .filter(|v| !v.trim().is_empty())
        .map(|v| Value::String(v.to_string()))
        .collect();
    json!({
        "title": info.text("Title").or(info.text("Series")).unwrap_or(""),
        "volume": int_or_empty(info.int("Volume", -1)),
        "issue": info.text("Number").unwrap_or(""),
        "cover": "",
        "pageId": 0,
        "credits": {
            "writers": split_list(info.text("Writer")),
            "artists": split_list(info.text("Penciller")),
            "inkers": split_list(info.text("Inker")),
            "colorists": split_list(info.text("Colorist")),
            "letterers": split_list(info.text("Letterer")),
            "editors": split_list(info.text("Editor")),
            "executiveEditors": [],
        },
        "releaseDate": {
            "releaseDay": "",
            "releaseMonth": int_or_empty(info.int("Month", -1)),
            "releaseYear": int_or_empty(info.int("Year", -1)),
        },
        "synopsis": info.text("Summary").unwrap_or(""),
        "rating": if rating != 0.0 { rating.to_string() } else { String::new() },
        "event": "",
        "storyTitles": [],
        "appearing": {
            "featuredCharacters": [],
            "supportingCharacters": [],
            "antagonists": [],
            "otherCharacters": other,
            "locations": appearances(info.text("Locations")),
            "items": [],
            "concepts": [],
        },
        "coverVariants": [],
        "notes": notes,
        "trivia": [],
        "sourceWiki": "",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ComicInfo xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <Series>Test Series</Series>
  <Number>1</Number>
  <Volume>2</Volume>
  <Year>2020</Year>
  <Month>5</Month>
  <Writer>A &amp; B, C</Writer>
  <Characters>Hero, Villain</Characters>
  <Teams>Team One</Teams>
  <CommunityRating>4.5</CommunityRating>
  <Notes>  a note </Notes>
  <Review> </Review>
  <Pages>
    <Page Image="0" Type="FrontCover" />
    <Page Image="2" Bookmark=" Chapter 2 " />
    <Page Image="x" Bookmark="Bad" />
    <Page Image="4" Bookmark="  " />
  </Pages>
</ComicInfo>"#;

    #[test]
    fn parses_fields_and_trims() {
        let i = parse(XML).unwrap();
        assert_eq!(i.text("Series"), Some("Test Series"));
        assert_eq!(i.text("Writer"), Some("A & B, C"));
        assert_eq!(i.text("Notes"), Some("a note"));
        assert_eq!(i.int("Year", -1), 2020);
        assert_eq!(i.int("Count", -1), -1);
    }

    #[test]
    fn bookmarks_are_one_based_and_filtered() {
        assert_eq!(
            parse(XML).unwrap().bookmarks(),
            vec![Bookmark {
                page: 3,
                label: "Chapter 2".into()
            }]
        );
    }

    #[test]
    fn missing_root_or_garbage_is_none() {
        assert!(parse("<Other/>").is_none());
        assert!(parse("").is_none());
    }

    #[test]
    fn empty_root_gives_defaults() {
        let i = parse("<ComicInfo/>").unwrap();
        assert!(i.fields.is_empty() && i.pages.is_empty());
    }

    #[test]
    fn dto_matches_the_ts_mapping() {
        let v = to_wiki_comic(&parse(XML).unwrap());
        assert_eq!(v["title"], "Test Series");
        assert_eq!(v["volume"], "2");
        assert_eq!(v["issue"], "1");
        assert_eq!(v["credits"]["writers"], json!(["A & B", "C"]));
        assert_eq!(
            v["releaseDate"],
            json!({ "releaseDay": "", "releaseMonth": "5", "releaseYear": "2020" })
        );
        assert_eq!(v["rating"], "4.5");
        assert_eq!(v["notes"], json!(["a note"]));
        assert_eq!(
            v["appearing"]["otherCharacters"],
            json!([
                { "name": "Hero", "pageTitle": "Hero" },
                { "name": "Villain", "pageTitle": "Villain" },
                { "name": "Team One", "pageTitle": "Team One" },
            ])
        );
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(&keys[..4], ["title", "volume", "issue", "cover"]);
        assert_eq!(*keys.last().unwrap(), "sourceWiki");
    }

    #[test]
    fn dto_absent_values_are_empty() {
        let v = to_wiki_comic(&parse("<ComicInfo><Title>T</Title></ComicInfo>").unwrap());
        assert_eq!(
            (
                v["title"].as_str(),
                v["volume"].as_str(),
                v["rating"].as_str()
            ),
            (Some("T"), Some(""), Some(""))
        );
    }

    #[test]
    fn js_number_matches_node() {
        assert_eq!(js_number(" 3 "), 3.0);
        assert_eq!(js_number(""), 0.0);
        assert!(js_number("abc").is_nan());
        assert_eq!(js_number("0x10"), 16.0);
        assert_eq!(js_positive_integer(js_number("1.0")), Some(1));
        assert_eq!(js_positive_integer(js_number("0")), None);
        assert_eq!(js_positive_integer(js_number("1.5")), None);
    }
}
