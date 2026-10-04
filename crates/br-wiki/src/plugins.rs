//! The `dc-fandom` and `marvel-fandom` plugins of `better-wiki`, limited to what BetterRack
//! calls: `getComic` (single best match and `multiple`) and `getComicById`. Output is the
//! `WikiComic` JSON stored in `comic_data.comic`, key for key and in the same
//! order. Volumes, characters and appearances are not used by the server.

use crate::client::{Page, PageFlags, WikiClient};
use crate::fuse::{self, js_trim};
use crate::template::{self as t, Content};
use crate::{Result, jsort};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

static LEADING_ZEROES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u:\b)0+([0-9])").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static PARENTHESISED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(.+\)").unwrap());
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(([0-9]{4})\)").unwrap());
static TRAILING_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([0-9]+)$").unwrap());
static NON_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^A-Za-z0-9]+").unwrap());
static VOL_ISSUE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Vol\.?\s*([0-9]+)\s+([0-9]+[A-Za-z]*)\s*$").unwrap());
static COVER_INDEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^Image([0-9]+)$").unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plugin {
    Dc,
    Marvel,
}

/// The DC single-match lookup builds only these, in this order.
const DC_SINGLE_FIELDS: [&str; 8] = [
    "cover",
    "credits",
    "pageId",
    "issue",
    "title",
    "volume",
    "releaseDate",
    "sourceWiki",
];

const DC_FIELDS: [&str; 17] = [
    "title",
    "volume",
    "issue",
    "cover",
    "pageId",
    "releaseDate",
    "credits",
    "synopsis",
    "rating",
    "event",
    "storyTitles",
    "appearing",
    "quotation",
    "coverVariants",
    "notes",
    "trivia",
    "sourceWiki",
];

pub struct PluginClient {
    pub client: WikiClient,
    pub plugin: Plugin,
}

/// A search hit with its infobox.
struct Candidate {
    page: Page,
    content: Content,
}

impl PluginClient {
    /// `getComic(title, { thumbnailSize, includeCollections: true [, fields] })`: the best match.
    pub async fn find_comic(
        &self,
        title: &str,
        thumbnail_size: Option<&str>,
    ) -> Result<Option<Value>> {
        let norm = pre_normalization(title);
        let candidates = self.candidates(&norm, None, thumbnail_size).await?;
        if candidates.is_empty() {
            return Ok(None);
        }
        let Some(best) = self.select_best(candidates, &norm, extract_year(title)) else {
            return Ok(None);
        };
        let cover = self.cover(&best.content, thumbnail_size).await?;
        let fields = (self.plugin == Plugin::Dc).then_some(&DC_SINGLE_FIELDS[..]);
        Ok(Some(self.build_comic(
            &best.page,
            &best.content,
            fields,
            cover,
        )))
    }

    /// `getComic(title, { multiple: true, ... })`: every hit, all fields.
    pub async fn find_comics(
        &self,
        title: &str,
        thumbnail_size: Option<&str>,
    ) -> Result<Vec<Value>> {
        let candidates = self
            .candidates(&pre_normalization(title), Some(50), thumbnail_size)
            .await?;
        let covers = futures_util::future::try_join_all(
            candidates
                .iter()
                .map(|c| self.cover(&c.content, thumbnail_size)),
        )
        .await?;
        Ok(candidates
            .iter()
            .zip(covers)
            .map(|(c, cover)| self.build_comic(&c.page, &c.content, None, cover))
            .collect())
    }

    /// `getComicById(id, { thumbnailSize })`.
    pub async fn find_comic_by_id(
        &self,
        id: i64,
        thumbnail_size: Option<&str>,
    ) -> Result<Option<Value>> {
        let Some(page) = self.client.get_page_by_id(id, thumbnail_size).await? else {
            return Ok(None);
        };
        let content = self.client.get_structured_content(&page).await?;
        let cover = self.cover(&content, thumbnail_size).await?;
        Ok(Some(self.build_comic(&page, &content, None, cover)))
    }

    async fn candidates(
        &self,
        norm_query: &str,
        limit: Option<usize>,
        thumbnail_size: Option<&str>,
    ) -> Result<Vec<Candidate>> {
        let flags = PageFlags {
            category: vec![],
            categories_or: vec![
                "Category:Comics".into(),
                "Category:Collected Editions".into(),
            ],
            limit,
            thumbnail_size: thumbnail_size.map(str::to_string),
        };
        let pages = self.client.get_page(norm_query, &flags).await?;
        let contents = futures_util::future::try_join_all(
            pages.iter().map(|p| self.client.get_structured_content(p)),
        )
        .await?;
        Ok(pages
            .into_iter()
            .zip(contents)
            .map(|(page, content)| Candidate { page, content })
            .collect())
    }

    /// The cover file named by the infobox, resolved to a URL. DC keeps it in `Image`, Marvel in `Image1`.
    async fn cover(
        &self,
        content: &Content,
        thumbnail_size: Option<&str>,
    ) -> Result<Option<String>> {
        let field = if self.plugin == Plugin::Dc {
            "Image"
        } else {
            "Image1"
        };
        let Some(name) = t::get(content, field).filter(|n| !n.is_empty()) else {
            return Ok(None);
        };
        Ok(Some(self.client.get_file_url(name, thumbnail_size).await?).filter(|u| !u.is_empty()))
    }

    /// `selectBest`: fuzzy title match, then bonuses for the year, the release month/day and a
    /// trailing issue number in the title. Ties and `NaN` scores resolve as V8's sort does.
    fn select_best(
        &self,
        candidates: Vec<Candidate>,
        norm_query: &str,
        query_year: Option<String>,
    ) -> Option<Candidate> {
        let titles: Vec<&str> = candidates.iter().map(|c| c.page.title.as_str()).collect();
        let hits = fuse::search(&titles, norm_query);
        if hits.is_empty() {
            return None;
        }
        let query_number = TRAILING_NUMBER
            .captures(norm_query)
            .map(|c| c[1].to_string());
        let mut scored: Vec<(usize, f64)> = hits
            .iter()
            .map(|hit| {
                let Candidate { page, content } = &candidates[hit.idx];
                let mut score = (1.0 - hit.score.unwrap_or(1.0)) * 100.0;
                if query_year
                    .as_deref()
                    .is_some_and(|y| t::get(content, "Year") == Some(y))
                {
                    score += 40.0;
                }
                if query_year.is_none() {
                    let (month, day) = match self.plugin {
                        Plugin::Dc => (
                            t::js_parse_int(t::get(content, "Month").unwrap_or("0")),
                            t::js_parse_int(t::get(content, "Day").unwrap_or("0")),
                        ),
                        Plugin::Marvel => {
                            let d = marvel_release_date(t::get(content, "ReleaseDate"));
                            (t::js_number(&d.1), t::js_number(&d.0))
                        }
                    };
                    score += month * 0.5;
                    score += day * 0.1;
                }
                if let Some(n) = &query_number {
                    let normalised = normalize(&page.title);
                    if Regex::new(&format!(r"(?-u:\b){n}(?-u:\b)"))
                        .is_ok_and(|re| re.is_match(&normalised))
                    {
                        score += 25.0;
                    }
                }
                (hit.idx, score)
            })
            .collect();
        jsort::sort_by(&mut scored, |a, b| b.1 - a.1);
        let best = scored.first()?.0;
        candidates.into_iter().nth(best)
    }

    fn build_comic(
        &self,
        page: &Page,
        content: &Content,
        fields: Option<&[&str]>,
        cover: Option<String>,
    ) -> Value {
        match self.plugin {
            Plugin::Dc => build_dc(page, content, fields, cover),
            Plugin::Marvel => build_marvel(page, content, cover),
        }
    }
}

/// Drop the extension, leading zeroes, double spaces and parenthesised text from a file name.
pub fn pre_normalization(candidate: &str) -> String {
    let s = candidate.split('.').next().unwrap_or_default();
    let s = LEADING_ZEROES.replace_all(s, "$1");
    let s = SPACES.replace_all(&s, " ");
    let s = PARENTHESISED.replace(&s, "");
    js_trim(&s).to_string()
}

fn extract_year(candidate: &str) -> Option<String> {
    YEAR.captures(candidate).map(|c| c[1].to_string())
}

fn normalize(s: &str) -> String {
    let lower = s.to_lowercase();
    let s = NON_WORD.replace_all(&lower, " ");
    js_trim(&SPACES.replace_all(&s, " ")).to_string()
}

fn month_number(name: &str) -> Option<&'static str> {
    Some(match name {
        "default" => "00",
        "January" => "01",
        "February" => "02",
        "March" => "03",
        "April" => "04",
        "May" => "05",
        "June" => "06",
        "July" => "07",
        "August" => "08",
        "September" => "09",
        "October" => "10",
        "November" => "11",
        "December" => "12",
        _ => return None,
    })
}

fn page_id_or_minus_one(page: &Page) -> i64 {
    if page.id == 0 { -1 } else { page.id }
}

/// Insert unless the builder returned `undefined`.
fn put(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(v) = value {
        map.insert(key.to_string(), v);
    }
}

fn quotation(quote: Option<&str>, speaker: Option<&str>) -> Option<Value> {
    let quote = quote.filter(|q| !q.is_empty());
    let speaker = speaker.filter(|s| !s.is_empty());
    if quote.is_none() && speaker.is_none() {
        return None;
    }
    let mut q = Map::new();
    put(&mut q, "quote", quote.map(Value::from));
    put(&mut q, "speaker", speaker.map(Value::from));
    Some(Value::Object(q))
}

fn cover_or_thumbnail(page: &Page, cover: &Option<String>) -> String {
    cover
        .clone()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| page.thumbnail.clone())
}

fn dc_release_date(content: &Content) -> Value {
    let day = t::get(content, "Day")
        .map(js_trim)
        .filter(|d| !d.is_empty());
    let month = t::get(content, "Month")
        .map(js_trim)
        .filter(|m| !m.is_empty());
    let year = t::get(content, "Year").map(js_trim).unwrap_or_default();
    json!({
        "releaseDay": day.map(t::pad2).unwrap_or_default(),
        "releaseMonth": month.and_then(month_number).unwrap_or_default(),
        "releaseYear": year,
    })
}

fn dc_cover_variants(content: &Content) -> Value {
    let mut variants = Vec::new();
    for c in 1usize.. {
        let prefix = if c == 1 {
            "CoverArtist".to_string()
        } else {
            format!("Cover{c}Artist")
        };
        if !content.contains_key(&format!("{prefix}1")) {
            break;
        }
        variants.push(json!({"coverNumber": c, "artists": t::collect_sequential(content, |n| format!("{prefix}{n}"))}));
    }
    Value::Array(variants)
}

fn build_dc(
    page: &Page,
    content: &Content,
    fields: Option<&[&str]>,
    cover: Option<String>,
) -> Value {
    let field = |name: &str| -> Option<Value> {
        Some(match name {
            "title" => page.title.clone().into(),
            "volume" => t::text(content, "Volume").into(),
            "issue" => t::text(content, "Issue").into(),
            "cover" => cover_or_thumbnail(page, &cover).into(),
            "pageId" => page_id_or_minus_one(page).into(),
            "releaseDate" => dc_release_date(content),
            "credits" => t::build_credits(content),
            "synopsis" => t::collect_sequential(content, |i| format!("Synopsis{i}"))
                .join("\n\n")
                .into(),
            "rating" => t::text(content, "Rating").into(),
            "event" => t::text(content, "Event").into(),
            "storyTitles" => t::collect_sequential(content, |i| format!("StoryTitle{i}")).into(),
            "appearing" => t::parse_appearing(t::get(content, "Appearing1")),
            "quotation" => {
                return quotation(t::get(content, "Quotation"), t::get(content, "Speaker"));
            }
            "coverVariants" => dc_cover_variants(content),
            "notes" => t::parse_bullets(t::get(content, "Notes")).into(),
            "trivia" => t::parse_bullets(t::get(content, "Trivia")).into(),
            "sourceWiki" => page.source_wiki.clone().into(),
            _ => return None,
        })
    };
    let mut out = Map::new();
    for name in fields.unwrap_or(&DC_FIELDS[..]) {
        put(&mut out, name, field(name));
    }
    Value::Object(out)
}

/// `parseReleaseDate` of the combined `ReleaseDate` field (`"June 6, 2018"`) as `(day, month, year)`.
/// A missing field gives month `"00"` (the `monthMap.default` lookup), an empty one gives `""`.
fn marvel_release_date(raw: Option<&str>) -> (String, String, String) {
    let Some(raw) = raw else {
        return (String::new(), "00".into(), String::new());
    };
    let mut parts = raw.split(' ');
    let (month, day, year) = (parts.next(), parts.next(), parts.next());
    (
        day.map(|d| t::pad2(&d.replacen(',', "", 1)))
            .unwrap_or_default(),
        month.and_then(month_number).unwrap_or_default().to_string(),
        year.unwrap_or_default().to_string(),
    )
}

fn marvel_cover_variants(content: &Content) -> Value {
    let mut indices: Vec<f64> = content
        .keys()
        .filter_map(|k| COVER_INDEX.captures(k))
        .filter_map(|c| c[1].parse::<f64>().ok())
        .collect();
    indices.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Value::Array(
        indices
            .into_iter()
            .map(|c| {
                let n = t::js_number_string(c);
                let label = t::get(content, &format!("Image{n}_Text"))
                    .map(js_trim)
                    .filter(|l| !l.is_empty());
                let mut v = Map::new();
                v.insert("coverNumber".into(), json!(c as i64));
                v.insert(
                    "artists".into(),
                    t::collect_sequential(content, |i| format!("Image{n}_Artist{i}")).into(),
                );
                put(&mut v, "imageLabel", label.map(Value::from));
                Value::Object(v)
            })
            .collect(),
    )
}

/// Marvel infoboxes carry no `Volume`/`Issue`; they are read off the title (`... Vol 1 25A`).
fn volume_issue_from_title(title: &str) -> (String, String) {
    VOL_ISSUE
        .captures(title)
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .unwrap_or_default()
}

fn build_marvel(page: &Page, content: &Content, cover: Option<String>) -> Value {
    let (day, month, year) = marvel_release_date(t::get(content, "ReleaseDate"));
    let speaker = t::extract_speaker(t::get(content, "Speaker"));
    let (fallback_volume, fallback_issue) = volume_issue_from_title(&page.title);
    let or = |key: &str, fallback: String| {
        t::get(content, key)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .unwrap_or(fallback)
    };

    let mut out = Map::new();
    out.insert("title".into(), page.title.clone().into());
    out.insert("volume".into(), or("Volume", fallback_volume).into());
    out.insert("issue".into(), or("Issue", fallback_issue).into());
    out.insert("cover".into(), cover_or_thumbnail(page, &cover).into());
    out.insert("pageId".into(), page_id_or_minus_one(page).into());
    out.insert(
        "releaseDate".into(),
        json!({"releaseDay": day, "releaseMonth": month, "releaseYear": year}),
    );
    out.insert("credits".into(), t::build_credits(content));
    out.insert(
        "synopsis".into(),
        t::collect_sequential(content, |i| format!("Synopsis{i}"))
            .join("\n\n")
            .into(),
    );
    out.insert("rating".into(), t::text(content, "Rating").into());
    out.insert("event".into(), t::text(content, "Event1").into());
    out.insert(
        "storyTitles".into(),
        t::collect_sequential(content, |i| format!("StoryTitle{i}")).into(),
    );
    out.insert(
        "appearing".into(),
        t::parse_appearing(t::get(content, "Appearing1")),
    );
    put(
        &mut out,
        "quotation",
        quotation(t::get(content, "Quotation"), speaker.as_deref()),
    );
    out.insert("coverVariants".into(), marvel_cover_variants(content));
    out.insert(
        "notes".into(),
        t::parse_bullets(t::get(content, "Notes")).into(),
    );
    out.insert(
        "trivia".into(),
        t::parse_bullets(t::get(content, "Trivia")).into(),
    );
    out.insert("sourceWiki".into(), page.source_wiki.clone().into());
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pre_normalization_matches_the_js_chain() {
        assert_eq!(pre_normalization("Batman 001 (2016).cbz"), "Batman 1");
        assert_eq!(pre_normalization("The   Flash  010.cbr"), "The Flash 10");
        assert_eq!(
            pre_normalization("Spider-Man Vol. 2 001.cbz"),
            "Spider-Man Vol"
        );
        assert_eq!(pre_normalization("X-Men 100"), "X-Men 100");
        assert_eq!(pre_normalization("Saga 000"), "Saga 0");
    }

    #[test]
    fn year_and_normalize() {
        assert_eq!(extract_year("Batman 1 (2016).cbz").as_deref(), Some("2016"));
        assert_eq!(extract_year("Batman 1 (16)"), None);
        assert_eq!(normalize("Batman: Vol 1 -- #404!"), "batman vol 1 404");
    }

    #[test]
    fn marvel_dates() {
        assert_eq!(
            marvel_release_date(Some("June 6, 2018")),
            ("06".into(), "06".into(), "2018".into())
        );
        assert_eq!(
            marvel_release_date(None),
            (String::new(), "00".into(), String::new())
        );
        assert_eq!(
            marvel_release_date(Some("")),
            (String::new(), String::new(), String::new())
        );
        assert_eq!(
            marvel_release_date(Some("Spring 1999")),
            ("1999".into(), String::new(), String::new())
        );
        assert_eq!(
            volume_issue_from_title("Immortal Hulk Vol 1 25A"),
            ("1".into(), "25A".into())
        );
        assert_eq!(
            volume_issue_from_title("Something else"),
            (String::new(), String::new())
        );
    }
}
