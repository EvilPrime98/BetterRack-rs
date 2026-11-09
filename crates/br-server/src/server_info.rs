//! Server version, update availability and recent log lines, shown in the web Settings page.

use crate::error::ApiError;
use axum::Json;
use axum::extract::Query;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::Write;
use std::sync::Mutex;
use std::time::Duration;
use tracing_subscriber::fmt::MakeWriter;

const MAX_LOG_LINES: usize = 1000;
const DEFAULT_LOG_LINES: usize = 200;
const DEFAULT_REPO: &str = "EvilPrime98/betterrack-gpui";

static LOGS: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

/// Appends `text` to the in-memory log, one entry per non-empty line, dropping the oldest past the cap.
pub fn push_log(text: &str) {
    let mut logs = LOGS.lock().unwrap_or_else(|e| e.into_inner());
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        if logs.len() == MAX_LOG_LINES {
            logs.pop_front();
        }
        logs.push_back(line.to_string());
    }
}

/// The last `n` log lines, oldest first.
pub fn recent_logs(n: usize) -> Vec<String> {
    let logs = LOGS.lock().unwrap_or_else(|e| e.into_inner());
    let skip = logs.len().saturating_sub(n);
    logs.iter().skip(skip).cloned().collect()
}

/// A `tracing_subscriber` writer that copies every line into the log buffer and to stderr.
#[derive(Clone, Copy, Default)]
pub struct LogTee;

impl<'a> MakeWriter<'a> for LogTee {
    type Writer = LogTee;
    fn make_writer(&'a self) -> Self::Writer {
        LogTee
    }
}

impl Write for LogTee {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        push_log(&String::from_utf8_lossy(buf));
        std::io::stderr().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::stderr().flush()
    }
}

#[derive(Deserialize)]
pub struct LogsQuery {
    lines: Option<usize>,
}

pub async fn info() -> Json<Value> {
    Json(json!({ "version": env!("CARGO_PKG_VERSION") }))
}

pub async fn logs(Query(q): Query<LogsQuery>) -> Json<Value> {
    let n = q.lines.unwrap_or(DEFAULT_LOG_LINES).clamp(1, MAX_LOG_LINES);
    Json(json!({ "lines": recent_logs(n) }))
}

#[derive(Debug, Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub url: Option<String>,
}

fn parts(v: &str) -> [u64; 3] {
    let mut out = [0; 3];
    for (slot, piece) in out.iter_mut().zip(v.trim_start_matches('v').split('.')) {
        *slot = piece
            .split(['-', '+'])
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
    }
    out
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    parts(candidate) > parts(current)
}

fn evaluate(release: RawRelease, current: &str) -> UpdateInfo {
    let latest = release.tag_name.trim_start_matches('v').to_string();
    let usable = !release.draft && !release.prerelease;
    let update_available = usable && is_newer(&latest, current);
    UpdateInfo {
        current: current.to_string(),
        update_available,
        url: update_available.then_some(release.html_url),
        latest: usable.then_some(latest),
    }
}

/// Never fails on network errors: a headless server may have no internet, so `latest` stays `None`.
pub async fn update() -> Result<Json<UpdateInfo>, ApiError> {
    let current = env!("CARGO_PKG_VERSION");
    let repo = std::env::var("BETTERRACK_UPDATE_REPO").unwrap_or_else(|_| DEFAULT_REPO.into());
    let fetched = async {
        let resp = reqwest::Client::new()
            .get(format!(
                "https://api.github.com/repos/{repo}/releases/latest"
            ))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "BetterRack")
            .timeout(Duration::from_secs(10))
            .send()
            .await?
            .error_for_status()?;
        resp.json::<RawRelease>().await
    }
    .await;
    Ok(Json(match fetched {
        Ok(release) => evaluate(release, current),
        Err(e) => {
            tracing::info!("update check failed: {e}");
            UpdateInfo {
                current: current.into(),
                latest: None,
                update_available: false,
                url: None,
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, prerelease: bool) -> RawRelease {
        RawRelease {
            tag_name: tag.into(),
            html_url: "https://example.test/r".into(),
            draft: false,
            prerelease,
        }
    }

    #[test]
    fn newer_release_is_offered_with_its_url() {
        let info = evaluate(release("v1.2.0", false), "1.1.9");
        assert!(info.update_available);
        assert_eq!(info.latest.as_deref(), Some("1.2.0"));
        assert_eq!(info.url.as_deref(), Some("https://example.test/r"));
    }

    #[test]
    fn same_or_older_or_prerelease_is_not_offered() {
        assert!(!evaluate(release("1.0.0", false), "1.0.0").update_available);
        assert!(!evaluate(release("0.9.0", false), "1.0.0").update_available);
        let pre = evaluate(release("2.0.0", true), "1.0.0");
        assert!(!pre.update_available);
        assert_eq!(pre.latest, None);
    }

    #[test]
    fn log_buffer_keeps_the_newest_lines() {
        for i in 0..(MAX_LOG_LINES + 5) {
            push_log(&format!("line-marker-{i}\n"));
        }
        let last = recent_logs(3);
        assert_eq!(last.len(), 3);
        assert_eq!(last[2], format!("line-marker-{}", MAX_LOG_LINES + 4));
        assert!(recent_logs(usize::MAX).len() <= MAX_LOG_LINES);
    }
}
