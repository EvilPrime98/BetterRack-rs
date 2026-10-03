//! `/api/wiki/*` and wiki-backed identify through the router, with `br-wiki` replaying real
//! Fandom responses (recorded for the `br-wiki` parity tests) from a local mock.

use axum::body::Body;
use axum::extract::{RawQuery, State};
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use br_core::config::Config;
use br_core::uid::{resolve_windows, uid_from_path};
use br_server::state::AppState;
use br_server::wiki::LiveWiki;
use br_server::app;
use br_wiki::WikiService;
use br_wiki::client::ClientOptions;
use br_wiki::plugins::Plugin;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

const MARVEL: &str = "https://marvel.fandom.com";
const HULK: &str = "Immortal Hulk 001 (2018).cbz";

fn recordings(wiki: &str) -> HashMap<String, Value> {
    let mut all: HashMap<String, HashMap<String, Value>> = serde_json::from_str(include_str!("../../br-wiki/tests/fixtures/wiki_recordings.json")).unwrap();
    all.remove(wiki).unwrap()
}

fn golden(op: &str, wiki: &str, key: &str) -> Value {
    let cases: Vec<Value> = serde_json::from_str(include_str!("../../br-wiki/tests/fixtures/wiki_golden.json")).unwrap();
    let case = cases.into_iter().find(|c| c["wiki"] == wiki && c["op"] == op && (c["title"] == key || c["pageId"].to_string() == key)).expect("golden case");
    serde_json::from_str(&case["result"].to_string().replace("{WIKI}", MARVEL)).unwrap()
}

async fn mock_api(State(responses): State<Arc<HashMap<String, Value>>>, RawQuery(q): RawQuery) -> axum::response::Response {
    match responses.get(&format!("?{}", q.unwrap_or_default())) {
        Some(v) => axum::Json(v.clone()).into_response(),
        // Anything not recorded behaves like a search with no results.
        None => axum::Json(json!({ "batchcomplete": "" })).into_response(),
    }
}

struct Harness {
    _data: tempfile::TempDir,
    lib: tempfile::TempDir,
    router: axum::Router,
}

async fn harness(wiki_search: bool) -> Harness {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = axum::Router::new().route("/api.php", get(mock_api)).with_state(Arc::new(recordings("marvel")));
    tokio::spawn(async move { axum::serve(listener, mock).await.unwrap() });

    let opts = ClientOptions { retries: 0, backoff_base: Duration::ZERO, api_base: Some(base), ..ClientOptions::default() };
    let service = WikiService::with_wikis(&[(MARVEL.to_string(), Plugin::Marvel)], reqwest::Client::new(), opts);
    let wiki = Arc::new(LiveWiki::with_handle(service, tokio::runtime::Handle::current()));

    let data = tempfile::tempdir().unwrap();
    let lib = tempfile::tempdir().unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(lib.path().join(HULK)).unwrap());
    zip.start_file("1.png", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(b"not really a png").unwrap();
    zip.finish().unwrap();

    let config = Config::from_lookup(|_| None, data.path().to_path_buf());
    let state = AppState::open_with_wiki(config, wiki).unwrap();
    state.prefs.update_app_settings(json!({ "outputDirs": [lib.path().to_string_lossy()], "wikiSearch": wiki_search }).as_object().unwrap()).unwrap();
    state.rescan_library().unwrap();
    Harness { _data: data, lib, router: app(state) }
}

impl Harness {
    async fn get(&self, uri: &str) -> (StatusCode, Value) {
        let res = self.router.clone().oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap()).await.unwrap();
        let status = res.status();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into_owned())))
    }

    fn uid(&self, rel: &str) -> String {
        let cwd = self.lib.path().to_string_lossy().into_owned();
        uid_from_path(&resolve_windows(&self.lib.path().join(rel).to_string_lossy(), &cwd))
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn parameter_validation_matches_the_controller() {
    let h = harness(false).await;
    for uri in ["/api/wiki/comic", "/api/wiki/comic?title=", "/api/wiki/comics"] {
        assert_eq!(h.get(uri).await, (StatusCode::BAD_REQUEST, json!({ "error": true, "message": "A comic title is required." })), "{uri}");
    }
    for uri in ["/api/wiki/comic/abc", "/api/wiki/comic/0", "/api/wiki/comic/abc?sourceWiki=https%3A%2F%2Fmarvel.fandom.com"] {
        assert_eq!(h.get(uri).await, (StatusCode::BAD_REQUEST, json!({ "error": true, "message": "A valid page id is required." })), "{uri}");
    }
    for uri in ["/api/wiki/comic/5", "/api/wiki/comic/5?sourceWiki=https%3A%2F%2Fexample.com", "/api/wiki/comic/5?sourceWiki="] {
        assert_eq!(h.get(uri).await, (StatusCode::BAD_REQUEST, json!({ "error": true, "message": "A valid sourceWiki is required." })), "{uri}");
    }
    // Not a page id the wiki could have: answered as "not found" without asking it.
    assert_eq!(h.get("/api/wiki/comic/1.5?sourceWiki=https%3A%2F%2Fmarvel.fandom.com").await, (StatusCode::OK, Value::Null));
    assert_eq!(h.get("/api/wiki/comic/-4?sourceWiki=https%3A%2F%2Fmarvel.fandom.com").await, (StatusCode::OK, Value::Null));
}

#[tokio::test(flavor = "multi_thread")]
async fn comic_lookup_returns_what_better_wiki_returned() {
    let h = harness(false).await;
    let (s, v) = h.get("/api/wiki/comic?title=Immortal%20Hulk%20001%20(2018).cbz&thumbnailSize=120").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v, golden("single", "marvel", HULK));
    assert_eq!(v["sourceWiki"], MARVEL);

    // The default size is 120 as well.
    assert_eq!(h.get("/api/wiki/comic?title=Immortal%20Hulk%20001%20(2018).cbz").await.1, v);

    let (s, by_id) = h.get("/api/wiki/comic/1132748?sourceWiki=https%3A%2F%2Fmarvel.fandom.com&thumbnailSize=120").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(by_id, golden("byId", "marvel", "1132748"));

    // No match is a 200 with `null` (and `[]` for the list).
    assert_eq!(h.get("/api/wiki/comic?title=zzqx%20nothing%20here").await, (StatusCode::OK, Value::Null));
    assert_eq!(h.get("/api/wiki/comics?title=zzqx%20nothing%20here").await, (StatusCode::OK, json!([])));
}

#[tokio::test(flavor = "multi_thread")]
async fn identify_falls_back_to_the_wiki_only_when_wiki_search_is_on() {
    let off = harness(false).await;
    let (_, v) = off.get(&format!("/api/library/{}/identify", off.uid(HULK))).await;
    assert_eq!(v, json!({ "identified": false }));

    let on = harness(true).await;
    let (s, v) = on.get(&format!("/api/library/{}/identify", on.uid(HULK))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!((v["identified"].as_bool(), v["metaSource"].as_str()), (Some(true), Some("wiki")));
    assert_eq!(v["comic"], golden("single", "marvel", HULK));

    // Stored with the page id and wiki, as the Bun server does.
    let (_, data) = on.get("/api/comic-data").await;
    let stored = &data[on.uid(HULK)];
    assert_eq!((stored["prefId"].as_i64(), stored["sourceWiki"].as_str()), (v["comic"]["pageId"].as_i64(), Some(MARVEL)));
}
