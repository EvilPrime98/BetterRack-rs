//! In-process tests of the Phase C reader endpoints against a throwaway library.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use br_core::config::Config;
use br_core::uid::{resolve_windows, uid_from_path};
use br_server::{app, state::AppState};
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use tower::ServiceExt;

struct Harness {
    _data: tempfile::TempDir,
    lib: tempfile::TempDir,
    router: axum::Router,
}

const XML: &[u8] = br#"<ComicInfo><Series>S</Series><Pages><Page Image="1" Bookmark="Two"/></Pages></ComicInfo>"#;

fn make_cbz(path: &Path, entries: &[(&str, &[u8])]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, data) in entries {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

fn harness(api_key: Option<&str>) -> Harness {
    let data = tempfile::tempdir().unwrap();
    let lib = tempfile::tempdir().unwrap();
    make_cbz(&lib.path().join("Series/Issue 1.cbz"), &[("page10.png", b"ten"), ("page2.png", b"two"), ("page1.png", b"one"), ("ComicInfo.xml", XML)]);
    make_cbz(&lib.path().join("Bad.cbz"), &[("readme.txt", b"no images")]);

    let mut config = Config::from_lookup(|_| None, data.path().to_path_buf());
    config.api_key = api_key.map(str::to_string);
    let state = AppState::open(config).unwrap();
    state.prefs.update_app_settings(json!({ "outputDirs": [lib.path().to_string_lossy()] }).as_object().unwrap()).unwrap();
    state.rescan_library().unwrap();
    Harness { _data: data, lib, router: app(state) }
}

impl Harness {
    fn uid(&self, rel: &str) -> String {
        let cwd = self.lib.path().to_string_lossy().into_owned();
        uid_from_path(&resolve_windows(&self.lib.path().join(rel).to_string_lossy(), &cwd))
    }

    async fn get(&self, uri: &str, headers: &[(&str, &str)]) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let mut req = Request::builder().uri(uri);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let res = self.router.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
        let (status, headers) = (res.status(), res.headers().clone());
        (status, headers, axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec())
    }
}

fn json_of(b: &[u8]) -> Value {
    serde_json::from_slice(b).unwrap()
}

#[tokio::test]
async fn lists_pages_in_natural_order() {
    let h = harness(None);
    let (s, hd, body) = h.get(&format!("/read/{}", h.uid("Series/Issue 1.cbz")), &[]).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(hd[header::CACHE_CONTROL], "no-store");
    let v = json_of(&body);
    assert_eq!(v["error"], false);
    assert_eq!(v["message"], "Comic pages listed successfully");
    assert_eq!(v["totalPages"], 3);
    assert_eq!(v["pages"], json!(["page1.png", "page2.png", "page10.png"]));
}

#[tokio::test]
async fn page_bytes_etag_and_304() {
    let h = harness(None);
    let uid = h.uid("Series/Issue 1.cbz");
    let (s, hd, body) = h.get(&format!("/read/{uid}/pages/2"), &[]).await;
    assert_eq!((s, body.as_slice()), (StatusCode::OK, &b"two"[..]));
    assert_eq!(hd[header::CONTENT_TYPE], "image/png");
    assert_eq!(hd[header::CACHE_CONTROL], "private, max-age=31536000, immutable");
    let etag = hd[header::ETAG].to_str().unwrap().to_string();
    assert!(etag.starts_with('"') && etag.len() == 42);

    let (s, hd, body) = h.get(&format!("/read/{uid}/pages/2"), &[("if-none-match", &etag)]).await;
    assert_eq!((s, body.len()), (StatusCode::NOT_MODIFIED, 0));
    assert_eq!(hd[header::ETAG].to_str().unwrap(), etag);
    assert_eq!(hd[header::CACHE_CONTROL], "private, max-age=31536000, immutable");

    // Different page, different tag.
    let (_, hd3, _) = h.get(&format!("/read/{uid}/pages/3"), &[]).await;
    assert_ne!(hd3[header::ETAG].to_str().unwrap(), etag);
}

#[tokio::test]
async fn page_errors() {
    let h = harness(None);
    let uid = h.uid("Series/Issue 1.cbz");
    for bad in ["0", "-1", "1.5", "abc"] {
        let (s, _, body) = h.get(&format!("/read/{uid}/pages/{bad}"), &[]).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(json_of(&body), json!({ "error": true, "message": "Invalid page number" }));
    }
    let (s, _, body) = h.get(&format!("/read/{uid}/pages/4"), &[]).await;
    assert_eq!((s, json_of(&body)["message"].as_str().map(str::to_string)), (StatusCode::NOT_FOUND, Some("Page not found".into())));

    let (s, _, body) = h.get("/read/00000000-0000-0000-0000-000000000000/pages/1", &[]).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json_of(&body), json!({ "error": true, "message": "File not found in library" }));
}

#[tokio::test]
async fn list_errors_carry_an_empty_array_and_no_store() {
    let h = harness(None);
    let (s, hd, body) = h.get("/read/nope", &[]).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(hd[header::CACHE_CONTROL], "no-store");
    assert_eq!(json_of(&body), json!({ "error": true, "message": "File not found in library", "pages": [] }));

    let (s, _, body) = h.get(&format!("/read/{}", h.uid("Bad.cbz")), &[]).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(json_of(&body)["message"].as_str().unwrap().starts_with("No image pages found in"));

    let (_, _, body) = h.get("/read/nope/bookmarks", &[]).await;
    assert_eq!(json_of(&body)["bookmarks"], json!([]));

    // A folder uid is not a readable file.
    let (_, _, body) = h.get(&format!("/read/{}", h.uid("Series")), &[]).await;
    assert_eq!(json_of(&body)["message"], "File not found");
}

#[tokio::test]
async fn bookmarks_and_refresh() {
    let h = harness(None);
    let uid = h.uid("Series/Issue 1.cbz");
    let (s, hd, body) = h.get(&format!("/read/{uid}/bookmarks"), &[]).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(hd[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        json_of(&body),
        json!({ "error": false, "message": "Comic bookmarks listed successfully", "bookmarks": [{ "page": 2, "label": "Two" }] })
    );

    // Replace the archive on disk; refresh must see the new pages.
    make_cbz(&h.lib.path().join("Series/Issue 1.cbz"), &[("a.png", b"a"), ("b.png", b"b"), ("c.png", b"c"), ("d.png", b"d")]);
    let (_, _, body) = h.get(&format!("/read/{uid}/refresh"), &[]).await;
    let v = json_of(&body);
    assert_eq!((v["message"].as_str(), v["totalPages"].as_i64()), (Some("Comic re-scanned successfully"), Some(4)));
}

#[tokio::test]
async fn read_requires_the_api_key() {
    let h = harness(Some("secret"));
    let uid = h.uid("Series/Issue 1.cbz");
    let (s, _, _) = h.get(&format!("/read/{uid}/pages/1"), &[]).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _, body) = h.get(&format!("/read/{uid}/pages/1?key=secret"), &[]).await;
    assert_eq!((s, body.as_slice()), (StatusCode::OK, &b"one"[..]));
}
