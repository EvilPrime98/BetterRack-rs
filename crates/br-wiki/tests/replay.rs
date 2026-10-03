//! Replays real Fandom responses (`tests/fixtures/record_wiki.mjs`) from a local mock and checks
//! that `br-wiki` returns exactly what the real `better-wiki` returned for the same scenarios,
//! key order included. A request the JS client never made is a 404 and fails the test, so this
//! also pins the request set.

use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use br_wiki::WikiService;
use br_wiki::client::ClientOptions;
use br_wiki::plugins::Plugin;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Mock {
    base: String,
    misses: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone)]
struct MockState {
    responses: Arc<HashMap<String, Value>>,
    misses: Arc<Mutex<Vec<String>>>,
}

async fn api(State(s): State<MockState>, RawQuery(q): RawQuery) -> axum::response::Response {
    let key = format!("?{}", q.unwrap_or_default());
    match s.responses.get(&key) {
        Some(v) => axum::Json(v.clone()).into_response(),
        None => {
            s.misses.lock().unwrap().push(key);
            (StatusCode::NOT_FOUND, "not recorded").into_response()
        }
    }
}

async fn serve(responses: HashMap<String, Value>) -> Mock {
    let misses = Arc::new(Mutex::new(Vec::new()));
    let state = MockState { responses: Arc::new(responses), misses: misses.clone() };
    let app = axum::Router::new().route("/api.php", get(api)).with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Mock { base, misses }
}

fn fast() -> ClientOptions {
    ClientOptions { retries: 0, backoff_base: Duration::ZERO, ..ClientOptions::default() }
}

fn recordings() -> HashMap<String, HashMap<String, Value>> {
    serde_json::from_str(include_str!("fixtures/wiki_recordings.json")).unwrap()
}

#[tokio::test]
async fn matches_better_wiki_on_recorded_wikis() {
    let all = recordings();
    let golden: Vec<Value> = serde_json::from_str(include_str!("fixtures/wiki_golden.json")).unwrap();
    assert!(golden.len() >= 9);

    let mut mocks: HashMap<String, Mock> = HashMap::new();
    for (key, responses) in all {
        mocks.insert(key, serve(responses).await);
    }

    let mut compared = 0;
    for case in &golden {
        let wiki = case["wiki"].as_str().unwrap();
        let op = case["op"].as_str().unwrap();
        let mock = &mocks[wiki];
        let plugin = if wiki == "marvel" { Plugin::Marvel } else { Plugin::Dc };
        let service = WikiService::with_wikis(&[(mock.base.clone(), plugin)], reqwest::Client::new(), fast());
        let label = format!("{wiki} {op} {}", case.get("title").or(case.get("pageId")).unwrap());

        let got: Value = match op {
            "single" => service.get_comic(case["title"].as_str().unwrap(), Some("120")).await.unwrap().into(),
            "multiple" => service.get_comics(case["title"].as_str().unwrap(), Some("120")).await.unwrap().into(),
            "byId" => service.get_comic_by_id(case["pageId"].as_i64().unwrap(), &mock.base, Some("120")).await.unwrap().into(),
            other => panic!("unknown op {other}"),
        };
        assert!(mock.misses.lock().unwrap().is_empty(), "{label}: requests the JS client never made: {:?}", mock.misses.lock().unwrap());

        let want: Value = serde_json::from_str(&case["result"].to_string().replace("{WIKI}", &mock.base)).unwrap();
        let (got_s, want_s) = (serde_json::to_string_pretty(&got).unwrap(), serde_json::to_string_pretty(&want).unwrap());
        assert_eq!(got_s, want_s, "{label}");
        compared += 1;
    }
    assert_eq!(compared, golden.len());
}
