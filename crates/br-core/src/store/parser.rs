//! `GcwHtmlParser`: pull download links out of a GetComics post. Each strategy keeps its quirks
//! (see the notes); only the PixelDrain resolution is async and lives
//! in `store::StoreApi`, which drives these pure steps. A [`GcwParser`] only stores the raw HTML:
//! `scraper::Html` is not `Send`, so each step parses and drops its own document.

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::sync::OnceLock;

const FORBIDDEN_PROVIDERS: [&str; 3] = ["terabox", "mega", "wetransfer"];

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub title: String,
    pub download_link: String,
}

pub struct GcwParser {
    raw_html: String,
    forbidden_urls: Vec<String>,
}

fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("static selector")
}

fn text_of(e: ElementRef) -> String {
    e.text().collect()
}

fn next_el(e: ElementRef) -> Option<ElementRef> {
    e.next_siblings().find_map(ElementRef::wrap)
}

fn prev_el(e: ElementRef) -> Option<ElementRef> {
    e.prev_siblings().find_map(ElementRef::wrap)
}

fn parent_el(e: ElementRef) -> Option<ElementRef> {
    e.parent().and_then(ElementRef::wrap)
}

fn href(e: Option<ElementRef>) -> String {
    e.and_then(|a| a.value().attr("href"))
        .unwrap_or_default()
        .to_string()
}

/// The text nodes that are direct children of `e`.
fn direct_text(e: ElementRef) -> String {
    e.children()
        .filter_map(|n| n.value().as_text())
        .map(|t| &**t)
        .collect()
}

pub fn normalize_text(text: &str) -> String {
    static WS: OnceLock<Regex> = OnceLock::new();
    let ws = WS.get_or_init(|| Regex::new(r"\s+").expect("ws"));
    ws.replace_all(&text.replace(['\n', ':'], ""), " ")
        .trim()
        .to_string()
}

impl GcwParser {
    /// `host_domain` is the store origin; links into its `/dc`, `/marvel` and `/other-comics`
    /// category pages are never downloads.
    pub fn new(raw_html: &str, host_domain: &str) -> Self {
        let forbidden_urls = if host_domain.is_empty() {
            vec![]
        } else {
            ["dc", "marvel", "other-comics"]
                .iter()
                .map(|s| format!("{host_domain}/{s}"))
                .collect()
        };
        Self {
            raw_html: raw_html.to_string(),
            forbidden_urls,
        }
    }

    pub fn normalize_links(&self, links: Vec<Link>) -> Vec<Link> {
        links
            .into_iter()
            .filter(|l| {
                let lower = l.download_link.to_lowercase();
                FORBIDDEN_PROVIDERS.iter().all(|p| !lower.contains(p))
                    && self
                        .forbidden_urls
                        .iter()
                        .all(|u| !lower.contains(u.as_str()))
            })
            .collect()
    }

    /// The single-issue strategies, in the order they run (3, 1, 2); the first that yields
    /// something wins.
    pub fn single(&self) -> Vec<Link> {
        let doc = Html::parse_document(&self.raw_html);
        for found in [
            self.single_3(&doc),
            self.single_1(&doc),
            self.single_2(&doc),
        ] {
            if !found.is_empty() {
                return found;
            }
        }
        vec![]
    }

    /// `a.aio-red[title="Download Now"]` under the "download ... comic/free" heading.
    fn single_3(&self, doc: &Html) -> Vec<Link> {
        let Some(a) = doc
            .select(&sel("a.aio-red[title=\"Download Now\" i]"))
            .next()
        else {
            return vec![];
        };
        let heading = doc.select(&sel("h2")).find(|el| {
            let t = text_of(*el).to_lowercase();
            t.contains("download") && (t.contains("comic") || t.contains("free"))
        });
        let title = heading
            .and_then(next_el)
            .and_then(|p| p.select(&sel("strong")).next())
            .map(text_of)
            .filter(|t| !t.is_empty())
            .or_else(|| {
                doc.select(&sel("p strong"))
                    .next()
                    .map(text_of)
                    .filter(|t| !t.is_empty())
            })
            .unwrap_or_default();
        self.normalize_links(vec![Link {
            title: normalize_text(&title),
            download_link: href(Some(a)),
        }])
    }

    /// The "free comics" heading, then paragraph / spacer / button block. A matching
    /// heading with a block that does not fit still yields one (empty) link.
    fn single_1(&self, doc: &Html) -> Vec<Link> {
        let free = doc.select(&sel("h2")).find(|el| {
            let c = el.inner_html().to_lowercase();
            c.contains("free") && c.contains("comics")
        });
        let Some(free) = free else { return vec![] };
        let p = next_el(free);
        let title = p
            .and_then(|p| p.select(&sel("strong")).next())
            .map(text_of)
            .unwrap_or_default();
        let div = p.and_then(next_el).and_then(next_el);
        let a = div.and_then(|d| d.select(&sel("a")).next());
        self.normalize_links(vec![Link {
            title,
            download_link: href(a),
        }])
    }

    /// The first anchor titled and labelled "download now"; the title sits two blocks above it.
    fn single_2(&self, doc: &Html) -> Vec<Link> {
        let anchor = doc.select(&sel("a")).find(|a| {
            a.value()
                .attr("title")
                .unwrap_or_default()
                .to_lowercase()
                .contains("download now")
                && text_of(*a).to_lowercase().contains("download now")
        });
        let Some(anchor) = anchor else { return vec![] };
        let title = parent_el(anchor)
            .and_then(parent_el)
            .and_then(prev_el)
            .and_then(prev_el)
            .and_then(|e| e.select(&sel("strong")).next())
            .map(text_of)
            .unwrap_or_default();
        self.normalize_links(vec![Link {
            title,
            download_link: href(Some(anchor)),
        }])
    }

    /// Plain multi-issue list: every `li` of the first `ul` with its first link. A "difficulties
    /// to download" item anywhere aborts the whole strategy.
    pub fn multiple_plain(&self) -> Vec<Link> {
        let doc = Html::parse_document(&self.raw_html);
        let Some(list) = doc.select(&sel("ul")).next() else {
            return vec![];
        };
        let mut out = vec![];
        for li in list.select(&sel("li")) {
            let text = direct_text(li);
            if text.to_lowercase().contains("difficulties to download") {
                return vec![];
            }
            out.push(Link {
                title: normalize_text(&text),
                download_link: href(li.select(&sel("a")).next()),
            });
        }
        self.normalize_links(out)
    }

    /// `(title, masked PixelDrain url)` for each list item that has a "pixeldrain" link, up to the
    /// "difficulties to download" item.
    pub fn pixeldrain_candidates(&self) -> Vec<(String, String)> {
        let doc = Html::parse_document(&self.raw_html);
        let Some(list) = doc.select(&sel("ul")).next() else {
            return vec![];
        };
        let a_sel = sel("a");
        let mut out = vec![];
        for li in list.select(&sel("li")) {
            let text = direct_text(li);
            if text.to_lowercase().contains("difficulties to download") {
                break;
            }
            let pd = li
                .select(&a_sel)
                .find(|a| text_of(*a).to_lowercase().contains("pixeldrain"));
            let url = href(pd);
            if url.is_empty() {
                continue;
            }
            out.push((normalize_text(&text), url));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: &str = "https://getcomics.org";

    fn links(l: &[Link]) -> Vec<(&str, &str)> {
        l.iter()
            .map(|l| (l.title.as_str(), l.download_link.as_str()))
            .collect()
    }

    #[test]
    fn normalize_text_strips_colons_and_collapses_space() {
        assert_eq!(
            normalize_text("  Batman:\n  #1   (2020) "),
            "Batman #1 (2020)"
        );
    }

    #[test]
    fn single_3_aio_red_button_with_heading_title() {
        let html = r#"<h2>Download Free Comic</h2><p><strong>Batman #1 : The Start</strong></p>
            <a class="aio-red" title="Download Now" href="https://getcomics.org/dlds/abc">Download Now</a>"#;
        let l = GcwParser::new(html, ORIGIN).single();
        assert_eq!(
            links(&l),
            [("Batman #1 The Start", "https://getcomics.org/dlds/abc")]
        );
    }

    #[test]
    fn single_3_falls_back_to_first_paragraph_title() {
        let html = r#"<p><strong>Only Title</strong></p><a class="aio-red" title="download now" href="https://x.test/f.cbz">x</a>"#;
        let l = GcwParser::new(html, ORIGIN).single();
        assert_eq!(links(&l), [("Only Title", "https://x.test/f.cbz")]);
    }

    #[test]
    fn single_1_free_comics_block() {
        let html = r#"<h2>Free Comics</h2><p><strong>Free Book</strong></p><div></div><div><a href="https://x.test/free.cbz">go</a></div>"#;
        let l = GcwParser::new(html, ORIGIN).single();
        assert_eq!(links(&l), [("Free Book", "https://x.test/free.cbz")]);
    }

    #[test]
    fn single_2_download_now_anchor_title_two_blocks_up() {
        let html = r#"<p><strong>Two Up Title</strong></p><p>skip</p>
            <div><p><a title="Download Now" href="https://x.test/two.cbz">DOWNLOAD NOW</a></p></div>"#;
        let l = GcwParser::new(html, ORIGIN).single();
        assert_eq!(links(&l), [("Two Up Title", "https://x.test/two.cbz")]);
    }

    #[test]
    fn nothing_found_is_empty() {
        assert!(
            GcwParser::new("<p>nothing here</p>", ORIGIN)
                .single()
                .is_empty()
        );
        // A free-comics heading without the expected block still yields one empty link.
        let l = GcwParser::new("<h2>Free Comics</h2>", ORIGIN).single();
        assert_eq!(links(&l), [("", "")]);
    }

    #[test]
    fn forbidden_providers_and_category_pages_are_dropped() {
        let html = r#"<ul>
            <li>Good #1 : <a href="https://x.test/good.cbz">Link</a></li>
            <li>Mega #2 : <a href="https://mega.nz/file/zzz">Link</a></li>
            <li>Tera #3 : <a href="https://terabox.com/s/1">Link</a></li>
            <li>Cat #4 : <a href="https://getcomics.org/dc/batman">Link</a></li>
        </ul>"#;
        let l = GcwParser::new(html, ORIGIN).multiple_plain();
        assert_eq!(links(&l), [("Good #1", "https://x.test/good.cbz")]);
    }

    #[test]
    fn multiple_plain_aborts_on_the_difficulties_item() {
        let html = r#"<ul><li>A : <a href="https://x.test/a.cbz">a</a></li>
            <li>Having difficulties to download? <a href="https://x.test/help">help</a></li></ul>"#;
        assert!(GcwParser::new(html, ORIGIN).multiple_plain().is_empty());
    }

    #[test]
    fn pixeldrain_candidates_stop_at_the_difficulties_item() {
        let html = r#"<ul>
            <li>Issue 1 : <a href="https://x.test/m1">Mirror</a> <a href="https://x.test/pd1">PixelDrain</a></li>
            <li>Issue 2 : <a href="https://x.test/m2">Mirror</a></li>
            <li>Issue 3 : <a href="https://x.test/pd3">pixeldrain</a></li>
            <li>Difficulties to download? <a href="https://x.test/pd4">PixelDrain</a></li>
            <li>Issue 5 : <a href="https://x.test/pd5">PixelDrain</a></li></ul>"#;
        let c = GcwParser::new(html, ORIGIN).pixeldrain_candidates();
        assert_eq!(
            c,
            [
                ("Issue 1".to_string(), "https://x.test/pd1".to_string()),
                ("Issue 3".to_string(), "https://x.test/pd3".to_string())
            ]
        );
    }

    /// Real GetComics posts (saved 2026-10-04); `expected.json` is the recorded
    /// parser output for the same HTML. `multiple` is only recorded for posts without PixelDrain
    /// (those would need the network).
    #[test]
    fn matches_bun_on_real_posts() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/store");
        let expected: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).unwrap())
                .unwrap();
        let to_links = |v: &serde_json::Value| -> Vec<Link> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|l| Link {
                    title: l["title"].as_str().unwrap().into(),
                    download_link: l["downloadLink"].as_str().unwrap().into(),
                })
                .collect()
        };
        for case in expected.as_array().unwrap() {
            let id = case["id"].as_i64().unwrap();
            let html = std::fs::read_to_string(dir.join(format!("{id}.html"))).unwrap();
            let parser = GcwParser::new(&html, ORIGIN);
            assert_eq!(
                parser.single(),
                to_links(&case["single"]),
                "single, post {id}"
            );
            if case.get("multiple").is_some() {
                assert_eq!(
                    parser.multiple_plain(),
                    to_links(&case["multiple"]),
                    "multiple, post {id}"
                );
            }
        }
    }
}
