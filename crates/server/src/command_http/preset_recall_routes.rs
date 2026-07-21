use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{ActionEnvelope, ActionError, ActionErrorKind};
use light_core::ShowId;
use light_wire::v2::preset_recall::{
    PresetRecallErrorKind, PresetRecallErrorResponse, PresetRecallRequest,
};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session};
use super::{preset_recording_wire, programming_ports::ServerProgrammingPorts};

const BODY_LIMIT: usize = 32 * 1024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/shows/{show_id}/presets/recall",
            post(recall_preset),
        )
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn recall_preset(
    State(state): State<AppState>,
    Path(show_id): Path<Uuid>,
    headers: HeaderMap,
    request: Result<Json<PresetRecallRequest>, JsonRejection>,
) -> Result<Response, PresetRecallHttpError> {
    let session = authenticated_mutation(&state, &headers)?;
    let Json(request) = request.map_err(PresetRecallHttpError::json)?;
    super::routes::validate_request_id(&request.request_id).map_err(PresetRecallHttpError::api)?;
    let address =
        preset_recording_wire::address(request.address).map_err(PresetRecallHttpError::invalid)?;
    let expectation = light_application::ProgrammingPresetRecallRevisionExpectation::Exact;
    let command = light_application::ProgrammingPresetRecallRequest {
        show_id: ShowId(show_id),
        address,
        expected_preset_revision: expectation(request.expected_preset_revision),
        expected_show_revision: expectation(request.expected_show_revision),
        expected_values_revision: expectation(request.expected_programmer_revision),
        expected_capture_mode_revision: expectation(request.expected_capture_mode_revision),
        expected_selection_revision: expectation(request.expected_selection_revision),
    };
    let context = super::routes::http_context(&session, Some(&request.request_id));
    let result = run_action(state, session, ActionEnvelope { context, command }).await?;
    let response = super::preset_recall_wire::outcome(result);
    Ok(json_with_etag(response.programmer_revision, response))
}

async fn run_action(
    state: AppState,
    session: Session,
    action: ActionEnvelope<light_application::ProgrammingPresetRecallRequest>,
) -> Result<light_application::ProgrammingPresetRecallResult, PresetRecallHttpError> {
    let activation = state.activation_lock.clone().lock_owned().await;
    tokio::task::spawn_blocking(move || {
        let ports = ServerProgrammingPorts::new(&state, &session, "http_preset_recall", true);
        let result = state.programming.handle_preset_recall(action, &ports);
        drop(activation);
        result
    })
    .await
    .map_err(PresetRecallHttpError::blocking)?
    .map_err(PresetRecallHttpError::application)
}

fn authenticated_mutation(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Session, PresetRecallHttpError> {
    let session = super::super::authenticate(state, headers).map_err(PresetRecallHttpError::api)?;
    if super::super::read_desk_lock(state, session.desk.id).locked {
        return Err(PresetRecallHttpError::conflict("desk is locked"));
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
        .expect("a numeric Programmer revision always forms a valid ETag")
}

struct PresetRecallHttpError {
    status: StatusCode,
    body: PresetRecallErrorResponse,
}

impl PresetRecallHttpError {
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
            PresetRecallErrorKind::Invalid,
            message,
            None,
            None,
            false,
        )
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            PresetRecallErrorKind::Conflict,
            message,
            None,
            None,
            false,
        )
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            PresetRecallErrorKind::Internal,
            format!("Preset recall service task failed: {error}"),
            None,
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: PresetRecallErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        current_related_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: PresetRecallErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                current_related_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for PresetRecallHttpError {
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

const fn wire_error_kind(kind: ActionErrorKind) -> PresetRecallErrorKind {
    match kind {
        ActionErrorKind::Invalid => PresetRecallErrorKind::Invalid,
        ActionErrorKind::Unauthorized => PresetRecallErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => PresetRecallErrorKind::Forbidden,
        ActionErrorKind::NotFound => PresetRecallErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => PresetRecallErrorKind::Conflict,
        ActionErrorKind::Unavailable => PresetRecallErrorKind::Unavailable,
        ActionErrorKind::Internal => PresetRecallErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> PresetRecallErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => PresetRecallErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => PresetRecallErrorKind::Forbidden,
        StatusCode::NOT_FOUND => PresetRecallErrorKind::NotFound,
        StatusCode::CONFLICT => PresetRecallErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => PresetRecallErrorKind::Unavailable,
        status if status.is_server_error() => PresetRecallErrorKind::Internal,
        _ => PresetRecallErrorKind::Invalid,
    }
}
