//! The MediaWiki side of `better-wiki` (`wiki()` in `better-wiki.js`): a cached, retrying
//! `api.php` client and the page queries the comic plugins are built on.
//!
//! Request URLs are byte-identical to the JS client's (`format=json&origin=*` first, parameters
//! in the same order, `URLSearchParams` encoding), so a recorded session replays against either.

use crate::template::{Content, parse_media_wiki_template};
use crate::{Result, WikiError};
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const USER_AGENT: &str = "better-wiki (https://www.npmjs.com/package/better-wiki)";
const CHUNK: usize = 50;

static SCALE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/scale-to-width-down/[0-9]+").unwrap());

/// Network behaviour; `Default` is better-wiki's (15 s timeout, 2 retries, 5 min cache).
#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub cache_ttl: Duration,
    pub timeout: Duration,
    pub retries: u32,
    /// Delay before retry `n` (0-based) is `backoff_base * 2^n`.
    pub backoff_base: Duration,
    /// Send API requests here instead of to the wiki URL (tests point it at a mock while the
    /// wiki URL stays the `sourceWiki` identity).
    pub api_base: Option<String>,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self { cache_ttl: Duration::from_secs(300), timeout: Duration::from_secs(15), retries: 2, backoff_base: Duration::from_millis(250), api_base: None }
    }
}

/// A page as `buildPage` exposes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub id: i64,
    pub title: String,
    /// Already scaled to the requested width; empty when the page has no image.
    pub thumbnail: String,
    pub categories: Vec<String>,
    pub source_wiki: String,
}

#[derive(Debug, Clone, Default)]
pub struct PageFlags {
    /// Every one of these categories must be present.
    pub category: Vec<String>,
    /// At least one of these categories must be present.
    pub categories_or: Vec<String>,
    pub limit: Option<usize>,
    /// JS-formatted width (`"120"`); `None` removes any `scale-to-width-down` segment.
    pub thumbnail_size: Option<String>,
}

pub struct WikiClient {
    http: reqwest::Client,
    wiki_url: String,
    opts: ClientOptions,
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    entries: HashMap<String, (Instant, Value)>,
    order: VecDeque<String>,
}

impl WikiClient {
    pub fn new(wiki_url: &str, http: reqwest::Client, opts: ClientOptions) -> Self {
        Self { http, wiki_url: wiki_url.to_string(), opts, cache: Mutex::new(Cache::default()) }
    }

    pub fn wiki_url(&self) -> &str {
        &self.wiki_url
    }

    fn api_url(&self, params: &[(&str, String)]) -> String {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query.append_pair("format", "json").append_pair("origin", "*");
        for (k, v) in params {
            query.append_pair(k, v);
        }
        format!("{}/api.php?{}", self.opts.api_base.as_deref().unwrap_or(&self.wiki_url), query.finish())
    }

    async fn fetch(&self, url: &str) -> Result<Value> {
        let mut last = WikiError::Http(format!("API request failed for {url}"));
        for attempt in 0..=self.opts.retries {
            let sent = self.http.get(url).header("User-Agent", USER_AGENT).timeout(self.opts.timeout).send().await;
            let outcome = match sent {
                Ok(res) if res.status().is_success() => res.json::<Value>().await.map_err(|e| WikiError::Http(e.to_string())),
                Ok(res) => Err(WikiError::Http(format!("API request failed: {}", res.status()))),
                Err(e) => Err(WikiError::Http(e.to_string())),
            };
            match outcome {
                Ok(v) => return Ok(v),
                Err(e) => {
                    tracing::debug!(attempt = attempt + 1, url, err = %e, "wiki fetch failed");
                    last = e;
                }
            }
            if attempt < self.opts.retries {
                tokio::time::sleep(self.opts.backoff_base * 2u32.pow(attempt)).await;
            }
        }
        Err(last)
    }

    async fn get(&self, params: &[(&str, String)]) -> Result<Value> {
        let url = self.api_url(params);
        let now = Instant::now();
        if let Some((at, data)) = self.cache.lock().unwrap().entries.get(&url) {
            if now.duration_since(*at) < self.opts.cache_ttl {
                return Ok(data.clone());
            }
        }
        let data = self.fetch(&url).await?;
        let mut cache = self.cache.lock().unwrap();
        if cache.entries.insert(url.clone(), (now, data.clone())).is_none() {
            cache.order.push_back(url);
        }
        if cache.entries.len() > 1000 {
            for _ in 0..500 {
                if let Some(oldest) = cache.order.pop_front() {
                    cache.entries.remove(&oldest);
                }
            }
        }
        Ok(data)
    }

    /// `getPage(query, flags)`: a generator search in namespace 0, filtered by category.
    pub async fn get_page(&self, query: &str, flags: &PageFlags) -> Result<Vec<Page>> {
        let targets: Vec<String> = flags.category.iter().chain(&flags.categories_or).cloned().collect();
        let limit = flags.limit.filter(|l| *l > 0);
        let gsrlimit = limit.unwrap_or(20);
        let mut params: Vec<(&str, String)> = vec![
            ("action", "query".into()),
            ("generator", "search".into()),
            ("gsrsearch", query.into()),
            ("gsrnamespace", "0".into()),
            ("gsrlimit", gsrlimit.to_string()),
            ("prop", if targets.is_empty() { "info|pageimages" } else { "info|pageimages|categories" }.into()),
            ("inprop", "url".into()),
            ("piprop", "thumbnail".into()),
            ("pithumbsize", "200".into()),
        ];
        if !targets.is_empty() {
            params.push(("cllimit", "max".into()));
            params.push(("clcategories", targets.join("|")));
        }

        let data = self.get(&params).await?;
        if data.get("query").is_none() {
            return Ok(vec![]);
        }
        let truncated = data["continue"].get("clcontinue").is_some();
        let mut pages = self.pages_from(&js_values(&data["query"]["pages"]), flags, truncated).await?;

        if !targets.is_empty() {
            let mut next_offset = data["continue"]["gsroffset"].as_i64();
            let (mut wave_size, mut waves) = (4i64, 0);
            while let (Some(limit), Some(offset)) = (limit, next_offset) {
                if pages.len() >= limit || waves >= 10 {
                    break;
                }
                let fetches = (0..wave_size).map(|i| {
                    let mut p = params.clone();
                    p.push(("gsroffset", (offset + i * gsrlimit as i64).to_string()));
                    async move { self.get(&p).await }
                });
                let results = futures_util::future::try_join_all(fetches).await?;
                next_offset = None;
                for res in results {
                    if res.get("query").is_none() {
                        break;
                    }
                    let truncated = res["continue"].get("clcontinue").is_some();
                    pages.extend(self.pages_from(&js_values(&res["query"]["pages"]), flags, truncated).await?);
                    match res["continue"]["gsroffset"].as_i64() {
                        Some(n) => next_offset = Some(n),
                        None => break,
                    }
                }
                wave_size = (wave_size * 2).min(16);
                waves += 1;
            }
        } else {
            let mut data = data;
            while limit.is_some_and(|l| pages.len() < l) {
                let Some(offset) = data["continue"]["gsroffset"].as_i64() else { break };
                let mut p = params.clone();
                p.push(("gsroffset", offset.to_string()));
                if let Some(c) = data["continue"].get("continue").and_then(Value::as_str) {
                    p.push(("continue", c.to_string()));
                }
                data = self.get(&p).await?;
                if data.get("query").is_none() {
                    break;
                }
                pages.extend(self.pages_from(&js_values(&data["query"]["pages"]), flags, false).await?);
            }
        }

        if let Some(limit) = limit {
            pages.truncate(limit);
        }
        Ok(pages)
    }

    /// `getWikiPagesFromPages`: build pages from raw generator results and apply the category filters.
    async fn pages_from(&self, raw: &[&Value], flags: &PageFlags, categories_truncated: bool) -> Result<Vec<Page>> {
        let targets: Vec<String> = flags.category.iter().chain(&flags.categories_or).cloned().collect();
        let ids_to_refetch: Vec<i64> = if !targets.is_empty() {
            if categories_truncated { raw.iter().map(|p| page_id(p)).collect() } else { vec![] }
        } else {
            raw.iter().filter(|p| p.get("categories").is_none()).map(|p| page_id(p)).collect()
        };
        let fetched = self.categories_for_pages(&ids_to_refetch, (!targets.is_empty()).then_some(&targets[..])).await?;

        let mut pages: Vec<Page> = raw
            .iter()
            .map(|p| {
                let id = page_id(p);
                let categories = fetched.get(&id).cloned().unwrap_or_else(|| inline_categories(p));
                Page {
                    id,
                    title: p["title"].as_str().unwrap_or_default().to_string(),
                    thumbnail: scale_url(p["thumbnail"]["source"].as_str().unwrap_or_default(), flags.thumbnail_size.as_deref()),
                    categories,
                    source_wiki: self.wiki_url.clone(),
                }
            })
            .collect();
        if !flags.category.is_empty() {
            pages.retain(|p| flags.category.iter().all(|c| p.categories.contains(c)));
        }
        if !flags.categories_or.is_empty() {
            pages.retain(|p| flags.categories_or.iter().any(|c| p.categories.contains(c)));
        }
        Ok(pages)
    }

    /// `getCategoriesForPages`: category titles per page id, paged through `clcontinue`.
    async fn categories_for_pages(&self, ids: &[i64], filter: Option<&[String]>) -> Result<HashMap<i64, Vec<String>>> {
        let chunks = ids.chunks(CHUNK).map(|chunk| async move {
            let mut base: Vec<(&str, String)> = vec![
                ("action", "query".into()),
                ("pageids", chunk.iter().map(i64::to_string).collect::<Vec<_>>().join("|")),
                ("prop", "categories".into()),
                ("cllimit", "max".into()),
            ];
            if let Some(f) = filter.filter(|f| !f.is_empty()) {
                base.push(("clcategories", f.join("|")));
            }
            let mut out: Vec<(i64, Vec<String>)> = Vec::new();
            let mut data = self.get(&base).await?;
            loop {
                for (key, page) in data["query"]["pages"].as_object().into_iter().flatten() {
                    out.push((key.parse().unwrap_or(-1), inline_categories(page)));
                }
                let Some(next) = data["continue"].get("clcontinue").cloned() else { break };
                let mut p = base.clone();
                p.push(("clcontinue", next.as_str().unwrap_or_default().to_string()));
                p.push(("continue", data["continue"]["continue"].as_str().unwrap_or_default().to_string()));
                data = self.get(&p).await?;
            }
            Ok::<_, WikiError>(out)
        });
        let mut total: HashMap<i64, Vec<String>> = HashMap::new();
        for (id, cats) in futures_util::future::try_join_all(chunks).await?.into_iter().flatten() {
            total.entry(id).or_default().extend(cats);
        }
        Ok(total)
    }

    /// `getPageById(id, { thumbnailSize })` for a single id: `None` for a missing page.
    pub async fn get_page_by_id(&self, id: i64, thumbnail_size: Option<&str>) -> Result<Option<Page>> {
        let data = self
            .get(&[
                ("action", "query".into()),
                ("pageids", id.to_string()),
                ("prop", "pageimages|categories".into()),
                ("cllimit", "max".into()),
                ("piprop", "thumbnail".into()),
                ("pithumbsize", "400".into()),
            ])
            .await?;
        // Only the first response is read: later `clcontinue` responses repeat the page.
        let found = js_values(&data["query"]["pages"]).into_iter().find(|p| p.is_object() && p.get("missing").is_none() && p.get("categories").is_some());
        Ok(found.map(|p| Page {
            id: page_id(p),
            title: p["title"].as_str().unwrap_or_default().to_string(),
            thumbnail: scale_url(p["thumbnail"]["source"].as_str().unwrap_or_default(), thumbnail_size),
            categories: inline_categories(p),
            source_wiki: self.wiki_url.clone(),
        }))
    }

    /// `getPageContent(id)`: the wikitext, or `None` when the page has no revision.
    pub async fn get_page_content(&self, id: i64) -> Result<Option<String>> {
        let data = self
            .get(&[
                ("action", "query".into()),
                ("pageids", id.to_string()),
                ("prop", "revisions".into()),
                ("rvprop", "content".into()),
                ("rvslots", "main".into()),
            ])
            .await?;
        let page = js_values(&data["query"]["pages"]).into_iter().next().ok_or_else(|| WikiError::Http("no page in response".into()))?;
        // A missing page has no `pageid` (JS throws on `undefined.toString()`); an invalid one is `-1`.
        match page["pageid"].as_i64() {
            Some(id) if id != -1 => {}
            _ => return Err(WikiError::Http(format!("Page \"{id}\" does not exist."))),
        }
        Ok(page["revisions"][0]["slots"]["main"]["*"].as_str().map(str::to_string))
    }

    /// `page.getStructuredContent()`.
    pub async fn get_structured_content(&self, page: &Page) -> Result<Content> {
        Ok(parse_media_wiki_template(&self.get_page_content(page.id).await?.unwrap_or_default()))
    }

    /// `getFileUrl(name, width)`. The lookup key is the raw file name, while the API answers with
    /// the normalised title (spaces, capital first letter), so a name written with underscores
    /// resolves to nothing, exactly as in the JS client.
    pub async fn get_file_url(&self, file_name: &str, width: Option<&str>) -> Result<String> {
        if file_name.is_empty() {
            return Ok(String::new());
        }
        let data = self
            .get(&[("action", "query".into()), ("titles", format!("File:{file_name}")), ("prop", "imageinfo".into()), ("iiprop", "url".into())])
            .await?;
        for page in js_values(&data["query"]["pages"]) {
            let Some(url) = page["imageinfo"][0]["url"].as_str().filter(|u| !u.is_empty()) else { continue };
            let title = page["title"].as_str().unwrap_or_default();
            let key = match title.get(..5) {
                Some(prefix) if prefix.eq_ignore_ascii_case("File:") => &title[5..],
                _ => title,
            };
            if key == file_name {
                return Ok(scale_url(url, width));
            }
        }
        Ok(String::new())
    }
}

fn page_id(p: &Value) -> i64 {
    p["pageid"].as_i64().unwrap_or(-1)
}

fn inline_categories(p: &Value) -> Vec<String> {
    p["categories"].as_array().into_iter().flatten().filter_map(|c| c["title"].as_str().map(str::to_string)).collect()
}

/// `Object.values(obj)`: integer-like keys ascending first, then the others in insertion order.
/// MediaWiki keys `query.pages` by page id, so this is the order the JS client iterates in.
pub fn js_values(obj: &Value) -> Vec<&Value> {
    let Some(map) = obj.as_object() else { return vec![] };
    let is_index = |k: &str| k.parse::<u32>().is_ok_and(|n| n != u32::MAX && n.to_string() == k);
    let mut indexed: Vec<(u32, &Value)> = map.iter().filter(|(k, _)| is_index(k)).map(|(k, v)| (k.parse().unwrap(), v)).collect();
    indexed.sort_by_key(|(n, _)| *n);
    indexed.into_iter().map(|(_, v)| v).chain(map.iter().filter(|(k, _)| !is_index(k)).map(|(_, v)| v)).collect()
}

/// `scaleUrl`: set (or, with no width, remove) the `scale-to-width-down` segment of a Fandom image URL.
pub fn scale_url(url: &str, width: Option<&str>) -> String {
    if url.is_empty() {
        return String::new();
    }
    let Some(width) = width else { return SCALE.replace(url, "").into_owned() };
    if SCALE.is_match(url) {
        return SCALE.replace(url, |_: &regex::Captures| format!("/scale-to-width-down/{width}")).into_owned();
    }
    let mut parts = url.split("/revision/latest");
    let (f1, f2) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
    if f2.is_empty() {
        return url.to_string();
    }
    format!("{f1}/revision/latest/scale-to-width-down/{width}{f2}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scale_url_cases() {
        let base = "https://static.wikia.nocookie.net/dc/images/a/ab/X.jpg/revision/latest";
        assert_eq!(scale_url(&format!("{base}?cb=1"), Some("120")), format!("{base}/scale-to-width-down/120?cb=1"));
        assert_eq!(scale_url(&format!("{base}/scale-to-width-down/200?cb=1"), Some("120")), format!("{base}/scale-to-width-down/120?cb=1"));
        assert_eq!(scale_url(&format!("{base}/scale-to-width-down/200?cb=1"), None), format!("{base}?cb=1"));
        assert_eq!(scale_url(base, Some("120")), base, "nothing after /revision/latest");
        assert_eq!(scale_url("https://x.test/plain.jpg", Some("120")), "https://x.test/plain.jpg");
        assert_eq!(scale_url("", Some("120")), "");
    }

    #[test]
    fn object_values_orders_integer_keys_first() {
        let pages = json!({"900": {"n": 1}, "-1": {"n": 2}, "12": {"n": 3}, "abc": {"n": 4}, "5": {"n": 5}});
        let order: Vec<i64> = js_values(&pages).iter().map(|v| v["n"].as_i64().unwrap()).collect();
        assert_eq!(order, vec![5, 3, 1, 2, 4]);
    }

    #[test]
    fn api_url_matches_urlsearchparams() {
        let c = WikiClient::new("https://dc.fandom.com", reqwest::Client::new(), ClientOptions::default());
        let url = c.api_url(&[("action", "query".into()), ("gsrsearch", "Batman & Robin: 1".into()), ("clcategories", "Category:Comics|Category:Collected Editions".into())]);
        assert_eq!(
            url,
            "https://dc.fandom.com/api.php?format=json&origin=*&action=query&gsrsearch=Batman+%26+Robin%3A+1&clcategories=Category%3AComics%7CCategory%3ACollected+Editions"
        );
    }
}
