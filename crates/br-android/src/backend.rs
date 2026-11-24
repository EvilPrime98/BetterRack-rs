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
use std::sync::Arc;
use tower::ServiceExt;

#[derive(Clone)]
pub struct Backend {
    router: Router,
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
        Ok(Self { router: br_server::app(state) })
    }

    pub async fn request(&self, req: Request<Body>) -> Result<(StatusCode, Vec<u8>), String> {
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

    /// Entry names of the library, in group order.
    pub async fn library_names(&self) -> Result<Vec<String>, String> {
        let page = self.get_json("/api/library").await?;
        let groups = page["groups"].as_array().ok_or("library: no groups")?;
        Ok(groups
            .iter()
            .flat_map(|g| g["entries"].as_array().into_iter().flatten())
            .filter_map(|e| e["name"].as_str().map(str::to_owned))
            .collect())
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
