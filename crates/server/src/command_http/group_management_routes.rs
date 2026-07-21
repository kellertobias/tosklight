use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{ActionEnvelope, ActionError, ActionErrorKind};
use light_core::ShowId;
use light_wire::v2::group_management::{
    GroupManagementErrorKind, GroupManagementErrorResponse, GroupManagementRequest,
};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session};
use super::{
    group_management_wire, programming_ports::ServerProgrammingPorts, routes::http_context,
};

const BODY_LIMIT: usize = 32 * 1024;
const GROUP_ID_LIMIT: usize = 256;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/shows/{show_id}/groups/manage", post(manage_group))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn manage_group(
    State(state): State<AppState>,
    Path(show_id): Path<Uuid>,
    headers: HeaderMap,
    request: Result<Json<GroupManagementRequest>, JsonRejection>,
) -> Result<Response, GroupManagementHttpError> {
    let session = authenticated_mutation(&state, &headers)?;
    let Json(request) = request.map_err(GroupManagementHttpError::json)?;
    super::routes::validate_request_id(&request.request_id)
        .map_err(GroupManagementHttpError::api)?;
    validate_group_id(&request.group_id)?;
    let context = http_context(&session, Some(&request.request_id));
    let command = light_application::GroupManagementRequest {
        show_id: ShowId(show_id),
        group_id: request.group_id,
        operation: group_management_wire::operation(request.operation),
        expected_object_revision: request.expected_object_revision,
        expected_show_revision: None,
    };
    let result = run_action(state, session, ActionEnvelope { context, command }).await?;
    let response = group_management_wire::outcome(result);
    Ok(json_with_etag(response.group_revision(), response))
}

async fn run_action(
    state: AppState,
    session: Session,
    action: ActionEnvelope<light_application::GroupManagementRequest>,
) -> Result<light_application::GroupManagementResult, GroupManagementHttpError> {
    let activation = state.activation_lock.clone().lock_owned().await;
    tokio::task::spawn_blocking(move || {
        let ports = ServerProgrammingPorts::new(&state, &session, "http_group_manage", true);
        let result = state.programming.handle_group_management(action, &ports);
        drop(activation);
        result
    })
    .await
    .map_err(GroupManagementHttpError::blocking)?
    .map_err(GroupManagementHttpError::application)
}

fn authenticated_mutation(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Session, GroupManagementHttpError> {
    let session =
        super::super::authenticate(state, headers).map_err(GroupManagementHttpError::api)?;
    if super::super::read_desk_lock(state, session.desk.id).locked {
        return Err(GroupManagementHttpError::conflict("desk is locked"));
    }
    Ok(session)
}

fn validate_group_id(group_id: &str) -> Result<(), GroupManagementHttpError> {
    if group_id.trim().is_empty()
        || group_id.len() > GROUP_ID_LIMIT
        || group_id.chars().any(char::is_control)
    {
        Err(GroupManagementHttpError::invalid(
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

struct GroupManagementHttpError {
    status: StatusCode,
    body: GroupManagementErrorResponse,
}

impl GroupManagementHttpError {
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
            GroupManagementErrorKind::Invalid,
            message,
            None,
            None,
            false,
        )
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            GroupManagementErrorKind::Conflict,
            message,
            None,
            None,
            false,
        )
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            GroupManagementErrorKind::Internal,
            format!("Group management service task failed: {error}"),
            None,
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: GroupManagementErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        current_related_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: GroupManagementErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                current_related_revision,
                retryable,
            },
        }
    }
}

impl IntoResponse for GroupManagementHttpError {
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

const fn wire_error_kind(kind: ActionErrorKind) -> GroupManagementErrorKind {
    match kind {
        ActionErrorKind::Invalid => GroupManagementErrorKind::Invalid,
        ActionErrorKind::Unauthorized => GroupManagementErrorKind::Unauthorized,
        ActionErrorKind::Forbidden => GroupManagementErrorKind::Forbidden,
        ActionErrorKind::NotFound => GroupManagementErrorKind::NotFound,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => GroupManagementErrorKind::Conflict,
        ActionErrorKind::Unavailable => GroupManagementErrorKind::Unavailable,
        ActionErrorKind::Internal => GroupManagementErrorKind::Internal,
    }
}

fn status_error_kind(status: StatusCode) -> GroupManagementErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => GroupManagementErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => GroupManagementErrorKind::Forbidden,
        StatusCode::NOT_FOUND => GroupManagementErrorKind::NotFound,
        StatusCode::CONFLICT => GroupManagementErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => GroupManagementErrorKind::Unavailable,
        status if status.is_server_error() => GroupManagementErrorKind::Internal,
        _ => GroupManagementErrorKind::Invalid,
    }
}
