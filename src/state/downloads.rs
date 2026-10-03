//! Download jobs (`store-downloads.page.tsx`). The Downloads page polls `GET /api/downloads/jobs`
//! every [`POLL_INTERVAL_MS`] while it is open (the React page does the same; the SSE stream is not
//! used by the current client). Jobs that flip to `done` while we watch toast and trigger a silent
//! library refresh, because the server rescans after a download finishes.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use gpui::{Context, Task};

use crate::api::ApiClient;
use crate::model::{JobState, JobStatus, POLL_INTERVAL_MS, StoreProgressEvent};
use crate::runtime;
use crate::state::Stores;
use crate::ui::toast;

#[derive(Default)]
pub struct DownloadsStore {
    pub client: Option<ApiClient>,
    pub jobs: Vec<JobStatus>,
    pub error: String,
    pub loaded: bool,
    pub retrying: HashSet<String>,
    pub stopping: HashSet<String>,
    previous: HashMap<String, JobState>,
}

/// Jobs that were seen in another state before and are `done` now. A job first seen already done
/// (opening the page after it finished) does not count.
pub fn newly_done<'a>(previous: &HashMap<String, JobState>, jobs: &'a [JobStatus]) -> Vec<&'a JobStatus> {
    jobs.iter()
        .filter(|j| j.state == JobState::Done && previous.get(&j.job_id).is_some_and(|s| *s != JobState::Done))
        .collect()
}

/// Progress bar value for the job, if it has one (`percentFor`).
pub fn percent_for(progress: Option<&StoreProgressEvent>) -> Option<u32> {
    match progress? {
        StoreProgressEvent::Progress { percent, .. } => Some(percent.round().clamp(0.0, 100.0) as u32),
        StoreProgressEvent::Extracting { done, total, .. } if *total > 0 => {
            Some(((*done as f32 / *total as f32) * 100.0).round().clamp(0.0, 100.0) as u32)
        }
        _ => None,
    }
}

/// The grey line under the bar (`detailFor`).
pub fn detail_for(progress: Option<&StoreProgressEvent>) -> String {
    match progress {
        None => String::new(),
        Some(StoreProgressEvent::Preparing { .. }) => "Preparing…".into(),
        Some(StoreProgressEvent::Retrying { status: Some(status), delay_sec, .. }) => {
            format!("Retrying after HTTP {status} — waiting {}s", delay_sec.round())
        }
        Some(StoreProgressEvent::Retrying { delay_sec, .. }) => {
            format!("Connection lost — retrying in {}s", delay_sec.round())
        }
        Some(StoreProgressEvent::Progress { received_mb, total_mb, .. }) => format!("{received_mb} / {total_mb} MB"),
        Some(StoreProgressEvent::Extracting { done, total, .. }) => format!("Extracting {done} / {total}"),
        Some(StoreProgressEvent::Done { filename }) => filename.clone(),
        Some(StoreProgressEvent::Error { message }) => message.clone(),
    }
}

/// Active jobs can be stopped, except while extracting (the server refuses).
pub fn can_stop(job: &JobStatus) -> bool {
    matches!(job.state, JobState::Queued | JobState::Running)
        && !matches!(job.progress, Some(StoreProgressEvent::Extracting { .. }))
}

pub fn state_label(state: JobState) -> &'static str {
    match state {
        JobState::Queued => "Queued",
        JobState::Running => "Downloading",
        JobState::Done => "Done",
        JobState::Error => "Failed",
    }
}

fn describe(e: impl std::fmt::Display, default: &str) -> String {
    let m = e.to_string();
    if m.is_empty() { default.to_string() } else { m }
}

impl DownloadsStore {
    /// Poll until the returned task is dropped (the page holds it).
    pub fn start_polling(&mut self, cx: &mut Context<Self>) -> Task<()> {
        self.previous.clear();
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some(client)) = this.read_with(cx, |s, _| s.client.clone()) else { return };
                let result = runtime::run(async move { client.download_jobs().await }).await;
                let alive = this.update(cx, |s, cx| s.apply(result.map_err(|e| e.to_string()), cx)).is_ok();
                if !alive {
                    return;
                }
                cx.background_executor().timer(Duration::from_millis(POLL_INTERVAL_MS)).await;
            }
        })
    }

    fn apply(&mut self, result: Result<Vec<JobStatus>, String>, cx: &mut Context<Self>) {
        self.loaded = true;
        match result {
            Ok(jobs) => {
                let finished: Vec<String> = newly_done(&self.previous, &jobs).iter().map(|j| j.label.clone()).collect();
                self.previous = jobs.iter().map(|j| (j.job_id.clone(), j.state)).collect();
                self.jobs = jobs;
                self.error.clear();
                if !finished.is_empty() {
                    for label in finished {
                        toast::success(cx, format!("{label} downloaded"));
                    }
                    if let Some(stores) = cx.try_global::<Stores>().cloned() {
                        stores.library.update(cx, |l, cx| l.refresh(true, cx));
                    }
                }
            }
            Err(message) => self.error = describe(message, "Failed to load download jobs."),
        }
        cx.notify();
    }

    pub fn retry(&mut self, job_id: String, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else { return };
        self.retrying.insert(job_id.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let id = job_id.clone();
            let c = client.clone();
            let result = runtime::run(async move {
                c.retry_download(&id).await?;
                c.download_jobs().await
            })
            .await;
            this.update(cx, |s, cx| {
                s.retrying.remove(&job_id);
                match result {
                    Ok(jobs) => s.jobs = jobs,
                    Err(e) => toast::error(cx, describe(e, "Failed to retry the download.")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn stop(&mut self, job_id: String, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else { return };
        self.stopping.insert(job_id.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let id = job_id.clone();
            let c = client.clone();
            let result = runtime::run(async move {
                c.cancel_download(&id).await?;
                c.download_jobs().await
            })
            .await;
            this.update(cx, |s, cx| {
                s.stopping.remove(&job_id);
                match result {
                    Ok(jobs) => s.jobs = jobs,
                    Err(e) => toast::error(cx, describe(e, "Failed to stop the download.")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str, state: JobState, progress: Option<StoreProgressEvent>) -> JobStatus {
        JobStatus { job_id: id.into(), state, label: id.into(), progress }
    }

    #[test]
    fn only_jobs_that_finish_while_watched_are_announced() {
        let prev = HashMap::from([("a".to_string(), JobState::Running), ("b".to_string(), JobState::Done)]);
        let now = vec![
            job("a", JobState::Done, None),
            job("b", JobState::Done, None),
            job("c", JobState::Done, None),
            job("d", JobState::Running, None),
        ];
        let ids: Vec<_> = newly_done(&prev, &now).iter().map(|j| j.job_id.as_str()).collect();
        assert_eq!(ids, ["a"]);
    }

    #[test]
    fn percent_comes_from_download_or_extraction() {
        let p = StoreProgressEvent::Progress { title: "t".into(), percent: 41.6, received_mb: "1".into(), total_mb: "2".into() };
        assert_eq!(percent_for(Some(&p)), Some(42));
        let e = StoreProgressEvent::Extracting { title: "t".into(), done: 1, total: 4 };
        assert_eq!(percent_for(Some(&e)), Some(25));
        let zero = StoreProgressEvent::Extracting { title: "t".into(), done: 0, total: 0 };
        assert_eq!(percent_for(Some(&zero)), None);
        assert_eq!(percent_for(Some(&StoreProgressEvent::Preparing { title: "t".into() })), None);
        assert_eq!(percent_for(None), None);
    }

    #[test]
    fn detail_text_matches_the_react_page() {
        use crate::model::RetryReason::{Http, Network};
        let http = StoreProgressEvent::Retrying { title: "t".into(), status: Some(503), reason: Some(Http), delay_sec: 5.0 };
        assert_eq!(detail_for(Some(&http)), "Retrying after HTTP 503 — waiting 5s");
        let net = StoreProgressEvent::Retrying { title: "t".into(), status: None, reason: Some(Network), delay_sec: 2.0 };
        assert_eq!(detail_for(Some(&net)), "Connection lost — retrying in 2s");
        let p = StoreProgressEvent::Progress { title: "t".into(), percent: 5.0, received_mb: "1.5".into(), total_mb: "30.0".into() };
        assert_eq!(detail_for(Some(&p)), "1.5 / 30.0 MB");
        assert_eq!(detail_for(None), "");
    }

    #[test]
    fn extracting_jobs_cannot_be_stopped() {
        let extracting = StoreProgressEvent::Extracting { title: "t".into(), done: 1, total: 2 };
        assert!(!can_stop(&job("a", JobState::Running, Some(extracting))));
        assert!(can_stop(&job("a", JobState::Queued, None)));
        assert!(!can_stop(&job("a", JobState::Done, None)));
        assert!(!can_stop(&job("a", JobState::Error, None)));
    }
}
