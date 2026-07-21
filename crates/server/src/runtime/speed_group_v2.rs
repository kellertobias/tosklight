//! Authenticated revisioned actions and repair projection for retained/manual Speed Groups.

use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActionSource, SpeedGroupAction, SpeedGroupChange,
    SpeedGroupOutcome, SpeedGroupProjection, SpeedGroupResult, SpeedGroupSnapshot,
};
use light_wire::v2::{events::EventSnapshotCursor, speed_group as wire};
use uuid::Uuid;

use super::{AppState, Session, authenticate, speed_group_service};

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v2/desks/{desk_id}/speed-groups",
        get(snapshot).post(action),
    )
}

async fn action(
    State(state): State<AppState>,
    Path(desk_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<wire::SpeedGroupActionRequest>, JsonRejection>,
) -> Result<Response, SpeedGroupHttpError> {
    let session =
        authenticated_desk(&state, &headers, &desk_id).map_err(SpeedGroupHttpError::api)?;
    let Json(request) = request.map_err(|error| SpeedGroupHttpError::invalid(error.body_text()))?;
    validate_request_id(&request.request_id)?;
    let command = speed_group_service::exact_command(
        request.expected_authority_id,
        request.expected_revision,
        application_action(request.action).map_err(SpeedGroupHttpError::api)?,
    );
    let _activation = state.activation_lock.clone().lock_owned().await;
    let desk_operation = state.programming.desk_lock(session.desk.id);
    let _desk_operation = desk_operation.lock();
    let context = http_context(&session).with_request_id(&request.request_id);
    let result = speed_group_service::execute_http_action(&state, &session, context, command)
        .map_err(SpeedGroupHttpError::action)?;
    Ok(Json(wire_outcome(result)).into_response())
}

async fn snapshot(
    State(state): State<AppState>,
    Path(desk_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<wire::SpeedGroupSnapshot>, super::ApiError> {
    let session = authenticated_desk(&state, &headers, &desk_id)?;
    let snapshot = speed_group_service::snapshot(&state, &session, http_context(&session))?;
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

fn http_context(session: &Session) -> ActionContext {
    ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Http,
    )
}

fn application_action(action: wire::SpeedGroupAction) -> Result<SpeedGroupAction, super::ApiError> {
    Ok(match action {
        wire::SpeedGroupAction::SetBpm { group, bpm } => SpeedGroupAction::SetBpm {
            group: application_group(group),
            bpm: speed_group_service::bpm(bpm)?,
        },
        wire::SpeedGroupAction::AdjustBpm { group, delta_bpm } => SpeedGroupAction::AdjustBpm {
            group: application_group(group),
            delta: speed_group_service::delta(delta_bpm)?,
        },
        wire::SpeedGroupAction::Synchronize { source, target } => SpeedGroupAction::Synchronize {
            source: application_group(source),
            target: application_group(target),
        },
    })
}

fn application_group(group: wire::SpeedGroupId) -> light_application::SpeedGroupId {
    let one_based = match group {
        wire::SpeedGroupId::A => 1,
        wire::SpeedGroupId::B => 2,
        wire::SpeedGroupId::C => 3,
        wire::SpeedGroupId::D => 4,
        wire::SpeedGroupId::E => 5,
    };
    light_application::SpeedGroupId::new(one_based).expect("wire Speed Group is within A-E")
}

fn wire_snapshot(snapshot: SpeedGroupSnapshot) -> wire::SpeedGroupSnapshot {
    wire::SpeedGroupSnapshot {
        cursor: EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        projection: wire::SpeedGroupAuthorityProjection {
            authority_id: snapshot.projection.authority_id,
            revision: snapshot.projection.revision,
            groups: snapshot
                .projection
                .groups
                .into_iter()
                .map(wire_projection)
                .collect(),
        },
    }
}

pub(super) fn wire_change(change: &SpeedGroupChange) -> wire::SpeedGroupChange {
    wire::SpeedGroupChange {
        authority_id: change.authority_id,
        revision: change.revision,
        applied_at_millis: change.applied_at_millis,
        groups: change.groups.iter().copied().map(wire_projection).collect(),
    }
}

fn wire_projection(projection: SpeedGroupProjection) -> wire::SpeedGroupProjection {
    wire::SpeedGroupProjection {
        group: wire_group(projection.group),
        manual_bpm: projection.manual_bpm,
        paused: projection.paused,
        speed_master_scale: projection.speed_master_scale,
        synchronized_with: projection.synchronized_with.map(wire_group),
        phase_origin_millis: projection.phase_origin_millis,
    }
}

fn wire_group(group: light_application::SpeedGroupId) -> wire::SpeedGroupId {
    match group.one_based() {
        1 => wire::SpeedGroupId::A,
        2 => wire::SpeedGroupId::B,
        3 => wire::SpeedGroupId::C,
        4 => wire::SpeedGroupId::D,
        5 => wire::SpeedGroupId::E,
        _ => unreachable!("application Speed Group is within A-E"),
    }
}

fn wire_outcome(result: SpeedGroupResult) -> wire::SpeedGroupActionOutcome {
    let outcome = match result.outcome {
        SpeedGroupOutcome::Applied => wire::SpeedGroupActionState::Changed {
            event_sequence: result
                .event_sequence
                .expect("changed Speed Group actions publish exactly one event"),
        },
        SpeedGroupOutcome::NoChange => wire::SpeedGroupActionState::NoChange {},
    };
    wire::SpeedGroupActionOutcome {
        request_id: result
            .context
            .request_id
            .clone()
            .expect("v2 Speed Group actions require a request ID"),
        correlation_id: result.context.correlation_id,
        authority_id: result.authority_id,
        revision: result.revision,
        applied_at_millis: result.applied_at_millis,
        groups: result.groups.into_iter().map(wire_projection).collect(),
        outcome,
        replayed: result.replayed,
        durability: match result.durability {
            light_application::SpeedGroupDurability::Durable => wire::SpeedGroupDurability::Durable,
            light_application::SpeedGroupDurability::PersistencePending => {
                wire::SpeedGroupDurability::PersistencePending
            }
        },
        warning: result.warning,
    }
}

fn validate_request_id(value: &str) -> Result<(), SpeedGroupHttpError> {
    if value.is_empty() || value.len() > 128 || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(SpeedGroupHttpError::invalid(
            "request_id must contain 1-128 printable bytes",
        ));
    }
    Ok(())
}

struct SpeedGroupHttpError {
    status: StatusCode,
    body: wire::SpeedGroupErrorResponse,
}

impl SpeedGroupHttpError {
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
            wire::SpeedGroupErrorKind::Invalid,
            message,
            None,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: wire::SpeedGroupErrorKind,
        error: impl Into<String>,
        current_revision: Option<u64>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: wire::SpeedGroupErrorResponse {
                kind,
                error: error.into(),
                current_revision,
                retryable,
            },
        }
    }
}

fn error_kind(status: StatusCode) -> wire::SpeedGroupErrorKind {
    match status {
        StatusCode::UNAUTHORIZED => wire::SpeedGroupErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => wire::SpeedGroupErrorKind::Forbidden,
        StatusCode::NOT_FOUND => wire::SpeedGroupErrorKind::NotFound,
        StatusCode::CONFLICT => wire::SpeedGroupErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => wire::SpeedGroupErrorKind::Unavailable,
        status if status.is_server_error() => wire::SpeedGroupErrorKind::Internal,
        _ => wire::SpeedGroupErrorKind::Invalid,
    }
}

impl IntoResponse for SpeedGroupHttpError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
