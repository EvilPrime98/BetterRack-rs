//! Store pull-list: followed title keywords persisted whole to `pull-list.json` in the data dir.
//! Same schema as the desktop's `PullEntry`, but a separate file.

use crate::error::ApiError;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const FILE_NAME: &str = "pull-list.json";

/// `exclude` is a comma-separated list of terms. `last_seen_id` is the highest post id the user
/// has acknowledged; posts with a greater id count as new.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PullEntry {
    pub id: String,
    pub keyword: String,
    pub exclude: String,
    pub last_seen_id: i64,
    pub created_at: i64,
}

pub struct PullList {
    file: PathBuf,
    items: Mutex<Vec<PullEntry>>,
}

fn load_file(path: &Path) -> Vec<PullEntry> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl PullList {
    pub fn open(data_dir: &Path) -> Self {
        let file = data_dir.join(FILE_NAME);
        let items = Mutex::new(load_file(&file));
        Self { file, items }
    }

    fn save(&self, items: &[PullEntry]) {
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let written = serde_json::to_string_pretty(items)
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(&self.file, json).map_err(|e| e.to_string()));
        if let Err(e) = written {
            tracing::warn!("could not save pull-list to {}: {e}", self.file.display());
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<PullEntry>> {
        self.items.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn list(&self) -> Vec<PullEntry> {
        self.lock().clone()
    }

    /// A keyword already followed (case-insensitive) returns the existing entry.
    pub fn add(&self, body: &AddBody) -> Option<PullEntry> {
        let keyword = body.keyword.trim();
        if keyword.is_empty() {
            return None;
        }
        let mut items = self.lock();
        if let Some(e) = items
            .iter()
            .find(|e| e.keyword.to_lowercase() == keyword.to_lowercase())
        {
            return Some(e.clone());
        }
        let created_at = now_ms();
        let entry = PullEntry {
            id: format!("{created_at:x}-{}", items.len()),
            keyword: keyword.to_string(),
            exclude: body.exclude.trim().to_string(),
            last_seen_id: body.last_seen_id,
            created_at,
        };
        items.push(entry.clone());
        self.save(&items);
        Some(entry)
    }

    pub fn update(&self, id: &str, patch: &EntryPatch) -> Result<Option<PullEntry>, ()> {
        let keyword = patch.keyword.as_deref().map(str::trim);
        if keyword == Some("") {
            return Err(());
        }
        let mut items = self.lock();
        let Some(e) = items.iter_mut().find(|e| e.id == id) else {
            return Ok(None);
        };
        if let Some(k) = keyword {
            e.keyword = k.to_string();
        }
        if let Some(x) = &patch.exclude {
            e.exclude = x.trim().to_string();
        }
        if let Some(seen) = patch.last_seen_id {
            e.last_seen_id = seen;
        }
        let out = e.clone();
        self.save(&items);
        Ok(Some(out))
    }

    pub fn remove(&self, id: &str) -> bool {
        let mut items = self.lock();
        let before = items.len();
        items.retain(|e| e.id != id);
        let removed = items.len() != before;
        if removed {
            self.save(&items);
        }
        removed
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPatch {
    pub keyword: Option<String>,
    pub exclude: Option<String>,
    pub last_seen_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddBody {
    pub keyword: String,
    #[serde(default)]
    pub exclude: String,
    #[serde(default)]
    pub last_seen_id: i64,
}

fn not_found() -> ApiError {
    ApiError::Json(StatusCode::NOT_FOUND, "Pull-list entry not found".into())
}

fn empty_keyword() -> ApiError {
    ApiError::Json(StatusCode::BAD_REQUEST, "Keyword is required".into())
}

pub async fn list(State(s): State<AppState>) -> Json<Vec<PullEntry>> {
    Json(s.pull_list.list())
}

pub async fn add(
    State(s): State<AppState>,
    Json(body): Json<AddBody>,
) -> Result<Json<PullEntry>, ApiError> {
    s.pull_list.add(&body).map(Json).ok_or_else(empty_keyword)
}

pub async fn patch(
    State(s): State<AppState>,
    UrlPath(id): UrlPath<String>,
    Json(patch): Json<EntryPatch>,
) -> Result<Json<PullEntry>, ApiError> {
    match s.pull_list.update(&id, &patch) {
        Err(()) => Err(empty_keyword()),
        Ok(entry) => entry.map(Json).ok_or_else(not_found),
    }
}

pub async fn remove(
    State(s): State<AppState>,
    UrlPath(id): UrlPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if s.pull_list.remove(&id) {
        Ok(Json(serde_json::json!({ "error": false })))
    } else {
        Err(not_found())
    }
}
