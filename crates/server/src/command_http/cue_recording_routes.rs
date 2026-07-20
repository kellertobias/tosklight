use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{ActionEnvelope, ActionError, ActionErrorKind};
use light_core::ShowId;
use light_programmer::CueRecordingCapturedSource;
use light_wire::v2::cue_recording::{CueRecordErrorKind, CueRecordErrorResponse, CueRecordRequest};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session};
use super::{cue_recording_wire, programming_ports::ServerProgrammingPorts, routes::http_context};

const BODY_LIMIT: usize = 32 * 1024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/shows/{show_id}/cues/record", post(record_cue))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn record_cue(
    State(state): State<AppState>,
    Path(show_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<CueRecordRequest>, JsonRejection>,
) -> Result<Response, CueRecordHttpError> {
    let session = authenticated_mutation(&state, &headers)?;
    let show_id = parse_show_id(&show_id)?;
    let expected_revision =
        super::super::parse_if_match(&headers).map_err(CueRecordHttpError::api)?;
    let Json(request) = request.map_err(CueRecordHttpError::json)?;
    super::routes::validate_request_id(&request.request_id).map_err(CueRecordHttpError::api)?;
    let (request_id, command) =
        cue_recording_wire::application_command(ShowId(show_id), expected_revision, request)
            .map_err(CueRecordHttpError::invalid)?;
    let context =
        http_context(&session, Some(&request_id)).with_expected_revision(expected_revision);
    let result = run_action(state, session, ActionEnvelope { context, command }).await?;
    let response = cue_recording_wire::outcome(result).map_err(CueRecordHttpError::application)?;
    Ok(json_with_etag(response.show_revision(), response))
}

async fn run_action(
    state: AppState,
    session: Session,
    action: ActionEnvelope<light_application::ProgrammingCueRecordRequest>,
) -> Result<light_application::ProgrammingCueRecordResult, CueRecordHttpError> {
    let activation = state.activation_lock.clone().lock_owned().await;
    tokio::task::spawn_blocking(move || {
        let ports = ServerProgrammingPorts::new(&state, &session, "http_cue_record", true);
        let result = state.programming.handle_cue_recording(action, &ports);
        persist_released_preload(&state, &session, result.as_ref().ok());
        drop(activation);
        result
    })
    .await
    .map_err(CueRecordHttpError::blocking)?
    .map_err(CueRecordHttpError::application)
}

fn persist_released_preload(
    state: &AppState,
    session: &Session,
    result: Option<&light_application::ProgrammingCueRecordResult>,
) {
    let Some(result) = result.filter(|result| {
        !result.replayed && result.captured_source == CueRecordingCapturedSource::ActivePreload
    }) else {
        return;
    };
    super::events::persist_with_warning(
        state,
        session,
        "http_cue_record",
        Some(&result.request_id),
        "programmer.cue_record",
    );
}

fn authenticated_mutation(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Session, CueRecordHttpError> {
    let session = super::super::authenticate(state, headers).map_err(CueRecordHttpError::api)?;
    if super::super::read_desk_lock(state, session.desk.id).locked {
        return Err(CueRecordHttpError::conflict("desk is locked"));
    }
    Ok(session)
}

fn parse_show_id(value: &str) -> Result<Uuid, CueRecordHttpError> {
    Uuid::parse_str(value).map_err(|_| CueRecordHttpError::invalid("show_id must be a UUID"))
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
        .expect("a numeric show revision always forms a valid ETag")
}

struct CueRecordHttpError {
    status: StatusCode,
    body: CueRecordErrorResponse,
}

impl CueRecordHttpError {
    fn application(error: ActionError) -> Self {
        let revision = error.current_related_revision.or(error.current_revision);
        Self::new(
            application_status(error.kind),
            wire_error_kind(error.kind),
            error.message,
            revision,
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
            CueRecordErrorKind::Invalid,
            message,
            None,
            false,
        )
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            CueRecordErrorKind::Conflict,
            message,
            None,
            false,
        )
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            CueRecordErrorKind::Internal,
            format!("Cue recording service task failed: {error}"),
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: CueRecordErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: CueRecordErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for CueRecordHttpError {
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

const fn wire_error_kind(kind: ActionErrorKind) -> CueRecordErrorKind {
    match kind {
        ActionErrorKind::Invalid => CueRecordErrorKind::Invalid,
        ActionErrorKind::Unauthorized => CueRecordErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => CueRecordErrorKind::Forbidden,
        ActionErrorKind::NotFound => CueRecordErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => CueRecordErrorKind::Conflict,
        ActionErrorKind::Unavailable => CueRecordErrorKind::Unavailable,
        ActionErrorKind::Internal => CueRecordErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> CueRecordErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => CueRecordErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => CueRecordErrorKind::Forbidden,
        StatusCode::NOT_FOUND => CueRecordErrorKind::NotFound,
        StatusCode::CONFLICT => CueRecordErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => CueRecordErrorKind::Unavailable,
        status if status.is_server_error() => CueRecordErrorKind::Internal,
        _ => CueRecordErrorKind::Invalid,
    }
}
