//! The single error type of the server. Non-2xx answers are `{ "error": true, "message": "..." }`,
//! except the two plain-text shapes (401 and 500).

use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use br_core::CoreError;

#[derive(Debug)]
pub enum ApiError {
    /// `{ error: true, message }` with the given status.
    Json(StatusCode, String),
    /// Default for an uncaught exception (bad JSON body, DB failure, ...).
    Internal(String),
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::Json(StatusCode::BAD_REQUEST, msg.into())
    }

    pub fn unprocessable(msg: impl Into<String>) -> Self {
        Self::Json(StatusCode::UNPROCESSABLE_ENTITY, msg.into())
    }
}

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::Json(status, message) => (
                status,
                Json(serde_json::json!({ "error": true, "message": message })),
            )
                .into_response(),
            Self::Internal(detail) => {
                tracing::error!(%detail, "request failed");
                text(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
            }
        }
    }
}

/// Plain-text reply.
pub fn text(status: StatusCode, body: &'static str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=UTF-8")],
        body,
    )
        .into_response()
}
