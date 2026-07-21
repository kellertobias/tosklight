use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{ActionEnvelope, ActionError, ActionErrorKind};
use light_wire::v2::preload_lifecycle::{
    ProgrammingPreloadLifecycleAction, ProgrammingPreloadLifecycleErrorKind,
    ProgrammingPreloadLifecycleErrorResponse, ProgrammingPreloadLifecycleRequest,
};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session};
use super::{programming_ports::ServerProgrammingPorts, routes::http_context};

const BODY_LIMIT: usize = 32 * 1024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/users/{user_id}/programmer-preload/actions",
            post(apply_action),
        )
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn apply_action(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<ProgrammingPreloadLifecycleRequest>, JsonRejection>,
) -> Result<Response, PreloadLifecycleHttpError> {
    let session = authenticated_user(&state, &headers, &user_id)?;
    let Json(request) = request.map_err(PreloadLifecycleHttpError::json)?;
    super::routes::validate_request_id(&request.request_id)
        .map_err(PreloadLifecycleHttpError::api)?;
    let needs_show_lock = matches!(request.action, ProgrammingPreloadLifecycleAction::Go { .. });
    let context = http_context(&session, Some(&request.request_id));
    let command = super::preload_lifecycle_wire::command(&request);
    let activation = if needs_show_lock {
        Some(state.activation_lock.clone().lock_owned().await)
    } else {
        None
    };
    let result = run_action(
        state,
        session,
        ActionEnvelope { context, command },
        activation,
    )
    .await?;
    Ok(Json(super::preload_lifecycle_wire::outcome(result)).into_response())
}

async fn run_action(
    state: AppState,
    session: Session,
    action: ActionEnvelope<light_application::ProgrammingPreloadLifecycleRequest>,
    activation: Option<tokio::sync::OwnedMutexGuard<()>>,
) -> Result<light_application::ProgrammingPreloadLifecycleResult, PreloadLifecycleHttpError> {
    tokio::task::spawn_blocking(move || {
        let ports = ServerProgrammingPorts::new(&state, &session, "http_preload_lifecycle", true);
        let result = state.programming.handle_preload_lifecycle(action, &ports);
        drop(activation);
        result
    })
    .await
    .map_err(PreloadLifecycleHttpError::blocking)?
    .map_err(PreloadLifecycleHttpError::application)
}

fn authenticated_user(
    state: &AppState,
    headers: &HeaderMap,
    path_user_id: &str,
) -> Result<Session, PreloadLifecycleHttpError> {
    let session =
        super::super::authenticate(state, headers).map_err(PreloadLifecycleHttpError::api)?;
    let user_id = Uuid::parse_str(path_user_id)
        .map_err(|_| PreloadLifecycleHttpError::invalid("user_id must be a UUID"))?;
    if session.user.id.0 != user_id {
        return Err(PreloadLifecycleHttpError::forbidden(
            "session is not authorized for this Programmer user",
        ));
    }
    if super::super::read_desk_lock(state, session.desk.id).locked {
        return Err(PreloadLifecycleHttpError::conflict("desk is locked"));
    }
    Ok(session)
}

struct PreloadLifecycleHttpError {
    status: StatusCode,
    body: ProgrammingPreloadLifecycleErrorResponse,
}

impl PreloadLifecycleHttpError {
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
        Self::new(
            error.status,
            status_error_kind(error.status),
            error.message,
            None,
            None,
            error.status == StatusCode::SERVICE_UNAVAILABLE,
        )
    }

    fn json(error: JsonRejection) -> Self {
        Self::invalid(error.body_text())
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            ProgrammingPreloadLifecycleErrorKind::Invalid,
            message,
            None,
            None,
            false,
        )
    }

    fn forbidden(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            ProgrammingPreloadLifecycleErrorKind::Forbidden,
            message,
            None,
            None,
            false,
        )
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            ProgrammingPreloadLifecycleErrorKind::Conflict,
            message,
            None,
            None,
            false,
        )
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ProgrammingPreloadLifecycleErrorKind::Internal,
            format!("Preload lifecycle service task failed: {error}"),
            None,
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: ProgrammingPreloadLifecycleErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        current_related_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: ProgrammingPreloadLifecycleErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                current_related_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for PreloadLifecycleHttpError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
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

const fn wire_error_kind(kind: ActionErrorKind) -> ProgrammingPreloadLifecycleErrorKind {
    match kind {
        ActionErrorKind::Invalid => ProgrammingPreloadLifecycleErrorKind::Invalid,
        ActionErrorKind::Unauthorized => ProgrammingPreloadLifecycleErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => ProgrammingPreloadLifecycleErrorKind::Forbidden,
        ActionErrorKind::NotFound => ProgrammingPreloadLifecycleErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => {
            ProgrammingPreloadLifecycleErrorKind::Conflict
        }
        ActionErrorKind::Unavailable => ProgrammingPreloadLifecycleErrorKind::Unavailable,
        ActionErrorKind::Internal => ProgrammingPreloadLifecycleErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> ProgrammingPreloadLifecycleErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => ProgrammingPreloadLifecycleErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => ProgrammingPreloadLifecycleErrorKind::Forbidden,
        StatusCode::NOT_FOUND => ProgrammingPreloadLifecycleErrorKind::NotFound,
        StatusCode::CONFLICT => ProgrammingPreloadLifecycleErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => ProgrammingPreloadLifecycleErrorKind::Unavailable,
        status if status.is_server_error() => ProgrammingPreloadLifecycleErrorKind::Internal,
        _ => ProgrammingPreloadLifecycleErrorKind::Invalid,
    }
}
