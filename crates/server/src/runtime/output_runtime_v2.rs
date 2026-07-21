//! Authenticated revisioned actions and repair projection for global output runtime.

use super::{AppState, Session, authenticate, output_runtime_service, read_desk_lock};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActionSource, OutputRuntimeChange,
    OutputRuntimeIdentity, OutputRuntimeOutcome, OutputRuntimeProjection, OutputRuntimeSnapshot,
};
use light_wire::v2::events as wire;
use light_wire::v2::output_runtime as action_wire;
use uuid::Uuid;

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v2/desks/{desk_id}/output-runtime/{identity}",
        get(output_runtime_snapshot).post(output_runtime_action),
    )
}

async fn output_runtime_action(
    State(state): State<AppState>,
    Path((desk_id, identity)): Path<(String, String)>,
    headers: HeaderMap,
    request: Result<Json<action_wire::OutputRuntimeActionRequest>, JsonRejection>,
) -> Result<Response, OutputRuntimeHttpError> {
    let session =
        authenticated_desk(&state, &headers, &desk_id).map_err(OutputRuntimeHttpError::api)?;
    parse_identity(&identity).map_err(OutputRuntimeHttpError::api)?;
    let Json(request) =
        request.map_err(|error| OutputRuntimeHttpError::invalid(error.body_text()))?;
    validate_request_id(&request.request_id)?;
    let command = output_runtime_service::exact_command(
        request.expected_show_id,
        request.expected_revision,
        request.grand_master,
        request.blackout,
    )
    .map_err(OutputRuntimeHttpError::api)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let desk_operation = state.programming.desk_lock(session.desk.id);
    let _desk_operation = desk_operation.lock();
    if read_desk_lock(&state, session.desk.id).locked {
        return Err(OutputRuntimeHttpError::conflict(
            "desk is locked",
            Some(state.output_control.lock().revision),
        ));
    }
    let context = http_context(&session).with_request_id(&request.request_id);
    let result = output_runtime_service::execute_action(&state, Some(&session), context, command)
        .map_err(OutputRuntimeHttpError::action)?;
    Ok(Json(wire_outcome(result)).into_response())
}

async fn output_runtime_snapshot(
    State(state): State<AppState>,
    Path((desk_id, identity)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<wire::OutputRuntimeSnapshot>, super::ApiError> {
    let session = authenticated_desk(&state, &headers, &desk_id)?;
    let identity = parse_identity(&identity)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let snapshot =
        output_runtime_service::snapshot(&state, &session, http_context(&session), identity)?;
    Ok(Json(wire_snapshot(snapshot)))
}

fn authenticated_desk(
    state: &AppState,
    headers: &HeaderMap,
    path_desk_id: &str,
) -> Result<Session, super::ApiError> {
    let session = authenticate(state, headers)?;
    let desk_id = Uuid::parse_str(path_desk_id)
        .map_err(|_| super::ApiError::bad_request("desk_id must be a UUID"))?;
    if session.desk.id != desk_id {
        return Err(super::ApiError::forbidden(
            "session is not authorized for this desk",
        ));
    }
    Ok(session)
}

fn parse_identity(value: &str) -> Result<OutputRuntimeIdentity, super::ApiError> {
    match value {
        "global-master" => Ok(OutputRuntimeIdentity::GlobalMaster),
        _ => Err(super::ApiError::not_found("output runtime identity")),
    }
}

fn http_context(session: &Session) -> ActionContext {
    ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Http,
    )
}

fn wire_snapshot(snapshot: OutputRuntimeSnapshot) -> wire::OutputRuntimeSnapshot {
    wire::OutputRuntimeSnapshot {
        cursor: wire::EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        projection: wire_projection(snapshot.projection),
    }
}

pub(super) fn wire_change(change: OutputRuntimeChange) -> wire::OutputRuntimeChange {
    wire::OutputRuntimeChange {
        projection: wire_projection(change.projection),
    }
}

fn wire_projection(projection: OutputRuntimeProjection) -> wire::OutputRuntimeProjection {
    wire::OutputRuntimeProjection {
        scope: wire::OutputRuntimeScope {
            show_id: projection.scope.show_id,
        },
        identity: match projection.identity {
            OutputRuntimeIdentity::GlobalMaster => wire::OutputRuntimeIdentity::GlobalMaster,
        },
        revision: projection.revision,
        grand_master: projection.grand_master,
        blackout: projection.blackout,
    }
}

fn wire_outcome(
    result: light_application::OutputRuntimeResult,
) -> action_wire::OutputRuntimeActionOutcome {
    let outcome = match result.outcome {
        OutputRuntimeOutcome::Applied => action_wire::OutputRuntimeActionState::Changed {
            event_sequence: result
                .event_sequence
                .expect("changed output actions publish exactly one event"),
        },
        OutputRuntimeOutcome::NoChange => action_wire::OutputRuntimeActionState::NoChange {},
    };
    action_wire::OutputRuntimeActionOutcome {
        request_id: result
            .context
            .request_id
            .clone()
            .expect("v2 output actions require a request ID"),
        correlation_id: result.context.correlation_id,
        projection: wire_projection(result.projection),
        outcome,
        replayed: result.replayed,
        durability: match result.durability {
            light_application::OutputRuntimeDurability::Durable => {
                action_wire::OutputRuntimeDurability::Durable
            }
            light_application::OutputRuntimeDurability::PersistencePending => {
                action_wire::OutputRuntimeDurability::PersistencePending
            }
        },
        warning: result.warning,
    }
}

fn validate_request_id(value: &str) -> Result<(), OutputRuntimeHttpError> {
    if value.is_empty() || value.len() > 128 || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(OutputRuntimeHttpError::invalid(
            "request_id must contain 1-128 printable bytes",
        ));
    }
    Ok(())
}

struct OutputRuntimeHttpError {
    status: StatusCode,
    body: action_wire::OutputRuntimeErrorResponse,
}

impl OutputRuntimeHttpError {
    fn action(error: ActionError) -> Self {
        let status = match error.kind {
            ActionErrorKind::Invalid => StatusCode::BAD_REQUEST,
            ActionErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            ActionErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ActionErrorKind::NotFound => StatusCode::NOT_FOUND,
            ActionErrorKind::Conflict | ActionErrorKind::Busy => StatusCode::CONFLICT,
            ActionErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            ActionErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(
            status,
            error_kind(status),
            error.message,
            error.current_revision,
            error.retryable,
        )
    }

    fn api(error: super::ApiError) -> Self {
        let retryable = error.status == StatusCode::SERVICE_UNAVAILABLE;
        Self::new(
            error.status,
            error_kind(error.status),
            error.message,
            None,
            retryable,
        )
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            action_wire::OutputRuntimeErrorKind::Invalid,
            message,
            None,
            false,
        )
    }

    fn conflict(message: impl Into<String>, current_revision: Option<u64>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            action_wire::OutputRuntimeErrorKind::Conflict,
            message,
            current_revision,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: action_wire::OutputRuntimeErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: action_wire::OutputRuntimeErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                retryable,
            },
        }
    }
}

fn error_kind(status: StatusCode) -> action_wire::OutputRuntimeErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => action_wire::OutputRuntimeErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => action_wire::OutputRuntimeErrorKind::Forbidden,
        StatusCode::NOT_FOUND => action_wire::OutputRuntimeErrorKind::NotFound,
        StatusCode::CONFLICT => action_wire::OutputRuntimeErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => action_wire::OutputRuntimeErrorKind::Unavailable,
        status if status.is_server_error() => action_wire::OutputRuntimeErrorKind::Internal,
        _ => action_wire::OutputRuntimeErrorKind::Invalid,
    }
}

impl IntoResponse for OutputRuntimeHttpError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
