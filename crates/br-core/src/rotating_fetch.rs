//! `RotatingFetchModel`: every attempt goes out with a fresh, internally consistent browser
//! fingerprint (User-Agent + Client Hints), and a retryable status or a thrown error triggers
//! another attempt with exponential backoff. Constants are the Bun ones, verbatim.

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub const DEFAULT_RETRIES: u32 = 3;
pub const DEFAULT_BACKOFF_MS: u64 = 2000;
pub const DEFAULT_JITTER_MS: u64 = 1000;
pub const BACKOFF_CAP_MS: u64 = 30_000;
pub const DEFAULT_RETRY_STATUSES: [u16; 8] = [403, 408, 425, 429, 500, 502, 503, 504];

/// A matched set of client fingerprint headers: the User-Agent and the Client Hints must agree.
#[derive(Debug, Clone, Copy)]
pub struct BrowserProfile {
    pub user_agent: &'static str,
    /// `Sec-CH-UA`; `None` for an engine that does not send Client Hints.
    pub sec_ch_ua: Option<&'static str>,
    pub platform: &'static str,
    pub mobile: bool,
    pub accept_languages: &'static [&'static str],
}

pub const DEFAULT_PROFILES: [BrowserProfile; 6] = [
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
        sec_ch_ua: Some("\"Chromium\";v=\"124\", \"Google Chrome\";v=\"124\", \"Not-A.Brand\";v=\"99\""),
        platform: "\"Windows\"",
        mobile: false,
        accept_languages: &["en-US,en;q=0.9", "en-GB,en;q=0.8"],
    },
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36",
        sec_ch_ua: Some("\"Chromium\";v=\"123\", \"Google Chrome\";v=\"123\", \"Not.A/Brand\";v=\"24\""),
        platform: "\"macOS\"",
        mobile: false,
        accept_languages: &["en-US,en;q=0.9"],
    },
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0",
        sec_ch_ua: None,
        platform: "\"Windows\"",
        mobile: false,
        accept_languages: &["en-US,en;q=0.5", "en-GB,en;q=0.7,en;q=0.3"],
    },
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:125.0) Gecko/20100101 Firefox/125.0",
        sec_ch_ua: None,
        platform: "\"macOS\"",
        mobile: false,
        accept_languages: &["en-US,en;q=0.5"],
    },
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4.1 Safari/605.1.15",
        sec_ch_ua: None,
        platform: "\"macOS\"",
        mobile: false,
        accept_languages: &["en-US,en;q=0.9"],
    },
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0",
        sec_ch_ua: Some("\"Chromium\";v=\"124\", \"Microsoft Edge\";v=\"124\", \"Not-A.Brand\";v=\"99\""),
        platform: "\"Windows\"",
        mobile: false,
        accept_languages: &["en-US,en;q=0.9"],
    },
];

#[derive(Debug)]
pub enum FetchError {
    /// The caller's cancellation token fired.
    Aborted,
    Network(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aborted => f.write_str(ABORT_MESSAGE),
            Self::Network(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for FetchError {}

/// What a cancelled `fetch` reports (the `AbortError` message in Bun).
pub const ABORT_MESSAGE: &str = "This operation was aborted";

#[derive(Clone)]
pub struct RotatingFetch {
    client: reqwest::Client,
    retries: u32,
    backoff_ms: u64,
    jitter_ms: u64,
    retry_statuses: Vec<u16>,
}

fn pick<T: Copy>(list: &[T]) -> T {
    list[fastrand::usize(..list.len())]
}

/// Sleep that returns `false` as soon as `cancel` fires.
pub async fn abortable_sleep(ms: u64, cancel: Option<&CancellationToken>) -> bool {
    match cancel {
        Some(c) => tokio::select! {
            () = tokio::time::sleep(Duration::from_millis(ms)) => true,
            () = c.cancelled() => false,
        },
        None => {
            tokio::time::sleep(Duration::from_millis(ms)).await;
            true
        }
    }
}

impl RotatingFetch {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client, retries: DEFAULT_RETRIES, backoff_ms: DEFAULT_BACKOFF_MS, jitter_ms: DEFAULT_JITTER_MS, retry_statuses: DEFAULT_RETRY_STATUSES.to_vec() }
    }

    /// Override the retry timing (tests use zeros).
    pub fn with_timing(mut self, retries: u32, backoff_ms: u64, jitter_ms: u64) -> Self {
        self.retries = retries;
        self.backoff_ms = backoff_ms;
        self.jitter_ms = jitter_ms;
        self
    }

    pub fn build_headers(&self, profile: BrowserProfile) -> HeaderMap {
        let mut h = HeaderMap::new();
        let mut put = |k: &'static str, v: &str| {
            if let Ok(v) = HeaderValue::from_str(v) {
                h.insert(HeaderName::from_static(k), v);
            }
        };
        put("user-agent", profile.user_agent);
        put("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8");
        put("accept-language", pick(profile.accept_languages));
        put("upgrade-insecure-requests", "1");
        put("sec-fetch-dest", "document");
        put("sec-fetch-mode", "navigate");
        put("sec-fetch-site", "none");
        put("sec-fetch-user", "?1");
        put("dnt", "1");
        if let Some(ua) = profile.sec_ch_ua {
            put("sec-ch-ua", ua);
            put("sec-ch-ua-mobile", if profile.mobile { "?1" } else { "?0" });
            put("sec-ch-ua-platform", profile.platform);
        }
        h
    }

    /// GET `url`, retrying with a fresh identity on a retryable status or a transport error.
    /// `extra` headers override the identity's. After the last attempt the last response is
    /// returned even if its status is retryable.
    pub async fn fetch(&self, url: &str, extra: &[(&str, &str)], cancel: Option<&CancellationToken>) -> Result<reqwest::Response, FetchError> {
        let mut last_response = None;
        let mut last_error = None;
        for attempt in 0..=self.retries {
            if cancel.is_some_and(CancellationToken::is_cancelled) {
                return Err(FetchError::Aborted);
            }
            if attempt > 0 {
                let wait = (2u64.saturating_pow(attempt) * self.backoff_ms).min(BACKOFF_CAP_MS) + fastrand::u64(0..=self.jitter_ms);
                if !abortable_sleep(wait, cancel).await {
                    return Err(FetchError::Aborted);
                }
            }
            let mut headers = self.build_headers(pick(&DEFAULT_PROFILES));
            for (k, v) in extra {
                if let (Ok(k), Ok(v)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v)) {
                    headers.insert(k, v);
                }
            }
            let send = self.client.get(url).headers(headers).send();
            let result = match cancel {
                Some(c) => tokio::select! {
                    r = send => r,
                    () = c.cancelled() => return Err(FetchError::Aborted),
                },
                None => send.await,
            };
            match result {
                Ok(res) => {
                    if self.retry_statuses.contains(&res.status().as_u16()) {
                        tracing::info!(%url, status = res.status().as_u16(), attempt = attempt + 1, "RotatingFetch: retrying with a fresh identity");
                        last_response = Some(res);
                        continue;
                    }
                    return Ok(res);
                }
                Err(e) => {
                    tracing::error!(%url, err = %e, attempt = attempt + 1, "RotatingFetch: request failed, retrying with a fresh identity");
                    last_error = Some(e.to_string());
                }
            }
        }
        match (last_response, last_error) {
            (Some(res), _) => Ok(res),
            (None, Some(e)) => Err(FetchError::Network(e)),
            (None, None) => Err(FetchError::Network(format!("RotatingFetch: all attempts failed for {url}"))),
        }
    }
}
