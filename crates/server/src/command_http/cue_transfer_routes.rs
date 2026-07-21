use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{ActionEnvelope, ActionError, ActionErrorKind};
use light_core::ShowId;
use light_wire::v2::cue_transfer::{
    CueTransferErrorKind, CueTransferErrorResponse, CueTransferRequest,
};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session};
use super::{ServerProgrammingCueTransferPorts, cue_transfer_wire, routes::http_context};

const BODY_LIMIT: usize = 16 * 1024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/shows/{show_id}/cues/transfer", post(transfer_cue))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn transfer_cue(
    State(state): State<AppState>,
    Path(show_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<CueTransferRequest>, JsonRejection>,
) -> Result<Response, CueTransferHttpError> {
    let session = authenticated_mutation(&state, &headers)?;
    let show_id = parse_show_id(&show_id)?;
    let expected_revision =
        super::super::parse_if_match(&headers).map_err(CueTransferHttpError::api)?;
    let Json(request) = request.map_err(CueTransferHttpError::json)?;
    super::routes::validate_request_id(&request.request_id).map_err(CueTransferHttpError::api)?;
    let (request_id, command) = cue_transfer_wire::application_command(show_id, request)
        .map_err(CueTransferHttpError::invalid)?;
    let context =
        http_context(&session, Some(&request_id)).with_expected_revision(expected_revision);
    let result = run_action(state, session, ActionEnvelope { context, command }).await?;
    let response = cue_transfer_wire::outcome(result);
    Ok(json_with_etag(response.show_revision(), response))
}

async fn run_action(
    state: AppState,
    session: Session,
    action: ActionEnvelope<light_application::ProgrammingCueTransferRequest>,
) -> Result<light_application::ProgrammingCueTransferResult, CueTransferHttpError> {
    tokio::task::spawn_blocking(move || {
        let ports = ServerProgrammingCueTransferPorts::new(state.clone(), session, false);
        state
            .programming
            .handle_cue_transfer(action, &state.active_show_service, &ports)
    })
    .await
    .map_err(CueTransferHttpError::blocking)?
    .map_err(CueTransferHttpError::application)
}

fn authenticated_mutation(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Session, CueTransferHttpError> {
    let session = super::super::authenticate(state, headers).map_err(CueTransferHttpError::api)?;
    if super::super::read_desk_lock(state, session.desk.id).locked {
        return Err(CueTransferHttpError::conflict("desk is locked"));
    }
    Ok(session)
}

fn parse_show_id(value: &str) -> Result<ShowId, CueTransferHttpError> {
    let id = Uuid::parse_str(value)
        .map_err(|_| CueTransferHttpError::invalid("show_id must be a UUID"))?;
    if id.is_nil() {
        return Err(CueTransferHttpError::invalid("show_id must not be nil"));
    }
    Ok(ShowId(id))
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

struct CueTransferHttpError {
    status: StatusCode,
    body: CueTransferErrorResponse,
}

impl CueTransferHttpError {
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
            CueTransferErrorKind::Invalid,
            message,
            None,
            None,
            false,
        )
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            CueTransferErrorKind::Conflict,
            message,
            None,
            None,
            false,
        )
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            CueTransferErrorKind::Internal,
            format!("Cue transfer service task failed: {error}"),
            None,
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: CueTransferErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        current_related_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: CueTransferErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                current_related_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for CueTransferHttpError {
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

const fn wire_error_kind(kind: ActionErrorKind) -> CueTransferErrorKind {
    match kind {
        ActionErrorKind::Invalid => CueTransferErrorKind::Invalid,
        ActionErrorKind::Unauthorized => CueTransferErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => CueTransferErrorKind::Forbidden,
        ActionErrorKind::NotFound => CueTransferErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => CueTransferErrorKind::Conflict,
        ActionErrorKind::Unavailable => CueTransferErrorKind::Unavailable,
        ActionErrorKind::Internal => CueTransferErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> CueTransferErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => CueTransferErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => CueTransferErrorKind::Forbidden,
        StatusCode::NOT_FOUND => CueTransferErrorKind::NotFound,
        StatusCode::CONFLICT => CueTransferErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => CueTransferErrorKind::Unavailable,
        status if status.is_server_error() => CueTransferErrorKind::Internal,
        _ => CueTransferErrorKind::Invalid,
    }
}
