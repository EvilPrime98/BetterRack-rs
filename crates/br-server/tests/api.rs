//! In-process tests of the Phase B endpoints against a throwaway data dir.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use br_core::config::Config;
use br_server::{app, state::AppState};
use serde_json::{Value, json};
use tower::ServiceExt;

struct Harness {
    _dir: tempfile::TempDir,
    router: axum::Router,
}

fn harness(api_key: Option<&str>) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::from_lookup(|_| None, dir.path().to_path_buf());
    config.api_key = api_key.map(str::to_string);
    let router = app(AppState::open(config).unwrap());
    Harness { _dir: dir, router }
}

async fn call(h: &Harness, method: &str, uri: &str, body: Option<Value>, key: Option<&str>) -> (StatusCode, String) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(k) = key {
        req = req.header("x-br-api-key", k);
    }
    let body = body.map_or(Body::empty(), |b| Body::from(b.to_string()));
    let res = h.router.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

async fn json_call(h: &Harness, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let (s, t) = call(h, method, uri, body, None).await;
    (s, serde_json::from_str(&t).unwrap_or(Value::String(t)))
}

#[tokio::test]
async fn healthz_and_unknown_route() {
    let h = harness(None);
    let (s, v) = json_call(&h, "GET", "/healthz", None).await;
    assert_eq!((s, v["app"].as_str()), (StatusCode::OK, Some("betterrack")));
    let (s, t) = call(&h, "GET", "/api/nope", None, None).await;
    assert_eq!((s, t.as_str()), (StatusCode::NOT_FOUND, "Not Found"));
}

#[tokio::test]
async fn settings_round_trip_and_output_dirs_are_protected() {
    let h = harness(None);
    let (_, v) = json_call(&h, "GET", "/api/settings", None).await;
    assert_eq!(v, json!({"outputDirs": [], "apiUrl": "", "downloadDir": "", "wikiSearch": false, "rescanOnStartup": true}));

    let (s, v) = json_call(&h, "PUT", "/api/settings", Some(json!({"apiUrl": "https://x.test", "wikiSearch": true, "outputDirs": ["C:\\evil"]}))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!((v["apiUrl"].as_str(), v["wikiSearch"].as_bool(), v["outputDirs"].as_array().unwrap().len()), (Some("https://x.test"), Some(true), 0));

    let (s, t) = call(&h, "PUT", "/api/settings", None, None).await;
    assert_eq!((s, t.as_str()), (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"));
}

#[tokio::test]
async fn library_folders_add_remove_validate() {
    let h = harness(None);
    let lib = tempfile::tempdir().unwrap();
    let path = lib.path().to_string_lossy().into_owned();

    let (s, v) = json_call(&h, "POST", "/api/settings/library-folder", Some(json!({}))).await;
    assert_eq!((s, v["message"].as_str()), (StatusCode::UNPROCESSABLE_ENTITY, Some("path is required")));

    let (s, v) = json_call(&h, "POST", "/api/settings/library-folder", Some(json!({"path": "Z:\\no\\such\\dir"}))).await;
    assert_eq!((s, v["error"].as_bool(), v["message"].as_str()), (StatusCode::BAD_REQUEST, Some(true), Some("Folder does not exist.")));

    let (s, v) = json_call(&h, "POST", "/api/settings/library-folder", Some(json!({"path": path}))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["outputDirs"].as_array().unwrap().len(), 1);
    // Adding again is a no-op.
    let (_, v) = json_call(&h, "POST", "/api/settings/library-folder", Some(json!({"path": path}))).await;
    assert_eq!(v["outputDirs"].as_array().unwrap().len(), 1);

    let (_, v) = json_call(&h, "GET", "/api/directories", None).await;
    assert_eq!(v["directories"].as_array().unwrap().len(), 1);

    let (s, v) = json_call(&h, "DELETE", "/api/settings/library-folder", Some(json!({"path": path}))).await;
    assert_eq!((s, v["outputDirs"].as_array().unwrap().len()), (StatusCode::OK, 0));
}

#[tokio::test]
async fn comic_data_patch_and_list() {
    let h = harness(None);
    assert_eq!(json_call(&h, "GET", "/api/comic-data", None).await.1, json!({}));
    let (s, v) = json_call(&h, "PATCH", "/api/comic-data/abc", Some(json!({"rating": 5, "read": true, "currentPage": 2}))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!((v["uid"].as_str(), v["rating"].as_i64(), v["read"].as_bool()), (Some("abc"), Some(5), Some(true)));
    assert!(v["lastReadAt"].as_i64().unwrap() > 0);
    let (_, all) = json_call(&h, "GET", "/api/comic-data", None).await;
    assert_eq!(all["abc"], v);
}

#[tokio::test]
async fn api_key_guard() {
    let h = harness(Some("s3cret"));
    let (s, t) = call(&h, "GET", "/api/settings", None, None).await;
    assert_eq!((s, t.as_str()), (StatusCode::UNAUTHORIZED, "Unauthorized"));
    assert_eq!(call(&h, "GET", "/api/nope", None, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&h, "GET", "/api/settings", None, Some("wrong")).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&h, "GET", "/api/settings", None, Some("s3cret")).await.0, StatusCode::OK);
    assert_eq!(call(&h, "GET", "/api/settings?key=s3cret", None, None).await.0, StatusCode::OK);
    assert_eq!(call(&h, "GET", "/healthz", None, None).await.0, StatusCode::OK);
}
