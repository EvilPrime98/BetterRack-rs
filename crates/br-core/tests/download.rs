//! `Downloader` against a local mock file host. Nothing here
//! talks to a real site.

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use br_core::download::{DownloadRequest, Downloader, ProgressCb, ProgressEvent, RetryOpts};
use br_core::rotating_fetch::RotatingFetch;
use futures_util::stream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

async fn serve(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{addr}")
}

fn downloader() -> Downloader {
    downloader_for("pixeldrain.test")
}

/// Treats the local mock host as PixelDrain (naming from Content-Disposition / title).
fn pixeldrain_downloader() -> Downloader {
    downloader_for("127.0.0.1")
}

fn downloader_for(pixeldrain_host: &str) -> Downloader {
    let client = reqwest::Client::new();
    let rotating = RotatingFetch::new(client.clone()).with_timing(0, 0, 0);
    let retry = RetryOpts {
        max_retries: 2,
        backoff_ms: 0,
        backoff_cap_ms: 0,
        request_delay_ms: 0,
    };
    Downloader::new(client, rotating, None, retry, pixeldrain_host)
}

fn collector() -> (Arc<Mutex<Vec<ProgressEvent>>>, ProgressCb) {
    let events = Arc::new(Mutex::new(vec![]));
    let sink = events.clone();
    (events, Arc::new(move |e| sink.lock().unwrap().push(e)))
}

fn request(url: &str, title: &str, dir: &Path) -> DownloadRequest {
    DownloadRequest {
        title: title.into(),
        download_link: url.into(),
        output_dir: dir.to_path_buf(),
        no_retry: false,
        cancel: CancellationToken::new(),
    }
}

fn body_of(chunks: &[&'static str], total: usize) -> Response {
    let items: Vec<Result<Bytes, std::io::Error>> = chunks
        .iter()
        .map(|c| Ok(Bytes::from_static(c.as_bytes())))
        .collect();
    let mut res = Body::from_stream(stream::iter(items)).into_response();
    res.headers_mut()
        .insert(header::CONTENT_LENGTH, HeaderValue::from(total));
    res
}

/// Sends `chunks` and then breaks the connection.
fn torn_body(chunks: &[&'static str], total: usize) -> Response {
    let mut items: Vec<Result<Bytes, std::io::Error>> = chunks
        .iter()
        .map(|c| Ok(Bytes::from_static(c.as_bytes())))
        .collect();
    items.push(Err(std::io::Error::other(
        "The socket connection was closed unexpectedly.",
    )));
    let mut res = Body::from_stream(stream::iter(items)).into_response();
    res.headers_mut()
        .insert(header::CONTENT_LENGTH, HeaderValue::from(total));
    res
}

fn kinds(events: &Mutex<Vec<ProgressEvent>>) -> Vec<&'static str> {
    events
        .lock()
        .unwrap()
        .iter()
        .map(|e| match e {
            ProgressEvent::Preparing { .. } => "preparing",
            ProgressEvent::Retrying { .. } => "retrying",
            ProgressEvent::Progress { .. } => "progress",
            ProgressEvent::Extracting { .. } => "extracting",
            ProgressEvent::Done { .. } => "done",
            ProgressEvent::Error { .. } => "error",
        })
        .collect()
}

fn count(events: &Mutex<Vec<ProgressEvent>>, kind: &str) -> usize {
    kinds(events).iter().filter(|k| **k == kind).count()
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn restarts_from_the_first_byte_when_the_connection_breaks_mid_stream() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let base = serve(Router::new().route(
        "/files/pack.cbz",
        get(move || {
            let n = c.fetch_add(1, Ordering::SeqCst);
            async move {
                if n == 0 {
                    torn_body(&["AA", "BB"], 6)
                } else {
                    body_of(&["AA", "BB", "CC"], 6)
                }
            }
        }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let (events, cb) = collector();

    let dest = downloader()
        .download_comic(
            request(&format!("{base}/files/pack.cbz"), "Pack", dir.path()),
            cb,
        )
        .await;

    assert_eq!(dest, Some(dir.path().join("pack.cbz")));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("pack.cbz")).unwrap(),
        "AABBCC"
    );
    assert!(events.lock().unwrap().iter().any(|e| matches!(e, ProgressEvent::Retrying { reason, .. } if *reason == br_core::download::RetryReason::Network)));
    assert!(
        matches!(events.lock().unwrap().last(), Some(ProgressEvent::Done { filename }) if filename == "pack.cbz")
    );
    assert_eq!(files_in(dir.path()), ["pack.cbz"], "no .part left behind");
}

#[tokio::test]
async fn one_terminal_error_after_the_retry_budget_when_the_host_is_unreachable() {
    // Bind and drop to get a port nothing listens on.
    let port = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let dir = tempfile::tempdir().unwrap();
    let (events, cb) = collector();

    let dest = downloader()
        .download_comic(
            request(
                &format!("http://127.0.0.1:{port}/files/dead.cbz"),
                "Dead",
                dir.path(),
            ),
            cb,
        )
        .await;

    assert_eq!(dest, None);
    assert_eq!(count(&events, "retrying"), 2, "maxRetries retries");
    assert_eq!(count(&events, "error"), 1);
    assert_eq!(count(&events, "done"), 0);
    assert!(files_in(dir.path()).is_empty());
}

#[tokio::test]
async fn removes_the_partial_file_when_every_attempt_breaks() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let base = serve(Router::new().route(
        "/files/torn.cbz",
        get(move || {
            c.fetch_add(1, Ordering::SeqCst);
            async { torn_body(&["AA"], 6) }
        }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let (events, cb) = collector();

    let dest = downloader()
        .download_comic(
            request(&format!("{base}/files/torn.cbz"), "Torn", dir.path()),
            cb,
        )
        .await;

    assert_eq!(dest, None);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(count(&events, "error"), 1);
    assert!(files_in(dir.path()).is_empty());
}

#[tokio::test]
async fn does_not_retry_when_no_retry_is_set() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let base = serve(Router::new().route(
        "/files/once.cbz",
        get(move || {
            c.fetch_add(1, Ordering::SeqCst);
            async { torn_body(&["AA"], 6) }
        }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let (events, cb) = collector();
    let mut req = request(&format!("{base}/files/once.cbz"), "Once", dir.path());
    req.no_retry = true;

    downloader().download_comic(req, cb).await;

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        (count(&events, "error"), count(&events, "retrying")),
        (1, 0)
    );
}

#[tokio::test]
async fn does_not_retry_a_cloudflare_challenge_page() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let base = serve(Router::new().route(
        "/files/blocked.cbz",
        get(move || {
            c.fetch_add(1, Ordering::SeqCst);
            async {
                (
                    [(header::CONTENT_TYPE, "text/html")],
                    "<title>Just a moment...</title><div class=\"cf-challenge\"></div>",
                )
            }
        }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let (events, cb) = collector();

    downloader()
        .download_comic(
            request(&format!("{base}/files/blocked.cbz"), "Blocked", dir.path()),
            cb,
        )
        .await;

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let events = events.lock().unwrap();
    let errors: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let ProgressEvent::Error { message } = e {
                Some(message)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("Cloudflare"));
}

#[tokio::test]
async fn an_ordinary_html_body_is_saved_like_any_other_file() {
    let base = serve(Router::new().route(
        "/page.html",
        get(|| async { ([(header::CONTENT_TYPE, "text/html")], "<p>hello</p>") }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let (_events, cb) = collector();

    let dest = downloader()
        .download_comic(
            request(&format!("{base}/page.html"), "Page", dir.path()),
            cb,
        )
        .await;

    assert_eq!(
        std::fs::read_to_string(dest.unwrap()).unwrap(),
        "<p>hello</p>"
    );
}

#[tokio::test]
async fn names_a_pixeldrain_file_from_content_disposition() {
    let base = serve(Router::new().route(
        "/api/file/aB3xK9m2",
        get(|| async {
            let mut res = body_of(&["data"], 4);
            res.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"Uncanny X-Men 001 (2019).cbz\""),
            );
            res
        }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let (_e, cb) = collector();

    let dest = pixeldrain_downloader()
        .download_comic(
            request(
                &format!("{base}/api/file/aB3xK9m2?download"),
                "Uncanny X-Men (2019) #1",
                dir.path(),
            ),
            cb,
        )
        .await;

    assert_eq!(dest, Some(dir.path().join("Uncanny X-Men 001 (2019).cbz")));
    assert!(dir.path().join("Uncanny X-Men 001 (2019).cbz").exists());
}

#[tokio::test]
async fn pixeldrain_falls_back_to_the_link_title_and_strips_illegal_characters() {
    let base =
        serve(Router::new().route("/api/file/z9Y8x7", get(|| async { body_of(&["data"], 4) })))
            .await;
    let dir = tempfile::tempdir().unwrap();
    let (_e, cb) = collector();

    let dest = pixeldrain_downloader()
        .download_comic(
            request(
                &format!("{base}/api/file/z9Y8x7?download"),
                "What If...? / Spider-Man",
                dir.path(),
            ),
            cb,
        )
        .await;

    assert_eq!(dest, Some(dir.path().join("What If... Spider-Man")));
}

#[tokio::test]
async fn retries_a_non_2xx_status_and_stops_after_the_budget() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let base = serve(Router::new().route(
        "/files/five-oh-three.cbz",
        get(move || {
            c.fetch_add(1, Ordering::SeqCst);
            async { StatusCode::SERVICE_UNAVAILABLE }
        }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let (events, cb) = collector();

    let dest = downloader()
        .download_comic(
            request(
                &format!("{base}/files/five-oh-three.cbz"),
                "503",
                dir.path(),
            ),
            cb,
        )
        .await;

    assert_eq!(dest, None);
    assert!(calls.load(Ordering::SeqCst) >= 2);
    assert!(events.lock().unwrap().iter().any(|e| matches!(
        e,
        ProgressEvent::Retrying {
            status: Some(503),
            ..
        }
    )));
    assert_eq!(count(&events, "error"), 1);
}

#[tokio::test]
async fn cancelling_mid_stream_stops_without_retrying_and_removes_the_partial_file() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let base = serve(Router::new().route(
        "/files/big.cbz",
        get(move || {
            c.fetch_add(1, Ordering::SeqCst);
            async {
                let first =
                    stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::from_static(b"AA"))]);
                let mut res =
                    Body::from_stream(futures_util::StreamExt::chain(first, stream::pending()))
                        .into_response();
                res.headers_mut()
                    .insert(header::CONTENT_LENGTH, HeaderValue::from(10));
                res
            }
        }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let events = Arc::new(Mutex::new(vec![]));
    let req = request(&format!("{base}/files/big.cbz"), "Big", dir.path());
    let (sink, cancel) = (events.clone(), req.cancel.clone());
    let cb: ProgressCb = Arc::new(move |e| {
        if matches!(e, ProgressEvent::Progress { .. }) {
            cancel.cancel();
        }
        sink.lock().unwrap().push(e);
    });

    let dest = downloader().download_comic(req, cb).await;

    assert_eq!(dest, None);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!((count(&events, "retrying"), count(&events, "done")), (0, 0));
    assert_eq!(kinds(&events).last(), Some(&"error"));
    assert!(files_in(dir.path()).is_empty());
}

#[tokio::test]
async fn a_signal_aborted_before_the_start_leaves_nothing_behind() {
    let base =
        serve(Router::new().route("/files/late.cbz", get(|| async { body_of(&["done"], 4) })))
            .await;
    let dir = tempfile::tempdir().unwrap();
    let (events, cb) = collector();
    let req = request(&format!("{base}/files/late.cbz"), "Late", dir.path());
    req.cancel.cancel();

    let dest = downloader().download_comic(req, cb).await;

    assert_eq!(dest, None);
    assert_eq!(count(&events, "done"), 0);
    assert!(files_in(dir.path()).is_empty());
}

#[tokio::test]
async fn only_exposes_the_final_name_once_the_transfer_is_complete() {
    let base = serve(Router::new().route(
        "/files/inflight.cbz",
        get(|| async { body_of(&["AB", "CD"], 4) }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let (sink, dir_path) = (seen.clone(), dir.path().to_path_buf());
    let cb: ProgressCb = Arc::new(move |e| {
        if matches!(e, ProgressEvent::Progress { .. }) {
            sink.lock().unwrap().extend(files_in(&dir_path));
        }
    });

    let dest = downloader()
        .download_comic(
            request(
                &format!("{base}/files/inflight.cbz"),
                "Inflight",
                dir.path(),
            ),
            cb,
        )
        .await;

    assert!(!seen.lock().unwrap().iter().any(|n| n == "inflight.cbz"));
    assert!(
        seen.lock()
            .unwrap()
            .iter()
            .any(|n| n == "inflight.cbz.part")
    );
    let dest: PathBuf = dest.unwrap();
    assert_eq!(std::fs::read_to_string(&dest).unwrap(), "ABCD");
    assert_eq!(files_in(dir.path()), ["inflight.cbz"]);
}

#[tokio::test]
async fn progress_reports_percent_and_megabytes_as_text() {
    let base = serve(Router::new().route(
        "/files/p.cbz",
        get(|| async { body_of(&["AAAA", "BBBB"], 8) }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let (events, cb) = collector();

    downloader()
        .download_comic(request(&format!("{base}/files/p.cbz"), "P", dir.path()), cb)
        .await;

    let events = events.lock().unwrap();
    let first = events.iter().find_map(|e| {
        if let ProgressEvent::Progress {
            percent,
            received_mb,
            total_mb,
            ..
        } = e
        {
            Some((*percent, received_mb.clone(), total_mb.clone()))
        } else {
            None
        }
    });
    // The two chunks may arrive together, so the first tick is 50 or 100 percent.
    let (percent, received, total) = first.unwrap();
    assert!(percent == 50 || percent == 100);
    assert_eq!((received.as_str(), total.as_str()), ("0.0", "0.0"));
    assert!(matches!(events.first(), Some(ProgressEvent::Preparing { title }) if title == "P"));
}
