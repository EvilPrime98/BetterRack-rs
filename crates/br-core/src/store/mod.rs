//! GetComics store client (`GetComicsApiModel`): WordPress REST posts, the weekly pack list and
//! download-link extraction. The store URL comes from the `apiUrl` setting on every call, so a
//! settings change takes effect immediately.
//!
//! Differences from Bun, all on paths that threw there: a post without a usable download link
//! yields no link (404 / "cannot be downloaded") instead of a `TypeError` 500, cached link lists are
//! returned as `{uuid, title}` only (Bun leaked `downloadLink` on a cache hit), and the unused
//! `getCoverFromPost` is not ported.

pub mod parser;

use crate::pixeldrain::PixelDrain;
use crate::rotating_fetch::{FetchError, abortable_sleep};
use crate::settings::Preferences;
use crate::{CoreError, Result};
use parser::{GcwParser, Link};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const LINKS_TTL: Duration = Duration::from_secs(60 * 60);
const LATEST_TTL: Duration = Duration::from_secs(5 * 60);
const POST_FIELDS: &str = "id,title,link,jetpack_featured_media_url,date";

/// `deriveStoreOrigin`: the origin of the configured store URL, empty when it does not parse.
pub fn derive_store_origin(api_url: &str) -> String {
    let t = api_url.trim();
    let Some((scheme, rest)) = t.split_once("://") else { return String::new() };
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..host_end];
    let host = authority.rsplit('@').next().unwrap_or("");
    let scheme_ok = !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c));
    if !scheme_ok || host.is_empty() {
        return String::new();
    }
    format!("{}://{}", scheme.to_ascii_lowercase(), host.to_ascii_lowercase())
}

/// `TStrat`: `single`, `multiple`, anything else behaves as `all`.
fn strat_kind(strat: &str) -> &str {
    match strat {
        "single" | "multiple" => strat,
        _ => "all",
    }
}

/// JS `String(number)` for the page numbers the controller forwards (`Number(q) || default`).
pub fn js_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 { format!("{}", n as i64) } else { format!("{n}") }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PostLink {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub title: String,
    pub link: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DownloadableObject {
    pub uuid: String,
    pub title: String,
}

#[derive(Debug, Clone)]
struct LinkEntry {
    uuid: String,
    title: String,
    download_link: String,
}

/// Delays and retry budget of the store client; tests shrink them.
#[derive(Debug, Clone)]
pub struct StoreTiming {
    pub max_retries: u32,
    pub backoff_base_ms: u64,
    pub backoff_cap_ms: u64,
    pub jitter_ms: u64,
    /// One `Retry-After` unit (seconds in production).
    pub retry_after_unit_ms: u64,
    pub random_delay_ms: (u64, u64),
}

impl Default for StoreTiming {
    fn default() -> Self {
        Self { max_retries: 4, backoff_base_ms: 2000, backoff_cap_ms: 30_000, jitter_ms: 1000, retry_after_unit_ms: 1000, random_delay_ms: (1200, 3000) }
    }
}

impl StoreTiming {
    pub fn instant() -> Self {
        Self { max_retries: 4, backoff_base_ms: 0, backoff_cap_ms: 0, jitter_ms: 0, retry_after_unit_ms: 0, random_delay_ms: (0, 0) }
    }
}

struct Ttl<T> {
    map: Mutex<HashMap<String, (Instant, T)>>,
}

impl<T: Clone> Ttl<T> {
    fn new() -> Self {
        Self { map: Mutex::new(HashMap::new()) }
    }

    fn get(&self, key: &str) -> Option<T> {
        let mut m = self.map.lock().unwrap_or_else(|e| e.into_inner());
        match m.get(key) {
            Some((expires, v)) if Instant::now() <= *expires => Some(v.clone()),
            Some(_) => {
                m.remove(key);
                None
            }
            None => None,
        }
    }

    fn set(&self, key: &str, value: T, ttl: Duration) {
        self.map.lock().unwrap_or_else(|e| e.into_inner()).insert(key.to_string(), (Instant::now() + ttl, value));
    }
}

pub struct StoreApi {
    prefs: Arc<Preferences>,
    client: reqwest::Client,
    pixel: Arc<PixelDrain>,
    timing: StoreTiming,
    links: Ttl<Vec<LinkEntry>>,
    latest: Ttl<Vec<PostLink>>,
}

fn net_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Invalid(e.to_string())
}

impl StoreApi {
    pub fn new(prefs: Arc<Preferences>, client: reqwest::Client, pixel: Arc<PixelDrain>, timing: StoreTiming) -> Self {
        Self { prefs, client, pixel, timing, links: Ttl::new(), latest: Ttl::new() }
    }

    fn api_url(&self) -> Result<String> {
        Ok(self.prefs.get_app_settings()?.api_url)
    }

    fn headers(&self, api_url: &str) -> HeaderMap {
        let origin = derive_store_origin(api_url);
        let mut h = HeaderMap::new();
        let mut put = |k: &'static str, v: &str| {
            if let Ok(v) = HeaderValue::from_str(v) {
                h.insert(HeaderName::from_static(k), v);
            }
        };
        put("user-agent", USER_AGENT);
        put("accept", "application/json, text/plain, */*");
        put("accept-language", "en-US,en;q=0.9");
        put("referer", &format!("{origin}/"));
        put("origin", &origin);
        put("dnt", "1");
        put("sec-fetch-dest", "empty");
        put("sec-fetch-mode", "cors");
        put("sec-fetch-site", "same-origin");
        h
    }

    async fn random_delay(&self, min: u64, max: u64) {
        let (min, max) = (min.min(max), max);
        abortable_sleep(min + fastrand::u64(0..=(max - min)), None).await;
    }

    /// GET with the store's headers; only a 429 is retried (honouring `Retry-After`). Transport
    /// errors are not retried, like Bun.
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let api_url = self.api_url()?;
        let mut last = None;
        for attempt in 0..=self.timing.max_retries {
            if attempt > 0 {
                let backoff = (2u64.saturating_pow(attempt) * self.timing.backoff_base_ms).min(self.timing.backoff_cap_ms) + fastrand::u64(0..=self.timing.jitter_ms);
                abortable_sleep(backoff, None).await;
            }
            let res = self.client.get(url).headers(self.headers(&api_url)).send().await.map_err(net_err)?;
            if res.status().as_u16() != 429 {
                return Ok(res);
            }
            if let Some(ra) = res.headers().get("retry-after").and_then(|v| v.to_str().ok()) {
                let secs = leading_int(ra).filter(|n| *n != 0).unwrap_or(10);
                let jitter = fastrand::u64(0..=(self.timing.retry_after_unit_ms / 2));
                abortable_sleep(secs.unsigned_abs() * self.timing.retry_after_unit_ms + jitter, None).await;
            }
            last = Some(res);
        }
        last.ok_or_else(|| net_err("store request failed"))
    }

    async fn get_json(&self, url: &str) -> Result<Option<Value>> {
        let res = self.fetch_with_retry(url).await?;
        if !res.status().is_success() {
            return Ok(None);
        }
        Ok(Some(res.json::<Value>().await.map_err(net_err)?))
    }

    fn normalize_title(title: &str) -> String {
        let once = html_escape::decode_html_entities(title);
        html_escape::decode_html_entities(&once).trim().to_string()
    }

    fn map_post(post: &Value) -> Result<PostLink> {
        let title = post["title"]["rendered"].as_str().ok_or_else(|| net_err("post without a title"))?;
        Ok(PostLink {
            id: post["id"].as_i64(),
            title: Self::normalize_title(title),
            link: post["link"].as_str().unwrap_or_default().to_string(),
            thumbnail_url: post["jetpack_featured_media_url"].as_str().map(str::to_string),
            upload_date: post["date"].as_str().map(str::to_string),
        })
    }

    fn map_posts(posts: &Value) -> Result<Vec<PostLink>> {
        posts.as_array().map(|a| a.iter().map(Self::map_post).collect()).unwrap_or(Ok(vec![]))
    }

    fn search_params(search: &str, per_page: Option<&str>) -> url::form_urlencoded::Serializer<'static, String> {
        let mut s = url::form_urlencoded::Serializer::new(String::new());
        s.append_pair("search", search).append_pair("_fields", POST_FIELDS);
        if let Some(p) = per_page.filter(|p| !p.is_empty() && *p != "0") {
            s.append_pair("per_page", p);
        }
        s
    }

    pub async fn get_post_links(&self, search: &str, page: &str, per_page: Option<&str>) -> Result<Vec<PostLink>> {
        let query = Self::search_params(search, per_page).append_pair("page", page).finish();
        let url = format!("{}/posts?{query}", self.api_url()?);
        match self.get_json(&url).await? {
            Some(posts) => Self::map_posts(&posts),
            None => Ok(vec![]),
        }
    }

    pub async fn get_post_links_for_pages(&self, search: &str, pages: &[i64], per_page: Option<&str>) -> Result<Vec<PostLink>> {
        let base = Self::search_params(search, per_page).finish();
        let mut results = vec![];
        for page in pages {
            self.random_delay(self.timing.random_delay_ms.0, self.timing.random_delay_ms.1).await;
            let url = format!("{}/posts?{base}&page={page}", self.api_url()?);
            if let Some(posts) = self.get_json(&url).await? {
                results.extend(Self::map_posts(&posts)?);
            }
        }
        Ok(results)
    }

    pub async fn get_latest(&self, page: &str, per_page: &str) -> Result<Vec<PostLink>> {
        let key = format!("latest_{page}_{per_page}");
        if let Some(hit) = self.latest.get(&key).filter(|h| !h.is_empty()) {
            return Ok(hit);
        }
        let params = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("orderby", "date")
            .append_pair("order", "desc")
            .append_pair("per_page", per_page)
            .append_pair("page", page)
            .append_pair("_fields", POST_FIELDS)
            .finish();
        let url = format!("{}/posts?{params}", self.api_url()?);
        let Some(posts) = self.get_json(&url).await? else { return Ok(vec![]) };
        let result = Self::map_posts(&posts)?;
        if !result.is_empty() {
            self.latest.set(&key, result.clone(), LATEST_TTL);
        }
        Ok(result)
    }

    /// The `<li><strong>Title : <span><a href="https://getcomics.org/...">Download</a>` items of
    /// the weekly pack post, optionally restricted to the `<h3><span>group</span></h3>` section.
    fn parse_weekly_list(html: &str, group: Option<&str>) -> Vec<(String, String)> {
        let section;
        let search_html = match group.filter(|g| !g.is_empty()) {
            Some(group) => {
                let heading = Regex::new(r"(?i)<h3><span[^>]*>([^<]+)</span></h3>").expect("heading regex");
                let lower = html.to_ascii_lowercase();
                let wanted = group.to_lowercase();
                let found = heading.captures_iter(html).find(|c| c[1].trim().to_lowercase().contains(&wanted));
                let Some(c) = found else { return vec![] };
                let start = c.get(0).map_or(0, |m| m.end());
                let end = lower[start..].find("<h3>").map_or(html.len(), |i| start + i);
                section = &html[start..end];
                section
            }
            None => html,
        };
        let item = Regex::new(r#"(?i)<li><strong>(.*?)\s*:\s*<span[^>]*>\s*<a[^>]+href="(https://getcomics\.org/[^"]+)"[^>]*>Download</a>"#).expect("item regex");
        item.captures_iter(search_html).map(|c| (Self::normalize_title(&c[1]), c[2].to_string())).collect()
    }

    fn slug_of(link: &str) -> Option<String> {
        url::Url::parse(link).ok()?.path_segments()?.filter(|s| !s.is_empty()).next_back().map(str::to_string)
    }

    pub async fn get_weekly_list_posts(&self, group: Option<&str>) -> Result<Vec<PostLink>> {
        let api = self.api_url()?;
        let Some(list) = self.get_json(&format!("{api}/posts?search=weekly-pack&per_page=1&_fields=id,content")).await? else { return Ok(vec![]) };
        let Some(content) = list.as_array().and_then(|a| a.first()).and_then(|p| p["content"]["rendered"].as_str()) else { return Ok(vec![]) };
        let parsed = Self::parse_weekly_list(content, group);
        if parsed.is_empty() {
            return Ok(vec![]);
        }
        let slugs: Vec<String> = parsed.iter().filter_map(|(_, link)| Self::slug_of(link)).collect();
        self.random_delay(self.timing.random_delay_ms.0, self.timing.random_delay_ms.1).await;
        let plain = |parsed: Vec<(String, String)>| -> Vec<PostLink> {
            parsed.into_iter().map(|(title, link)| PostLink { id: None, title, link, thumbnail_url: None, upload_date: None }).collect()
        };
        let Some(posts) = self.get_json(&format!("{api}/posts?slug={}&_fields={POST_FIELDS}&per_page=100", slugs.join(","))).await? else { return Ok(plain(parsed)) };
        let by_slug: HashMap<String, &Value> = posts
            .as_array()
            .map(|a| a.iter().filter_map(|p| Some((Self::slug_of(p["link"].as_str()?)?, p))).collect())
            .unwrap_or_default();
        Ok(parsed
            .into_iter()
            .map(|(title, link)| {
                let post = Self::slug_of(&link).and_then(|s| by_slug.get(&s).copied());
                PostLink {
                    id: post.and_then(|p| p["id"].as_i64()),
                    title,
                    link,
                    thumbnail_url: post.and_then(|p| p["jetpack_featured_media_url"].as_str()).map(str::to_string),
                    upload_date: post.and_then(|p| p["date"].as_str()).map(str::to_string),
                }
            })
            .collect())
    }

    /// Run the strategies over a post's HTML. `all` tries the multi-issue strategies first, then
    /// the single-issue ones.
    pub async fn parse_links(&self, html: &str, strat: &str) -> Result<Vec<Link>> {
        let origin = derive_store_origin(&self.api_url()?);
        let parser = GcwParser::new(html, &origin);
        let kind = strat_kind(strat);
        if kind == "single" {
            return Ok(parser.single());
        }
        let mut found = self.multiple_pixeldrain(&parser).await;
        if found.is_empty() {
            found = parser.multiple_plain();
        }
        if kind == "multiple" || !found.is_empty() {
            return Ok(found);
        }
        Ok(parser.single())
    }

    async fn multiple_pixeldrain(&self, parser: &GcwParser) -> Vec<Link> {
        let candidates = parser.pixeldrain_candidates();
        let resolved = futures_util::future::join_all(candidates.into_iter().map(|(title, masked)| {
            let pixel = self.pixel.clone();
            async move {
                let Ok(files) = pixel.resolve(&masked).await else { return vec![] };
                let many = files.len() > 1;
                files
                    .into_iter()
                    .filter(|f| f.can_download)
                    .map(|f| Link {
                        title: if many { parser::normalize_text(&format!("{title} - {}", f.name)) } else { title.clone() },
                        download_link: f.download_link,
                    })
                    .collect::<Vec<_>>()
            }
        }))
        .await;
        parser.normalize_links(resolved.into_iter().flatten().collect())
    }

    fn links_key(post_id: i64, strat: &str) -> String {
        format!("download_link_{post_id}_{strat}")
    }

    /// The downloadable links of a post, each tagged with a fresh uuid that is later passed back
    /// to [`StoreApi::get_download_link_from_post`]. `None` stands for a store failure (non-2xx).
    pub async fn get_download_links(&self, post_id: i64, strat: &str) -> Result<Vec<DownloadableObject>> {
        let key = Self::links_key(post_id, strat);
        let to_objects = |l: &[LinkEntry]| l.iter().map(|e| DownloadableObject { uuid: e.uuid.clone(), title: e.title.clone() }).collect();
        if let Some(hit) = self.links.get(&key).filter(|h| !h.is_empty()) {
            return Ok(to_objects(&hit));
        }
        let url = format!("{}/posts/{post_id}?_fields=content,jetpack_featured_media_url", self.api_url()?);
        let Some(post) = self.get_json(&url).await? else { return Ok(vec![]) };
        let html = post["content"]["rendered"].as_str().ok_or_else(|| net_err("post without content"))?;
        let links = self.parse_links(html, strat).await?;
        if links.is_empty() {
            return Ok(vec![]);
        }
        let entries: Vec<LinkEntry> = links
            .into_iter()
            .map(|l| LinkEntry { uuid: uuid::Uuid::new_v4().to_string(), title: l.title, download_link: l.download_link })
            .collect();
        self.links.set(&key, entries.clone(), LINKS_TTL);
        Ok(to_objects(&entries))
    }

    /// The direct link for `uuid` (from the cache of [`StoreApi::get_download_links`]); if the
    /// cache entry is gone, the post is parsed again and its first link is used.
    pub async fn get_download_link_from_post(&self, post_id: i64, strat: Option<&str>, uuid: Option<&str>) -> Result<Option<String>> {
        let key = Self::links_key(post_id, strat.unwrap_or("undefined"));
        if let Some(hit) = self.links.get(&key).and_then(|c| c.into_iter().find(|e| Some(e.uuid.as_str()) == uuid)) {
            return Ok(Some(hit.download_link));
        }
        let url = format!("{}/posts/{post_id}?_fields=content", self.api_url()?);
        let Some(post) = self.get_json(&url).await? else { return Ok(None) };
        let html = post["content"]["rendered"].as_str().ok_or_else(|| net_err("post without content"))?;
        Ok(self.parse_links(html, "all").await?.into_iter().next().map(|l| l.download_link))
    }
}

/// JS `parseInt` on a decimal string: optional sign, leading digits, anything after is ignored.
pub fn leading_int(s: &str) -> Option<i64> {
    let t = s.trim_start();
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let n: i64 = digits.parse().ok()?;
    Some(if neg { -n } else { n })
}

impl From<FetchError> for CoreError {
    fn from(e: FetchError) -> Self {
        CoreError::Invalid(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_origin() {
        assert_eq!(derive_store_origin(" https://Getcomics.org/api/x "), "https://getcomics.org");
        assert_eq!(derive_store_origin("http://host:8080/p"), "http://host:8080");
        assert_eq!(derive_store_origin(""), "");
        assert_eq!(derive_store_origin("not a url"), "");
        assert_eq!(derive_store_origin("https://"), "");
    }

    #[test]
    fn js_helpers() {
        assert_eq!(js_number(2.0), "2");
        assert_eq!(js_number(2.5), "2.5");
        assert_eq!(leading_int("12abc"), Some(12));
        assert_eq!(leading_int(" -3"), Some(-3));
        assert_eq!(leading_int("abc"), None);
    }

    const WEEKLY: &str = r#"<h3><span style="x">DC Comics</span></h3>
<ul><li><strong>Batman #1 &amp;amp; Co : <span class="a"> <a href="https://getcomics.org/dc/batman-1/">Download</a></span></strong></li>
<li><strong>Robin #2 : <span> <a href="https://getcomics.org/dc/robin-2/">Download</a></span></strong></li></ul>
<h3><span>Marvel</span></h3>
<ul><li><strong>Thor #3 : <span> <a href="https://getcomics.org/marvel/thor-3/">Download</a></span></strong></li></ul>"#;

    #[test]
    fn weekly_list_whole_and_by_group() {
        let all = StoreApi::parse_weekly_list(WEEKLY, None);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], ("Batman #1 & Co".to_string(), "https://getcomics.org/dc/batman-1/".to_string()));
        let marvel = StoreApi::parse_weekly_list(WEEKLY, Some("marvel"));
        assert_eq!(marvel, [("Thor #3".to_string(), "https://getcomics.org/marvel/thor-3/".to_string())]);
        let dc = StoreApi::parse_weekly_list(WEEKLY, Some("dc"));
        assert_eq!(dc.len(), 2);
        assert!(StoreApi::parse_weekly_list(WEEKLY, Some("image")).is_empty());
    }

    #[test]
    fn slugs() {
        assert_eq!(StoreApi::slug_of("https://getcomics.org/dc/batman-1/").as_deref(), Some("batman-1"));
    }

    /// Real weekly-pack post (saved 2026-10-04); expected lists are Bun's `parseWeeklyListPostLinks`.
    #[test]
    fn weekly_list_matches_bun_on_a_real_post() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/store");
        let html = std::fs::read_to_string(dir.join("weekly.html")).unwrap();
        let expected: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("weekly_expected.json")).unwrap()).unwrap();
        for (group, want) in expected.as_object().unwrap() {
            let got = StoreApi::parse_weekly_list(&html, (!group.is_empty()).then_some(group.as_str()));
            let want: Vec<(String, String)> = want.as_array().unwrap().iter().map(|l| (l["title"].as_str().unwrap().into(), l["link"].as_str().unwrap().into())).collect();
            assert_eq!(got, want, "group {group:?}");
        }
    }
}
