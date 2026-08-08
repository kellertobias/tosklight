//! What the API says when it cannot do something.
//!
//! Every failure carries a stable machine-readable code and a message safe to show an operator.
//! Absolute paths, decoder internals, and arbitrary exception text stay in the logs.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// The body every failure returns.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorBody {
    /// Stable across releases. Clients branch on this, never on the message.
    pub code: String,
    pub message: String,
}

/// A failure, with the status it should carry.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}: {}", body.code, body.message)]
pub struct ApiError {
    status: StatusCode,
    body: ApiErrorBody,
}

impl ApiError {
    pub fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            body: ApiErrorBody {
                code: code.into(),
                message: message.into(),
            },
        }
    }

    pub fn bad_request(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn not_found(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    pub const fn status(&self) -> StatusCode {
        self.status
    }

    pub const fn body(&self) -> &ApiErrorBody {
        &self.body
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, axum::Json(self.body)).into_response()
    }
}
