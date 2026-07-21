use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{ActionEnvelope, ActionError, ActionErrorKind};
use light_core::ShowId;
use light_wire::v2::cue_deletion::{
    CueDeletionErrorKind, CueDeletionErrorResponse, CueDeletionRequest,
};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session};
use super::{ServerProgrammingCueDeletionPorts, cue_deletion_wire, routes::http_context};

const BODY_LIMIT: usize = 16 * 1024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/desks/{desk_id}/shows/{show_id}/cues/delete",
            post(delete_cue),
        )
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn delete_cue(
    State(state): State<AppState>,
    Path((desk_id, show_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
    request: Result<Json<CueDeletionRequest>, JsonRejection>,
) -> Result<Response, CueDeletionHttpError> {
    let session = authenticated_desk(&state, &headers, desk_id)?;
    let show_id = parse_show_id(&show_id)?;
    let expected_revision =
        super::super::parse_if_match(&headers).map_err(CueDeletionHttpError::api)?;
    let Json(request) = request.map_err(CueDeletionHttpError::json)?;
    super::routes::validate_request_id(&request.request_id).map_err(CueDeletionHttpError::api)?;
    let (request_id, command) = cue_deletion_wire::application_command(show_id, request)
        .map_err(CueDeletionHttpError::invalid)?;
    let context =
        http_context(&session, Some(&request_id)).with_expected_revision(expected_revision);
    let result = run_action(state, session, ActionEnvelope { context, command }).await?;
    let response = cue_deletion_wire::outcome(result);
    Ok(json_with_etag(response.show_revision(), response))
}

async fn run_action(
    state: AppState,
    session: Session,
    action: ActionEnvelope<light_application::ProgrammingCueDeletionRequest>,
) -> Result<light_application::ProgrammingCueDeletionResult, CueDeletionHttpError> {
    tokio::task::spawn_blocking(move || {
        let ports = ServerProgrammingCueDeletionPorts::new(state.clone(), session, false);
        state
            .programming
            .handle_cue_deletion(action, &state.active_show_service, &ports)
    })
    .await
    .map_err(CueDeletionHttpError::blocking)?
    .map_err(CueDeletionHttpError::application)
}

fn authenticated_desk(
    state: &AppState,
    headers: &HeaderMap,
    desk_id: Uuid,
) -> Result<Session, CueDeletionHttpError> {
    let session = super::super::authenticate(state, headers).map_err(CueDeletionHttpError::api)?;
    if session.desk.id != desk_id {
        return Err(CueDeletionHttpError::new(
            StatusCode::FORBIDDEN,
            CueDeletionErrorKind::Forbidden,
            "the authenticated session does not belong to this desk",
            None,
            None,
            false,
        ));
    }
    Ok(session)
}

fn parse_show_id(value: &str) -> Result<ShowId, CueDeletionHttpError> {
    let id = Uuid::parse_str(value)
        .map_err(|_| CueDeletionHttpError::invalid("show_id must be a UUID"))?;
    if id.is_nil() {
        return Err(CueDeletionHttpError::invalid("show_id must not be nil"));
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

struct CueDeletionHttpError {
    status: StatusCode,
    body: CueDeletionErrorResponse,
}

impl CueDeletionHttpError {
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
            CueDeletionErrorKind::Invalid,
            message,
            None,
            None,
            false,
        )
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            CueDeletionErrorKind::Internal,
            format!("Cue deletion service task failed: {error}"),
            None,
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: CueDeletionErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        current_related_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: CueDeletionErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                current_related_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for CueDeletionHttpError {
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

const fn wire_error_kind(kind: ActionErrorKind) -> CueDeletionErrorKind {
    match kind {
        ActionErrorKind::Invalid => CueDeletionErrorKind::Invalid,
        ActionErrorKind::Unauthorized => CueDeletionErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => CueDeletionErrorKind::Forbidden,
        ActionErrorKind::NotFound => CueDeletionErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => CueDeletionErrorKind::Conflict,
        ActionErrorKind::Unavailable => CueDeletionErrorKind::Unavailable,
        ActionErrorKind::Internal => CueDeletionErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> CueDeletionErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => CueDeletionErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => CueDeletionErrorKind::Forbidden,
        StatusCode::NOT_FOUND => CueDeletionErrorKind::NotFound,
        StatusCode::CONFLICT => CueDeletionErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => CueDeletionErrorKind::Unavailable,
        status if status.is_server_error() => CueDeletionErrorKind::Internal,
        _ => CueDeletionErrorKind::Invalid,
    }
}
