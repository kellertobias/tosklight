//! Authenticated, desk-scoped v2 Playback command and narrow repair endpoints.

mod wire;

pub(super) use wire::{desk_projection, runtime_change, runtime_projection};

use super::{
    AppState, ProgrammingLockPolicy, Session, authenticate, playback_service,
    run_programming_interaction,
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{ActionContext, ActionSource};
use light_wire::v2::playback::{
    PlaybackActionRequest, PlaybackErrorKind, PlaybackErrorResponse, PlaybackRuntimeSnapshotRequest,
};
use uuid::Uuid;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/desks/{desk_id}/playback-actions",
            post(unscoped_playback_action),
        )
        .route(
            "/api/v2/shows/{show_id}/desks/{desk_id}/playback-actions",
            post(scoped_playback_action),
        )
        .route(
            "/api/v2/desks/{desk_id}/playback-runtime/snapshot",
            post(playback_snapshot),
        )
}

async fn unscoped_playback_action(
    State(state): State<AppState>,
    Path(desk_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<PlaybackActionRequest>, JsonRejection>,
) -> Result<Response, PlaybackHttpError> {
    playback_action(state, None, desk_id, headers, request).await
}

async fn scoped_playback_action(
    State(state): State<AppState>,
    Path((show_id, desk_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
    request: Result<Json<PlaybackActionRequest>, JsonRejection>,
) -> Result<Response, PlaybackHttpError> {
    playback_action(state, Some(show_id), desk_id, headers, request).await
}

async fn playback_action(
    state: AppState,
    expected_show: Option<Uuid>,
    desk_id: String,
    headers: HeaderMap,
    request: Result<Json<PlaybackActionRequest>, JsonRejection>,
) -> Result<Response, PlaybackHttpError> {
    let session = authenticated_desk(&state, &headers, &desk_id)?;
    let Json(request) = request.map_err(|error| PlaybackHttpError::invalid(error.body_text()))?;
    let (request_id, command) =
        wire::application_command(request).map_err(PlaybackHttpError::invalid)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    if let Some(show_id) = expected_show {
        require_active_show(&state, show_id)?;
    }
    let context = http_context(&session).with_request_id(request_id);
    let playback_context = context.clone();
    let result = run_programming_interaction(
        &state,
        &session,
        &context,
        "http",
        ProgrammingLockPolicy::RequireUnlocked,
        || {
            playback_service::execute(
                &state,
                Some(&session),
                Some(&session.desk),
                playback_context,
                command,
            )
        },
    )
    .map_err(PlaybackHttpError::api)?
    .output
    .map_err(PlaybackHttpError::api)?;
    Ok(Json(wire::action_outcome(result)).into_response())
}

fn require_active_show(state: &AppState, requested: Uuid) -> Result<(), PlaybackHttpError> {
    let active = state
        .active_show
        .read()
        .as_ref()
        .map(|show| show.id.0)
        .ok_or_else(|| PlaybackHttpError::conflict("no show is active"))?;
    if active != requested {
        return Err(PlaybackHttpError::conflict(
            "requested show is no longer active",
        ));
    }
    Ok(())
}

async fn playback_snapshot(
    State(state): State<AppState>,
    Path(desk_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<PlaybackRuntimeSnapshotRequest>, JsonRejection>,
) -> Result<Response, PlaybackHttpError> {
    let session = authenticated_desk(&state, &headers, &desk_id)?;
    let Json(request) = request.map_err(|error| PlaybackHttpError::invalid(error.body_text()))?;
    let identities =
        wire::application_identities(request.identities).map_err(PlaybackHttpError::invalid)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let context = http_context(&session);
    let snapshot = playback_service::snapshot(&state, &session, context, &identities)
        .map_err(PlaybackHttpError::api)?;
    Ok(Json(wire::runtime_snapshot(snapshot)).into_response())
}

fn authenticated_desk(
    state: &AppState,
    headers: &HeaderMap,
    path_desk_id: &str,
) -> Result<Session, PlaybackHttpError> {
    let session = authenticate(state, headers).map_err(PlaybackHttpError::api)?;
    let desk_id = Uuid::parse_str(path_desk_id)
        .map_err(|_| PlaybackHttpError::invalid("desk_id must be a UUID"))?;
    if session.desk.id != desk_id {
        return Err(PlaybackHttpError::forbidden(
            "session is not authorized for this desk",
        ));
    }
    Ok(session)
}

fn http_context(session: &Session) -> ActionContext {
    ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Http,
    )
}

struct PlaybackHttpError {
    status: StatusCode,
    body: PlaybackErrorResponse,
}

impl PlaybackHttpError {
    fn api(error: super::ApiError) -> Self {
        let kind = match error.status {
            StatusCode::UNAUTHORIZED => PlaybackErrorKind::Unauthorized,
            StatusCode::FORBIDDEN => PlaybackErrorKind::Forbidden,
            StatusCode::NOT_FOUND => PlaybackErrorKind::NotFound,
            StatusCode::CONFLICT => PlaybackErrorKind::Conflict,
            StatusCode::SERVICE_UNAVAILABLE => PlaybackErrorKind::Unavailable,
            status if status.is_server_error() => PlaybackErrorKind::Internal,
            _ => PlaybackErrorKind::Invalid,
        };
        Self::new(
            error.status,
            kind,
            error.message,
            error.status == StatusCode::SERVICE_UNAVAILABLE,
        )
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            PlaybackErrorKind::Invalid,
            message,
            false,
        )
    }

    fn forbidden(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            PlaybackErrorKind::Forbidden,
            message,
            false,
        )
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            PlaybackErrorKind::Conflict,
            message,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: PlaybackErrorKind,
        error: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: PlaybackErrorResponse {
                kind,
                error: error.into(),
                retryable,
            },
        }
    }
}

impl IntoResponse for PlaybackHttpError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
