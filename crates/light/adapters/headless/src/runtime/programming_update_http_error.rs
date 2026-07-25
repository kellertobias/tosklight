use super::ApiError;
use axum::{
    Json,
    extract::rejection::JsonRejection,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use light_application::{ActionError, ActionErrorKind};
use light_wire::v2::programming_update::{
    ProgrammingUpdateErrorKind, ProgrammingUpdateErrorResponse,
};

pub(super) struct ProgrammingUpdateHttpError {
    status: StatusCode,
    body: ProgrammingUpdateErrorResponse,
}

impl ProgrammingUpdateHttpError {
    pub(super) fn application(error: ActionError) -> Self {
        Self::new(
            application_status(error.kind),
            wire_error_kind(error.kind),
            error.message,
            error.current_revision,
            error.current_related_revision,
            error.retryable,
        )
    }

    pub(super) fn api(error: ApiError) -> Self {
        Self::new(
            error.status,
            status_error_kind(error.status),
            error.message,
            None,
            None,
            error.status == StatusCode::SERVICE_UNAVAILABLE,
        )
    }

    pub(super) fn json(error: JsonRejection) -> Self {
        Self::invalid(error.body_text())
    }

    pub(super) fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            ProgrammingUpdateErrorKind::Invalid,
            message,
            None,
            None,
            false,
        )
    }

    pub(super) fn forbidden(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            ProgrammingUpdateErrorKind::Forbidden,
            message,
            None,
            None,
            false,
        )
    }

    pub(super) fn conflict(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            ProgrammingUpdateErrorKind::Conflict,
            message,
            None,
            None,
            false,
        )
    }

    pub(super) fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ProgrammingUpdateErrorKind::Internal,
            format!("Programming Update service task failed: {error}"),
            None,
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: ProgrammingUpdateErrorKind,
        error: impl Into<String>,
        current_object_revision: Option<u64>,
        current_show_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: ProgrammingUpdateErrorResponse {
                kind,
                error: error.into(),
                current_object_revision,
                current_show_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for ProgrammingUpdateHttpError {
    fn into_response(self) -> Response {
        let show_revision = self.body.current_show_revision;
        let mut response = (self.status, Json(self.body)).into_response();
        if let Some(revision) = show_revision {
            response
                .headers_mut()
                .insert(header::ETAG, revision_etag(revision));
        }
        response
    }
}

const fn application_status(kind: ActionErrorKind) -> StatusCode {
    match kind {
        ActionErrorKind::Invalid => StatusCode::BAD_REQUEST,
        ActionErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
        ActionErrorKind::Forbidden => StatusCode::FORBIDDEN,
        ActionErrorKind::NotFound => StatusCode::NOT_FOUND,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => StatusCode::CONFLICT,
        ActionErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ActionErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

const fn wire_error_kind(kind: ActionErrorKind) -> ProgrammingUpdateErrorKind {
    match kind {
        ActionErrorKind::Invalid => ProgrammingUpdateErrorKind::Invalid,
        ActionErrorKind::Unauthorized => ProgrammingUpdateErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => ProgrammingUpdateErrorKind::Forbidden,
        ActionErrorKind::NotFound => ProgrammingUpdateErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => ProgrammingUpdateErrorKind::Conflict,
        ActionErrorKind::Unavailable => ProgrammingUpdateErrorKind::Unavailable,
        ActionErrorKind::Internal => ProgrammingUpdateErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> ProgrammingUpdateErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => ProgrammingUpdateErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => ProgrammingUpdateErrorKind::Forbidden,
        StatusCode::NOT_FOUND => ProgrammingUpdateErrorKind::NotFound,
        StatusCode::CONFLICT => ProgrammingUpdateErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => ProgrammingUpdateErrorKind::Unavailable,
        status if status.is_server_error() => ProgrammingUpdateErrorKind::Internal,
        _ => ProgrammingUpdateErrorKind::Invalid,
    }
}

fn revision_etag(revision: u64) -> HeaderValue {
    HeaderValue::from_str(&format!("\"{revision}\""))
        .expect("a numeric Show revision always forms a valid ETag")
}
