use super::super::{AppState, Session};
use light_application::{PlaybackAction, PlaybackAddress, PlaybackExecution};
use light_core::CueListId;

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
        PlaybackExecution::ActiveList { .. } => Ok(serde_json::json!({"paused":true})),
        PlaybackExecution::Released(released) => Ok(serde_json::json!({"released":released})),
        PlaybackExecution::Pool { .. } => Err("cue-list action returned a pool result".to_owned()),
        PlaybackExecution::Target { .. } => {
            Err("cue-list action returned a target result".to_owned())
        }
    }
}
