use crate::error::text;
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use br_core::config::keys_match;

const HEADER_NAME: &str = "x-br-api-key";
const QUERY_PARAM_NAME: &str = "key";

fn guarded(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/") || path == "/read" || path.starts_with("/read/")
}

fn query_key(query: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|kv| kv.split_once('=').or(Some((kv, ""))))
        .find(|(k, _)| *k == QUERY_PARAM_NAME)
        .map(|(_, v)| percent_decode(v))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if b.get(i + 1..i + 3).is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)) => {
                out.push(u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("0"), 16).unwrap_or(0));
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `BR_API_KEY` guard for `/api/*` and `/read/*`: header `x-br-api-key`, falling back to `?key=`
/// (which `<img src>` cannot do with a header). Open when no key is configured.
pub async fn api_key_auth(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let Some(required) = state.config.api_key.as_deref() else { return next.run(req).await };
    if !guarded(req.uri().path()) {
        return next.run(req).await;
    }
    let provided = req
        .headers()
        .get(HEADER_NAME)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or_else(|| req.uri().query().and_then(query_key));
    match provided {
        Some(p) if keys_match(&p, required) => next.run(req).await,
        _ => text(StatusCode::UNAUTHORIZED, "Unauthorized"),
    }
}

pub use br_core::store::derive_store_origin;

/// 409 unless the store API URL is configured; wired onto `/api/comics*`, `GET`/`POST
/// /api/downloads` and `POST /api/downloads/:jobId/retry`.
pub async fn require_store_api_url(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let prefs = state.prefs.clone();
    let configured = tokio::task::spawn_blocking(move || prefs.get_app_settings().map(|s| !derive_store_origin(&s.api_url).is_empty()))
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false);
    if configured {
        next.run(req).await
    } else {
        crate::error::ApiError::Json(StatusCode::CONFLICT, "Store API URL is not configured. Set it in Settings.".into()).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_key_parsing() {
        assert_eq!(query_key("a=1&key=s%20x&b=2").as_deref(), Some("s x"));
        assert_eq!(query_key("a=1"), None);
    }

    #[test]
    fn store_origin() {
        assert_eq!(derive_store_origin(" https://Getcomics.org/api/x "), "https://getcomics.org");
        assert_eq!(derive_store_origin("http://host:8080/p"), "http://host:8080");
        assert_eq!(derive_store_origin(""), "");
        assert_eq!(derive_store_origin("not a url"), "");
        assert_eq!(derive_store_origin("https://"), "");
    }
}
