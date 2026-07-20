use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{ActionEnvelope, ActionError, ActionErrorKind};
use light_core::ShowId;
use light_wire::v2::group_recording::{
    GroupRecordErrorKind, GroupRecordErrorResponse, GroupRecordRequest,
};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session};
use super::{
    group_recording_wire, programming_ports::ServerProgrammingPorts, routes::http_context,
};

const BODY_LIMIT: usize = 32 * 1024;
const GROUP_ID_LIMIT: usize = 256;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/shows/{show_id}/groups/record", post(record_group))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn record_group(
    State(state): State<AppState>,
    Path(show_id): Path<Uuid>,
    headers: HeaderMap,
    request: Result<Json<GroupRecordRequest>, JsonRejection>,
) -> Result<Response, GroupRecordHttpError> {
    let session = authenticated_mutation(&state, &headers)?;
    let Json(request) = request.map_err(GroupRecordHttpError::json)?;
    super::routes::validate_request_id(&request.request_id).map_err(GroupRecordHttpError::api)?;
    validate_group_id(&request.group_id)?;
    let context = http_context(&session, Some(&request.request_id));
    let command = light_application::ProgrammingGroupRecordRequest {
        show_id: ShowId(show_id),
        group_id: request.group_id,
        operation: group_recording_wire::operation(request.operation),
        expected_object_revision: light_application::ProgrammingGroupRevisionExpectation::Exact(
            request.expected_object_revision,
        ),
        expected_show_revision: None,
    };
    let result = run_action(state, session, ActionEnvelope { context, command }).await?;
    let response =
        group_recording_wire::outcome(result).map_err(GroupRecordHttpError::application)?;
    Ok(json_with_etag(response.group_revision(), response))
}

async fn run_action(
    state: AppState,
    session: Session,
    action: ActionEnvelope<light_application::ProgrammingGroupRecordRequest>,
) -> Result<light_application::ProgrammingGroupRecordResult, GroupRecordHttpError> {
    let activation = state.activation_lock.clone().lock_owned().await;
    tokio::task::spawn_blocking(move || {
        let ports = ServerProgrammingPorts::new(&state, &session, "http_group_record", true);
        let result = state.programming.handle_group_recording(action, &ports);
        drop(activation);
        result
    })
    .await
    .map_err(GroupRecordHttpError::blocking)?
    .map_err(GroupRecordHttpError::application)
}

fn authenticated_mutation(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Session, GroupRecordHttpError> {
    let session = super::super::authenticate(state, headers).map_err(GroupRecordHttpError::api)?;
    if super::super::read_desk_lock(state, session.desk.id).locked {
        return Err(GroupRecordHttpError::conflict("desk is locked"));
    }
    Ok(session)
}

fn validate_group_id(group_id: &str) -> Result<(), GroupRecordHttpError> {
    if group_id.trim().is_empty()
        || group_id.len() > GROUP_ID_LIMIT
        || group_id.chars().any(char::is_control)
    {
        Err(GroupRecordHttpError::invalid(
            "group_id must contain 1-256 printable bytes",
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
        .expect("a numeric Group revision always forms a valid ETag")
}

struct GroupRecordHttpError {
    status: StatusCode,
    body: GroupRecordErrorResponse,
}

impl GroupRecordHttpError {
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
            GroupRecordErrorKind::Invalid,
            message,
            None,
            false,
        )
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            GroupRecordErrorKind::Conflict,
            message,
            None,
            false,
        )
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            GroupRecordErrorKind::Internal,
            format!("Group recording service task failed: {error}"),
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: GroupRecordErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: GroupRecordErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for GroupRecordHttpError {
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

const fn wire_error_kind(kind: ActionErrorKind) -> GroupRecordErrorKind {
    match kind {
        ActionErrorKind::Invalid => GroupRecordErrorKind::Invalid,
        ActionErrorKind::Unauthorized => GroupRecordErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => GroupRecordErrorKind::Forbidden,
        ActionErrorKind::NotFound => GroupRecordErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => GroupRecordErrorKind::Conflict,
        ActionErrorKind::Unavailable => GroupRecordErrorKind::Unavailable,
        ActionErrorKind::Internal => GroupRecordErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> GroupRecordErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => GroupRecordErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => GroupRecordErrorKind::Forbidden,
        StatusCode::NOT_FOUND => GroupRecordErrorKind::NotFound,
        StatusCode::CONFLICT => GroupRecordErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => GroupRecordErrorKind::Unavailable,
        status if status.is_server_error() => GroupRecordErrorKind::Internal,
        _ => GroupRecordErrorKind::Invalid,
    }
}
