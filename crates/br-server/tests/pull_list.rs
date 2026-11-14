use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use br_core::config::Config;
use br_server::{app_with_web, state::AppState};
use serde_json::{Value, json};
use tower::ServiceExt;

fn router_at(dir: &std::path::Path) -> axum::Router {
    let mut config = Config::from_lookup(|_| None, dir.to_path_buf());
    config.api_key = Some("k".into());
    app_with_web(AppState::open(config).unwrap(), None)
}

async fn call(
    r: &axum::Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
    let res = r.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

#[tokio::test]
async fn requires_the_key() {
    let d = tempfile::tempdir().unwrap();
    let r = router_at(d.path());
    let (status, _) = call(&r, Method::GET, "/api/pull-list", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn crud_and_dedupe() {
    let d = tempfile::tempdir().unwrap();
    let r = router_at(d.path());
    let (s, e) = call(
        &r,
        Method::POST,
        "/api/pull-list?key=k",
        Some(json!({ "keyword": " Batman ", "exclude": "annual, special", "lastSeenId": 5 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(e["keyword"], "Batman");
    assert_eq!(e["exclude"], "annual, special");
    assert_eq!(e["lastSeenId"], 5);
    let id = e["id"].as_str().unwrap().to_string();

    let (_, again) = call(
        &r,
        Method::POST,
        "/api/pull-list?key=k",
        Some(json!({ "keyword": "batman" })),
    )
    .await;
    assert_eq!(again["id"], id.as_str(), "same keyword does not duplicate");
    let (_, list) = call(&r, Method::GET, "/api/pull-list?key=k", None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);

    let (s, e) = call(
        &r,
        Method::PATCH,
        &format!("/api/pull-list/{id}?key=k"),
        Some(json!({ "lastSeenId": 9 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(e["lastSeenId"], 9);
    assert_eq!(e["keyword"], "Batman", "absent fields are left alone");

    let (s, _) = call(
        &r,
        Method::PATCH,
        &format!("/api/pull-list/{id}?key=k"),
        Some(json!({ "keyword": "  " })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &r,
        Method::POST,
        "/api/pull-list?key=k",
        Some(json!({ "keyword": "" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &r,
        Method::PATCH,
        "/api/pull-list/nope?key=k",
        Some(json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, _) = call(
        &r,
        Method::DELETE,
        &format!("/api/pull-list/{id}?key=k"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &r,
        Method::DELETE,
        &format!("/api/pull-list/{id}?key=k"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn persists_across_restarts_and_corrupt_file_loads_empty() {
    let d = tempfile::tempdir().unwrap();
    {
        let r = router_at(d.path());
        call(
            &r,
            Method::POST,
            "/api/pull-list?key=k",
            Some(json!({ "keyword": "Saga" })),
        )
        .await;
    }
    let r = router_at(d.path());
    let (_, list) = call(&r, Method::GET, "/api/pull-list?key=k", None).await;
    assert_eq!(list[0]["keyword"], "Saga");

    std::fs::write(d.path().join("pull-list.json"), "{not json").unwrap();
    let r = router_at(d.path());
    let (s, list) = call(&r, Method::GET, "/api/pull-list?key=k", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list, json!([]));
}
