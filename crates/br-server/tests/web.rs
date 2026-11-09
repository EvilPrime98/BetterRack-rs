use axum::body::Body;
use axum::http::{Request, StatusCode};
use br_core::config::Config;
use br_server::{app_with_web, state::AppState};
use tower::ServiceExt;

async fn get(router: &axum::Router, uri: &str) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

fn router(web: bool) -> (tempfile::TempDir, axum::Router) {
    let dir = tempfile::tempdir().unwrap();
    let web_dir = dir.path().join("web");
    std::fs::create_dir(&web_dir).unwrap();
    std::fs::write(web_dir.join("index.html"), "<html>app</html>").unwrap();
    std::fs::write(web_dir.join("a.js"), "console.log(1)").unwrap();
    let mut config = Config::from_lookup(|_| None, dir.path().to_path_buf());
    config.api_key = Some("k".into());
    let router = app_with_web(AppState::open(config).unwrap(), web.then_some(web_dir));
    (dir, router)
}

#[tokio::test]
async fn serves_static_files_without_the_key() {
    let (_d, r) = router(true);
    assert_eq!(
        get(&r, "/").await,
        (StatusCode::OK, "<html>app</html>".into())
    );
    assert_eq!(get(&r, "/a.js").await.1, "console.log(1)");
    assert_eq!(get(&r, "/some/spa/route").await.1, "<html>app</html>");
}

#[tokio::test]
async fn api_stays_gated_and_unknown_api_is_404() {
    let (_d, r) = router(true);
    assert_eq!(get(&r, "/api/library").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(get(&r, "/read/x").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(get(&r, "/api/nope?key=k").await.0, StatusCode::NOT_FOUND);
    assert_eq!(get(&r, "/healthz").await.0, StatusCode::OK);
}

#[tokio::test]
async fn without_web_dir_it_is_api_only() {
    let (_d, r) = router(false);
    assert_eq!(get(&r, "/").await.0, StatusCode::NOT_FOUND);
}
