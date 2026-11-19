//! Store bookmarks: a mutex-guarded list persisted whole to `bookmarks.json` in the data dir.
//! Same schema as the desktop's `StoreBookmark`, but a separate file.

use crate::error::ApiError;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const FILE_NAME: &str = "bookmarks.json";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StoreBookmark {
    pub key: String,
    pub id: Option<i64>,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub link: String,
    pub upload_date: Option<String>,
    pub folder: Option<String>,
    pub rating: f32,
    pub read: bool,
    pub created_at: i64,
}

/// The post as the store returns it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorePost {
    pub id: Option<i64>,
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub link: String,
    pub upload_date: Option<String>,
}

/// `id:{id}` when the post has an id, otherwise `link:{link}`.
pub fn post_key(post: &StorePost) -> String {
    match post.id {
        Some(id) if id != 0 => format!("id:{id}"),
        _ => format!("link:{}", post.link),
    }
}

pub struct Bookmarks {
    file: PathBuf,
    items: Mutex<Vec<StoreBookmark>>,
}

fn load_file(path: &Path) -> Vec<StoreBookmark> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

impl Bookmarks {
    pub fn open(data_dir: &Path) -> Self {
        let file = data_dir.join(FILE_NAME);
        let items = Mutex::new(load_file(&file));
        Self { file, items }
    }

    fn save(&self, items: &[StoreBookmark]) {
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let written = serde_json::to_string_pretty(items)
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(&self.file, json).map_err(|e| e.to_string()));
        if let Err(e) = written {
            tracing::warn!("could not save bookmarks to {}: {e}", self.file.display());
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<StoreBookmark>> {
        self.items.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn list(&self) -> Vec<StoreBookmark> {
        self.lock().clone()
    }

    /// An existing key moves instead of duplicating.
    pub fn add(&self, post: &StorePost, folder: Option<String>) -> StoreBookmark {
        let key = post_key(post);
        let mut items = self.lock();
        let out = if let Some(b) = items.iter_mut().find(|b| b.key == key) {
            b.folder = folder;
            b.clone()
        } else {
            let b = StoreBookmark {
                key,
                id: post.id,
                title: post.title.clone(),
                thumbnail_url: post.thumbnail_url.clone(),
                link: post.link.clone(),
                upload_date: post.upload_date.clone(),
                folder,
                rating: 0.0,
                read: false,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as i64),
            };
            items.push(b.clone());
            b
        };
        self.save(&items);
        out
    }

    pub fn update(&self, key: &str, patch: &BookmarkPatch) -> Option<StoreBookmark> {
        let mut items = self.lock();
        let b = items.iter_mut().find(|b| b.key == key)?;
        if let Some(folder) = &patch.folder {
            b.folder = folder.clone();
        }
        if let Some(rating) = patch.rating {
            b.rating = rating;
        }
        if let Some(read) = patch.read {
            b.read = read;
        }
        let out = b.clone();
        self.save(&items);
        Some(out)
    }

    pub fn remove(&self, key: &str) -> bool {
        let mut items = self.lock();
        let before = items.len();
        items.retain(|b| b.key != key);
        let removed = items.len() != before;
        if removed {
            self.save(&items);
        }
        removed
    }
}

/// `folder` distinguishes absent (leave alone) from `null` (library root).
#[derive(Debug, Default, Deserialize)]
pub struct BookmarkPatch {
    #[serde(default, deserialize_with = "some_option")]
    pub folder: Option<Option<String>>,
    pub rating: Option<f32>,
    pub read: Option<bool>,
}

fn some_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

#[derive(Debug, Deserialize)]
pub struct AddBody {
    pub post: StorePost,
    #[serde(default)]
    pub folder: Option<String>,
}

fn not_found() -> ApiError {
    ApiError::Json(StatusCode::NOT_FOUND, "Bookmark not found".into())
}

pub async fn list(State(s): State<AppState>) -> Json<Vec<StoreBookmark>> {
    Json(s.bookmarks.list())
}

pub async fn add(State(s): State<AppState>, Json(body): Json<AddBody>) -> Json<StoreBookmark> {
    Json(s.bookmarks.add(&body.post, body.folder))
}

pub async fn patch(
    State(s): State<AppState>,
    UrlPath(key): UrlPath<String>,
    Json(patch): Json<BookmarkPatch>,
) -> Result<Json<StoreBookmark>, ApiError> {
    s.bookmarks
        .update(&key, &patch)
        .map(Json)
        .ok_or_else(not_found)
}

pub async fn remove(
    State(s): State<AppState>,
    UrlPath(key): UrlPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if s.bookmarks.remove(&key) {
        Ok(Json(serde_json::json!({ "error": false })))
    } else {
        Err(not_found())
    }
}
