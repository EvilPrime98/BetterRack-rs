//! `/api/wiki/*` (`WikiController`) and [`LiveWiki`], the `br-wiki` implementation of the
//! blocking [`WikiLookup`] seam that `Library::identify` also uses.

use crate::error::ApiError;
use crate::state::{AppState, blocking};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use br_core::CoreError;
use br_core::comic_info::js_number;
use br_core::wiki::WikiLookup;
use br_wiki::WikiService;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Handle;

type Q = Query<HashMap<String, String>>;

/// Runs the async [`WikiService`] to completion from the blocking threads the core works on.
/// The runtime handle is captured up front because identify-all calls the lookup from plain
/// `std::thread` workers that have no runtime context of their own.
pub struct LiveWiki {
    service: Arc<WikiService>,
    runtime: Handle,
}

impl LiveWiki {
    /// Must be called from inside the tokio runtime.
    pub fn new(service: WikiService) -> Self {
        Self::with_handle(service, Handle::current())
    }

    pub fn with_handle(service: WikiService, runtime: Handle) -> Self {
        Self {
            service: Arc::new(service),
            runtime,
        }
    }
}

fn wiki_err(e: br_wiki::WikiError) -> CoreError {
    CoreError::Wiki(e.to_string())
}

impl WikiLookup for LiveWiki {
    fn get_comic(
        &self,
        title: &str,
        thumbnail_size: Option<&str>,
    ) -> br_core::Result<Option<Value>> {
        self.runtime
            .block_on(self.service.get_comic(title, thumbnail_size))
            .map_err(wiki_err)
    }

    fn get_comics(&self, title: &str, thumbnail_size: Option<&str>) -> br_core::Result<Vec<Value>> {
        self.runtime
            .block_on(self.service.get_comics(title, thumbnail_size))
            .map_err(wiki_err)
    }

    fn get_comic_by_id(
        &self,
        page_id: i64,
        source_wiki: &str,
        thumbnail_size: Option<&str>,
    ) -> br_core::Result<Option<Value>> {
        self.runtime
            .block_on(
                self.service
                    .get_comic_by_id(page_id, source_wiki, thumbnail_size),
            )
            .map_err(wiki_err)
    }
}

fn title_param(q: &HashMap<String, String>) -> Result<String, ApiError> {
    q.get("title")
        .filter(|t| !t.is_empty())
        .cloned()
        .ok_or_else(|| ApiError::bad_request("A comic title is required."))
}

/// `thumbnailSize ? Number(thumbnailSize) : undefined`.
fn size_param(q: &HashMap<String, String>) -> Option<String> {
    br_wiki::thumbnail_size_param(q.get("thumbnailSize").map(String::as_str))
}

pub async fn comic(State(s): State<AppState>, Query(q): Q) -> Result<Response, ApiError> {
    let title = title_param(&q)?;
    let size = size_param(&q);
    let wiki = s.wiki.clone();
    let found = blocking(move || wiki.get_comic(&title, size.as_deref())).await?;
    Ok(Json(found).into_response())
}

pub async fn comics(State(s): State<AppState>, Query(q): Q) -> Result<Response, ApiError> {
    let title = title_param(&q)?;
    let size = size_param(&q);
    let wiki = s.wiki.clone();
    let found = blocking(move || wiki.get_comics(&title, size.as_deref())).await?;
    Ok(Json(found).into_response())
}

pub async fn comic_by_id(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Query(q): Q,
) -> Result<Response, ApiError> {
    let number = js_number(&id);
    if number == 0.0 || number.is_nan() {
        return Err(ApiError::bad_request("A valid page id is required."));
    }
    let Some(source) = q
        .get("sourceWiki")
        .filter(|w| br_wiki::is_known_wiki(w))
        .cloned()
    else {
        return Err(ApiError::bad_request("A valid sourceWiki is required."));
    };
    // A fractional, negative or huge id is not a page; the wiki would answer "missing".
    if number.fract() != 0.0 || number < 1.0 || number > 9_007_199_254_740_991.0 {
        return Ok(Json(Value::Null).into_response());
    }
    let size = size_param(&q);
    let wiki = s.wiki.clone();
    let found =
        blocking(move || wiki.get_comic_by_id(number as i64, &source, size.as_deref())).await?;
    Ok(Json(found).into_response())
}
