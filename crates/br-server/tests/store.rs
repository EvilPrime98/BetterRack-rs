//! Store + downloads through the router, against a local mock of the store API and file host.

use axum::body::Body;
use axum::extract::Path;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use br_core::config::Config;
use br_core::download::RetryOpts;
use br_core::store::StoreTiming;
use br_core::wiki::NoWiki;
use br_server::state::{AppState, NetOptions};
use br_server::app;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

struct Harness {
    _dir: tempfile::TempDir,
    out: tempfile::TempDir,
    router: axum::Router,
    base: String,
}

async fn harness(configure_store: bool) -> Harness {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let b = base.clone();
    let mock = axum::Router::new()
        .route(
            "/wp/v2/posts",
            get(|| async {
                axum::Json(json!([{"id": 1, "title": {"rendered": "Batman &amp;#038; Robin"}, "link": "https://x.test/b/", "jetpack_featured_media_url": "https://x.test/c.jpg", "date": "2024-01-01T00:00:00"},
                                  {"id": 2, "title": {"rendered": "Superman"}, "link": "https://x.test/s/", "jetpack_featured_media_url": "", "date": "2024-01-02T00:00:00"}]))
            }),
        )
        .route(
            "/wp/v2/posts/{id}",
            get(move |Path(id): Path<String>| {
                let b = b.clone();
                async move {
                    if id != "1" {
                        return (StatusCode::NOT_FOUND, axum::Json(json!({}))).into_response_();
                    }
                    let html = format!(r#"<h2>Download Free Comic</h2><p><strong>Batman 1</strong></p><a class="aio-red" title="Download Now" href="{b}/files/batman.cbz">Download Now</a>"#);
                    (StatusCode::OK, axum::Json(json!({"content": {"rendered": html}, "jetpack_featured_media_url": ""}))).into_response_()
                }
            }),
        )
        .route("/files/batman.cbz", get(|| async { "COMICBYTES" }));
    tokio::spawn(async move { axum::serve(listener, mock).await.unwrap() });

    let dir = tempfile::tempdir().unwrap();
    let config = Config::from_lookup(|_| None, dir.path().to_path_buf());
    let net = NetOptions {
        store_timing: StoreTiming::instant(),
        download_retry: RetryOpts { max_retries: 1, backoff_ms: 0, backoff_cap_ms: 0, request_delay_ms: 0 },
        rotating: Some((0, 0, 0)),
        pixeldrain: None,
    };
    let state = AppState::open_with(config, Arc::new(NoWiki), net).unwrap();
    if configure_store {
        state.prefs.update_app_settings(json!({"apiUrl": format!("{base}/wp/v2")}).as_object().unwrap()).unwrap();
    }
    Harness { router: app(state), _dir: dir, out: tempfile::tempdir().unwrap(), base }
}

trait IntoResponse_ {
    fn into_response_(self) -> axum::response::Response;
}
impl<T: axum::response::IntoResponse> IntoResponse_ for T {
    fn into_response_(self) -> axum::response::Response {
        axum::response::IntoResponse::into_response(self)
    }
}

async fn call(h: &Harness, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let req = Request::builder().method(method).uri(uri).header("content-type", "application/json");
    let body = body.map_or(Body::empty(), |b| Body::from(b.to_string()));
    let res = h.router.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn store_routes_answer_409_until_the_api_url_is_set() {
    let h = harness(false).await;
    for (m, u) in [("GET", "/api/comics?search=x"), ("GET", "/api/comics/1/links"), ("POST", "/api/downloads"), ("GET", "/api/downloads?id=1")] {
        let (s, v) = call(&h, m, u, None).await;
        assert_eq!(s, StatusCode::CONFLICT, "{u}");
        assert_eq!(v["error"], true);
    }
    let (s, _) = call(&h, "GET", "/api/downloads/jobs", None).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn search_decodes_titles_and_requires_a_query() {
    let h = harness(true).await;
    let (s, v) = call(&h, "GET", "/api/comics?search=bat", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v[0], json!({"id": 1, "title": "Batman & Robin", "link": "https://x.test/b/", "thumbnailUrl": "https://x.test/c.jpg", "uploadDate": "2024-01-01T00:00:00"}));
    let (_, exact) = call(&h, "GET", "/api/comics?search=superman&exact=true", None).await;
    assert_eq!(exact.as_array().unwrap().len(), 1);
    let (s, v) = call(&h, "GET", "/api/comics", None).await;
    assert_eq!((s, v), (StatusCode::UNPROCESSABLE_ENTITY, json!([])));
}

#[tokio::test]
async fn links_then_download_runs_to_done_and_the_file_lands() {
    let h = harness(true).await;
    let (s, v) = call(&h, "GET", "/api/comics/1/links", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["links"][0]["title"], "Batman 1");
    assert!(v["links"][0].get("downloadLink").is_none());
    let uuid = v["links"][0]["uuid"].as_str().unwrap().to_string();

    let (s, v) = call(&h, "GET", "/api/comics/99/links", None).await;
    assert_eq!((s, v["message"].as_str()), (StatusCode::NOT_FOUND, Some("No links found")));

    let dir = h.out.path().to_string_lossy().to_string();
    let (s, v) = call(&h, "POST", "/api/downloads", Some(json!({"id": 1, "title": "Batman 1", "uuid": uuid, "outputDir": dir, "strat": "all"}))).await;
    assert_eq!(s, StatusCode::CREATED);
    let job_id = v["jobId"].as_str().unwrap().to_string();

    let mut state = String::new();
    for _ in 0..100 {
        let (_, v) = call(&h, "GET", &format!("/api/downloads/{job_id}"), None).await;
        state = v["state"].as_str().unwrap().to_string();
        if state == "done" || state == "error" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(state, "done");
    assert_eq!(std::fs::read_to_string(h.out.path().join("batman.cbz")).unwrap(), "COMICBYTES");

    let (_, v) = call(&h, "GET", "/api/downloads/jobs", None).await;
    assert_eq!(v["jobs"][0]["jobId"], job_id.as_str());
    // A finished job cannot be stopped; unknown jobs are 404.
    let (s, _) = call(&h, "DELETE", &format!("/api/downloads/{job_id}"), None).await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = call(&h, "DELETE", "/api/downloads/missing", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(&h, "POST", &format!("/api/downloads/{job_id}/retry"), None).await;
    assert_eq!(s, StatusCode::CONFLICT);
    let _ = &h.base;
}

#[tokio::test]
async fn download_validation() {
    let h = harness(true).await;
    let (s, _) = call(&h, "POST", "/api/downloads", Some(json!({"title": "x"}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let (s, v) = call(&h, "POST", "/api/downloads", Some(json!({"id": 99, "title": "x", "uuid": "u"}))).await;
    assert_eq!((s, v["message"].as_str()), (StatusCode::BAD_REQUEST, Some("This comic cannot be downloaded")));
    let (_, v) = call(&h, "GET", "/api/downloads/resource/1", None).await;
    assert_eq!(v, json!({"error": false, "job": null}));
}
