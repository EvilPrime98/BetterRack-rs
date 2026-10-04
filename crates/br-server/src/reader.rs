//! `/read/:uuid*`: page list, refresh, bookmarks and page bytes.
//!
//! Error shapes: 500 `{ error: true, message, pages: [] }` for the list endpoints
//! (`bookmarks: []` for bookmarks), 500 `{ error: true, message }` for a page.

use crate::http_cache::{if_none_match_satisfied, json_num, json_str, strong_etag};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use br_core::archive::{file_stamp, page_mime_type};
use br_core::comic_info::{js_number, js_positive_integer};
use br_core::{CoreError, Result as CoreResult};
use serde_json::{Value, json};

const NO_STORE: &str = "no-store";
const IMMUTABLE: &str = "private, max-age=31536000, immutable";

fn invalid(msg: &str) -> CoreError {
    CoreError::Invalid(msg.to_string())
}

fn json_reply(status: StatusCode, cache_control: Option<&'static str>, body: Value) -> Response {
    let mut res = (status, Json(body)).into_response();
    if let Some(cc) = cache_control {
        res.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(cc));
    }
    res
}

fn failure(
    e: &CoreError,
    empty_key: Option<&str>,
    cache_control: Option<&'static str>,
) -> Response {
    tracing::error!(err = %e, "reader request failed");
    let mut body = json!({ "error": true, "message": e.to_string() });
    if let Some(k) = empty_key {
        body[k] = json!([]);
    }
    json_reply(StatusCode::INTERNAL_SERVER_ERROR, cache_control, body)
}

/// uid -> archive path + its page list. `LibraryModel.get`, exists check, `listPages`.
fn pages_of(s: &AppState, uid: &str) -> CoreResult<(std::path::PathBuf, Vec<String>)> {
    let path = s
        .library
        .path_of(uid)
        .ok_or_else(|| invalid("File not found in library"))?;
    let path = std::path::PathBuf::from(path);
    if !path.is_file() {
        return Err(invalid("File not found"));
    }
    let pages = s.archives.list_pages(&path)?;
    if pages.is_empty() {
        return Err(invalid("No pages found in comic"));
    }
    Ok((path, pages))
}

async fn run<T: Send + 'static>(
    f: impl FnOnce() -> CoreResult<T> + Send + 'static,
) -> CoreResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| CoreError::Invalid(e.to_string()))?
}

fn listing(message: &str, pages: Vec<String>) -> Response {
    json_reply(
        StatusCode::OK,
        Some(NO_STORE),
        json!({ "error": false, "message": message, "totalPages": pages.len(), "pages": pages }),
    )
}

pub async fn get(State(s): State<AppState>, Path(uid): Path<String>) -> Response {
    match run(move || pages_of(&s, &uid)).await {
        Ok((_, pages)) => listing("Comic pages listed successfully", pages),
        Err(e) => failure(&e, Some("pages"), Some(NO_STORE)),
    }
}

pub async fn refresh(State(s): State<AppState>, Path(uid): Path<String>) -> Response {
    let result = run(move || {
        let path = s
            .library
            .path_of(&uid)
            .ok_or_else(|| invalid("File not found in library"))?;
        s.archives.evict(std::path::Path::new(&path));
        pages_of(&s, &uid)
    })
    .await;
    match result {
        Ok((_, pages)) => listing("Comic re-scanned successfully", pages),
        Err(e) => failure(&e, Some("pages"), Some(NO_STORE)),
    }
}

pub async fn bookmarks(State(s): State<AppState>, Path(uid): Path<String>) -> Response {
    let result = run(move || {
        let (path, _) = pages_of(&s, &uid)?;
        s.archives.bookmarks(&path)
    })
    .await;
    match result {
        Ok(list) => {
            let bookmarks: Vec<Value> = list
                .into_iter()
                .map(|b| json!({ "page": b.page, "label": b.label }))
                .collect();
            json_reply(
                StatusCode::OK,
                Some(NO_STORE),
                json!({ "error": false, "message": "Comic bookmarks listed successfully", "bookmarks": bookmarks }),
            )
        }
        Err(e) => failure(&e, Some("bookmarks"), Some(NO_STORE)),
    }
}

enum PageOutcome {
    NotFound,
    NotModified(String),
    Bytes(String, &'static str, Vec<u8>),
}

fn cache_headers(etag: &str) -> [(header::HeaderName, String); 2] {
    [
        (header::CACHE_CONTROL, IMMUTABLE.to_string()),
        (header::ETAG, etag.to_string()),
    ]
}

pub async fn page(
    State(s): State<AppState>,
    Path((uid, page)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let Some(number) = js_positive_integer(js_number(&page)) else {
        return json_reply(
            StatusCode::BAD_REQUEST,
            None,
            json!({ "error": true, "message": "Invalid page number" }),
        );
    };
    let if_none_match = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let result = run(move || {
        let (path, pages) = pages_of(&s, &uid)?;
        let Some(entry) = pages.get(number as usize - 1) else {
            return Ok(PageOutcome::NotFound);
        };
        // The etag covers archive size and mtime, so replacing the file on disk changes it.
        let stamp = file_stamp(&path)?;
        let etag = strong_etag(&[
            json_str(&path.to_string_lossy()),
            json_str(entry),
            stamp.size.to_string(),
            json_num(stamp.mtime_ms),
        ]);
        if if_none_match_satisfied(if_none_match.as_deref(), &etag) {
            return Ok(PageOutcome::NotModified(etag));
        }
        let bytes = s.archives.read_entry(&path, entry)?;
        Ok(PageOutcome::Bytes(etag, page_mime_type(entry), bytes))
    })
    .await;

    match result {
        Ok(PageOutcome::NotFound) => json_reply(
            StatusCode::NOT_FOUND,
            None,
            json!({ "error": true, "message": "Page not found" }),
        ),
        Ok(PageOutcome::NotModified(etag)) => {
            (StatusCode::NOT_MODIFIED, cache_headers(&etag)).into_response()
        }
        Ok(PageOutcome::Bytes(etag, mime, bytes)) => (
            StatusCode::OK,
            cache_headers(&etag),
            [(header::CONTENT_TYPE, mime)],
            bytes,
        )
            .into_response(),
        Err(e) => failure(&e, None, None),
    }
}
