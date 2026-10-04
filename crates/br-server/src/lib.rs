//! Axum app exposing `br-core`.

pub mod error;
pub mod http_cache;
pub mod library;
pub mod middleware;
pub mod reader;
pub mod routes;
pub mod state;
pub mod store;
pub mod thumbnail;
pub mod wiki;

use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router, middleware::from_fn_with_state};
use state::AppState;
use tower_http::cors::CorsLayer;

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "app": "betterrack", "version": env!("CARGO_PKG_VERSION") }))
}

async fn not_found() -> Response {
    error::text(StatusCode::NOT_FOUND, "Not Found")
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route(
            "/api/settings",
            get(routes::get_settings).put(routes::update_settings),
        )
        .route(
            "/api/settings/library-folder",
            axum::routing::post(routes::add_library_folder).delete(routes::remove_library_folder),
        )
        .route("/api/comic-data", get(routes::get_comic_data))
        .route("/api/comic-data/{uid}", patch(routes::patch_comic_data))
        .route("/api/directories", get(routes::directories))
        .route("/api/library", get(library::list))
        .route("/api/library/", get(library::list))
        .route("/api/library/by-series", get(library::by_series))
        .route("/api/library/index", get(library::index))
        .route("/api/library/recent", get(library::recent))
        .route("/api/library/reading", get(library::reading))
        .route(
            "/api/library/preferences/{uid}",
            get(library::get_preferences)
                .patch(library::update_preferences)
                .put(library::update_preferences)
                .post(library::update_preferences),
        )
        .route("/api/library/refresh", get(library::refresh))
        .route(
            "/api/library/folder",
            post(library::create_folder).delete(library::delete_folder),
        )
        .route("/api/library/file", delete(library::delete_file))
        .route("/api/library/file/move", post(library::move_file))
        .route(
            "/api/library/file/unidentify",
            post(library::unidentify_file),
        )
        .route("/api/library/file/identify", post(library::commit_identify))
        .route(
            "/api/library/identify/reset-all",
            post(library::reidentify_all),
        )
        .route(
            "/api/library/identify/all",
            post(library::start_identify_library).get(library::identify_library_status),
        )
        .route("/api/library/{uid}/identify", get(library::identify))
        .route(
            "/api/library/{uid}/identify/reset",
            post(library::reidentify_file),
        )
        .route("/api/wiki/comic", get(wiki::comic))
        .route("/api/wiki/comic/{id}", get(wiki::comic_by_id))
        .route("/api/wiki/comics", get(wiki::comics))
        .route(
            "/api/comics",
            get(store::get_comics).route_layer(from_fn_with_state(
                state.clone(),
                middleware::require_store_api_url,
            )),
        )
        .route(
            "/api/comics/",
            get(store::get_comics).route_layer(from_fn_with_state(
                state.clone(),
                middleware::require_store_api_url,
            )),
        )
        .route(
            "/api/comics/{id}/links",
            get(store::get_links).route_layer(from_fn_with_state(
                state.clone(),
                middleware::require_store_api_url,
            )),
        )
        .route(
            "/api/downloads",
            get(store::download_by_query)
                .post(store::download_by_body)
                .route_layer(from_fn_with_state(
                    state.clone(),
                    middleware::require_store_api_url,
                )),
        )
        .route("/api/downloads/jobs", get(store::list_jobs))
        .route("/api/downloads/resource/{id}", get(store::job_by_resource))
        .route(
            "/api/downloads/{job_id}/retry",
            post(store::retry_job).route_layer(from_fn_with_state(
                state.clone(),
                middleware::require_store_api_url,
            )),
        )
        .route(
            "/api/downloads/{job_id}",
            get(store::job_status).delete(store::cancel_job),
        )
        .route("/api/downloads/{job_id}/stream", get(store::stream_job))
        .route("/api/thumbnail/{uuid}", get(thumbnail::get))
        .route("/api/thumbnail/{uuid}/retry", post(thumbnail::retry))
        .route("/read/{uuid}", get(reader::get))
        .route("/read/{uuid}/refresh", get(reader::refresh))
        .route("/read/{uuid}/bookmarks", get(reader::bookmarks))
        .route("/read/{uuid}/pages/{page}", get(reader::page))
        .fallback(not_found)
        .layer(from_fn_with_state(state.clone(), middleware::api_key_auth))
        // Outermost, registered first so preflights never reach the key check.
        .layer(CorsLayer::permissive())
        .with_state(state)
}
