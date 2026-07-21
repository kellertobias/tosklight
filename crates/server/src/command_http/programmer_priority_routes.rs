use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use light_application::{ActionEnvelope, ActionError, ActionErrorKind};
use light_wire::v2::programmer_priority::{
    ProgrammerPriorityActionRequest, ProgrammerPriorityErrorKind, ProgrammerPriorityErrorResponse,
};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session};
use super::{programming_ports::ServerProgrammingPorts, routes::http_context};

const BODY_LIMIT: usize = 8 * 1024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/users/{user_id}/programmer-priority/snapshot",
            get(get_priority),
        )
        .route(
            "/api/v2/users/{user_id}/programmer-priority/actions",
            post(apply_action),
        )
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn get_priority(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, PriorityHttpError> {
    let session = authenticated_user(&state, &headers, &user_id, false)?;
    let context = http_context(&session, None);
    let ports = ServerProgrammingPorts::new(&state, &session, "http_priority_snapshot", false);
    let snapshot = state
        .programming
        .priority_snapshot(&context, &ports)
        .map_err(PriorityHttpError::application)?;
    let response = super::programmer_priority_wire::snapshot(snapshot);
    Ok(json_with_etag(response.projection.revision, response))
}

async fn apply_action(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<ProgrammerPriorityActionRequest>, JsonRejection>,
) -> Result<Response, PriorityHttpError> {
    let session = authenticated_user(&state, &headers, &user_id, true)?;
    let Json(request) = request.map_err(PriorityHttpError::json)?;
    super::routes::validate_request_id(&request.request_id).map_err(PriorityHttpError::api)?;
    let command = light_application::ProgrammingPriorityRequest {
        expected_revision: light_application::ProgrammingPriorityRevisionExpectation::Exact(
            request.expected_revision,
        ),
        priority: request.priority,
    };
    let context = http_context(&session, Some(&request.request_id));
    let result = run_action(state, session, ActionEnvelope { context, command }).await?;
    let response = super::programmer_priority_wire::outcome(result);
    Ok(json_with_etag(response.projection.revision, response))
}

async fn run_action(
    state: AppState,
    session: Session,
    action: ActionEnvelope<light_application::ProgrammingPriorityRequest>,
) -> Result<light_application::ProgrammingPriorityResult, PriorityHttpError> {
    tokio::task::spawn_blocking(move || {
        let ports = ServerProgrammingPorts::new(&state, &session, "http_priority", true);
        state.programming.handle_priority(action, &ports)
    })
    .await
    .map_err(PriorityHttpError::blocking)?
    .map_err(PriorityHttpError::application)
}

fn authenticated_user(
    state: &AppState,
    headers: &HeaderMap,
    path_user_id: &str,
    require_unlocked: bool,
) -> Result<Session, PriorityHttpError> {
    let session = super::super::authenticate(state, headers).map_err(PriorityHttpError::api)?;
    let user_id = Uuid::parse_str(path_user_id)
        .map_err(|_| PriorityHttpError::invalid("user_id must be a UUID"))?;
    if session.user.id.0 != user_id {
        return Err(PriorityHttpError::forbidden(
            "session is not authorized for this Programmer user",
        ));
    }
    if require_unlocked && super::super::read_desk_lock(state, session.desk.id).locked {
        return Err(PriorityHttpError::conflict("desk is locked"));
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
        .expect("a numeric Programmer priority revision always forms a valid ETag")
}

struct PriorityHttpError {
    status: StatusCode,
    body: ProgrammerPriorityErrorResponse,
}

impl PriorityHttpError {
    fn application(error: ActionError) -> Self {
        Self::new(
            application_status(error.kind),
            wire_error_kind(error.kind),
            error.message,
            error.current_revision,
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
            retryable,
        )
    }

    fn json(error: JsonRejection) -> Self {
        Self::invalid(error.body_text())
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            ProgrammerPriorityErrorKind::Invalid,
            message,
            None,
            false,
        )
    }

    fn forbidden(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            ProgrammerPriorityErrorKind::Forbidden,
            message,
            None,
            false,
        )
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            ProgrammerPriorityErrorKind::Conflict,
            message,
            None,
            false,
        )
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ProgrammerPriorityErrorKind::Internal,
            format!("Programmer priority service task failed: {error}"),
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: ProgrammerPriorityErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: ProgrammerPriorityErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for PriorityHttpError {
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

const fn wire_error_kind(kind: ActionErrorKind) -> ProgrammerPriorityErrorKind {
    match kind {
        ActionErrorKind::Invalid => ProgrammerPriorityErrorKind::Invalid,
        ActionErrorKind::Unauthorized => ProgrammerPriorityErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => ProgrammerPriorityErrorKind::Forbidden,
        ActionErrorKind::NotFound => ProgrammerPriorityErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => ProgrammerPriorityErrorKind::Conflict,
        ActionErrorKind::Unavailable => ProgrammerPriorityErrorKind::Unavailable,
        ActionErrorKind::Internal => ProgrammerPriorityErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> ProgrammerPriorityErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => ProgrammerPriorityErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => ProgrammerPriorityErrorKind::Forbidden,
        StatusCode::NOT_FOUND => ProgrammerPriorityErrorKind::NotFound,
        StatusCode::CONFLICT => ProgrammerPriorityErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => ProgrammerPriorityErrorKind::Unavailable,
        status if status.is_server_error() => ProgrammerPriorityErrorKind::Internal,
        _ => ProgrammerPriorityErrorKind::Invalid,
    }
}
