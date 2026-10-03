//! `/api/comics*` (`ComicsController`) and `/api/downloads*` (`DownloadController`).

use crate::error::ApiError;
use crate::state::AppState;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use br_core::downloads::{CancelOutcome, RetryOutcome, status_payload};
use br_core::jobs::{Job, JobRequest};
use br_core::store::{js_number, leading_int};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::convert::Infallible;

type Q = Query<HashMap<String, String>>;

/// `Number(q) || fallback`, as the forwarded page parameters are computed in the controller.
fn number_or(q: &HashMap<String, String>, key: &str, fallback: f64) -> String {
    let n = q.get(key).map(|v| v.trim()).filter(|v| !v.is_empty()).and_then(|v| v.parse::<f64>().ok()).filter(|n| n.is_finite() && *n != 0.0);
    js_number(n.unwrap_or(fallback))
}

fn store_err(e: br_core::CoreError) -> ApiError {
    ApiError::Internal(e.to_string())
}

pub async fn get_comics(State(s): State<AppState>, Query(q): Q) -> Result<axum::response::Response, ApiError> {
    if q.get("latest").is_some_and(|v| v == "true") {
        let posts = s.store.get_latest(&number_or(&q, "page", 1.0), &number_or(&q, "perPage", 10.0)).await.map_err(store_err)?;
        return Ok(Json(posts).into_response());
    }
    if q.get("weekly").is_some_and(|v| v == "true") {
        let group = q.get("group").map(String::as_str).filter(|g| !g.is_empty());
        return Ok(Json(s.store.get_weekly_list_posts(group).await.map_err(store_err)?).into_response());
    }
    let Some(search) = q.get("search").filter(|v| !v.is_empty()) else {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(json!([]))).into_response());
    };
    let per_page = number_or(&q, "perPage", 20.0);
    let posts = s.store.get_post_links(search, &number_or(&q, "page", 1.0), Some(&per_page)).await.map_err(store_err)?;
    if q.get("exact").is_some_and(|v| v == "true") {
        let needle = search.to_lowercase();
        let filtered: Vec<_> = posts.into_iter().filter(|p| p.title.to_lowercase().contains(&needle)).collect();
        return Ok(Json(filtered).into_response());
    }
    Ok(Json(posts).into_response())
}

pub async fn get_links(State(s): State<AppState>, Path(id): Path<String>, Query(q): Q) -> Result<axum::response::Response, ApiError> {
    let strat = q.get("strat").filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| "all".to_string());
    // `parseInt` of a non-number is NaN; the store answers 404 for `posts/NaN`.
    let links = match leading_int(&id) {
        Some(post_id) => s.store.get_download_links(post_id, &strat).await.map_err(store_err)?,
        None => vec![],
    };
    if links.is_empty() {
        return Ok((StatusCode::NOT_FOUND, Json(json!({ "error": true, "message": "No links found", "strat": strat, "links": [] }))).into_response());
    }
    Ok(Json(json!({ "error": false, "message": "OK", "strat": strat, "links": links })).into_response())
}

fn start_failed(e: impl std::fmt::Display) -> ApiError {
    tracing::error!(err = %e, "failed to start comic download");
    ApiError::Json(StatusCode::INTERNAL_SERVER_ERROR, "Failed to start comic download".into())
}

struct StartParams {
    id: Option<i64>,
    resource_key: String,
    title: String,
    output_dir: Option<String>,
    uuid: String,
    strat: Option<String>,
    csd: bool,
}

/// The part of `downloadComic` / `downloadComicSSE` after the parameters are read.
async fn start_download(s: &AppState, p: StartParams) -> Result<axum::response::Response, ApiError> {
    let link = match p.id {
        Some(id) => s.downloads.resolve_link(id, p.strat.as_deref(), Some(&p.uuid)).await.map_err(start_failed)?,
        None => None,
    };
    let (Some(link), Some(id)) = (link, p.id) else {
        return Ok(ApiError::Json(StatusCode::BAD_REQUEST, "This comic cannot be downloaded".into()).into_response());
    };
    if p.csd {
        return Ok(Json(json!({ "error": false, "message": "OK", "downloadLink": link })).into_response());
    }
    let request = JobRequest { comic_id: id, output_dir: p.output_dir, uuid: Some(p.uuid), strat: p.strat };
    let (job, created) = s.downloads.start(&p.resource_key, &p.title, request, link).map_err(start_failed)?;
    let status = if created { StatusCode::CREATED } else { StatusCode::OK };
    Ok((status, Json(json!({ "error": false, "jobId": job.id, "state": job.state.as_str() }))).into_response())
}

fn missing_params() -> axum::response::Response {
    ApiError::unprocessable("Missing id, title, or uuid").into_response()
}

/// `GET /api/downloads?id&title&uuid&outputDir&csd&strat` (the legacy start-by-query form).
pub async fn download_by_query(State(s): State<AppState>, Query(q): Q) -> Result<axum::response::Response, ApiError> {
    let get = |k: &str| q.get(k).filter(|v| !v.is_empty()).cloned();
    let (Some(id), Some(title), Some(uuid)) = (get("id"), get("title"), get("uuid")) else { return Ok(missing_params()) };
    start_download(
        &s,
        StartParams {
            id: leading_int(&id),
            resource_key: id,
            title,
            output_dir: q.get("outputDir").cloned(),
            uuid,
            strat: q.get("strat").cloned(),
            csd: q.get("csd").is_some_and(|v| v == "true"),
        },
    )
    .await
}

/// A JSON value Bun would use as is: a number, or a numeric string.
fn body_id(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().filter(|n| *n != 0),
        Value::String(t) => leading_int(t).filter(|n| *n != 0),
        _ => None,
    }
}

fn truthy_str(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

pub async fn download_by_body(State(s): State<AppState>, body: Bytes) -> Result<axum::response::Response, ApiError> {
    let body = serde_json::from_slice::<Value>(&body).ok().filter(Value::is_object).unwrap_or_else(|| json!({}));
    let (Some(id), Some(title), Some(uuid)) = (body_id(&body["id"]), truthy_str(&body["title"]), truthy_str(&body["uuid"])) else { return Ok(missing_params()) };
    start_download(
        &s,
        StartParams {
            id: Some(id),
            resource_key: id.to_string(),
            title,
            output_dir: body["outputDir"].as_str().map(str::to_string),
            uuid,
            strat: body["strat"].as_str().map(str::to_string),
            csd: match &body["csd"] {
                Value::Bool(b) => *b,
                Value::Null => false,
                Value::String(t) => !t.is_empty(),
                Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
                _ => true,
            },
        },
    )
    .await
}

fn not_found() -> axum::response::Response {
    ApiError::Json(StatusCode::NOT_FOUND, "Job not found".into()).into_response()
}

pub async fn retry_job(State(s): State<AppState>, Path(job_id): Path<String>) -> Result<axum::response::Response, ApiError> {
    let fail = |e: br_core::CoreError| {
        tracing::error!(err = %e, "failed to retry comic download");
        ApiError::Json(StatusCode::INTERNAL_SERVER_ERROR, "Failed to retry comic download".into())
    };
    Ok(match s.downloads.retry(&job_id).await.map_err(fail)? {
        RetryOutcome::NotFound => not_found(),
        RetryOutcome::NotFailed => ApiError::Json(StatusCode::CONFLICT, "Only failed jobs can be retried".into()).into_response(),
        RetryOutcome::NoLink => ApiError::Json(StatusCode::BAD_REQUEST, "This comic cannot be downloaded".into()).into_response(),
        RetryOutcome::Gone => ApiError::Json(StatusCode::CONFLICT, "Job is no longer retryable".into()).into_response(),
        RetryOutcome::Started(job) => Json(json!({ "error": false, "jobId": job.id, "state": job.state.as_str() })).into_response(),
    })
}

pub async fn cancel_job(State(s): State<AppState>, Path(job_id): Path<String>) -> Result<axum::response::Response, ApiError> {
    let outcome = s.downloads.cancel(&job_id).map_err(store_err)?;
    Ok(match outcome {
        CancelOutcome::NotFound => not_found(),
        CancelOutcome::NotActive => ApiError::Json(StatusCode::CONFLICT, "Only active downloads can be stopped".into()).into_response(),
        CancelOutcome::Extracting => ApiError::Json(StatusCode::CONFLICT, "The download is being extracted and cannot be stopped".into()).into_response(),
        CancelOutcome::Cancelled => Json(json!({ "error": false, "jobId": job_id })).into_response(),
    })
}

pub async fn list_jobs(State(s): State<AppState>) -> Json<Value> {
    let jobs: Vec<Value> = s.downloads.list().iter().map(status_payload).collect();
    Json(json!({ "error": false, "jobs": jobs }))
}

pub async fn job_by_resource(State(s): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    let job = s.jobs.get_by_resource(&id).map(|j| status_payload(&j));
    Json(json!({ "error": false, "job": job }))
}

pub async fn job_status(State(s): State<AppState>, Path(job_id): Path<String>) -> axum::response::Response {
    match s.jobs.get(&job_id) {
        None => not_found(),
        Some(job) => {
            // `{ error: false, ...payload }`: `error` first.
            let mut out = serde_json::Map::new();
            out.insert("error".into(), json!(false));
            if let Value::Object(m) = status_payload(&job) {
                out.extend(m);
            }
            Json(Value::Object(out)).into_response()
        }
    }
}

fn sse_event(job: &Job) -> Event {
    Event::default().data(status_payload(job).to_string())
}

/// `GET /api/downloads/:jobId/stream`: the job now, then every update until it finishes.
pub async fn stream_job(State(s): State<AppState>, Path(job_id): Path<String>) -> axum::response::Response {
    let Some((job, rx)) = s.jobs.subscribe(&job_id) else { return not_found() };
    let first = futures_util::stream::once(async move { Ok::<_, Infallible>(sse_event(&job)) });
    let updates = futures_util::stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(job) => Some((Ok::<_, Infallible>(sse_event(&job)), rx)),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => Some((Ok(Event::default().comment("lagged")), rx)),
            Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
        }
    });
    use futures_util::StreamExt;
    Sse::new(first.chain(updates)).keep_alive(KeepAlive::default()).into_response()
}
