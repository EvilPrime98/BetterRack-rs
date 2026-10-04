//! `jobs.sqlite` (`jobs`): download and identify-library jobs.
//!
//! Mirrors `JobModel`: every row is loaded into memory at startup, `queued`/`running` rows left by
//! a previous process are flipped to `error` ("Interrupted by server restart"), history is pruned
//! (30 days, 200 terminal rows). `progress` is opaque JSON. `subscribe` feeds the SSE stream.

use crate::Result;
use crate::db::{Db, add_column_if_missing};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

const MAX_HISTORY_ROWS: usize = 200;
const HISTORY_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Error,
}

impl JobState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Done => "done",
            Self::Error => "error",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "done" => Self::Done,
            "error" => Self::Error,
            _ => Self::Queued,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JobRequest {
    pub comic_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strat: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    /// `download` or `identify-library`.
    pub kind: String,
    pub resource_key: String,
    pub label: String,
    pub state: JobState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<Value>,
    pub request: JobRequest,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Default)]
struct Inner {
    jobs: HashMap<String, Job>,
    by_resource: HashMap<String, String>,
    /// One channel per job that has a subscriber; dropped when the job reaches a terminal state.
    senders: HashMap<String, broadcast::Sender<Job>>,
}

impl Inner {
    fn notify(&mut self, job: &Job) {
        if let Some(tx) = self.senders.get(&job.id) {
            let _ = tx.send(job.clone());
            if job.state.is_terminal() {
                self.senders.remove(&job.id);
            }
        }
    }
}

pub struct JobStore {
    db: Db,
    inner: Mutex<Inner>,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl JobStore {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Db::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Db::open_in_memory()?)
    }

    fn init(db: Db) -> Result<Self> {
        {
            let c = db.conn();
            let has_table = |name: &str| -> rusqlite::Result<bool> {
                c.query_row(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [name],
                    |_| Ok(()),
                )
                .map(|_| true)
                .or_else(|e| {
                    if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
                        Ok(false)
                    } else {
                        Err(e)
                    }
                })
            };
            if has_table("download_jobs")? && !has_table("jobs")? {
                c.execute("ALTER TABLE download_jobs RENAME TO jobs", [])?;
            }
            c.execute_batch(
                "CREATE TABLE IF NOT EXISTS jobs (
                    id TEXT PRIMARY KEY,
                    kind TEXT NOT NULL DEFAULT 'download',
                    resource_key TEXT NOT NULL,
                    label TEXT NOT NULL,
                    state TEXT NOT NULL,
                    progress TEXT,
                    comic_id INTEGER NOT NULL,
                    output_dir TEXT,
                    uuid TEXT,
                    strat TEXT,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )",
            )?;
            add_column_if_missing(&c, "jobs", "kind TEXT NOT NULL DEFAULT 'download'");
        }
        let store = Self {
            db,
            inner: Mutex::new(Inner::default()),
        };
        store.load_and_reconcile()?;
        store.prune_history()?;
        Ok(store)
    }

    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn load_and_reconcile(&self) -> Result<()> {
        let rows: Vec<Job> = {
            let c = self.db.conn();
            let mut stmt = c.prepare(
                "SELECT id, kind, resource_key, label, state, progress, comic_id, output_dir, uuid, strat, created_at, updated_at FROM jobs",
            )?;
            stmt.query_map([], |r| {
                Ok(Job {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    resource_key: r.get(2)?,
                    label: r.get(3)?,
                    state: JobState::parse(&r.get::<_, String>(4)?),
                    progress: r
                        .get::<_, Option<String>>(5)?
                        .filter(|s| !s.is_empty())
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    request: JobRequest {
                        comic_id: r.get(6)?,
                        output_dir: r.get(7)?,
                        uuid: r.get(8)?,
                        strat: r.get(9)?,
                    },
                    created_at: r.get(10)?,
                    updated_at: r.get(11)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?
        };
        for mut job in rows {
            if !job.state.is_terminal() {
                job.state = JobState::Error;
                job.progress =
                    Some(json!({"type": "error", "message": "Interrupted by server restart"}));
                job.updated_at = now_ms();
                self.persist(&job)?;
            }
            self.inner().jobs.insert(job.id.clone(), job);
        }
        Ok(())
    }

    fn persist(&self, job: &Job) -> Result<()> {
        let progress = job.progress.as_ref().map(|p| p.to_string());
        self.db.conn().execute(
            "INSERT INTO jobs (id, kind, resource_key, label, state, progress, comic_id, output_dir, uuid, strat, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET state = excluded.state, progress = excluded.progress, updated_at = excluded.updated_at",
            params![
                job.id, job.kind, job.resource_key, job.label, job.state.as_str(), progress,
                job.request.comic_id, job.request.output_dir, job.request.uuid, job.request.strat,
                job.created_at, job.updated_at
            ],
        )?;
        Ok(())
    }

    fn prune_history(&self) -> Result<()> {
        let cutoff = now_ms() - HISTORY_RETENTION_MS;
        let mut inner = self.inner();
        let mut terminal: Vec<&Job> = inner
            .jobs
            .values()
            .filter(|j| j.state.is_terminal())
            .collect();
        terminal.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        let mut stale: Vec<String> = terminal
            .iter()
            .filter(|j| j.updated_at < cutoff)
            .map(|j| j.id.clone())
            .collect();
        stale.extend(terminal.iter().skip(MAX_HISTORY_ROWS).map(|j| j.id.clone()));
        stale.sort();
        stale.dedup();
        if stale.is_empty() {
            return Ok(());
        }
        let c = self.db.conn();
        for id in &stale {
            c.execute("DELETE FROM jobs WHERE id = ?1", [id])?;
            inner.jobs.remove(id);
        }
        Ok(())
    }

    /// Returns the existing job for `resource_key` (and `false`) or creates a queued one.
    pub fn get_or_create(
        &self,
        resource_key: &str,
        label: &str,
        request: JobRequest,
        kind: &str,
    ) -> Result<(Job, bool)> {
        {
            let inner = self.inner();
            if let Some(job) = inner
                .by_resource
                .get(resource_key)
                .and_then(|id| inner.jobs.get(id))
            {
                return Ok((job.clone(), false));
            }
        }
        let now = now_ms();
        let job = Job {
            id: uuid::Uuid::new_v4().to_string(),
            kind: kind.to_string(),
            resource_key: resource_key.to_string(),
            label: label.to_string(),
            state: JobState::Queued,
            progress: None,
            request,
            created_at: now,
            updated_at: now,
        };
        self.persist(&job)?;
        {
            let mut inner = self.inner();
            inner.jobs.insert(job.id.clone(), job.clone());
            inner
                .by_resource
                .insert(resource_key.to_string(), job.id.clone());
        }
        self.prune_history()?;
        Ok((job, true))
    }

    pub fn get(&self, id: &str) -> Option<Job> {
        self.inner().jobs.get(id).cloned()
    }

    pub fn get_by_resource(&self, resource_key: &str) -> Option<Job> {
        let inner = self.inner();
        inner
            .by_resource
            .get(resource_key)
            .and_then(|id| inner.jobs.get(id))
            .cloned()
    }

    pub fn list(&self, kind: Option<&str>) -> Vec<Job> {
        self.inner()
            .jobs
            .values()
            .filter(|j| kind.is_none_or(|k| j.kind == k))
            .cloned()
            .collect()
    }

    pub fn update(
        &self,
        id: &str,
        state: JobState,
        progress: Option<Value>,
    ) -> Result<Option<Job>> {
        let job = {
            let mut inner = self.inner();
            let Some(job) = inner.jobs.get_mut(id) else {
                return Ok(None);
            };
            job.state = state;
            if progress.is_some() {
                job.progress = progress;
            }
            job.updated_at = now_ms();
            let job = job.clone();
            if state.is_terminal() {
                inner.by_resource.remove(&job.resource_key);
            }
            inner.notify(&job);
            job
        };
        self.persist(&job)?;
        if state.is_terminal() {
            self.prune_history()?;
        }
        Ok(Some(job))
    }

    /// Re-queue an errored job.
    pub fn retry(&self, id: &str) -> Result<Option<Job>> {
        let job = {
            let mut inner = self.inner();
            let Some(job) = inner
                .jobs
                .get_mut(id)
                .filter(|j| j.state == JobState::Error)
            else {
                return Ok(None);
            };
            job.state = JobState::Queued;
            job.progress = None;
            job.updated_at = now_ms();
            let job = job.clone();
            inner
                .by_resource
                .insert(job.resource_key.clone(), job.id.clone());
            inner.notify(&job);
            job
        };
        self.persist(&job)?;
        Ok(Some(job))
    }

    /// The job as it is now plus a receiver for its later states; the channel closes once the job
    /// is done, failed or removed. `None` for an unknown job.
    pub fn subscribe(&self, id: &str) -> Option<(Job, broadcast::Receiver<Job>)> {
        let mut inner = self.inner();
        let job = inner.jobs.get(id)?.clone();
        if job.state.is_terminal() {
            return Some((job, broadcast::channel(1).1));
        }
        let rx = inner
            .senders
            .entry(id.to_string())
            .or_insert_with(|| broadcast::channel(64).0)
            .subscribe();
        Some((job, rx))
    }

    pub fn remove(&self, id: &str) -> Result<bool> {
        let removed = {
            let mut inner = self.inner();
            let Some(job) = inner.jobs.remove(id) else {
                return Ok(false);
            };
            if inner
                .by_resource
                .get(&job.resource_key)
                .is_some_and(|j| j == id)
            {
                inner.by_resource.remove(&job.resource_key);
            }
            let mut cancelled = job.clone();
            cancelled.state = JobState::Error;
            cancelled.progress = Some(json!({"type": "error", "message": "Download cancelled"}));
            cancelled.updated_at = now_ms();
            inner.notify(&cancelled);
            inner.senders.remove(id);
            job
        };
        self.db
            .conn()
            .execute("DELETE FROM jobs WHERE id = ?1", [&removed.id])?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> JobRequest {
        JobRequest {
            comic_id: 7,
            ..Default::default()
        }
    }

    #[test]
    fn lifecycle() {
        let s = JobStore::open_in_memory().unwrap();
        let (a, created) = s.get_or_create("r1", "Batman", req(), "download").unwrap();
        assert!(created && a.state == JobState::Queued);
        let (b, created) = s.get_or_create("r1", "Batman", req(), "download").unwrap();
        assert!(!created && b.id == a.id);

        s.update(&a.id, JobState::Running, Some(json!({"type": "progress"})))
            .unwrap();
        assert_eq!(s.get_by_resource("r1").unwrap().state, JobState::Running);
        s.update(&a.id, JobState::Error, Some(json!({"type": "error"})))
            .unwrap();
        assert!(s.get_by_resource("r1").is_none());
        assert_eq!(s.retry(&a.id).unwrap().unwrap().state, JobState::Queued);
        assert!(s.get_by_resource("r1").is_some());
        assert_eq!(s.list(Some("identify-library")).len(), 0);
        assert!(s.remove(&a.id).unwrap() && s.get(&a.id).is_none());
    }

    #[test]
    fn interrupted_jobs_become_errors_on_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.sqlite");
        let id = {
            let s = JobStore::open(&path).unwrap();
            let (j, _) = s.get_or_create("r", "x", req(), "download").unwrap();
            s.update(&j.id, JobState::Running, None).unwrap();
            j.id
        };
        let s = JobStore::open(&path).unwrap();
        let j = s.get(&id).unwrap();
        assert_eq!(j.state, JobState::Error);
        assert_eq!(
            j.progress.unwrap()["message"],
            "Interrupted by server restart"
        );
        assert!(s.get_by_resource("r").is_none());
    }

    #[test]
    fn migrates_legacy_download_jobs_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.sqlite");
        let now = now_ms();
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch(&format!(
                "CREATE TABLE download_jobs (id TEXT PRIMARY KEY, resource_key TEXT NOT NULL, label TEXT NOT NULL, state TEXT NOT NULL,
                    progress TEXT, comic_id INTEGER NOT NULL, output_dir TEXT, uuid TEXT, strat TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
                 INSERT INTO download_jobs VALUES ('a', 'k', 'L', 'done', NULL, 1, NULL, NULL, NULL, {now}, {now});"
            ))
            .unwrap();
        let s = JobStore::open(&path).unwrap();
        let j = s.get("a").unwrap();
        assert_eq!((j.kind.as_str(), j.state), ("download", JobState::Done));
    }
}
