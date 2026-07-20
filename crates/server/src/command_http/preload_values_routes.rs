use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use light_application::{ActionEnvelope, ActionError, ActionErrorKind};
use light_wire::v2::preload_values::{
    ProgrammingPreloadValuesActionRequest, ProgrammingPreloadValuesErrorKind,
    ProgrammingPreloadValuesErrorResponse,
};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session};
use super::{programming_ports::ServerProgrammingPorts, routes::http_context};

const BODY_LIMIT: usize = 2 * 1024 * 1024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/users/{user_id}/programmer-preload-values/snapshot",
            get(get_snapshot),
        )
        .route(
            "/api/v2/users/{user_id}/programmer-preload-values/actions",
            post(apply_action),
        )
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn get_snapshot(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, PreloadValuesHttpError> {
    let session = authenticated_user(&state, &headers, &user_id)?;
    let context = http_context(&session, None);
    let ports = ServerProgrammingPorts::new(&state, &session, "http", false);
    let snapshot = state
        .programming
        .preload_values_snapshot(&context, &ports)
        .map_err(PreloadValuesHttpError::application)?;
    let response = super::preload_values_wire::snapshot(snapshot);
    Ok(json_with_etag(response.projection.revision, response))
}

async fn apply_action(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<ProgrammingPreloadValuesActionRequest>, JsonRejection>,
) -> Result<Response, PreloadValuesHttpError> {
    let session = authenticated_user(&state, &headers, &user_id)?;
    let Json(request) = request.map_err(PreloadValuesHttpError::json)?;
    let request_id = request.request_id.clone();
    let context =
        http_context(&session, Some(&request_id)).with_expected_revision(request.expected_revision);
    let command = light_application::ProgrammingPreloadValuesRequest {
        expected_capture_mode_revision: request.expected_capture_mode_revision,
        command: super::preload_values_wire::command(request.action),
    };
    let result = run_action(state, session, ActionEnvelope { context, command }).await?;
    let response = super::preload_values_wire::outcome(request_id, result);
    Ok(json_with_etag(response.revision, response))
}

async fn run_action(
    state: AppState,
    session: Session,
    action: ActionEnvelope<light_application::ProgrammingPreloadValuesRequest>,
) -> Result<light_application::ProgrammingPreloadValuesResult, PreloadValuesHttpError> {
    let activation = state.activation_lock.clone().lock_owned().await;
    tokio::task::spawn_blocking(move || {
        let ports = ServerProgrammingPorts::new(&state, &session, "http_preload_values", true);
        let result = state.programming.handle_preload_values(action, &ports);
        drop(activation);
        result
    })
    .await
    .map_err(PreloadValuesHttpError::blocking)?
    .map_err(PreloadValuesHttpError::application)
}

fn authenticated_user(
    state: &AppState,
    headers: &HeaderMap,
    path_user_id: &str,
) -> Result<Session, PreloadValuesHttpError> {
    let session =
        super::super::authenticate(state, headers).map_err(PreloadValuesHttpError::api)?;
    let user_id = Uuid::parse_str(path_user_id)
        .map_err(|_| PreloadValuesHttpError::invalid("user_id must be a UUID"))?;
    if session.user.id.0 != user_id {
        return Err(PreloadValuesHttpError::forbidden(
            "session is not authorized for this Programmer user",
        ));
    }
    Ok(session)
}

fn json_with_etag<T: serde::Serialize>(revision: u64, body: T) -> Response {
    let mut response = Json(body).into_response();
    response
        .headers_mut()
        .insert(header::ETAG, revision_etag(revision));
    response
}

fn revision_etag(revision: u64) -> HeaderValue {
    HeaderValue::from_str(&format!("\"{revision}\""))
        .expect("a numeric Preload values revision always forms a valid ETag")
}

struct PreloadValuesHttpError {
    status: StatusCode,
    body: ProgrammingPreloadValuesErrorResponse,
}

impl PreloadValuesHttpError {
    fn application(error: ActionError) -> Self {
        Self::new(
            application_status(error.kind),
            wire_error_kind(error.kind),
            error.message,
            error.current_revision,
            error.current_related_revision,
            error.retryable,
        )
    }

    fn api(error: ApiError) -> Self {
        let retryable = error.status == StatusCode::SERVICE_UNAVAILABLE;
        Self::new(
            error.status,
            status_error_kind(error.status),
            error.message,
            None,
            None,
            retryable,
        )
    }

    fn json(error: JsonRejection) -> Self {
        Self::invalid(error.body_text())
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            ProgrammingPreloadValuesErrorKind::Invalid,
            message,
            None,
            None,
            false,
        )
    }

    fn forbidden(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            ProgrammingPreloadValuesErrorKind::Forbidden,
            message,
            None,
            None,
            false,
        )
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ProgrammingPreloadValuesErrorKind::Internal,
            format!("Preload values service task failed: {error}"),
            None,
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: ProgrammingPreloadValuesErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        current_capture_mode_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: ProgrammingPreloadValuesErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                current_capture_mode_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for PreloadValuesHttpError {
    fn into_response(self) -> Response {
        let revision = self.body.current_revision;
        let mut response = (self.status, Json(self.body)).into_response();
        if let Some(revision) = revision {
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

const fn wire_error_kind(kind: ActionErrorKind) -> ProgrammingPreloadValuesErrorKind {
    match kind {
        ActionErrorKind::Invalid => ProgrammingPreloadValuesErrorKind::Invalid,
        ActionErrorKind::Unauthorized => ProgrammingPreloadValuesErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => ProgrammingPreloadValuesErrorKind::Forbidden,
        ActionErrorKind::NotFound => ProgrammingPreloadValuesErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => {
            ProgrammingPreloadValuesErrorKind::Conflict
        }
        ActionErrorKind::Unavailable => ProgrammingPreloadValuesErrorKind::Unavailable,
        ActionErrorKind::Internal => ProgrammingPreloadValuesErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> ProgrammingPreloadValuesErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => ProgrammingPreloadValuesErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => ProgrammingPreloadValuesErrorKind::Forbidden,
        StatusCode::NOT_FOUND => ProgrammingPreloadValuesErrorKind::NotFound,
        StatusCode::CONFLICT => ProgrammingPreloadValuesErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => ProgrammingPreloadValuesErrorKind::Unavailable,
        status if status.is_server_error() => ProgrammingPreloadValuesErrorKind::Internal,
        _ => ProgrammingPreloadValuesErrorKind::Invalid,
    }
}
