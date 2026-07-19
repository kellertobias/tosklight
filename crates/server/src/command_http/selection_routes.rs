use super::{
    adapter::run_service,
    events::publish_service_result,
    routes::{authenticate_desk_mutation, http_context, validate_request_id},
    selection_wire::{selection_command, selection_response},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::post,
};
use light_wire::v2::command_line::{ProgrammingSelectionAction, ProgrammingSelectionActionRequest};
use uuid::Uuid;

use super::super::{ApiError, AppState};

const FIXTURE_LIMIT: usize = 10_000;
const GROUP_ID_LIMIT: usize = 256;
const BODY_LIMIT: usize = 512 * 1024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/desks/{desk_id}/programming-selection/actions",
            post(apply_selection_action),
        )
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn apply_selection_action(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ProgrammingSelectionActionRequest>,
) -> Result<Response, ApiError> {
    validate_request(&input)?;
    let session = authenticate_desk_mutation(&state, &headers, desk_id)?;
    let context = http_context(&session, Some(&input.request_id));
    let command = selection_command(input.action)?;
    let activation = state.activation_lock.clone().lock_owned().await;
    let worker_state = state.clone();
    let worker_session = session.clone();
    let (result, _activation) = tokio::task::spawn_blocking(move || {
        (
            run_service(&worker_state, &worker_session, context, command),
            activation,
        )
    })
    .await
    .map_err(|error| ApiError::internal(format!("selection service task failed: {error}")))?;
    let result = result?;
    publish_service_result(
        &state,
        &session,
        &result,
        "http_selection",
        Some(&input.request_id),
        None,
    );
    Ok(Json(selection_response(input.request_id, result)?).into_response())
}

fn validate_request(input: &ProgrammingSelectionActionRequest) -> Result<(), ApiError> {
    validate_request_id(&input.request_id)?;
    match &input.action {
        ProgrammingSelectionAction::Replace { fixtures, .. } if fixtures.len() > FIXTURE_LIMIT => {
            Err(ApiError::bad_request(
                "selection replacement must not exceed 10000 fixtures",
            ))
        }
        ProgrammingSelectionAction::Gesture { source, .. } => validate_source(source),
        ProgrammingSelectionAction::SelectGroup { group_id, .. } => validate_group_id(group_id),
        _ => Ok(()),
    }
}

fn validate_source(
    source: &light_wire::v2::command_line::ProgrammingSelectionGestureSource,
) -> Result<(), ApiError> {
    match source {
        light_wire::v2::command_line::ProgrammingSelectionGestureSource::Fixture { .. } => Ok(()),
        light_wire::v2::command_line::ProgrammingSelectionGestureSource::LiveGroup { group_id }
        | light_wire::v2::command_line::ProgrammingSelectionGestureSource::DereferencedGroup {
            group_id,
        } => validate_group_id(group_id),
    }
}

fn validate_group_id(group_id: &str) -> Result<(), ApiError> {
    if group_id.trim().is_empty()
        || group_id.len() > GROUP_ID_LIMIT
        || group_id.chars().any(char::is_control)
    {
        Err(ApiError::bad_request(
            "group_id must contain 1-256 printable bytes",
        ))
    } else {
        Ok(())
    }
}
