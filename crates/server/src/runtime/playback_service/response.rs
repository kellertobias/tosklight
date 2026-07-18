use super::super::{ApiError, AppState, Session, authoritative_playback_controls, emit};
use super::conversion::pending_name;
use light_application::{
    PlaybackAction, PlaybackAddress, PlaybackExecution, PlaybackResult, ResolvedPlaybackAddress,
};
use light_core::CueListId;

pub(in crate::runtime) fn cue_list_http_payload(
    result: PlaybackResult,
) -> Result<serde_json::Value, ApiError> {
    match result.execution {
        PlaybackExecution::Active(active) => {
            serde_json::to_value(active).map_err(|error| ApiError::internal(error.to_string()))
        }
        PlaybackExecution::ActiveList(active) => {
            serde_json::to_value(active).map_err(|error| ApiError::internal(error.to_string()))
        }
        PlaybackExecution::Released(released) => Ok(serde_json::json!({"released":released})),
        PlaybackExecution::Pool { .. } => {
            Err(ApiError::internal("cue-list action returned a pool result"))
        }
    }
}

pub(in crate::runtime) fn pool_http_payload(
    state: &AppState,
    session: &Session,
    action_name: &str,
    result: PlaybackResult,
) -> Result<serde_json::Value, ApiError> {
    let number = playback_number(result.resolved)?;
    let definition = state
        .engine
        .snapshot()
        .playbacks
        .iter()
        .find(|playback| playback.number == number)
        .cloned()
        .ok_or_else(|| ApiError::not_found("playback"))?;
    let PlaybackExecution::Pool { changed, pending } = result.execution else {
        return Err(ApiError::internal("pool action returned a cue-list result"));
    };
    if let Some(pending) = pending {
        return Ok(serde_json::json!({
            "pending":true,
            "action":pending_name(pending),
            "playback":definition
        }));
    }
    if changed {
        emit(
            state,
            "playback_changed",
            serde_json::json!({"playback_number":number,"action":action_name,"session_id":session.id}),
        );
    }
    let snapshot = state.engine.snapshot();
    Ok(serde_json::json!({
        "playback":definition,
        "active":state.engine.playback().read().runtime_status(),
        "groups":snapshot.groups,
        "authoritative_controls":authoritative_playback_controls(state),
        "changed":changed
    }))
}

pub(in crate::runtime) fn websocket_payload(
    state: &AppState,
    session: &Session,
    command_name: &str,
    cue_list_id: CueListId,
    request_id: &str,
) -> Result<serde_json::Value, String> {
    let action = match command_name {
        "playback.go" => PlaybackAction::Go { pressed: true },
        "playback.back" => PlaybackAction::Back { pressed: true },
        "playback.pause" => PlaybackAction::Pause { pressed: true },
        _ => PlaybackAction::Release,
    };
    let result = super::websocket_action(
        state,
        session,
        PlaybackAddress::CueList(cue_list_id),
        action,
        request_id,
    )
    .map_err(|error| error.message)?;
    match result.execution {
        PlaybackExecution::Active(active) => {
            serde_json::to_value(active).map_err(|error| error.to_string())
        }
        PlaybackExecution::ActiveList(_) => Ok(serde_json::json!({"paused":true})),
        PlaybackExecution::Released(released) => Ok(serde_json::json!({"released":released})),
        PlaybackExecution::Pool { .. } => Err("cue-list action returned a pool result".to_owned()),
    }
}

fn playback_number(address: ResolvedPlaybackAddress) -> Result<u16, ApiError> {
    address
        .playback_number()
        .ok_or_else(|| ApiError::internal("pool action resolved to a cue list"))
}
