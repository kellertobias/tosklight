use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{ActionEnvelope, ActionError, ActionErrorKind};
use light_core::ShowId;
use light_wire::v2::preset_recording::{
    PresetRecordErrorKind, PresetRecordErrorResponse, PresetRecordRequest,
};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session};
use super::{
    preset_recording_wire, programming_ports::ServerProgrammingPorts, routes::http_context,
};

const BODY_LIMIT: usize = 32 * 1024;
const NAME_LIMIT: usize = 256;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/shows/{show_id}/presets/record",
            post(record_preset),
        )
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn record_preset(
    State(state): State<AppState>,
    Path(show_id): Path<Uuid>,
    headers: HeaderMap,
    request: Result<Json<PresetRecordRequest>, JsonRejection>,
) -> Result<Response, PresetRecordHttpError> {
    let session = authenticated_mutation(&state, &headers)?;
    let Json(request) = request.map_err(PresetRecordHttpError::json)?;
    super::routes::validate_request_id(&request.request_id).map_err(PresetRecordHttpError::api)?;
    validate_name(&request.name)?;
    let address =
        preset_recording_wire::address(request.address).map_err(PresetRecordHttpError::invalid)?;
    let context = http_context(&session, Some(&request.request_id));
    let command = light_application::ProgrammingPresetRecordRequest {
        show_id: ShowId(show_id),
        address,
        name: request.name,
        mode: preset_recording_wire::mode(request.mode),
        expected_object_revision: light_application::ProgrammingPresetRevisionExpectation::Exact(
            request.expected_object_revision,
        ),
        expected_show_revision: None,
    };
    let result = run_action(state, session, ActionEnvelope { context, command }).await?;
    let response = preset_recording_wire::outcome(result);
    Ok(json_with_etag(response.preset().revision, response))
}

async fn run_action(
    state: AppState,
    session: Session,
    action: ActionEnvelope<light_application::ProgrammingPresetRecordRequest>,
) -> Result<light_application::ProgrammingPresetRecordResult, PresetRecordHttpError> {
    let activation = state.activation_lock.clone().lock_owned().await;
    tokio::task::spawn_blocking(move || {
        let ports = ServerProgrammingPorts::new(&state, &session, "http_preset_record", true);
        let result = state.programming.handle_preset_recording(action, &ports);
        drop(activation);
        result
    })
    .await
    .map_err(PresetRecordHttpError::blocking)?
    .map_err(PresetRecordHttpError::application)
}

fn authenticated_mutation(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Session, PresetRecordHttpError> {
    let session = super::super::authenticate(state, headers).map_err(PresetRecordHttpError::api)?;
    if super::super::read_desk_lock(state, session.desk.id).locked {
        return Err(PresetRecordHttpError::conflict("desk is locked"));
    }
    Ok(session)
}

fn validate_name(name: &str) -> Result<(), PresetRecordHttpError> {
    if name.trim().is_empty() || name.len() > NAME_LIMIT || name.chars().any(char::is_control) {
        Err(PresetRecordHttpError::invalid(
            "name must contain 1-256 printable bytes",
        ))
    } else {
        Ok(())
    }
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
        .expect("a numeric Preset revision always forms a valid ETag")
}

struct PresetRecordHttpError {
    status: StatusCode,
    body: PresetRecordErrorResponse,
}

impl PresetRecordHttpError {
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
        Self::new(
            error.status,
            status_error_kind(error.status),
            error.message,
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
            PresetRecordErrorKind::Invalid,
            message,
            None,
            false,
        )
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            PresetRecordErrorKind::Conflict,
            message,
            None,
            false,
        )
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            PresetRecordErrorKind::Internal,
            format!("Preset recording service task failed: {error}"),
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: PresetRecordErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: PresetRecordErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for PresetRecordHttpError {
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

const fn wire_error_kind(kind: ActionErrorKind) -> PresetRecordErrorKind {
    match kind {
        ActionErrorKind::Invalid => PresetRecordErrorKind::Invalid,
        ActionErrorKind::Unauthorized => PresetRecordErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => PresetRecordErrorKind::Forbidden,
        ActionErrorKind::NotFound => PresetRecordErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => PresetRecordErrorKind::Conflict,
        ActionErrorKind::Unavailable => PresetRecordErrorKind::Unavailable,
        ActionErrorKind::Internal => PresetRecordErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> PresetRecordErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => PresetRecordErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => PresetRecordErrorKind::Forbidden,
        StatusCode::NOT_FOUND => PresetRecordErrorKind::NotFound,
        StatusCode::CONFLICT => PresetRecordErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => PresetRecordErrorKind::Unavailable,
        status if status.is_server_error() => PresetRecordErrorKind::Internal,
        _ => PresetRecordErrorKind::Invalid,
    }
}
