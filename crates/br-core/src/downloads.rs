//! The download job lifecycle (the logic of `DownloadController`, minus HTTP): start, retry,
//! cancel and list jobs, run the [`Downloader`] for each and keep the [`JobStore`] and the library
//! index in step.

use crate::download::{DownloadRequest, Downloader, ProgressCb, ProgressEvent};
use crate::jobs::{Job, JobRequest, JobState, JobStore};
use crate::library::Library;
use crate::store::StoreApi;
use crate::uid::resolve_windows;
use crate::{CoreError, Result};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

pub const KIND_DOWNLOAD: &str = "download";

pub struct DownloadService {
    jobs: Arc<JobStore>,
    store: Arc<StoreApi>,
    downloader: Arc<Downloader>,
    library: Arc<Library>,
    cwd: String,
    active: Mutex<HashMap<String, CancellationToken>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CancelOutcome {
    Cancelled,
    NotFound,
    /// Finished already (`done`/`error`).
    NotActive,
    /// The pack is being unpacked, which cannot be interrupted safely.
    Extracting,
}

#[derive(Debug, PartialEq)]
pub enum RetryOutcome {
    Started(Box<Job>),
    NotFound,
    /// Only `error` jobs can be retried.
    NotFailed,
    /// The store no longer offers a link for this comic.
    NoLink,
    /// The job changed state between the checks.
    Gone,
}

fn event_state(event: &ProgressEvent) -> JobState {
    match event {
        ProgressEvent::Done { .. } => JobState::Done,
        ProgressEvent::Error { .. } => JobState::Error,
        _ => JobState::Running,
    }
}

/// The wire form of a job in `/api/downloads*` answers.
pub fn status_payload(job: &Job) -> Value {
    let mut v = json!({ "jobId": job.id, "state": job.state.as_str(), "label": job.label });
    if let Some(p) = &job.progress {
        v["progress"] = p.clone();
    }
    v
}

/// Jobs as `GET /api/downloads/jobs` orders them: running, queued, failed, finished, then newest
/// first (`createdAt`, not `updatedAt`: progress ticks would churn the order).
pub fn sorted_download_jobs(mut jobs: Vec<Job>) -> Vec<Job> {
    let rank = |s: JobState| match s {
        JobState::Running => 0,
        JobState::Queued => 1,
        JobState::Error => 2,
        JobState::Done => 3,
    };
    jobs.sort_by(|a, b| rank(a.state).cmp(&rank(b.state)).then(b.created_at.cmp(&a.created_at)).then_with(|| a.id.cmp(&b.id)));
    jobs
}

impl DownloadService {
    pub fn new(jobs: Arc<JobStore>, store: Arc<StoreApi>, downloader: Arc<Downloader>, library: Arc<Library>, cwd: String) -> Self {
        Self { jobs, store, downloader, library, cwd, active: Mutex::new(HashMap::new()) }
    }

    pub fn jobs(&self) -> &Arc<JobStore> {
        &self.jobs
    }

    pub fn store(&self) -> &Arc<StoreApi> {
        &self.store
    }

    /// `fsModel.getFullPath(outputDir || '/')`.
    pub fn full_path(&self, output_dir: Option<&str>) -> String {
        resolve_windows(output_dir.filter(|d| !d.is_empty()).unwrap_or("/"), &self.cwd)
    }

    fn active(&self) -> std::sync::MutexGuard<'_, HashMap<String, CancellationToken>> {
        self.active.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The direct link behind a listed `uuid`, `None` when the store cannot provide one.
    pub async fn resolve_link(&self, post_id: i64, strat: Option<&str>, uuid: Option<&str>) -> Result<Option<String>> {
        Ok(self.store.get_download_link_from_post(post_id, strat, uuid).await?.filter(|l| !l.is_empty()))
    }

    /// Create the job for `resource_key` and start it, or return the one already in flight.
    pub fn start(self: &Arc<Self>, resource_key: &str, title: &str, request: JobRequest, link: String) -> Result<(Job, bool)> {
        let (job, created) = self.jobs.get_or_create(resource_key, title, request.clone(), KIND_DOWNLOAD)?;
        if created {
            let dir = self.full_path(request.output_dir.as_deref());
            self.run_download(job.id.clone(), title.to_string(), link, PathBuf::from(dir));
        }
        // Re-read: starting moved the job to `running`.
        Ok((self.jobs.get(&job.id).unwrap_or(job), created))
    }

    pub async fn retry(self: &Arc<Self>, job_id: &str) -> Result<RetryOutcome> {
        let Some(job) = self.jobs.get(job_id) else { return Ok(RetryOutcome::NotFound) };
        if job.state != JobState::Error {
            return Ok(RetryOutcome::NotFailed);
        }
        let link = self.resolve_link(job.request.comic_id, job.request.strat.as_deref(), job.request.uuid.as_deref()).await?;
        let Some(link) = link else { return Ok(RetryOutcome::NoLink) };
        let Some(retried) = self.jobs.retry(job_id)? else { return Ok(RetryOutcome::Gone) };
        let dir = self.full_path(retried.request.output_dir.as_deref());
        self.run_download(retried.id.clone(), retried.label.clone(), link, PathBuf::from(dir));
        let current = self.jobs.get(&retried.id).unwrap_or(retried);
        Ok(RetryOutcome::Started(Box::new(current)))
    }

    pub fn cancel(&self, job_id: &str) -> Result<CancelOutcome> {
        let Some(job) = self.jobs.get(job_id) else { return Ok(CancelOutcome::NotFound) };
        if !matches!(job.state, JobState::Queued | JobState::Running) {
            return Ok(CancelOutcome::NotActive);
        }
        if job.progress.as_ref().and_then(|p| p["type"].as_str()) == Some("extracting") {
            return Ok(CancelOutcome::Extracting);
        }
        if let Some(token) = self.active().get(job_id) {
            token.cancel();
        }
        self.jobs.remove(job_id)?;
        Ok(CancelOutcome::Cancelled)
    }

    pub fn list(&self) -> Vec<Job> {
        sorted_download_jobs(self.jobs.list(Some(KIND_DOWNLOAD)))
    }

    async fn rescan_library(library: Arc<Library>) {
        let ticket = library.ticket();
        if let Err(e) = tokio::task::spawn_blocking(move || library.rescan_with(ticket)).await.map_err(|e| CoreError::Invalid(e.to_string())).and_then(|r| r) {
            tracing::error!(err = %e, "library rescan after download failed");
        }
    }

    fn run_download(self: &Arc<Self>, job_id: String, title: String, link: String, output_dir: PathBuf) {
        if let Err(e) = self.jobs.update(&job_id, JobState::Running, None) {
            tracing::error!(err = %e, "could not mark the download as running");
        }
        let cancel = CancellationToken::new();
        self.active().insert(job_id.clone(), cancel.clone());

        let svc = self.clone();
        let on_progress: ProgressCb = {
            let (svc, cancel, job_id) = (svc.clone(), cancel.clone(), job_id.clone());
            Arc::new(move |event: ProgressEvent| {
                if cancel.is_cancelled() {
                    return;
                }
                let state = event_state(&event);
                let value = event.to_value();
                if state == JobState::Done {
                    // The job turns `done` only once the library knows the new file.
                    let (svc, job_id) = (svc.clone(), job_id.clone());
                    tokio::spawn(async move {
                        Self::rescan_library(svc.library.clone()).await;
                        if let Err(e) = svc.jobs.update(&job_id, state, Some(value)) {
                            tracing::error!(err = %e, "could not finish the download job");
                        }
                    });
                } else if let Err(e) = svc.jobs.update(&job_id, state, Some(value)) {
                    tracing::error!(err = %e, "could not update the download job");
                }
            })
        };

        let request = DownloadRequest { title, download_link: link, output_dir, no_retry: false, cancel };
        let task_id = job_id.clone();
        let downloader = self.downloader.clone();
        let handle = tokio::spawn(async move { downloader.download_comic(request, on_progress).await });
        tokio::spawn(async move {
            if let Err(e) = handle.await {
                tracing::error!(err = %e, "download job failed");
                let _ = svc.jobs.update(&task_id, JobState::Error, Some(json!({"type": "error", "message": "Failed to download"})));
            }
            svc.active().remove(&task_id);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str, state: JobState, created_at: i64) -> Job {
        Job {
            id: id.into(),
            kind: KIND_DOWNLOAD.into(),
            resource_key: id.into(),
            label: id.into(),
            state,
            progress: None,
            request: JobRequest::default(),
            created_at,
            updated_at: created_at,
        }
    }

    #[test]
    fn jobs_sort_by_state_then_newest() {
        let sorted = sorted_download_jobs(vec![
            job("done-old", JobState::Done, 1),
            job("err", JobState::Error, 5),
            job("run-old", JobState::Running, 2),
            job("run-new", JobState::Running, 9),
            job("queued", JobState::Queued, 3),
            job("done-new", JobState::Done, 8),
        ]);
        let ids: Vec<&str> = sorted.iter().map(|j| j.id.as_str()).collect();
        assert_eq!(ids, ["run-new", "run-old", "queued", "err", "done-new", "done-old"]);
    }

    #[test]
    fn payload_omits_missing_progress() {
        let mut j = job("a", JobState::Queued, 1);
        assert_eq!(status_payload(&j), json!({"jobId": "a", "state": "queued", "label": "a"}));
        j.progress = Some(json!({"type": "preparing", "title": "a"}));
        assert_eq!(status_payload(&j)["progress"]["type"], "preparing");
    }
}
