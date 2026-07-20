//! Authenticated v2 actions for portable Cuelist, Playback, and Page topology.

use super::{
    ApiError, AppState, ServerPlaybackTopologyPorts, Session, authenticate, parse_if_match,
    playback_topology_wire,
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    PlaybackTopologyCommand,
};
use light_core::ShowId;
use light_wire::v2::playback_topology::{
    PlaybackTopologyActionRequest, PlaybackTopologyErrorKind, PlaybackTopologyErrorResponse,
};
use uuid::Uuid;

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v2/shows/{show_id}/playback-topology/actions",
        post(apply_action),
    )
}

async fn apply_action(
    State(state): State<AppState>,
    Path(show_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<PlaybackTopologyActionRequest>, JsonRejection>,
) -> Result<Response, PlaybackTopologyHttpError> {
    let session = authenticate(&state, &headers).map_err(PlaybackTopologyHttpError::api)?;
    let show_id = parse_show_id(&show_id)?;
    let expected_revision = parse_if_match(&headers).map_err(PlaybackTopologyHttpError::api)?;
    let Json(request) = request.map_err(PlaybackTopologyHttpError::json)?;
    validate_request_id(&request.request_id)?;
    let (request_id, command) = playback_topology_wire::application_command(show_id, request)
        .map_err(PlaybackTopologyHttpError::invalid)?;
    let context = http_context(&session)
        .with_request_id(request_id)
        .with_expected_revision(expected_revision);
    let result = run_action(state, session, ActionEnvelope { context, command }).await?;
    let response =
        playback_topology_wire::outcome(result).map_err(PlaybackTopologyHttpError::application)?;
    Ok(json_with_etag(response.show_revision, response))
}

async fn run_action(
    state: AppState,
    session: Session,
    action: ActionEnvelope<PlaybackTopologyCommand>,
) -> Result<light_application::PlaybackTopologyResult, PlaybackTopologyHttpError> {
    let service = state.playback_topology.clone();
    let show_id = action.command.show_id;
    tokio::task::spawn_blocking(move || {
        let ports = ServerPlaybackTopologyPorts::new(state, session, show_id);
        service.handle(action, &ports)
    })
    .await
    .map_err(PlaybackTopologyHttpError::blocking)?
    .map_err(PlaybackTopologyHttpError::application)
}

fn parse_show_id(value: &str) -> Result<ShowId, PlaybackTopologyHttpError> {
    let id = Uuid::parse_str(value)
        .map_err(|_| PlaybackTopologyHttpError::invalid("show_id must be a UUID"))?;
    if id.is_nil() {
        return Err(PlaybackTopologyHttpError::invalid(
            "show_id must not be nil",
        ));
    }
    Ok(ShowId(id))
}

fn validate_request_id(value: &str) -> Result<(), PlaybackTopologyHttpError> {
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(PlaybackTopologyHttpError::invalid(
            "request_id must contain 1-128 printable bytes",
        ));
    }
    Ok(())
}

fn http_context(session: &Session) -> ActionContext {
    ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Http,
    )
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
        .expect("a numeric Show revision always forms a valid ETag")
}

struct PlaybackTopologyHttpError {
    status: StatusCode,
    body: PlaybackTopologyErrorResponse,
}

impl PlaybackTopologyHttpError {
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
            PlaybackTopologyErrorKind::Invalid,
            message,
            None,
            None,
            false,
        )
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            PlaybackTopologyErrorKind::Internal,
            format!("Playback topology service task failed: {error}"),
            None,
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: PlaybackTopologyErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        current_related_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: PlaybackTopologyErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                current_related_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for PlaybackTopologyHttpError {
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

const fn wire_error_kind(kind: ActionErrorKind) -> PlaybackTopologyErrorKind {
    match kind {
        ActionErrorKind::Invalid => PlaybackTopologyErrorKind::Invalid,
        ActionErrorKind::Unauthorized => PlaybackTopologyErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => PlaybackTopologyErrorKind::Forbidden,
        ActionErrorKind::NotFound => PlaybackTopologyErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => PlaybackTopologyErrorKind::Conflict,
        ActionErrorKind::Unavailable => PlaybackTopologyErrorKind::Unavailable,
        ActionErrorKind::Internal => PlaybackTopologyErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> PlaybackTopologyErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => PlaybackTopologyErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => PlaybackTopologyErrorKind::Forbidden,
        StatusCode::NOT_FOUND => PlaybackTopologyErrorKind::NotFound,
        StatusCode::CONFLICT => PlaybackTopologyErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => PlaybackTopologyErrorKind::Unavailable,
        status if status.is_server_error() => PlaybackTopologyErrorKind::Internal,
        _ => PlaybackTopologyErrorKind::Invalid,
    }
}
