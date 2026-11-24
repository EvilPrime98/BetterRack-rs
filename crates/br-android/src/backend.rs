//! The embedded server: `br-server`'s router called in-process, no socket.
use axum::Router;
use axum::body::Body;
use br_core::config::Config;
use br_core::archive::Archives;
use br_core::fs::LibraryFs;
use br_core::wiki::NoWiki;
use br_server::state::{AppState, NetOptions};
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tower::ServiceExt;

const API_KEY_HEADER: &str = "x-br-api-key";

/// A linked BetterRack server (the source of truth while linked).
#[derive(Clone)]
struct Remote {
    http: reqwest::Client,
    base: String,
    api_key: Option<String>,
}

/// `http://` is prepended when the scheme is missing; the trailing `/` is stripped.
pub fn normalize_base_url(raw: &str) -> String {
    let s = raw.trim();
    let s = if s.contains("://") { s.to_string() } else { format!("http://{s}") };
    s.trim_end_matches('/').to_string()
}

/// The on-device server, plus an optional remote one that takes over while linked.
/// The local router stays alive so unlinking (or going offline) is instant.
#[derive(Clone)]
pub struct Backend {
    router: Router,
    remote: Arc<RwLock<Option<Remote>>>,
}

impl Backend {
    /// Opens the SQLite stores under `data_dir` and builds the router.
    /// `AppState::open` does blocking file I/O, so call this off the UI thread.
    /// Library files are read through `fs` (the SAF tree filesystem); RAR copies go to `<data>/cache`.
    pub fn start(data_dir: &Path, fs: Arc<dyn LibraryFs>) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let config = Config {
            port: 0,
            bind_addr: "127.0.0.1".into(),
            api_key: None,
            log_level: "info".into(),
            seven_zip_path: None,
            data_dir: data_dir.to_path_buf(),
        };
        let archives = Archives::with_fs(None, fs).with_cache_dir(data_dir.join("cache"));
        let net = NetOptions { archives: Some(Arc::new(archives)), ..NetOptions::default() };
        let state = AppState::open_with(config, Arc::new(NoWiki), net).map_err(|e| e.to_string())?;
        Ok(Self { router: br_server::app(state), remote: Arc::default() })
    }

    /// Sends every later request to `url` (with `api_key`) instead of the on-device router.
    pub fn link_remote(&self, url: &str, api_key: Option<String>) -> Result<(), String> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let api_key = api_key.filter(|k| !k.is_empty());
        *self.remote.write().unwrap() = Some(Remote { http, base: normalize_base_url(url), api_key });
        Ok(())
    }

    pub fn unlink_remote(&self) {
        *self.remote.write().unwrap() = None;
    }

    pub fn is_remote(&self) -> bool {
        self.remote.read().unwrap().is_some()
    }

    async fn request_remote(remote: Remote, req: Request<Body>) -> Result<(StatusCode, Vec<u8>), String> {
        let (parts, body) = req.into_parts();
        let bytes = body.collect().await.map_err(|e| e.to_string())?.to_bytes();
        let path = parts.uri.path_and_query().map_or("/", |p| p.as_str());
        let mut rb = remote.http.request(parts.method, format!("{}{path}", remote.base)).headers(parts.headers).body(bytes);
        if let Some(key) = &remote.api_key {
            rb = rb.header(API_KEY_HEADER, key);
        }
        let res = rb.send().await.map_err(|e| e.to_string())?;
        let status = res.status();
        let body = res.bytes().await.map_err(|e| e.to_string())?;
        Ok((status, body.to_vec()))
    }

    pub async fn request(&self, req: Request<Body>) -> Result<(StatusCode, Vec<u8>), String> {
        let remote = self.remote.read().unwrap().clone();
        if let Some(remote) = remote {
            return Self::request_remote(remote, req).await;
        }
        let res = self.router.clone().oneshot(req).await.map_err(|e| e.to_string())?;
        let status = res.status();
        let bytes = res.into_body().collect().await.map_err(|e| e.to_string())?.to_bytes();
        Ok((status, bytes.to_vec()))
    }

    pub async fn get_json(&self, uri: &str) -> Result<serde_json::Value, String> {
        let (status, body) = self.request(Request::get(uri).body(Body::empty()).unwrap()).await?;
        if !status.is_success() {
            return Err(format!("{uri}: {status}"));
        }
        serde_json::from_slice(&body).map_err(|e| e.to_string())
    }

    pub async fn get_bytes(&self, uri: &str) -> Result<Vec<u8>, String> {
        let (status, body) = self.request(Request::get(uri).body(Body::empty()).unwrap()).await?;
        if !status.is_success() {
            return Err(format!("{uri}: {status}"));
        }
        Ok(body)
    }

    /// Adds a library folder (a path the configured `LibraryFs` can list) and rescans.
    pub async fn add_folder(&self, path: &str) -> Result<(), String> {
        let body = serde_json::json!({ "path": path }).to_string();
        let req = Request::post("/api/settings/library-folder")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let (status, bytes) = self.request(req).await?;
        if !status.is_success() {
            return Err(String::from_utf8_lossy(&bytes).into_owned());
        }
        self.rescan().await
    }

    /// Rescans synchronously (the request returns when the scan is done).
    pub async fn rescan(&self) -> Result<(), String> {
        self.get_json("/api/library/refresh").await.map(|_| ())
    }

    /// Library folders as stored in the settings.
    pub async fn folders(&self) -> Result<Vec<String>, String> {
        let settings = self.get_json("/api/settings").await?;
        Ok(settings["outputDirs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect())
    }

    pub async fn healthy(&self) -> bool {
        self.request(Request::get("/healthz").body(Body::empty()).unwrap())
            .await
            .is_ok_and(|(s, _)| s.is_success())
    }
}
