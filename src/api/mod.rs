//! HTTP client for the BetterRack server. One fn per endpoint.
//!
//! Runtime-agnostic API (plain `async fn`s) but `reqwest` needs Tokio: call these through
//! [`crate::runtime::run`], never directly on the GPUI executor.

use std::collections::HashMap;

use reqwest::{Method, RequestBuilder, Response, Url};
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::model::*;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Non-2xx (or `{error:true}`) with the server's `message`, which is what the UI should show.
    #[error("{message}")]
    Server { status: u16, message: String },
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("unexpected response: {0}")]
    Decode(String),
    #[error("invalid server url: {0}")]
    Url(String),
}

pub type ApiResult<T> = Result<T, ApiError>;

/// Header used by the server when `BR_API_KEY` is set.
const API_KEY_HEADER: &str = "x-br-api-key";

#[derive(Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    base: Url,
    api_key: Option<String>,
    /// Local mode (stage 2): requests are handed to the `br-server` router directly, no socket.
    local: Option<axum::Router>,
}

/// Placeholder authority for in-process requests (never resolved).
const IN_PROCESS_BASE: &str = "http://br.local";

/// `http://` is prepended when the scheme is missing; trailing `/` is stripped.
pub fn normalize_base_url(raw: &str) -> String {
    let s = raw.trim();
    let s = if s.contains("://") {
        s.to_string()
    } else {
        format!("http://{s}")
    };
    s.trim_end_matches('/').to_string()
}

impl ApiClient {
    pub fn new(base_url: &str, api_key: Option<String>) -> ApiResult<Self> {
        let norm = normalize_base_url(base_url);
        let base = Url::parse(&norm).map_err(|e| ApiError::Url(format!("{norm}: {e}")))?;
        let api_key = api_key.filter(|k| !k.is_empty());
        Ok(Self {
            http: reqwest::Client::new(),
            base,
            api_key,
            local: None,
        })
    }

    /// Local mode without a sidecar: the same HTTP contract, served by calling the router.
    pub fn in_process(router: axum::Router) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: Url::parse(IN_PROCESS_BASE).expect("valid constant url"),
            api_key: None,
            local: Some(router),
        }
    }

    /// Send over the network, or into the in-process router.
    async fn send(&self, rb: RequestBuilder) -> ApiResult<Response> {
        let Some(router) = &self.local else {
            return Ok(rb.send().await?);
        };
        let req = rb.build()?;
        let mut builder = http::Request::builder()
            .method(req.method().clone())
            .uri(req.url().as_str());
        for (name, value) in req.headers() {
            builder = builder.header(name, value);
        }
        let body = req
            .body()
            .and_then(|b| b.as_bytes())
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        let request = builder
            .body(axum::body::Body::from(body))
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        let response = tower::ServiceExt::oneshot(router.clone(), request)
            .await
            .unwrap_or_else(|e| match e {});
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, usize::MAX)
            .await
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        Ok(Response::from(http::Response::from_parts(parts, bytes)))
    }

    pub fn base_url(&self) -> &str {
        self.base.as_str().trim_end_matches('/')
    }

    /// Build a URL from path segments (each one percent-encoded, so uids are safe).
    fn url(&self, segments: &[&str]) -> Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("http(s) base url")
            .pop_if_empty()
            .extend(segments);
        url
    }

    fn request(&self, method: Method, segments: &[&str]) -> RequestBuilder {
        let rb = self.http.request(method, self.url(segments));
        match &self.api_key {
            Some(k) => rb.header(API_KEY_HEADER, k),
            None => rb,
        }
    }

    pub(crate) fn get(&self, segments: &[&str]) -> RequestBuilder {
        self.request(Method::GET, segments)
    }

    #[allow(dead_code)] // Phase 6: remote mode loads images by URL with `?key=`
    /// URL for consumers that cannot set headers (image loaders, EventSource): the key goes in
    /// `?key=`. Local mode (no key) yields a plain URL.
    pub fn authed_url(&self, segments: &[&str], query: &[(&str, String)]) -> String {
        let mut url = self.url(segments);
        {
            let mut q = url.query_pairs_mut();
            for (k, v) in query {
                q.append_pair(k, v);
            }
            if let Some(key) = &self.api_key {
                q.append_pair("key", key);
            }
        }
        // `query_pairs_mut` leaves a dangling `?` when nothing was appended.
        url.to_string().trim_end_matches('?').to_string()
    }

    /// Send, mapping non-2xx to [`ApiError::Server`] using the `{error, message}` body.
    pub(crate) async fn check(&self, rb: RequestBuilder) -> ApiResult<Response> {
        let resp = self.send(rb).await?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let text = resp.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str().map(str::to_owned)))
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| {
                if text.is_empty() {
                    status.to_string()
                } else {
                    text
                }
            });
        Err(ApiError::Server {
            status: status.as_u16(),
            message,
        })
    }

    async fn json<T: DeserializeOwned>(&self, rb: RequestBuilder) -> ApiResult<T> {
        let resp = self.check(rb).await?;
        let bytes = resp.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|e| ApiError::Decode(e.to_string()))
    }

    /// For endpoints whose body we ignore. Some answer 200 with `{error:true,message}`.
    async fn done(&self, rb: RequestBuilder) -> ApiResult<()> {
        let status;
        let bytes = {
            let resp = self.check(rb).await?;
            status = resp.status().as_u16();
            resp.bytes().await?
        };
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if v.get("error").and_then(|e| e.as_bool()) == Some(true) {
                let message = v
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("request failed");
                return Err(ApiError::Server {
                    status,
                    message: message.to_string(),
                });
            }
        }
        Ok(())
    }

    fn post_json(&self, segments: &[&str], body: serde_json::Value) -> RequestBuilder {
        self.request(Method::POST, segments).json(&body)
    }

    pub async fn healthz(&self) -> ApiResult<Health> {
        self.json(self.get(&["healthz"])).await
    }

    pub async fn library_page(&self, by_series: bool, offset: usize) -> ApiResult<LibraryPage> {
        let seg: &[&str] = if by_series {
            &["api", "library", "by-series"]
        } else {
            &["api", "library"]
        };
        self.json(self.get(seg).query(&[("offset", offset)])).await
    }

    /// Loops until `hasMore=false` advancing `offset += limit`, merging groups by `uid` and
    /// de-duping entries by `uid`: a single page is a truncated library.
    pub async fn library_all(&self, by_series: bool) -> ApiResult<Vec<LibraryGroup>> {
        let mut groups: Vec<LibraryGroup> = Vec::new();
        let mut offset = 0;
        loop {
            let page = self.library_page(by_series, offset).await?;
            merge_groups(&mut groups, page.groups);
            if !page.has_more || page.limit == 0 {
                break;
            }
            offset += page.limit;
        }
        Ok(groups)
    }

    /// `window_hours`: invalid/missing is treated as 24 by the server.
    pub async fn library_recent(&self, window_hours: u32) -> ApiResult<RecentResponse> {
        self.json(
            self.get(&["api", "library", "recent"])
                .query(&[("windowHours", window_hours)]),
        )
        .await
    }

    pub async fn library_reading(&self) -> ApiResult<ReadingResponse> {
        self.json(self.get(&["api", "library", "reading"])).await
    }

    /// Rescan.
    pub async fn library_refresh(&self) -> ApiResult<()> {
        self.done(self.get(&["api", "library", "refresh"])).await
    }

    pub async fn create_folder(
        &self,
        folder_name: &str,
        parent_folder_uid: Option<&str>,
    ) -> ApiResult<()> {
        let body = json!({ "folderName": folder_name, "parentFolderUid": parent_folder_uid });
        self.done(self.post_json(&["api", "library", "folder"], body))
            .await
    }

    pub async fn delete_folder(&self, folder_uid: &str) -> ApiResult<()> {
        let rb = self
            .request(Method::DELETE, &["api", "library", "folder"])
            .json(&json!({ "folderUid": folder_uid }));
        self.done(rb).await
    }

    /// Deletes the file from disk.
    pub async fn delete_file(&self, file_uid: &str) -> ApiResult<()> {
        let rb = self
            .request(Method::DELETE, &["api", "library", "file"])
            .json(&json!({ "fileUid": file_uid }));
        self.done(rb).await
    }

    pub async fn move_file(
        &self,
        file_uid: &str,
        target_folder_uid: Option<&str>,
    ) -> ApiResult<()> {
        let body = json!({ "fileUid": file_uid, "targetFolderUid": target_folder_uid });
        self.done(self.post_json(&["api", "library", "file", "move"], body))
            .await
    }

    pub async fn unidentify_file(&self, file_uid: &str) -> ApiResult<()> {
        let body = json!({ "fileUid": file_uid });
        self.done(self.post_json(&["api", "library", "file", "unidentify"], body))
            .await
    }

    /// Commit a manual pick.
    pub async fn identify_file(&self, file_uid: &str, comic: &WikiComic) -> ApiResult<()> {
        let body = json!({ "fileUid": file_uid, "comic": comic });
        self.done(self.post_json(&["api", "library", "file", "identify"], body))
            .await
    }

    /// Lazy identify (called when a card scrolls into view).
    pub async fn identify_lazy(&self, uid: &str) -> ApiResult<IdentifyResponse> {
        self.json(self.get(&["api", "library", uid, "identify"]))
            .await
    }

    /// Re-identify one file (the card's refresh button); answers with the new state.
    pub async fn identify_reset(&self, uid: &str) -> ApiResult<IdentifyResponse> {
        self.json(self.request(Method::POST, &["api", "library", uid, "identify", "reset"]))
            .await
    }

    pub async fn identify_reset_all(&self) -> ApiResult<()> {
        self.done(self.request(Method::POST, &["api", "library", "identify", "reset-all"]))
            .await
    }

    /// Start the background identify job.
    pub async fn identify_all_start(&self) -> ApiResult<IdentifyLibraryStatus> {
        let v: serde_json::Value = self
            .json(self.request(Method::POST, &["api", "library", "identify", "all"]))
            .await?;
        Ok(IdentifyLibraryStatus::from_value(v))
    }

    /// Poll status (client polls every [`IDENTIFY_POLL_INTERVAL_MS`]).
    pub async fn identify_all_status(&self) -> ApiResult<IdentifyLibraryStatus> {
        let v: serde_json::Value = self
            .json(self.get(&["api", "library", "identify", "all"]))
            .await?;
        Ok(IdentifyLibraryStatus::from_value(v))
    }

    pub async fn reader_pages(&self, uid: &str) -> ApiResult<ReaderPages> {
        self.json(self.get(&["read", uid])).await
    }

    /// Re-list pages (evicts the server's archive cache).
    pub async fn reader_refresh(&self, uid: &str) -> ApiResult<ReaderPages> {
        self.json(self.get(&["read", uid, "refresh"])).await
    }

    pub async fn bookmarks(&self, uid: &str) -> ApiResult<Vec<Bookmark>> {
        #[derive(serde::Deserialize)]
        struct R {
            #[serde(default)]
            bookmarks: Vec<Bookmark>,
        }
        let r: R = self
            .json(
                self.get(&["read", uid, "bookmarks"])
                    .header("cache-control", "no-store"),
            )
            .await?;
        Ok(r.bookmarks)
    }

    #[allow(dead_code)] // Phase 6: remote mode loads images by URL with `?key=`
    /// Image URL for page `page` (**1-based**). Immutable + ETagged: cache aggressively.
    pub fn page_url(&self, uid: &str, page: u32) -> String {
        self.authed_url(&["read", uid, "pages", &page.to_string()], &[])
    }

    /// Raw page bytes, for callers that cache on disk / decode off-thread.
    pub async fn page_bytes(&self, uid: &str, page: u32) -> ApiResult<Vec<u8>> {
        let resp = self
            .check(self.get(&["read", uid, "pages", &page.to_string()]))
            .await?;
        Ok(resp.bytes().await?.to_vec())
    }

    pub async fn comic_data(&self) -> ApiResult<HashMap<String, ComicCache>> {
        self.json(self.get(&["api", "comic-data"])).await
    }

    /// The client sends the full merged object.
    pub async fn patch_comic_data(&self, uid: &str, data: &ComicCache) -> ApiResult<()> {
        self.done(
            self.request(Method::PATCH, &["api", "comic-data", uid])
                .json(data),
        )
        .await
    }

    pub async fn settings(&self) -> ApiResult<AppSettings> {
        self.json(self.get(&["api", "settings"])).await
    }

    pub async fn update_settings(&self, update: &SettingsUpdate) -> ApiResult<AppSettings> {
        self.json(self.request(Method::PUT, &["api", "settings"]).json(update))
            .await
    }

    pub async fn add_library_folder(&self, path: &str) -> ApiResult<AppSettings> {
        self.json(self.post_json(
            &["api", "settings", "library-folder"],
            json!({ "path": path }),
        ))
        .await
    }

    pub async fn remove_library_folder(&self, path: &str) -> ApiResult<AppSettings> {
        let rb = self
            .request(Method::DELETE, &["api", "settings", "library-folder"])
            .json(&json!({ "path": path }));
        self.json(rb).await
    }

    /// Move-file target picker. Cache + invalidate on folder/move/settings changes.
    pub async fn directories(&self) -> ApiResult<Vec<String>> {
        #[derive(serde::Deserialize)]
        struct R {
            #[serde(default)]
            directories: Vec<String>,
        }
        Ok(self
            .json::<R>(self.get(&["api", "directories"]))
            .await?
            .directories)
    }

    /// Bytes of an image hosted elsewhere (store covers). Deliberately a bare request: the API key
    /// must never be sent to a third party.
    pub async fn external_bytes(&self, url: &str) -> ApiResult<Vec<u8>> {
        let mut request = self.http.get(url);
        // Fandom's image CDN refuses hotlinks without a fandom referer (a browser <img> sends one).
        if url.contains("wikia.nocookie.net") || url.contains("fandom.com") {
            request = request.header(reqwest::header::REFERER, "https://dc.fandom.com/");
        }
        let resp = request.send().await?;
        if !resp.status().is_success() {
            return Err(ApiError::Server {
                status: resp.status().as_u16(),
                message: resp.status().to_string(),
            });
        }
        Ok(resp.bytes().await?.to_vec())
    }

    #[allow(dead_code)] // Phase 6: remote mode loads images by URL with `?key=`
    pub fn thumbnail_url(&self, uid: &str) -> String {
        self.authed_url(&["api", "thumbnail", uid], &[])
    }

    pub async fn thumbnail_bytes(&self, uid: &str) -> ApiResult<Vec<u8>> {
        let resp = self.check(self.get(&["api", "thumbnail", uid])).await?;
        Ok(resp.bytes().await?.to_vec())
    }

    pub async fn retry_thumbnail(&self, uid: &str) -> ApiResult<()> {
        self.done(self.request(Method::POST, &["api", "thumbnail", uid, "retry"]))
            .await
    }

    pub async fn wiki_search(&self, title: &str, thumbnail_size: u32) -> ApiResult<Vec<WikiComic>> {
        let rb = self.get(&["api", "wiki", "comics"]).query(&[
            ("title", title.to_string()),
            ("thumbnailSize", thumbnail_size.to_string()),
        ]);
        self.json(rb).await
    }

    pub async fn wiki_comic(
        &self,
        id: &str,
        source_wiki: &str,
        thumbnail_size: u32,
    ) -> ApiResult<WikiComic> {
        let rb = self.get(&["api", "wiki", "comic", id]).query(&[
            ("sourceWiki", source_wiki.to_string()),
            ("thumbnailSize", thumbnail_size.to_string()),
        ]);
        self.json(rb).await
    }

    /// Search (or the "latest" feed when `search` is `None`). `page` is 1-based and the server
    /// answers with a bare array (a `{items:[…]}` envelope is tolerated). The store page does not
    /// send `exact`, so neither do we.
    pub async fn store_posts(
        &self,
        search: Option<&str>,
        page: u32,
        per_page: usize,
    ) -> ApiResult<Vec<StorePost>> {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Resp {
            List(Vec<StorePost>),
            Wrapped { items: Vec<StorePost> },
        }
        let mut q: Vec<(&str, String)> = vec![
            ("page", page.to_string()),
            ("perPage", per_page.to_string()),
        ];
        match search {
            Some(s) if !s.trim().is_empty() => q.push(("search", s.to_string())),
            _ => q.push(("latest", "true".into())),
        }
        Ok(
            match self
                .json::<Resp>(self.get(&["api", "comics"]).query(&q))
                .await?
            {
                Resp::List(v) | Resp::Wrapped { items: v } => v,
            },
        )
    }

    pub async fn comic_links(&self, id: i64) -> ApiResult<Vec<StoreLink>> {
        #[derive(serde::Deserialize)]
        struct R {
            #[serde(default)]
            links: Vec<StoreLink>,
        }
        let rb = self
            .get(&["api", "comics", &id.to_string(), "links"])
            .query(&[("strat", "all")]);
        Ok(self.json::<R>(rb).await?.links)
    }

    /// Queue a download job (the server de-dupes by comic id). The jobs list is polled for progress.
    pub async fn start_download(&self, req: &StartDownload) -> ApiResult<()> {
        self.done(self.request(Method::POST, &["api", "downloads"]).json(req))
            .await
    }

    pub async fn download_jobs(&self) -> ApiResult<Vec<JobStatus>> {
        #[derive(serde::Deserialize)]
        struct R {
            #[serde(default)]
            jobs: Vec<JobStatus>,
        }
        Ok(self
            .json::<R>(self.get(&["api", "downloads", "jobs"]))
            .await?
            .jobs)
    }

    pub async fn retry_download(&self, job_id: &str) -> ApiResult<()> {
        self.done(self.request(Method::POST, &["api", "downloads", job_id, "retry"]))
            .await
    }

    /// Cancel.
    pub async fn cancel_download(&self, job_id: &str) -> ApiResult<()> {
        self.done(self.request(Method::DELETE, &["api", "downloads", job_id]))
            .await
    }

    /// Close guard: number of `queued` + `running` jobs.
    pub async fn active_download_count(&self) -> ApiResult<usize> {
        Ok(self
            .download_jobs()
            .await?
            .iter()
            .filter(|j| matches!(j.state, JobState::Queued | JobState::Running))
            .count())
    }
}

/// Merge `incoming` into `into`: groups by `uid`, entries de-duped by `uid`.
pub fn merge_groups(into: &mut Vec<LibraryGroup>, incoming: Vec<LibraryGroup>) {
    for g in incoming {
        match into.iter_mut().find(|e| e.uid == g.uid) {
            Some(existing) => {
                for entry in g.entries {
                    if !existing.entries.iter().any(|e| e.uid == entry.uid) {
                        existing.entries.push(entry);
                    }
                }
            }
            None => into.push(g),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(uid: &str) -> LibraryEntry {
        serde_json::from_value(json!({"uid": uid, "name": uid})).unwrap()
    }

    fn group(uid: &str, entries: &[&str]) -> LibraryGroup {
        LibraryGroup {
            uid: uid.into(),
            name: uid.into(),
            path: String::new(),
            entries: entries.iter().map(|e| entry(e)).collect(),
        }
    }

    #[tokio::test]
    async fn in_process_client_serves_the_contract_without_a_socket() {
        let dir = std::env::temp_dir().join(format!("br-inproc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = br_core::config::Config {
            port: 0,
            api_key: None,
            log_level: "info".into(),
            seven_zip_path: None,
            data_dir: dir.clone(),
        };
        let client = ApiClient::in_process(br_server::app(
            br_server::state::AppState::open(config).unwrap(),
        ));

        assert_eq!(client.healthz().await.unwrap().app, "betterrack");
        assert!(client.settings().await.is_ok());
        // Non-2xx keeps the server's message.
        let err = client.reader_pages("nope").await.unwrap_err();
        assert!(
            matches!(&err, ApiError::Server { status: 500, message } if message == "File not found in library"),
            "{err:?}"
        );
        drop(client);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalizes_urls() {
        assert_eq!(
            normalize_base_url("localhost:3000/"),
            "http://localhost:3000"
        );
        assert_eq!(normalize_base_url(" https://x.dev// "), "https://x.dev");
    }

    #[test]
    fn urls_encode_segments_and_carry_the_key() {
        let c = ApiClient::new("localhost:3000", Some("s3cret".into())).unwrap();
        assert_eq!(
            c.page_url("a/b c", 3),
            "http://localhost:3000/read/a%2Fb%20c/pages/3?key=s3cret"
        );
        let local = ApiClient::new("http://localhost:3000", None).unwrap();
        assert_eq!(
            local.thumbnail_url("abc"),
            "http://localhost:3000/api/thumbnail/abc"
        );
    }

    #[test]
    fn merges_pages_by_uid() {
        let mut all = vec![group("g1", &["a", "b"])];
        merge_groups(
            &mut all,
            vec![group("g1", &["b", "c"]), group("g2", &["x"])],
        );
        assert_eq!(all.len(), 2);
        let uids: Vec<_> = all[0].entries.iter().map(|e| e.uid.as_str()).collect();
        assert_eq!(uids, ["a", "b", "c"]);
    }
}
