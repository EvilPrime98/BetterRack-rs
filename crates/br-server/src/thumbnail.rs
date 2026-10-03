//! `/api/thumbnail/:uuid` and `/api/thumbnail/:uuid/retry` (port of `thumbnailController`).

use crate::http_cache::{if_none_match_satisfied, json_num, json_str, strong_etag};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use br_core::archive::file_stamp;
use serde_json::json;
use std::path::PathBuf;

const GENERIC: &str = "There was an error generating the thumbnail.";
const UNAVAILABLE: &str = "No thumbnail available for this comic.";

fn fail(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": true, "message": message }))).into_response()
}

fn content_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("webp") => "image/webp",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    }
}

/// The comic file of a library entry (folders have none).
fn file_path_for(s: &AppState, uid: &str) -> Option<PathBuf> {
    s.library.get(uid).filter(|e| !e.did).map(|e| PathBuf::from(e.path))
}

enum Served {
    Missing,
    NotModified(String),
    File(String, &'static str, Vec<u8>),
}

pub async fn get(State(s): State<AppState>, Path(uid): Path<String>, headers: HeaderMap) -> Response {
    let if_none_match = headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()).map(str::to_string);
    let result = tokio::task::spawn_blocking(move || -> br_core::Result<Served> {
        s.library.wait_ready();
        let file = file_path_for(&s, &uid);
        let Some(thumb) = s.thumbnails.get_thumbnail(&uid, file.as_deref()) else { return Ok(Served::Missing) };
        let Ok(stamp) = file_stamp(&thumb) else { return Ok(Served::Missing) };
        // A re-identify can regenerate the thumbnail at the same path, so the ETag (path + size +
        // mtime) pairs with a day-long max-age rather than `immutable`.
        let etag = strong_etag(&[json_str(&thumb.to_string_lossy()), stamp.size.to_string(), json_num(stamp.mtime_ms.floor())]);
        if if_none_match_satisfied(if_none_match.as_deref(), &etag) {
            return Ok(Served::NotModified(etag));
        }
        Ok(Served::File(etag, content_type(&thumb), std::fs::read(&thumb)?))
    })
    .await;

    let cache = |etag: &str| [(header::CACHE_CONTROL, "private, max-age=86400".to_string()), (header::ETAG, etag.to_string())];
    match result {
        Ok(Ok(Served::Missing)) => fail(StatusCode::NOT_FOUND, UNAVAILABLE),
        Ok(Ok(Served::NotModified(etag))) => (StatusCode::NOT_MODIFIED, cache(&etag)).into_response(),
        Ok(Ok(Served::File(etag, mime, bytes))) => (StatusCode::OK, cache(&etag), [(header::CONTENT_TYPE, mime)], bytes).into_response(),
        Ok(Err(e)) => {
            tracing::error!(err = %e, "failed to serve thumbnail");
            fail(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
        }
        Err(e) => {
            tracing::error!(err = %e, "thumbnail task failed");
            fail(StatusCode::INTERNAL_SERVER_ERROR, GENERIC)
        }
    }
}

pub async fn retry(State(s): State<AppState>, Path(uid): Path<String>) -> Response {
    let result = tokio::task::spawn_blocking(move || {
        s.library.wait_ready();
        let file = file_path_for(&s, &uid)?;
        Some(s.thumbnails.retry(&uid, &file).is_some())
    })
    .await;
    match result {
        Ok(None) => fail(StatusCode::NOT_FOUND, "No comic file found for this uid."),
        Ok(Some(false)) => fail(StatusCode::NOT_FOUND, UNAVAILABLE),
        Ok(Some(true)) => Json(json!({ "ok": true })).into_response(),
        Err(e) => {
            tracing::error!(err = %e, "thumbnail retry task failed");
            fail(StatusCode::INTERNAL_SERVER_ERROR, GENERIC)
        }
    }
}
