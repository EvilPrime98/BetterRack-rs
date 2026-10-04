//! Phase B endpoints: settings, comic-data, directories.

use crate::error::ApiError;
use crate::state::{AppState, blocking};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use br_core::directories::directories_under;
use br_core::settings::AppSettings;
use serde_json::{Map, Value, json};

/// A bad JSON body answers 500.
fn json_object(body: &Bytes) -> Result<Map<String, Value>, ApiError> {
    match serde_json::from_slice::<Value>(body) {
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => Ok(Map::new()),
        Err(e) => Err(ApiError::Internal(format!("invalid JSON body: {e}"))),
    }
}

pub async fn get_settings(State(s): State<AppState>) -> Result<Json<AppSettings>, ApiError> {
    Ok(Json(blocking(move || s.prefs.get_app_settings()).await?))
}

/// `outputDirs` is stripped: library folders change only through `/library-folder`.
pub async fn update_settings(
    State(s): State<AppState>,
    body: Bytes,
) -> Result<Json<AppSettings>, ApiError> {
    let mut partial = json_object(&body)?;
    partial.shift_remove("outputDirs");
    Ok(Json(
        blocking(move || s.prefs.update_app_settings(&partial)).await?,
    ))
}

fn folder_path(body: &Bytes) -> Result<String, ApiError> {
    let obj = json_object(body)?;
    match obj.get("path").and_then(Value::as_str) {
        Some(p) if !p.is_empty() => Ok(p.to_string()),
        _ => Err(ApiError::unprocessable("path is required")),
    }
}

/// Resolved library roots (`LibraryModel.libPaths`), in the form they are persisted.
fn library_paths(s: &AppState) -> br_core::Result<Vec<String>> {
    let cwd = s.config.data_dir.to_string_lossy().into_owned();
    Ok(s.prefs
        .get_app_settings()?
        .output_dirs
        .iter()
        .map(|p| br_core::uid::resolve_windows(p, &cwd))
        .collect())
}

fn set_library_paths(s: &AppState, paths: Vec<String>) -> br_core::Result<AppSettings> {
    s.prefs
        .update_app_settings(json!({ "outputDirs": paths }).as_object().unwrap())
}

pub async fn add_library_folder(
    State(s): State<AppState>,
    body: Bytes,
) -> Result<Json<AppSettings>, ApiError> {
    let path = folder_path(&body)?;
    let scan_state = s.clone();
    let result = blocking(move || {
        let cwd = s.config.data_dir.to_string_lossy().into_owned();
        let resolved = br_core::uid::resolve_windows(&path, &cwd);
        let mut paths = library_paths(&s)?;
        if !paths.contains(&resolved) {
            if !std::path::Path::new(&resolved).is_dir() {
                return Ok(Err("Folder does not exist.".to_string()));
            }
            paths.push(resolved);
            set_library_paths(&s, paths)?;
        }
        Ok(Ok(s.prefs.get_app_settings()?))
    })
    .await?;
    if result.is_ok() {
        scan_state.spawn_rescan();
    }
    result.map(Json).map_err(ApiError::bad_request)
}

pub async fn remove_library_folder(
    State(s): State<AppState>,
    body: Bytes,
) -> Result<Json<AppSettings>, ApiError> {
    let path = folder_path(&body)?;
    let scan_state = s.clone();
    let settings = blocking(move || {
        let cwd = s.config.data_dir.to_string_lossy().into_owned();
        let resolved = br_core::uid::resolve_windows(&path, &cwd);
        let paths = library_paths(&s)?
            .into_iter()
            .filter(|p| *p != resolved)
            .collect();
        set_library_paths(&s, paths)
    })
    .await?;
    scan_state.spawn_rescan();
    Ok(Json(settings))
}

pub async fn get_comic_data(
    State(s): State<AppState>,
) -> Result<Json<Map<String, Value>>, ApiError> {
    Ok(Json(blocking(move || s.comic_data.get_all()).await?))
}

pub async fn patch_comic_data(
    State(s): State<AppState>,
    Path(uid): Path<String>,
    body: Bytes,
) -> Result<Json<Map<String, Value>>, ApiError> {
    if uid.is_empty() {
        return Err(ApiError::bad_request("A valid uid is required."));
    }
    let partial = json_object(&body)?;
    Ok(Json(
        blocking(move || s.comic_data.upsert(&uid, &partial)).await?,
    ))
}

pub async fn directories(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    let dirs = blocking(move || {
        let settings = s.prefs.get_app_settings()?;
        let roots: Vec<String> = Some(settings.download_dir)
            .filter(|d| !d.is_empty())
            .into_iter()
            .chain(settings.output_dirs)
            .collect();
        Ok(directories_under(
            &roots,
            &s.config.data_dir.to_string_lossy(),
        ))
    })
    .await?;
    Ok(Json(json!({ "directories": dirs })))
}
