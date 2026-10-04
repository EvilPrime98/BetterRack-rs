//! Parity harness: sends every request in `contract/requests.txt` to a reference (Bun) server and
//! a candidate (Rust) server and diffs status, `ETag`/`Cache-Control`/`Content-Type` and JSON body
//! (key-order insensitive, ignoring `generatedAt`).
//!
//! Run (both servers must use copies of the same data dir and the shared test library, never the
//! user's real files):
//!   BR_REF_URL=http://127.0.0.1:3000 BR_CAND_URL=http://127.0.0.1:3001 \
//!   cargo test -p br-server --test contract -- --ignored --nocapture

use serde_json::Value;

const REQUESTS: &str = include_str!("contract/requests.txt");
const HEADERS: [&str; 3] = ["etag", "cache-control", "content-type"];

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn send(base: &str, method: &str, path: &str, key: Option<&str>) -> Reply {
    let client = reqwest::blocking::Client::new();
    let m = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
    let mut req = client.request(m, format!("{base}{path}"));
    if let Some(k) = key {
        req = req.header("x-br-api-key", k);
    }
    let r = req
        .send()
        .unwrap_or_else(|e| panic!("{method} {base}{path}: {e}"));
    let status = r.status().as_u16();
    let headers = HEADERS
        .iter()
        .filter_map(|h| {
            r.headers()
                .get(*h)
                .map(|v| (h.to_string(), v.to_str().unwrap_or("").to_string()))
        })
        .collect();
    Reply {
        status,
        headers,
        body: r.bytes().unwrap().to_vec(),
    }
}

/// Drop volatile keys; objects compare order-insensitively already (`serde_json::Map` is sorted).
fn normalize(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.remove("generatedAt");
            m.values_mut().for_each(normalize);
        }
        Value::Array(a) => a.iter_mut().for_each(normalize),
        _ => {}
    }
}

fn diff(a: &Reply, b: &Reply) -> Vec<String> {
    let mut out = Vec::new();
    if a.status != b.status {
        out.push(format!("status {} != {}", a.status, b.status));
    }
    if a.headers != b.headers {
        out.push(format!("headers {:?} != {:?}", a.headers, b.headers));
    }
    match (
        serde_json::from_slice::<Value>(&a.body),
        serde_json::from_slice::<Value>(&b.body),
    ) {
        (Ok(mut x), Ok(mut y)) => {
            normalize(&mut x);
            normalize(&mut y);
            if x != y {
                out.push("json body differs".into());
            }
        }
        _ if a.body != b.body => out.push(format!(
            "body bytes differ ({} vs {})",
            a.body.len(),
            b.body.len()
        )),
        _ => {}
    }
    out
}

#[test]
#[ignore = "needs BR_REF_URL and BR_CAND_URL pointing at running servers"]
fn rust_server_matches_bun_server() {
    let reference = std::env::var("BR_REF_URL").expect("BR_REF_URL");
    let candidate = std::env::var("BR_CAND_URL").expect("BR_CAND_URL");
    let key = std::env::var("BR_API_KEY").ok();

    let mut failures = 0;
    for line in REQUESTS
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let (method, path) = line.split_once(' ').expect("METHOD PATH");
        let (a, b) = (
            send(&reference, method, path, key.as_deref()),
            send(&candidate, method, path, key.as_deref()),
        );
        let d = diff(&a, &b);
        if d.is_empty() {
            println!("ok    {method} {path}");
        } else {
            failures += 1;
            println!("FAIL  {method} {path}\n      {}", d.join("\n      "));
        }
    }
    assert_eq!(failures, 0, "{failures} request(s) differ");
}

#[test]
fn diff_ignores_generated_at_and_key_order() {
    let mk = |s: &str| Reply {
        status: 200,
        headers: vec![],
        body: s.as_bytes().to_vec(),
    };
    assert!(
        diff(
            &mk(r#"{"a":1,"b":2,"generatedAt":5}"#),
            &mk(r#"{"b":2,"a":1,"generatedAt":9}"#)
        )
        .is_empty()
    );
    assert!(!diff(&mk(r#"{"a":1}"#), &mk(r#"{"a":2}"#)).is_empty());
}
