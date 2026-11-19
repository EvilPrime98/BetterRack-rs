use crate::error;
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tokio_util::io::ReaderStream;

pub fn content_disposition(name: &str) -> String {
    let fallback: String = name
        .chars()
        .map(|c| match c {
            '"' | '\\' | '%' => '_',
            c if c.is_ascii() && !c.is_ascii_control() => c,
            _ => '_',
        })
        .collect();
    let mut encoded = String::new();
    for b in name.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'!'
            | b'#'
            | b'$'
            | b'&'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~' => encoded.push(b as char),
            _ => encoded.push_str(&format!("%{b:02X}")),
        }
    }
    format!("attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}")
}

fn not_found(message: &str) -> Response {
    let body = serde_json::json!({ "error": true, "message": message });
    (StatusCode::NOT_FOUND, axum::Json(body)).into_response()
}

pub async fn get(State(s): State<AppState>, Path(uid): Path<String>) -> Response {
    let Some(path) = s.library.path_of(&uid).map(std::path::PathBuf::from) else {
        return not_found("File not found in library");
    };
    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return not_found("File not found"),
        Err(e) => {
            tracing::error!(err = %e, "comic file open failed");
            return error::text(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error");
        }
    };
    let len = file.metadata().await.ok().map(|m| m.len());
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "comic".to_string());
    let mut res = Response::new(Body::from_stream(ReaderStream::new(file)));
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(v) = HeaderValue::from_str(&content_disposition(&name)) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    if let Some(len) = len {
        h.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    }
    res
}

#[cfg(test)]
mod tests {
    use super::content_disposition;

    #[test]
    fn ascii_name() {
        assert_eq!(
            content_disposition("Issue 1.cbz"),
            "attachment; filename=\"Issue 1.cbz\"; filename*=UTF-8''Issue%201.cbz"
        );
    }

    #[test]
    fn non_ascii_and_quotes() {
        assert_eq!(
            content_disposition("Ñandú \"x\".cbz"),
            "attachment; filename=\"_and_ _x_.cbz\"; filename*=UTF-8''%C3%91and%C3%BA%20%22x%22.cbz"
        );
    }
}
