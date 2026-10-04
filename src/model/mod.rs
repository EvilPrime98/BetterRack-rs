//! Wire types. All JSON is camelCase; every optional field is `Option`.

mod view;
#[allow(unused_imports)]
pub use view::*;

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

fn null_to_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// The server reports file mtimes as fractional ms (e.g. `1789670099289.6057`); accept int, float or null.
fn ms_to_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?
        .map(|n| n as i64)
        .unwrap_or_default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetaSource {
    Wiki,
    Comicinfo,
}

/// One file OR folder entry. NOTE: `did` == "is dir". `identified` is tri-state (files only):
/// `None` = never looked up, `Some(true)` = identified, `Some(false)` = looked up, no match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    pub uid: String,
    #[serde(default)]
    pub did: bool,
    pub name: String,
    #[serde(default)]
    pub path: String,
    /// `""` for top level inside a group.
    #[serde(default, deserialize_with = "null_to_default")]
    pub parent_id: String,
    /// ms since epoch (file mtime at scan time).
    #[serde(default, deserialize_with = "ms_to_i64")]
    pub created_at: i64,
    pub identified: Option<bool>,
    /// Present when identified. Metadata only, never the cover source.
    pub comic: Option<WikiComic>,
    pub meta_source: Option<MetaSource>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryGroup {
    pub uid: String,
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub entries: Vec<LibraryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
    pub groups: Vec<LibraryGroup>,
    #[serde(default)]
    pub total: usize,
    #[serde(default)]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub has_more: bool,
}

/// `GET /api/library/recent`. `generated_at` type not confirmed (verify), so kept loose.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentResponse {
    pub items: Vec<LibraryEntry>,
    pub window_hours: Option<u32>,
    pub generated_at: Option<serde_json::Value>,
}

/// `GET /api/library/reading`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingResponse {
    pub items: Vec<LibraryEntry>,
    pub generated_at: Option<serde_json::Value>,
}

/// Per-comic client cache (`IComicLSCache`), keyed by uid.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ComicCache {
    pub rating: f32,
    pub current_page: u32,
    /// 0..=100
    pub read_per: f32,
    pub read: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub output_dirs: Vec<String>,
    pub api_url: String,
    pub download_dir: String,
    pub wiki_search: bool,
    pub rescan_on_startup: bool,
}

/// `PUT /api/settings` body: a partial that must NOT contain `outputDirs`.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wiki_search: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rescan_on_startup: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    /// 1-based.
    pub page: u32,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderPages {
    /// Archive entry names.
    pub pages: Vec<String>,
    pub total_pages: usize,
}

/// Mirror of `better-wiki`'s `dc-fandom` comic. Known fields are typed;
/// everything else lands in `extra` so nothing is lost on a round trip (identify commits it back).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WikiComic {
    pub id: Option<serde_json::Value>,
    pub title: Option<String>,
    pub volume: Option<serde_json::Value>,
    pub issue: Option<serde_json::Value>,
    pub release_date: Option<ReleaseDate>,
    pub credits: Option<Credits>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// The wiki sends month/day as strings (`"05"`) and sometimes numbers, so keep the raw JSON (it is
/// committed back verbatim on identify) and read it through the accessors.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReleaseDate {
    pub release_year: Option<serde_json::Value>,
    pub release_month: Option<serde_json::Value>,
    pub release_day: Option<serde_json::Value>,
}

fn json_num(v: &Option<serde_json::Value>) -> Option<i32> {
    match v.as_ref()? {
        serde_json::Value::Number(n) => n.as_i64().map(|n| n as i32),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

impl ReleaseDate {
    pub fn year(&self) -> Option<i32> {
        json_num(&self.release_year).filter(|n| *n != 0)
    }
    pub fn month(&self) -> Option<i32> {
        json_num(&self.release_month).filter(|n| *n != 0)
    }
    pub fn day(&self) -> Option<i32> {
        json_num(&self.release_day).filter(|n| *n != 0)
    }
    /// Sort key; `None` when the comic has no usable date.
    pub fn sort_key(&self) -> Option<(i32, i32, i32)> {
        Some((
            self.year()?,
            self.month().unwrap_or(0),
            self.day().unwrap_or(0),
        ))
    }
    /// `MM/DD/YYYY`, only when all three parts exist.
    pub fn display(&self) -> String {
        match (self.month(), self.day(), self.year()) {
            (Some(m), Some(d), Some(y)) => format!("{m:02}/{d:02}/{y}"),
            _ => String::new(),
        }
    }
}

impl WikiComic {
    pub fn writers(&self) -> &[String] {
        self.credits
            .as_ref()
            .map(|c| c.writers.as_slice())
            .unwrap_or(&[])
    }

    pub fn artists(&self) -> Vec<String> {
        self.credits
            .as_ref()
            .and_then(|c| c.extra.get("artists"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Wiki text arrives with MediaWiki markup (`[[Page|label]]`, `'''bold'''`, `<!-- comments -->`).
/// The readable form is shown: links become their label, emphasis
/// quotes and comments are dropped.
pub fn clean_wiki_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.split_once("-->").map_or("", |(_, tail)| tail);
        } else if let Some(after) = rest.strip_prefix("[[") {
            match after.split_once("]]") {
                Some((inner, tail)) => {
                    out.push_str(inner.rsplit_once('|').map_or(inner, |(_, label)| label));
                    rest = tail;
                }
                None => {
                    out.push_str("[[");
                    rest = after;
                }
            }
        } else if rest.starts_with("''") {
            rest = rest.trim_start_matches('\'');
        } else {
            let mut chars = rest.chars();
            out.extend(chars.next());
            rest = chars.as_str();
        }
    }
    out.trim().to_string()
}

impl WikiComic {
    pub fn cover(&self) -> String {
        self.extra
            .get("cover")
            .and_then(|v| v.as_str())
            .map(clean_wiki_text)
            .unwrap_or_default()
    }
}

/// JSON scalar as display text (`""` for null/absent, so callers can hide empty rows).
pub fn json_text(v: &Option<serde_json::Value>) -> String {
    match v {
        Some(serde_json::Value::String(s)) if s != "undefined" => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Credits {
    pub writers: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyResponse {
    pub identified: Option<bool>,
    pub comic: Option<WikiComic>,
    pub meta_source: Option<MetaSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum IdentifyProgress {
    Identifying { done: u32, total: u32 },
    Done { total: u32 },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyJob {
    pub job_id: String,
    pub state: JobState,
    pub progress: Option<IdentifyProgress>,
}

/// `GET/POST /api/library/identify/all`. The "idle" shape is not confirmed (verify): we accept
/// either a bare job or a `{job: {...}}` wrapper and treat anything without a job as idle.
#[derive(Debug, Clone)]
pub enum IdentifyLibraryStatus {
    Idle,
    Job(IdentifyJob),
}

impl IdentifyLibraryStatus {
    pub fn from_value(v: serde_json::Value) -> Self {
        match serde_json::from_value::<IdentifyJob>(v.clone()) {
            Ok(job) => Self::Job(job),
            Err(_) => match v
                .get("job")
                .cloned()
                .map(serde_json::from_value::<IdentifyJob>)
            {
                Some(Ok(job)) => Self::Job(job),
                _ => Self::Idle,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorePost {
    pub id: Option<i64>,
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub link: String,
    pub upload_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoreLink {
    pub uuid: String,
    #[serde(default)]
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoreStrat {
    All,
    Single,
    Multiple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RetryReason {
    Network,
    Http,
}

/// SSE / job progress, tagged by `type` (variant spellings are camelCase; note the server spells the sizes `receivedMB` / `totalMB`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StoreProgressEvent {
    #[serde(rename_all = "camelCase")]
    Preparing { title: String },
    #[serde(rename_all = "camelCase")]
    Retrying {
        title: String,
        status: Option<u16>,
        reason: Option<RetryReason>,
        delay_sec: f32,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        title: String,
        percent: f32,
        #[serde(rename = "receivedMB")]
        received_mb: String,
        #[serde(rename = "totalMB")]
        total_mb: String,
    },
    #[serde(rename_all = "camelCase")]
    Extracting {
        title: String,
        done: u32,
        total: u32,
    },
    #[serde(rename_all = "camelCase")]
    Done { filename: String },
    #[serde(rename_all = "camelCase")]
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub job_id: String,
    pub state: JobState,
    #[serde(default)]
    pub label: String,
    /// An unknown progress shape must not take the whole jobs list down: it reads as `None`.
    #[serde(default, deserialize_with = "lenient_progress")]
    pub progress: Option<StoreProgressEvent>,
}

fn lenient_progress<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<StoreProgressEvent>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| serde_json::from_value(v).ok()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDownload {
    pub id: i64,
    pub title: String,
    pub uuid: String,
    pub output_dir: String,
    pub strat: StoreStrat,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Health {
    pub app: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_tolerates_nulls_and_missing_fields() {
        let e: LibraryEntry = serde_json::from_str(
            r#"{"uid":"a","did":false,"name":"X","path":"/x","parentId":null,"createdAt":1700000000000}"#,
        )
        .unwrap();
        assert_eq!(e.parent_id, "");
        assert_eq!(e.identified, None);
        assert!(!e.did);
    }

    #[test]
    fn identified_is_tristate() {
        let f: LibraryEntry =
            serde_json::from_str(r#"{"uid":"a","name":"X","identified":false}"#).unwrap();
        assert_eq!(f.identified, Some(false));
    }

    #[test]
    fn wiki_comic_keeps_unknown_sections() {
        let c: WikiComic = serde_json::from_str(
            r#"{"title":"T","releaseDate":{"releaseYear":1999},"trivia":["x"],"credits":{"writers":["A"],"pencillers":["B"]}}"#,
        )
        .unwrap();
        assert_eq!(c.release_date.unwrap().year(), Some(1999));
        assert!(c.extra.contains_key("trivia"));
        let back = serde_json::to_value(&c.credits).unwrap();
        assert!(back.get("pencillers").is_some());
    }

    #[test]
    fn release_date_accepts_strings_and_numbers() {
        let c: WikiComic = serde_json::from_str(
            r#"{"releaseDate":{"releaseYear":"1999","releaseMonth":"05","releaseDay":7}}"#,
        )
        .unwrap();
        let d = c.release_date.unwrap();
        assert_eq!(d.sort_key(), Some((1999, 5, 7)));
        assert_eq!(d.display(), "05/07/1999");
    }

    #[test]
    fn progress_event_roundtrip() {
        let e: StoreProgressEvent = serde_json::from_str(
            r#"{"type":"progress","title":"t","percent":12.5,"receivedMB":"1.0","totalMB":"8.0"}"#,
        )
        .unwrap();
        assert!(
            matches!(&e, StoreProgressEvent::Progress { received_mb, total_mb, .. } if received_mb == "1.0" && total_mb == "8.0")
        );
    }

    #[test]
    fn retrying_without_a_reason_still_parses() {
        let e: StoreProgressEvent =
            serde_json::from_str(r#"{"type":"retrying","title":"t","delaySec":3}"#).unwrap();
        assert!(matches!(
            e,
            StoreProgressEvent::Retrying {
                status: None,
                reason: None,
                ..
            }
        ));
    }

    #[test]
    fn job_list_survives_an_unknown_progress_shape() {
        let jobs: Vec<JobStatus> = serde_json::from_str(
            r#"[{"jobId":"a","state":"running","label":"A","progress":{"type":"mystery"}},
                {"jobId":"b","state":"queued","label":"B"}]"#,
        )
        .unwrap();
        assert_eq!(jobs.len(), 2);
        assert!(jobs[0].progress.is_none() && jobs[1].progress.is_none());
    }

    #[test]
    fn settings_update_omits_output_dirs_and_nones() {
        let v = serde_json::to_value(SettingsUpdate {
            wiki_search: Some(true),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({"wikiSearch": true}));
    }
}
