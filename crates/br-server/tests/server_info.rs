use axum::body::Body;
use axum::http::{Request, StatusCode};
use br_core::config::Config;
use br_server::{app_with_web, server_info::push_log, state::AppState};
use tower::ServiceExt;

fn router() -> (tempfile::TempDir, axum::Router) {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::from_lookup(|_| None, dir.path().to_path_buf());
    config.api_key = Some("k".into());
    let router = app_with_web(AppState::open(config).unwrap(), None);
    (dir, router)
}

async fn get(router: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let res = router
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

#[tokio::test]
async fn server_endpoints_require_the_key() {
    let (_d, r) = router();
    for path in ["/api/server/info", "/api/server/logs", "/api/server/update"] {
        assert_eq!(get(&r, path).await.0, StatusCode::UNAUTHORIZED, "{path}");
    }
}

#[tokio::test]
async fn info_reports_the_version() {
    let (_d, r) = router();
    let (status, body) = get(&r, "/api/server/info?key=k").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn logs_returns_the_requested_tail() {
    let (_d, r) = router();
    push_log("tail-test-a\ntail-test-b\n");
    let (status, body) = get(&r, "/api/server/logs?key=k&lines=2").await;
    assert_eq!(status, StatusCode::OK);
    let lines = body["lines"].as_array().unwrap();
    assert!(lines.len() <= 2);
    assert_eq!(lines.last().unwrap(), "tail-test-b");
}
