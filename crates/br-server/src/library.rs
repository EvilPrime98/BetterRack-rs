//! `/api/library*`: Every failure is a
//! 500 `{ error: true, message }` with the controller's own wording (identify and re-identify pass
//! the model's message through, a rejected move is a 400).

use crate::state::AppState;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use br_core::jobs::{JobRequest, JobState};
use br_core::library::{LibraryEntry, parse_page_option};
use br_core::{CoreError, Result as CoreResult};
use serde_json::{Map, Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

const IDENTIFY_LIBRARY_RESOURCE: &str = "identify-library";
const IDENTIFY_LIBRARY_KIND: &str = "identify-library";

type Params = Query<Vec<(String, String)>>;

fn reply(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn fail(status: StatusCode, message: &str) -> Response {
    reply(status, json!({ "error": true, "message": message }))
}

fn server_error(message: &str) -> Response {
    fail(StatusCode::INTERNAL_SERVER_ERROR, message)
}

fn ok(message: &str) -> Response {
    reply(
        StatusCode::OK,
        json!({ "error": false, "message": message }),
    )
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// The first value of query parameter `name`.
fn query<'a>(params: &'a [(String, String)], name: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// `await c.req.json()` then destructuring: bad JSON and `null` throw, other non-objects have no
/// fields.
fn body_object(body: &Bytes) -> Option<Map<String, Value>> {
    match serde_json::from_slice::<Value>(body).ok()? {
        Value::Object(m) => Some(m),
        Value::Null => None,
        _ => Some(Map::new()),
    }
}

fn str_field<'a>(m: &'a Map<String, Value>, key: &str) -> &'a str {
    m.get(key).and_then(Value::as_str).unwrap_or("")
}

/// JS truthiness of a JSON value.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// Run model work on a blocking thread once the library scan has settled.
async fn model<T: Send + 'static>(
    s: AppState,
    f: impl FnOnce(&AppState) -> T + Send + 'static,
) -> Option<T> {
    tokio::task::spawn_blocking(move || {
        s.library.wait_ready();
        f(&s)
    })
    .await
    .map_err(|e| tracing::error!(err = %e, "library task failed"))
    .ok()
}

/// Like `model` for work that must not wait for the scan (it may itself change the library).
async fn model_now<T: Send + 'static>(
    s: AppState,
    f: impl FnOnce(&AppState) -> T + Send + 'static,
) -> Option<T> {
    tokio::task::spawn_blocking(move || f(&s))
        .await
        .map_err(|e| tracing::error!(err = %e, "library task failed"))
        .ok()
}

fn paging(params: &[(String, String)]) -> (Option<i64>, Option<i64>) {
    (
        parse_page_option(query(params, "limit")),
        parse_page_option(query(params, "offset")),
    )
}

pub async fn index(State(s): State<AppState>) -> Response {
    match model(s, |s| s.library.index()).await {
        Some(v) => reply(StatusCode::OK, v),
        None => server_error("Internal Server Error"),
    }
}

pub async fn list(State(s): State<AppState>, Query(params): Params) -> Response {
    let (limit, offset) = paging(&params);
    match model(s, move |s| s.library.page(limit, offset)).await {
        Some(v) => reply(StatusCode::OK, v),
        None => server_error("Internal Server Error"),
    }
}

pub async fn by_series(State(s): State<AppState>, Query(params): Params) -> Response {
    let (limit, offset) = paging(&params);
    match model(s, move |s| s.library.page_by_series(limit, offset)).await {
        Some(v) => reply(StatusCode::OK, v),
        None => server_error("Internal Server Error"),
    }
}

pub async fn recent(State(s): State<AppState>, Query(params): Params) -> Response {
    let window = parse_page_option(query(&params, "windowHours"));
    match model(s, move |s| s.library.recent(window, now_ms())).await {
        Some(v) => reply(StatusCode::OK, v),
        None => server_error("Internal Server Error"),
    }
}

pub async fn reading(State(s): State<AppState>) -> Response {
    match model(s, |s| s.library.reading(now_ms())).await {
        Some(Ok(v)) => reply(StatusCode::OK, v),
        _ => server_error("Internal Server Error"),
    }
}

pub async fn get_preferences(State(s): State<AppState>, Path(uid): Path<String>) -> Response {
    match model_now(s, move |s| s.library.get_preferences(&uid)).await {
        Some(Ok(Some(pref))) => reply(
            StatusCode::OK,
            serde_json::to_value(pref).unwrap_or(Value::Null),
        ),
        Some(Ok(None)) => fail(StatusCode::NOT_FOUND, "No preferences found for this uid."),
        _ => server_error("Internal Server Error"),
    }
}

pub async fn update_preferences(
    State(s): State<AppState>,
    Path(uid): Path<String>,
    body: Bytes,
) -> Response {
    const MESSAGE: &str = "There was an issue updating preferences. Please, try again later.";
    let result = model_now(s, move |s| -> CoreResult<Option<()>> {
        if s.library.get(&uid).is_none() {
            return Ok(None);
        }
        let Some(fields) = body_object(&body) else {
            return Err(CoreError::Invalid("invalid body".into()));
        };
        let text = |k: &str| fields.get(k).and_then(Value::as_str).map(str::to_string);
        s.library.update_preferences(
            &uid,
            text("prefPublisher"),
            fields.get("recursive").and_then(Value::as_bool),
            text("prefCover"),
        )?;
        Ok(Some(()))
    })
    .await;
    match result {
        Some(Ok(Some(()))) => ok("Preferences updated successfully."),
        Some(Ok(None)) => fail(StatusCode::NOT_FOUND, "Entry not found for this uid."),
        Some(Err(e)) => {
            tracing::error!(err = %e, "failed to update preferences");
            server_error(MESSAGE)
        }
        None => server_error(MESSAGE),
    }
}

pub async fn refresh(State(s): State<AppState>, Query(params): Params) -> Response {
    const MESSAGE: &str = "There was an issue re-scanning the library. Please, try again later.";
    let (limit, offset) = paging(&params);
    let result = model_now(s, move |s| -> CoreResult<Value> {
        s.library.rescan()?;
        Ok(s.library.page(limit, offset))
    })
    .await;
    match result {
        Some(Ok(page)) => reply(
            StatusCode::OK,
            json!({ "error": false, "message": "Library re-scan completed.", "page": page }),
        ),
        Some(Err(e)) => {
            tracing::error!(err = %e, "failed to re-scan library");
            server_error(MESSAGE)
        }
        None => server_error(MESSAGE),
    }
}

/// Shared tail of the mutating endpoints: run `work`, answer `success` or the 500 `failure`.
async fn mutate(
    s: AppState,
    body: Bytes,
    label: &'static str,
    success: &'static str,
    failure: &'static str,
    work: impl FnOnce(&AppState, Map<String, Value>) -> CoreResult<()> + Send + 'static,
) -> Response {
    let result = model_now(s, move |s| {
        let fields = body_object(&body).ok_or_else(|| CoreError::Invalid("invalid body".into()))?;
        work(s, fields)
    })
    .await;
    match result {
        Some(Ok(())) => ok(success),
        Some(Err(CoreError::Move(message))) => fail(StatusCode::BAD_REQUEST, &message),
        Some(Err(e)) => {
            tracing::error!(err = %e, "failed to {label}");
            server_error(failure)
        }
        None => server_error(failure),
    }
}

pub async fn create_folder(State(s): State<AppState>, body: Bytes) -> Response {
    mutate(
        s,
        body,
        "create folder",
        "Folder created succesfully.",
        "There was an issue creating the folder. Please, try again later.",
        |s, f| {
            // `path.resolve(root, undefined)` throws, so a missing name is an error.
            let name = f
                .get("folderName")
                .and_then(Value::as_str)
                .ok_or_else(|| CoreError::Invalid("folderName is required".into()))?;
            s.library
                .create_folder(name, f.get("parentFolderUid").and_then(Value::as_str))
        },
    )
    .await
}

pub async fn move_file(State(s): State<AppState>, body: Bytes) -> Response {
    mutate(
        s,
        body,
        "move file",
        "File moved successfully.",
        "There was an issue moving the file. Please, try again later.",
        |s, f| {
            s.library
                .move_file(str_field(&f, "fileUid"), str_field(&f, "targetFolderUid"))
        },
    )
    .await
}

pub async fn delete_folder(State(s): State<AppState>, body: Bytes) -> Response {
    mutate(
        s,
        body,
        "delete folder",
        "Folder deleted successfully.",
        "There was an issue deleting the folder. Please, try again later.",
        |s, f| s.library.delete_folder(str_field(&f, "folderUid")),
    )
    .await
}

pub async fn delete_file(State(s): State<AppState>, body: Bytes) -> Response {
    mutate(
        s,
        body,
        "delete file",
        "File deleted successfully.",
        "There was an issue deleting the file. Please, try again later.",
        |s, f| s.library.delete_file(str_field(&f, "fileUid")),
    )
    .await
}

pub async fn unidentify_file(State(s): State<AppState>, body: Bytes) -> Response {
    mutate(
        s,
        body,
        "un-identify file",
        "File un-identified successfully.",
        "There was an issue un-identifying the file. Please, try again later.",
        |s, f| s.library.unidentify_file(str_field(&f, "fileUid")),
    )
    .await
}

pub async fn commit_identify(State(s): State<AppState>, body: Bytes) -> Response {
    const MESSAGE: &str = "There was an issue identifying the file. Please, try again later.";
    let result = model_now(s, move |s| -> CoreResult<Option<()>> {
        let fields = body_object(&body).ok_or_else(|| CoreError::Invalid("invalid body".into()))?;
        if !truthy(fields.get("fileUid")) || !truthy(fields.get("comic")) {
            return Ok(None);
        }
        let uid = fields.get("fileUid").and_then(Value::as_str).unwrap_or("");
        s.library
            .commit_identify(uid, fields.get("comic").cloned().unwrap_or(Value::Null))?;
        Ok(Some(()))
    })
    .await;
    match result {
        Some(Ok(Some(()))) => ok("File identified successfully."),
        Some(Ok(None)) => fail(
            StatusCode::BAD_REQUEST,
            "A file uid and comic are required.",
        ),
        Some(Err(e)) => {
            tracing::error!(err = %e, "failed to identify file");
            server_error(MESSAGE)
        }
        None => server_error(MESSAGE),
    }
}

/// `{ identified, comic, metaSource }` with the `undefined` ones left out.
fn identification(entry: &LibraryEntry) -> Value {
    let mut m = Map::new();
    if let Some(v) = entry.identified {
        m.insert("identified".into(), v.into());
    }
    if let Some(v) = &entry.comic {
        m.insert("comic".into(), v.clone());
    }
    if let Some(v) = &entry.meta_source {
        m.insert("metaSource".into(), v.clone().into());
    }
    Value::Object(m)
}

async fn identify_with(s: AppState, uid: String, reset: bool, fallback: &'static str) -> Response {
    let result = model(s, move |s| {
        if reset {
            s.library.reidentify_file(&uid)
        } else {
            s.library.identify(&uid)
        }
    })
    .await;
    match result {
        Some(Ok(entry)) => reply(StatusCode::OK, identification(&entry)),
        Some(Err(e)) => {
            tracing::error!(err = %e, "{fallback}");
            server_error(&e.to_string())
        }
        None => server_error(fallback),
    }
}

pub async fn identify(State(s): State<AppState>, Path(uid): Path<String>) -> Response {
    identify_with(
        s,
        uid,
        false,
        "There was an issue identifying the comic. Please, try again later.",
    )
    .await
}

pub async fn reidentify_file(State(s): State<AppState>, Path(uid): Path<String>) -> Response {
    identify_with(
        s,
        uid,
        true,
        "There was an issue re-identifying the comic. Please, try again later.",
    )
    .await
}

pub async fn reidentify_all(State(s): State<AppState>) -> Response {
    const MESSAGE: &str =
        "There was an issue flagging the library for re-identification. Please, try again later.";
    match model_now(s, |s| s.library.reidentify_all()).await {
        Some(Ok(())) => ok("Library flagged for re-identification."),
        _ => server_error(MESSAGE),
    }
}

fn job_payload(job: &br_core::jobs::Job) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("jobId".into(), job.id.clone().into());
    m.insert("state".into(), job.state.as_str().into());
    if let Some(p) = &job.progress {
        m.insert("progress".into(), p.clone());
    }
    m
}

fn job_reply(status: StatusCode, job: &br_core::jobs::Job) -> Response {
    let mut m = Map::new();
    m.insert("error".into(), false.into());
    m.extend(job_payload(job));
    reply(status, Value::Object(m))
}

/// `runIdentifyLibrary`: identify every file on a blocking thread, mirroring progress into the job.
fn run_identify_library(s: AppState, job_id: String) {
    tokio::task::spawn_blocking(move || {
        let set = |state: JobState, progress: Option<Value>| {
            if let Err(e) = s.jobs.update(&job_id, state, progress) {
                tracing::error!(err = %e, "failed to update identify job");
            }
        };
        s.library.wait_ready();
        let outcome = s.library.identify_library(&|done, total| {
            set(
                JobState::Running,
                Some(json!({ "type": "identifying", "done": done, "total": total })),
            )
        });
        match outcome {
            Ok(()) => {
                let total = s
                    .jobs
                    .get(&job_id)
                    .and_then(|j| j.progress)
                    .filter(|p| p["type"] == "identifying")
                    .and_then(|p| p["total"].as_u64())
                    .unwrap_or(0);
                set(
                    JobState::Done,
                    Some(json!({ "type": "done", "total": total })),
                );
            }
            Err(e) => {
                tracing::error!(err = %e, "identify library job failed");
                set(
                    JobState::Error,
                    Some(json!({ "type": "error", "message": e.to_string() })),
                );
            }
        }
    });
}

pub async fn start_identify_library(State(s): State<AppState>) -> Response {
    let request = JobRequest {
        comic_id: 0,
        ..JobRequest::default()
    };
    let jobs = s.jobs.clone();
    let created = tokio::task::spawn_blocking(move || {
        jobs.get_or_create(
            IDENTIFY_LIBRARY_RESOURCE,
            "Identify library",
            request,
            IDENTIFY_LIBRARY_KIND,
        )
    })
    .await;
    match created {
        Ok(Ok((job, created))) => {
            // The job flips to `running` (with the 0/total progress) before it answers.
            let job = if created {
                let total = s.library.file_count();
                let started = s.jobs.update(
                    &job.id,
                    JobState::Running,
                    Some(json!({ "type": "identifying", "done": 0, "total": total })),
                );
                run_identify_library(s, job.id.clone());
                started.ok().flatten().unwrap_or(job)
            } else {
                job
            };
            job_reply(
                if created {
                    StatusCode::ACCEPTED
                } else {
                    StatusCode::OK
                },
                &job,
            )
        }
        other => {
            tracing::error!(?other, "failed to start identify library job");
            server_error("There was an issue identifying the library. Please, try again later.")
        }
    }
}

pub async fn identify_library_status(State(s): State<AppState>) -> Response {
    let latest = s
        .jobs
        .list(Some(IDENTIFY_LIBRARY_KIND))
        .into_iter()
        .max_by_key(|j| j.created_at);
    match latest {
        None => reply(StatusCode::OK, json!({ "error": false, "state": "idle" })),
        Some(job) => job_reply(StatusCode::OK, &job),
    }
}
