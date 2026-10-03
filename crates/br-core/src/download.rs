//! `DownloadModel`: stream one file to disk with retries, naming, Cloudflare detection, `.part`
//! files, cancellation and pack extraction. Progress is reported with the `StoreProgressEvent`
//! union the clients already parse (`type` plus camelCase fields, `receivedMB`/`totalMB` as text).

use crate::pack::{PackAction, PackExtractor};
use crate::rotating_fetch::{ABORT_MESSAGE, FetchError, RotatingFetch, abortable_sleep};
use futures_util::StreamExt;
use reqwest::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, HeaderMap};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

const REQUEST_DELAY_MS: u64 = 3000;
const IN_FLIGHT_SUFFIX: &str = ".part";
const MAX_NETWORK_RETRIES: u32 = 3;
const RETRY_BACKOFF_MS: u64 = 2000;
const RETRY_BACKOFF_CAP_MS: u64 = 30_000;
/// Progress ticks are coalesced to this interval (Bun emitted one per chunk).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

const CUSTOM_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const CLOUDFLARE_CHALLENGE_MARKERS: [&str; 6] = ["cloudflare", "just a moment", "attention required", "challenge-platform", "turnstile", "cf-challenge"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RetryReason {
    Network,
    Http,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProgressEvent {
    Preparing {
        title: String,
    },
    Retrying {
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<u16>,
        reason: RetryReason,
        #[serde(rename = "delaySec")]
        delay_sec: u64,
    },
    Progress {
        title: String,
        percent: u64,
        #[serde(rename = "receivedMB")]
        received_mb: String,
        #[serde(rename = "totalMB")]
        total_mb: String,
    },
    Extracting {
        title: String,
        done: usize,
        total: usize,
    },
    Done {
        filename: String,
    },
    Error {
        message: String,
    },
}

impl ProgressEvent {
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

pub type ProgressCb = Arc<dyn Fn(ProgressEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct RetryOpts {
    pub max_retries: u32,
    pub backoff_ms: u64,
    pub backoff_cap_ms: u64,
    /// Pause before re-requesting after a non-2xx status.
    pub request_delay_ms: u64,
}

impl Default for RetryOpts {
    fn default() -> Self {
        Self { max_retries: MAX_NETWORK_RETRIES, backoff_ms: RETRY_BACKOFF_MS, backoff_cap_ms: RETRY_BACKOFF_CAP_MS, request_delay_ms: REQUEST_DELAY_MS }
    }
}

pub struct DownloadRequest {
    pub title: String,
    pub download_link: String,
    pub output_dir: PathBuf,
    pub no_retry: bool,
    pub cancel: CancellationToken,
}

#[derive(Debug)]
enum DlError {
    /// Retrying will not help (a Cloudflare challenge).
    Fatal(String),
    Aborted,
    Failed(String),
}

impl DlError {
    fn message(&self) -> String {
        match self {
            Self::Fatal(m) | Self::Failed(m) => m.clone(),
            Self::Aborted => ABORT_MESSAGE.to_string(),
        }
    }
}

impl From<FetchError> for DlError {
    fn from(e: FetchError) -> Self {
        match e {
            FetchError::Aborted => Self::Aborted,
            FetchError::Network(m) => Self::Failed(m),
        }
    }
}

pub struct Downloader {
    client: reqwest::Client,
    rotating: RotatingFetch,
    pack: Option<Arc<PackExtractor>>,
    retry: RetryOpts,
    pixeldrain_host: String,
}

/// JS `Number.prototype.toFixed(1)` of `bytes` expressed in MiB (round half up on exact ties).
pub fn mb_text(bytes: u64) -> String {
    let v = bytes as f64 / 1024.0 / 1024.0;
    let f = v.fract();
    if f == 0.25 || f == 0.75 { format!("{:.1}", v + 0.01) } else { format!("{v:.1}") }
}

pub fn sanitize_filename(name: &str) -> String {
    let replaced: String = name.chars().map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (c as u32) < 0x20 { ' ' } else { c }).collect();
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim_end_matches(['.', ' ']).to_string()
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(h) = b.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(std::str::from_utf8(h).unwrap_or("zz"), 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn filename_from_disposition(header: Option<&str>) -> Option<String> {
    let header = header?;
    let lower = header.to_ascii_lowercase();
    if let Some(i) = lower.find("filename*=") {
        let v = header[i + "filename*=".len()..].split(';').next().unwrap_or("").trim();
        let v = v.strip_prefix("UTF-8''").or_else(|| v.strip_prefix("utf-8''")).unwrap_or(v);
        let v = v.trim_matches(|c| c == '"' || c == '\'');
        if !v.is_empty() {
            return Some(percent_decode(v));
        }
    }
    let i = lower.find("filename=")?;
    let v = header[i + "filename=".len()..].trim_start_matches('"');
    let v = v.split(['"', ';']).next().unwrap_or("").trim();
    (!v.is_empty()).then(|| v.to_string())
}

struct Meta {
    status: reqwest::StatusCode,
    headers: HeaderMap,
    url: String,
}

enum Body {
    /// An HTML body that was read to look for a challenge page; it is saved as is.
    Prefetched(Vec<u8>),
    Stream(reqwest::Response),
}

fn is_cloudflare_challenge(text: &str) -> bool {
    let lower = text.to_lowercase();
    CLOUDFLARE_CHALLENGE_MARKERS.iter().any(|m| lower.contains(m))
}

impl Downloader {
    pub fn new(client: reqwest::Client, rotating: RotatingFetch, pack: Option<Arc<PackExtractor>>, retry: RetryOpts, pixeldrain_host: &str) -> Self {
        Self { client, rotating, pack, retry, pixeldrain_host: pixeldrain_host.to_string() }
    }

    fn is_pixeldrain_url(&self, url: &str) -> bool {
        url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).is_some_and(|h| h == self.pixeldrain_host || h.ends_with(&format!(".{}", self.pixeldrain_host)))
    }

    async fn fetch_source(&self, url: &str, cancel: &CancellationToken) -> Result<reqwest::Response, DlError> {
        if self.is_pixeldrain_url(url) {
            return Ok(self.rotating.fetch(url, &[("content-type", "application/octet-stream")], Some(cancel)).await?);
        }
        let send = self.client.get(url).header("user-agent", CUSTOM_USER_AGENT).header(CONTENT_TYPE, "application/octet-stream").send();
        tokio::select! {
            r = send => r.map_err(|e| DlError::Failed(e.to_string())),
            () = cancel.cancelled() => Err(DlError::Aborted),
        }
    }

    fn resolve_filename(&self, meta: &Meta, req: &DownloadRequest) -> String {
        let from_disposition = filename_from_disposition(meta.headers.get(CONTENT_DISPOSITION).and_then(|v| v.to_str().ok()));
        let tail = meta.url.rsplit('/').next().unwrap_or("");
        let from_url = percent_decode(tail.split(['?', '#']).next().unwrap_or(""));
        let pick = |opts: [Option<String>; 3]| opts.into_iter().flatten().find(|s| !s.is_empty()).unwrap_or_default();
        let raw = if self.is_pixeldrain_url(&req.download_link) {
            pick([from_disposition, Some(req.title.clone()), Some(from_url)])
        } else {
            pick([Some(from_url), from_disposition, Some(req.title.clone())])
        };
        let name = sanitize_filename(&raw);
        if name.is_empty() { "download".to_string() } else { name }
    }

    /// Download `req`; returns the final path (a folder when a pack was unpacked). Failures are
    /// reported through a single terminal `error` event and yield `None`.
    pub async fn download_comic(&self, req: DownloadRequest, on_progress: ProgressCb) -> Option<PathBuf> {
        if req.download_link.is_empty() {
            return None;
        }
        tracing::info!(title = %req.title, "download started");
        on_progress(ProgressEvent::Preparing { title: req.title.clone() });
        match self.run(&req, &on_progress).await {
            Ok(dest) => Some(dest),
            Err(e) => {
                on_progress(ProgressEvent::Error { message: e.message() });
                None
            }
        }
    }

    async fn run(&self, req: &DownloadRequest, on_progress: &ProgressCb) -> Result<PathBuf, DlError> {
        let mut last_err = None;
        let mut dest = None;
        for attempt in 0..=self.retry.max_retries {
            if attempt > 0 {
                let base = (2u64.saturating_pow(attempt) * self.retry.backoff_ms).min(self.retry.backoff_cap_ms);
                let backoff = base + fastrand::u64(0..=self.retry.backoff_ms.min(1000));
                on_progress(ProgressEvent::Retrying { title: req.title.clone(), status: None, reason: RetryReason::Network, delay_sec: (backoff as f64 / 1000.0).round() as u64 });
                if !abortable_sleep(backoff, Some(&req.cancel)).await {
                    return Err(DlError::Aborted);
                }
            }
            match self.stream_to_disk(req, on_progress).await {
                Ok(d) => {
                    dest = Some(d);
                    last_err = None;
                    break;
                }
                Err(e) => {
                    if matches!(e, DlError::Fatal(_) | DlError::Aborted) || req.no_retry || req.cancel.is_cancelled() {
                        return Err(e);
                    }
                    tracing::error!(title = %req.title, attempt = attempt + 1, of = self.retry.max_retries + 1, err = %e.message(), "download attempt failed");
                    last_err = Some(e);
                }
            }
        }
        if let Some(e) = last_err {
            return Err(e);
        }
        let mut dest = dest.ok_or_else(|| DlError::Failed("Failed to download".into()))?;

        // Named now: pack extraction can turn `dest` into a folder.
        let filename = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| req.title.clone());

        if req.cancel.is_cancelled() {
            let _ = tokio::fs::remove_file(&dest).await;
            return Err(DlError::Aborted);
        }

        if let Some(pack) = &self.pack {
            let size = tokio::fs::metadata(&dest).await.map(|m| m.len()).unwrap_or(0);
            if pack.should_inspect(&dest, size) {
                let (pack, file, out_dir, title, cb) = (pack.clone(), dest.clone(), req.output_dir.clone(), req.title.clone(), on_progress.clone());
                let result = tokio::task::spawn_blocking(move || {
                    let progress = |done: usize, total: usize| cb(ProgressEvent::Extracting { title: title.clone(), done, total });
                    pack.extract_pack(&file, &out_dir, Some(&progress))
                })
                .await;
                match result {
                    Ok(Ok(r)) => match r.action {
                        PackAction::Extracted => dest = r.dest_dir.unwrap_or(dest),
                        PackAction::Renamed => dest = r.renamed_to.unwrap_or(dest),
                        PackAction::Skipped => {}
                    },
                    // A failed unpack keeps the wrapper on disk; the library still rescans.
                    Ok(Err(e)) => tracing::error!(err = %e, "pack extraction failed"),
                    Err(e) => tracing::error!(err = %e, "pack extraction failed"),
                }
            }
        }

        on_progress(ProgressEvent::Done { filename });
        Ok(dest)
    }

    async fn stream_to_disk(&self, req: &DownloadRequest, on_progress: &ProgressCb) -> Result<PathBuf, DlError> {
        let mut status_retries = 0;
        let (meta, body) = loop {
            let response = self.fetch_source(&req.download_link, &req.cancel).await?;
            let meta = Meta { status: response.status(), headers: response.headers().clone(), url: response.url().to_string() };
            let is_html = meta.headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|t| t.contains("text/html"));
            let body = if is_html {
                let bytes = tokio::select! {
                    b = response.bytes() => b.map_err(|e| DlError::Failed(e.to_string()))?,
                    () = req.cancel.cancelled() => return Err(DlError::Aborted),
                };
                if is_cloudflare_challenge(&String::from_utf8_lossy(&bytes)) {
                    let msg = "Cloudflare challenge detected. Open the comic in a browser or use a browser-side download path.";
                    tracing::error!("{msg}");
                    return Err(DlError::Fatal(msg.to_string()));
                }
                Body::Prefetched(bytes.to_vec())
            } else {
                Body::Stream(response)
            };
            if meta.status.is_success() {
                break (meta, body);
            }
            // A persistent non-2xx status must terminate, not loop forever.
            if req.no_retry || status_retries >= self.retry.max_retries {
                return Err(DlError::Failed(format!("HTTP {}", meta.status.as_u16())));
            }
            status_retries += 1;
            on_progress(ProgressEvent::Retrying {
                title: req.title.clone(),
                status: Some(meta.status.as_u16()),
                reason: RetryReason::Http,
                delay_sec: self.retry.request_delay_ms / 1000,
            });
            if !abortable_sleep(self.retry.request_delay_ms, Some(&req.cancel)).await {
                return Err(DlError::Aborted);
            }
        };

        let filename = self.resolve_filename(&meta, req);
        let dest = req.output_dir.join(&filename);
        let in_flight = PathBuf::from(format!("{}{IN_FLIGHT_SUFFIX}", dest.display()));
        let total: u64 = meta.headers.get(CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0);
        let total_mb = mb_text(total);

        tokio::fs::create_dir_all(&req.output_dir).await.map_err(|e| DlError::Failed(e.to_string()))?;

        let result = write_body(body, &in_flight, total, &total_mb, &req.title, on_progress, &req.cancel).await;
        if let Err(e) = result {
            // A broken transfer leaves a truncated file; remove it before retrying or failing.
            let _ = tokio::fs::remove_file(&in_flight).await;
            return Err(e);
        }
        tokio::fs::rename(&in_flight, &dest).await.map_err(|e| DlError::Failed(e.to_string()))?;
        Ok(dest)
    }
}

async fn write_body(body: Body, path: &Path, total: u64, total_mb: &str, title: &str, on_progress: &ProgressCb, cancel: &CancellationToken) -> Result<(), DlError> {
    let io = |e: std::io::Error| DlError::Failed(e.to_string());
    let mut file = tokio::fs::File::create(path).await.map_err(io)?;
    let mut received: u64 = 0;
    let mut last_emit: Option<Instant> = None;
    let mut tick = |received: u64, force: bool| {
        if !force && last_emit.is_some_and(|t| t.elapsed() < PROGRESS_INTERVAL) {
            return;
        }
        last_emit = Some(Instant::now());
        let percent = if total > 0 { received * 100 / total } else { 0 };
        on_progress(ProgressEvent::Progress { title: title.to_string(), percent, received_mb: mb_text(received), total_mb: total_mb.to_string() });
    };
    match body {
        Body::Prefetched(bytes) => {
            received = bytes.len() as u64;
            file.write_all(&bytes).await.map_err(io)?;
            tick(received, true);
        }
        Body::Stream(res) => {
            let mut stream = res.bytes_stream();
            loop {
                let next = tokio::select! {
                    n = stream.next() => n,
                    () = cancel.cancelled() => return Err(DlError::Aborted),
                };
                let Some(chunk) = next else { break };
                let chunk = chunk.map_err(|e| DlError::Failed(e.to_string()))?;
                received += chunk.len() as u64;
                tick(received, false);
                file.write_all(&chunk).await.map_err(io)?;
            }
        }
    }
    file.flush().await.map_err(io)?;
    file.sync_all().await.map_err(io)?;
    drop(file);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mb_text_matches_to_fixed() {
        assert_eq!(mb_text(0), "0.0");
        assert_eq!(mb_text(1024 * 1024), "1.0");
        assert_eq!(mb_text(262_144), "0.3"); // 0.25 rounds up in JS
        assert_eq!(mb_text(786_432), "0.8");
        assert_eq!(mb_text(5 * 1024 * 1024 + 104_858), "5.1");
    }

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize_filename("What If...? / Spider-Man"), "What If... Spider-Man");
        assert_eq!(sanitize_filename("a<b>c:d"), "a b c d");
        assert_eq!(sanitize_filename("..."), "");
        assert_eq!(sanitize_filename("name. "), "name");
    }

    #[test]
    fn parses_content_disposition() {
        assert_eq!(filename_from_disposition(Some("attachment; filename=\"Uncanny X-Men 001 (2019).cbz\"")).as_deref(), Some("Uncanny X-Men 001 (2019).cbz"));
        assert_eq!(filename_from_disposition(Some("attachment; filename*=UTF-8''Caf%C3%A9.cbz")).as_deref(), Some("Café.cbz"));
        assert_eq!(filename_from_disposition(Some("attachment; filename=plain.cbz")).as_deref(), Some("plain.cbz"));
        assert_eq!(filename_from_disposition(Some("inline")), None);
        assert_eq!(filename_from_disposition(None), None);
    }

    #[test]
    fn event_json_shapes() {
        let v = ProgressEvent::Retrying { title: "T".into(), status: Some(503), reason: RetryReason::Http, delay_sec: 3 }.to_value();
        assert_eq!(v, serde_json::json!({"type": "retrying", "title": "T", "status": 503, "reason": "http", "delaySec": 3}));
        let v = ProgressEvent::Progress { title: "T".into(), percent: 5, received_mb: "1.0".into(), total_mb: "20.0".into() }.to_value();
        assert_eq!(v, serde_json::json!({"type": "progress", "title": "T", "percent": 5, "receivedMB": "1.0", "totalMB": "20.0"}));
        assert_eq!(ProgressEvent::Done { filename: "a.cbz".into() }.to_value(), serde_json::json!({"type": "done", "filename": "a.cbz"}));
    }
}
