use super::adapter::{run_service, run_snapshot};
use super::events::publish_service_result;
use super::interaction_wire::interaction_snapshot;
use super::wire::{
    command_key, command_key_phase, command_line_from_state, operation_response, with_etag,
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use light_application::{
    ActionContext, ActionSource, ExecutionPolicy, ProgrammingCommand, ProgrammingResult,
};
use light_programmer::CommandLineState;
use light_wire::v2::command_line::{
    CommandKeyRequest, CommandLineResponse, ExecuteCommandLineRequest, ReplaceCommandLineRequest,
};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session};

const REQUEST_ID_LIMIT: usize = 128;
const COMMAND_LINE_LIMIT: usize = 16 * 1024;

pub(crate) fn router() -> Router<AppState> {
    let command_line = Router::new()
        .route(
            "/api/v2/desks/{desk_id}/command-line",
            get(get_command_line).put(put_command_line),
        )
        .route(
            "/api/v2/desks/{desk_id}/command-line/keys",
            post(apply_command_key),
        )
        .route(
            "/api/v2/desks/{desk_id}/command-line/execute",
            post(execute_command_line),
        )
        .route(
            "/api/v2/desks/{desk_id}/programming-interaction/snapshot",
            get(get_programming_interaction),
        )
        .layer(DefaultBodyLimit::max(32 * 1024));
    command_line
        .merge(super::lifecycle_routes::router())
        .merge(super::preload_playback_queue_routes::router())
        .merge(super::preload_values_routes::router())
        .merge(super::preset_recording_routes::router())
        .merge(super::selection_routes::router())
        .merge(super::values_routes::router())
}

async fn get_programming_interaction(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = authenticate_desk(&state, &headers, desk_id)?;
    let snapshot = run_snapshot(&state, &session, http_context(&session, None))?;
    Ok(Json(interaction_snapshot(snapshot)).into_response())
}

async fn get_command_line(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = authenticate_desk(&state, &headers, desk_id)?;
    let response = command_line_response(&state, &session)?;
    Ok(with_etag(response))
}

async fn put_command_line(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReplaceCommandLineRequest>,
) -> Result<Response, ApiError> {
    validate_command(&input.text)?;
    let session = authenticate_desk_mutation(&state, &headers, desk_id)?;
    let expected_revision = super::super::parse_if_match(&headers)?;
    let context = http_context(&session, None).with_expected_revision(expected_revision);
    let result = run_service(
        &state,
        &session,
        context,
        ProgrammingCommand::ReplaceCommandLine {
            text: input.text,
            expected_revision,
        },
    )?;
    if let Some(warning) = publish_service_result(&state, &session, &result, "http", None, None) {
        return Err(ApiError::internal(warning));
    }
    Ok(with_etag(command_line_from_state(result.command_line)))
}

async fn apply_command_key(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CommandKeyRequest>,
) -> Result<Response, ApiError> {
    validate_request_id(&input.request_id)?;
    let session = authenticate_desk_mutation(&state, &headers, desk_id)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let context = http_context(&session, Some(&input.request_id));
    let result = run_service(
        &state,
        &session,
        context,
        ProgrammingCommand::ApplyKey {
            key: command_key(input.key),
            phase: command_key_phase(input.phase),
            execute_policy: ExecutionPolicy::AtomicProgrammer,
        },
    )?;
    if let Some(warning) = publish_service_result(
        &state,
        &session,
        &result,
        "http_key",
        Some(&input.request_id),
        None,
    ) {
        return Err(ApiError::internal(warning));
    }
    respond(input.request_id, result)
}

async fn execute_command_line(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ExecuteCommandLineRequest>,
) -> Result<Response, ApiError> {
    validate_request_id(&input.request_id)?;
    if let Some(command) = &input.command {
        validate_command(command)?;
    }
    let session = authenticate_desk_mutation(&state, &headers, desk_id)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let context = http_context(&session, Some(&input.request_id));
    let result = run_service(
        &state,
        &session,
        context,
        ProgrammingCommand::Execute {
            command: input.command.clone(),
            policy: ExecutionPolicy::AtomicProgrammer,
        },
    )?;
    if let Some(warning) = publish_service_result(
        &state,
        &session,
        &result,
        "http",
        Some(&input.request_id),
        input.command.as_deref(),
    ) {
        return Err(ApiError::internal(warning));
    }
    respond(input.request_id, result)
}

fn respond(request_id: String, result: ProgrammingResult) -> Result<Response, ApiError> {
    let response = operation_response(request_id, result)?;
    Ok(with_etag(response))
}

pub(super) fn http_context(session: &Session, request_id: Option<&str>) -> ActionContext {
    let context = ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Http,
    );
    request_id.map_or(context.clone(), |id| context.with_request_id(id))
}

fn authenticate_desk(
    state: &AppState,
    headers: &HeaderMap,
    desk_id: Uuid,
) -> Result<Session, ApiError> {
    let session = super::super::authenticate(state, headers)?;
    if session.desk.id != desk_id {
        return Err(ApiError::forbidden(
            "the authenticated session does not belong to this desk",
        ));
    }
    Ok(session)
}

pub(super) fn authenticate_desk_mutation(
    state: &AppState,
    headers: &HeaderMap,
    desk_id: Uuid,
) -> Result<Session, ApiError> {
    let session = authenticate_desk(state, headers, desk_id)?;
    ensure_desk_unlocked(state, desk_id)?;
    Ok(session)
}

fn ensure_desk_unlocked(state: &AppState, desk_id: Uuid) -> Result<(), ApiError> {
    if super::super::read_desk_lock(state, desk_id).locked {
        Err(ApiError::conflict("desk is locked"))
    } else {
        Ok(())
    }
}

fn command_state(state: &AppState, session: &Session) -> Result<CommandLineState, ApiError> {
    state
        .programmers
        .command_line_state(session.id)
        .ok_or_else(|| ApiError::not_found("programmer command line"))
}

fn command_line_response(
    state: &AppState,
    session: &Session,
) -> Result<CommandLineResponse, ApiError> {
    command_state(state, session).map(command_line_from_state)
}

pub(super) fn validate_request_id(request_id: &str) -> Result<(), ApiError> {
    if request_id.trim().is_empty()
        || request_id.len() > REQUEST_ID_LIMIT
        || request_id.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request(
            "request_id must contain 1-128 printable bytes",
        ));
    }
    Ok(())
}

fn validate_command(command: &str) -> Result<(), ApiError> {
    if command.len() > COMMAND_LINE_LIMIT {
        return Err(ApiError::bad_request(
            "command line must not exceed 16384 bytes",
        ));
    }
    Ok(())
}
