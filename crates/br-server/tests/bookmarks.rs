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
    if body.is_none() {}
    let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
    let res = r.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

fn post(id: Option<i64>, link: &str) -> Value {
    json!({ "id": id, "title": "T", "link": link, "thumbnailUrl": null, "uploadDate": null })
}

#[tokio::test]
async fn requires_the_key() {
    let d = tempfile::tempdir().unwrap();
    let r = router_at(d.path());
    let (status, _) = call(&r, Method::GET, "/api/bookmarks", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn crud_and_dedupe() {
    let d = tempfile::tempdir().unwrap();
    let r = router_at(d.path());
    let (s, b) = call(
        &r,
        Method::POST,
        "/api/bookmarks?key=k",
        Some(json!({ "post": post(Some(7), "x"), "folder": "f1" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["key"], "id:7");
    assert_eq!(b["folder"], "f1");

    let (_, b) = call(
        &r,
        Method::POST,
        "/api/bookmarks?key=k",
        Some(json!({ "post": post(Some(7), "x"), "folder": "f2" })),
    )
    .await;
    assert_eq!(b["folder"], "f2");
    let (_, list) = call(&r, Method::GET, "/api/bookmarks?key=k", None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);

    let (s, b) = call(
        &r,
        Method::PATCH,
        "/api/bookmarks/id%3A7?key=k",
        Some(json!({ "rating": 3.5, "read": true, "folder": null })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["rating"], 3.5);
    assert_eq!(b["read"], true);
    assert_eq!(b["folder"], Value::Null);

    let (_, b) = call(
        &r,
        Method::PATCH,
        "/api/bookmarks/id%3A7?key=k",
        Some(json!({ "rating": 1.0 })),
    )
    .await;
    assert_eq!(b["read"], true, "absent fields are left alone");

    let (s, _) = call(
        &r,
        Method::PATCH,
        "/api/bookmarks/id%3A9?key=k",
        Some(json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, _) = call(&r, Method::DELETE, "/api/bookmarks/id%3A7?key=k", None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&r, Method::DELETE, "/api/bookmarks/id%3A7?key=k", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn posts_without_an_id_are_keyed_by_link() {
    let d = tempfile::tempdir().unwrap();
    let r = router_at(d.path());
    let link = "https://a.b/c?d=1";
    let (_, b) = call(
        &r,
        Method::POST,
        "/api/bookmarks?key=k",
        Some(json!({ "post": post(None, link) })),
    )
    .await;
    assert_eq!(b["key"], format!("link:{link}"));
    let enc = "link%3Ahttps%3A%2F%2Fa.b%2Fc%3Fd%3D1";
    let (s, _) = call(
        &r,
        Method::DELETE,
        &format!("/api/bookmarks/{enc}?key=k"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn persists_across_restarts() {
    let d = tempfile::tempdir().unwrap();
    {
        let r = router_at(d.path());
        call(
            &r,
            Method::POST,
            "/api/bookmarks?key=k",
            Some(json!({ "post": post(Some(1), "x") })),
        )
        .await;
    }
    let r = router_at(d.path());
    let (_, list) = call(&r, Method::GET, "/api/bookmarks?key=k", None).await;
    assert_eq!(list[0]["key"], "id:1");
}

#[tokio::test]
async fn a_corrupt_file_loads_empty() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("bookmarks.json"), "{not json").unwrap();
    let r = router_at(d.path());
    let (s, list) = call(&r, Method::GET, "/api/bookmarks?key=k", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list, json!([]));
}
