//! Authenticated, desk-scoped v2 Playback command and narrow repair endpoints.

mod wire;

pub(super) use wire::{
    action_outcome, application_command, desk_projection, runtime_change, runtime_projection,
    telemetry_tick,
};

use super::{
    AppState, DeskContext, ProgrammingLockPolicy, Session, ShowContext, playback_service,
    run_programming_interaction, session_for_desk,
};
use crate::tolerant_json::TolerantJson;
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{ActionContext, ActionSource};
use light_wire::v2::playback::{
    PlaybackActionRequest, PlaybackErrorKind, PlaybackErrorResponse, PlaybackRuntimeSnapshotRequest,
};
pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/playback-actions", post(playback_action))
        .route("/api/v2/playback-runtime/snapshot", post(playback_snapshot))
}

async fn playback_action(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    request: Result<TolerantJson<PlaybackActionRequest>, JsonRejection>,
) -> Result<Response, PlaybackHttpError> {
    let session = session_for_desk(&state, &headers, &desk).map_err(PlaybackHttpError::api)?;
    let TolerantJson(request) =
        request.map_err(|error| PlaybackHttpError::invalid(error.body_text()))?;
    let (request_id, command) =
        wire::application_command(request).map_err(PlaybackHttpError::invalid)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    show.verify(&state).map_err(PlaybackHttpError::api)?;
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

async fn playback_snapshot(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    request: Result<Json<PlaybackRuntimeSnapshotRequest>, JsonRejection>,
) -> Result<Response, PlaybackHttpError> {
    let session = session_for_desk(&state, &headers, &desk).map_err(PlaybackHttpError::api)?;
    let Json(request) = request.map_err(|error| PlaybackHttpError::invalid(error.body_text()))?;
    let identities =
        wire::application_identities(request.identities).map_err(PlaybackHttpError::invalid)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    show.verify(&state).map_err(PlaybackHttpError::api)?;
    let context = http_context(&session);
    let snapshot = playback_service::snapshot(&state, &session, context, &identities)
        .map_err(PlaybackHttpError::api)?;
    Ok(Json(wire::runtime_snapshot(snapshot)).into_response())
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
