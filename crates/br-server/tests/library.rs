//! In-process tests of the Phase D endpoints (`/api/library*`, `/api/thumbnail*`) against a
//! throwaway library.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use br_core::config::Config;
use br_core::uid::{resolve_windows, uid_from_path};
use br_server::{app, state::AppState};
use serde_json::{Value, json};
use std::io::{Cursor, Write};
use std::path::Path;
use tower::ServiceExt;

struct Harness {
    _data: tempfile::TempDir,
    lib: tempfile::TempDir,
    router: axum::Router,
}

const XML: &[u8] =
    br#"<ComicInfo><Series>Saga</Series><Title>Chapter One</Title><Number>1</Number></ComicInfo>"#;

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 7])
    });
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn make_cbz(path: &Path, entries: &[(&str, &[u8])]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, data) in entries {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

fn harness() -> Harness {
    let data = tempfile::tempdir().unwrap();
    let lib = tempfile::tempdir().unwrap();
    let cover = png(360, 540);
    make_cbz(
        &lib.path().join("Saga/Saga 1.cbz"),
        &[("1.png", &cover), ("ComicInfo.xml", XML)],
    );
    make_cbz(&lib.path().join("Saga/Saga 2.cbz"), &[("1.png", &cover)]);
    make_cbz(&lib.path().join("Broken.cbz"), &[("1.png", b"not a png")]);
    std::fs::create_dir_all(lib.path().join("Empty")).unwrap();

    let config = Config::from_lookup(|_| None, data.path().to_path_buf());
    let state = AppState::open(config).unwrap();
    state
        .prefs
        .update_app_settings(
            json!({ "outputDirs": [lib.path().to_string_lossy()] })
                .as_object()
                .unwrap(),
        )
        .unwrap();
    state.rescan_library().unwrap();
    Harness {
        _data: data,
        lib,
        router: app(state),
    }
}

struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&self.body).into_owned()))
    }
}

impl Harness {
    fn uid(&self, rel: &str) -> String {
        let cwd = self.lib.path().to_string_lossy().into_owned();
        uid_from_path(&resolve_windows(
            &self.lib.path().join(rel).to_string_lossy(),
            &cwd,
        ))
    }

    async fn send(
        &self,
        method: &str,
        uri: &str,
        body: Option<Value>,
        headers: &[(&str, &str)],
    ) -> Reply {
        let mut req = Request::builder().method(method).uri(uri);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let body = body.map_or(Body::empty(), |b| Body::from(b.to_string()));
        let res = self
            .router
            .clone()
            .oneshot(req.body(body).unwrap())
            .await
            .unwrap();
        let (status, headers) = (res.status(), res.headers().clone());
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec();
        Reply {
            status,
            headers,
            body,
        }
    }

    async fn call(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let r = self.send(method, uri, body, &[]).await;
        (r.status, r.json())
    }
}

fn entry_names(page: &Value) -> Vec<&str> {
    page["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["entries"].as_array().unwrap())
        .map(|e| e["name"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn listing_endpoints() {
    let h = harness();
    let (s, page) = h.call("GET", "/api/library", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        entry_names(&page),
        ["Empty", "Saga", "Broken.cbz", "Saga 1.cbz", "Saga 2.cbz"]
    );
    assert_eq!(
        (
            page["total"].as_u64(),
            page["limit"].as_u64(),
            page["hasMore"].as_bool()
        ),
        (Some(5), Some(100), Some(false))
    );
    assert_eq!(h.call("GET", "/api/library/", None).await.1["total"], 5);

    let (_, paged) = h.call("GET", "/api/library?limit=2&offset=1", None).await;
    assert_eq!(
        (entry_names(&paged), paged["hasMore"].as_bool()),
        (vec!["Saga", "Broken.cbz"], Some(true))
    );
    assert_eq!(
        h.call("GET", "/api/library?limit=abc&offset=-3", None)
            .await
            .1["limit"],
        100
    );

    let (_, idx) = h.call("GET", "/api/library/index", None).await;
    assert_eq!(idx[0]["count"], 5);
    assert_eq!(
        idx[0]["uid"],
        uid_from_path(&resolve_windows(&h.lib.path().to_string_lossy(), "C:\\"))
    );

    let (_, series) = h.call("GET", "/api/library/by-series", None).await;
    let names: Vec<&str> = series["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Broken", "Saga"]);

    let (_, recent) = h
        .call("GET", "/api/library/recent?windowHours=1", None)
        .await;
    assert_eq!(
        (
            recent["items"].as_array().unwrap().len(),
            recent["windowHours"].as_u64()
        ),
        (3, Some(1))
    );
    assert!(recent["generatedAt"].is_number());

    let (_, reading) = h.call("GET", "/api/library/reading", None).await;
    assert_eq!(reading["items"], json!([]));
}

#[tokio::test]
async fn reading_lists_started_comics() {
    let h = harness();
    let uid = h.uid("Saga/Saga 2.cbz");
    let (s, _) = h
        .call(
            "PATCH",
            &format!("/api/comic-data/{uid}"),
            Some(json!({ "currentPage": 3, "readPer": 40 })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let (_, reading) = h.call("GET", "/api/library/reading", None).await;
    assert_eq!(reading["items"][0]["uid"], uid);
}

#[tokio::test]
async fn preferences_round_trip_and_inheritance() {
    let h = harness();
    let folder = h.uid("Saga");
    let uri = format!("/api/library/preferences/{folder}");
    let (s, v) = h.call("GET", &uri, None).await;
    assert_eq!(
        (s, v["message"].as_str()),
        (
            StatusCode::NOT_FOUND,
            Some("No preferences found for this uid.")
        )
    );
    let (s, v) = h
        .call(
            "PATCH",
            "/api/library/preferences/nope",
            Some(json!({ "prefPublisher": "X" })),
        )
        .await;
    assert_eq!(
        (s, v["message"].as_str()),
        (StatusCode::NOT_FOUND, Some("Entry not found for this uid."))
    );

    let (s, v) = h
        .call(
            "PUT",
            &uri,
            Some(json!({ "prefPublisher": "Image", "recursive": true })),
        )
        .await;
    assert_eq!(
        (s, v),
        (
            StatusCode::OK,
            json!({ "error": false, "message": "Preferences updated successfully." })
        )
    );
    let (_, v) = h.call("GET", &uri, None).await;
    assert_eq!(
        v,
        json!({ "uid": folder, "prefPublisher": "Image", "recursive": true, "prefCover": "" })
    );

    let (_, page) = h.call("GET", "/api/library", None).await;
    let saga1 = page["groups"][0]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "Saga 1.cbz")
        .unwrap();
    assert_eq!(saga1["prefPublisher"], "Image");

    let r = h.send("PATCH", &uri, None, &[]).await;
    assert_eq!(
        (r.status, r.json()["message"].as_str().map(str::to_string)),
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Some("There was an issue updating preferences. Please, try again later.".into())
        )
    );
}

#[tokio::test]
async fn identify_endpoints() {
    let h = harness();
    let uid = h.uid("Saga/Saga 1.cbz");

    let (s, v) = h
        .call("GET", &format!("/api/library/{uid}/identify"), None)
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (
            v["identified"].as_bool(),
            v["metaSource"].as_str(),
            v["comic"]["title"].as_str()
        ),
        (Some(true), Some("comicinfo"), Some("Chapter One"))
    );

    // Persisted: shows up in comic-data and in the listing.
    let (_, data) = h.call("GET", "/api/comic-data", None).await;
    assert_eq!(data[&uid]["identified"], true);
    let (_, page) = h.call("GET", "/api/library", None).await;
    let e = page["groups"][0]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["uid"] == uid.as_str())
        .unwrap();
    assert_eq!(e["identified"], true);

    // No ComicInfo and wikiSearch off: looked up, no match -> `identified: false`, no comic.
    let plain = h.uid("Saga/Saga 2.cbz");
    let (_, v) = h
        .call("GET", &format!("/api/library/{plain}/identify"), None)
        .await;
    assert_eq!(v, json!({ "identified": false }));

    let (s, v) = h.call("GET", "/api/library/nope/identify", None).await;
    assert_eq!(
        (s, v),
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": true, "message": "Comic not found." })
        )
    );

    // Un-identify, then reset (re-run) the lookup.
    let (s, v) = h
        .call(
            "POST",
            "/api/library/file/unidentify",
            Some(json!({ "fileUid": uid })),
        )
        .await;
    assert_eq!(
        (s, v["message"].as_str()),
        (StatusCode::OK, Some("File un-identified successfully."))
    );
    let (_, v) = h.call("GET", "/api/library", None).await;
    let e = v["groups"][0]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["uid"] == uid.as_str())
        .unwrap()
        .clone();
    assert_eq!(e["identified"], false);
    assert!(e.get("comic").is_none());
    let (_, v) = h
        .call("POST", &format!("/api/library/{uid}/identify/reset"), None)
        .await;
    assert_eq!(v["metaSource"], "comicinfo");

    // Manual pick.
    let (s, v) = h.call("POST", "/api/library/file/identify", Some(json!({ "fileUid": plain, "comic": { "title": "Picked", "pageId": 9, "sourceWiki": "dc" } }))).await;
    assert_eq!(
        (s, v["message"].as_str()),
        (StatusCode::OK, Some("File identified successfully."))
    );
    let (_, v) = h
        .call("GET", &format!("/api/library/{plain}/identify"), None)
        .await;
    assert_eq!(
        (v["metaSource"].as_str(), v["comic"]["title"].as_str()),
        (Some("wiki"), Some("Picked"))
    );
    let (s, v) = h
        .call(
            "POST",
            "/api/library/file/identify",
            Some(json!({ "fileUid": plain })),
        )
        .await;
    assert_eq!(
        (s, v["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("A file uid and comic are required.")
        )
    );

    // Reset everything.
    let (s, v) = h
        .call("POST", "/api/library/identify/reset-all", None)
        .await;
    assert_eq!(
        (s, v["message"].as_str()),
        (
            StatusCode::OK,
            Some("Library flagged for re-identification.")
        )
    );
    let (_, data) = h.call("GET", "/api/comic-data", None).await;
    assert!(data[&uid].get("identified").is_none());
}

#[tokio::test]
async fn identify_all_runs_as_a_job() {
    let h = harness();
    let (s, v) = h.call("GET", "/api/library/identify/all", None).await;
    assert_eq!(
        (s, v),
        (StatusCode::OK, json!({ "error": false, "state": "idle" }))
    );

    let (s, v) = h.call("POST", "/api/library/identify/all", None).await;
    assert_eq!(s, StatusCode::ACCEPTED);
    let job_id = v["jobId"].as_str().unwrap().to_string();

    let mut last = Value::Null;
    for _ in 0..200 {
        let (s, v) = h.call("GET", "/api/library/identify/all", None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["jobId"], job_id.as_str());
        last = v;
        if last["state"] == "done" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(last["state"], "done", "{last}");
    assert_eq!(last["progress"], json!({ "type": "done", "total": 3 }));

    let (_, data) = h.call("GET", "/api/comic-data", None).await;
    assert_eq!(data.as_object().unwrap().len(), 3);
}

#[tokio::test]
async fn folder_and_file_operations() {
    let h = harness();
    let (s, v) = h
        .call(
            "POST",
            "/api/library/folder",
            Some(json!({ "folderName": "New", "parentFolderUid": h.uid("Saga") })),
        )
        .await;
    assert_eq!(
        (s, v["message"].as_str()),
        (StatusCode::OK, Some("Folder created succesfully."))
    );
    assert!(h.lib.path().join("Saga/New").is_dir());
    let (s, v) = h.call("POST", "/api/library/folder", Some(json!({}))).await;
    assert_eq!(
        (s, v["message"].as_str()),
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Some("There was an issue creating the folder. Please, try again later.")
        )
    );

    // The new folder only shows after a refresh.
    let (_, page) = h.call("GET", "/api/library/refresh", None).await;
    assert_eq!(page["message"], "Library re-scan completed.");
    assert!(entry_names(&page["page"]).contains(&"New"));

    let (s, v) = h
        .call(
            "POST",
            "/api/library/file/move",
            Some(json!({ "fileUid": h.uid("Saga"), "targetFolderUid": h.uid("Saga/New") })),
        )
        .await;
    assert_eq!(
        (s, v["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("A folder cannot be moved into one of its own subfolders.")
        )
    );
    let (s, v) = h
        .call(
            "POST",
            "/api/library/file/move",
            Some(json!({ "fileUid": h.uid("Broken.cbz"), "targetFolderUid": h.uid("Empty") })),
        )
        .await;
    assert_eq!(
        (s, v["message"].as_str()),
        (StatusCode::OK, Some("File moved successfully."))
    );
    assert!(h.lib.path().join("Empty/Broken.cbz").exists());
    let (s, _) = h
        .call(
            "POST",
            "/api/library/file/move",
            Some(json!({ "fileUid": "nope", "targetFolderUid": "" })),
        )
        .await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    let (s, v) = h
        .call(
            "DELETE",
            "/api/library/file",
            Some(json!({ "fileUid": h.uid("Empty/Broken.cbz") })),
        )
        .await;
    assert_eq!(
        (s, v["message"].as_str()),
        (StatusCode::OK, Some("File deleted successfully."))
    );
    assert!(!h.lib.path().join("Empty/Broken.cbz").exists());
    let (s, v) = h
        .call(
            "DELETE",
            "/api/library/folder",
            Some(json!({ "folderUid": h.uid("Saga") })),
        )
        .await;
    assert_eq!(
        (s, v["message"].as_str()),
        (StatusCode::OK, Some("Folder deleted successfully."))
    );
    assert!(!h.lib.path().join("Saga").exists());
    let (_, page) = h.call("GET", "/api/library", None).await;
    assert_eq!(entry_names(&page), ["Empty"]);
}

#[tokio::test]
async fn thumbnails_are_generated_cached_and_revalidated() {
    let h = harness();
    let uid = h.uid("Saga/Saga 1.cbz");
    let uri = format!("/api/thumbnail/{uid}");

    let r = h.send("GET", &uri, None, &[]).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.headers[header::CONTENT_TYPE], "image/webp");
    assert_eq!(r.headers[header::CACHE_CONTROL], "private, max-age=86400");
    let img = image::load_from_memory_with_format(&r.body, image::ImageFormat::WebP).unwrap();
    assert_eq!((img.width(), img.height()), (180, 270));

    let etag = r.headers[header::ETAG].to_str().unwrap().to_string();
    let r304 = h.send("GET", &uri, None, &[("if-none-match", &etag)]).await;
    assert_eq!(r304.status, StatusCode::NOT_MODIFIED);
    assert!(r304.body.is_empty());

    let retry = h.send("POST", &format!("{uri}/retry"), None, &[]).await;
    assert_eq!(
        (retry.status, retry.json()),
        (StatusCode::OK, json!({ "ok": true }))
    );

    // Unknown uid, a folder, a broken page and a hostile uid all answer 404 JSON.
    for bad in [
        "nope".to_string(),
        h.uid("Saga"),
        h.uid("Broken.cbz"),
        "..".to_string(),
    ] {
        let r = h
            .send("GET", &format!("/api/thumbnail/{bad}"), None, &[])
            .await;
        assert_eq!(
            (r.status, r.json()["message"].as_str().map(str::to_string)),
            (
                StatusCode::NOT_FOUND,
                Some("No thumbnail available for this comic.".into())
            ),
            "{bad}"
        );
    }
    let r = h.send("POST", "/api/thumbnail/nope/retry", None, &[]).await;
    assert_eq!(
        (r.status, r.json()["message"].as_str().map(str::to_string)),
        (
            StatusCode::NOT_FOUND,
            Some("No comic file found for this uid.".into())
        )
    );
    let r = h
        .send(
            "POST",
            &format!("/api/thumbnail/{}/retry", h.uid("Broken.cbz")),
            None,
            &[],
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}
