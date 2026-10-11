use super::*;

pub(super) fn ws_playback_action(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
    context: Option<&light_application::ActionContext>,
    socket: Option<&software_playback_sources::SoftwarePlaybackSocket>,
) -> Result<serde_json::Value, String> {
    let request: light_wire::v2::playback::PlaybackActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<light_wire::v2::playback::PlaybackActionRequest>(
        "/api/v2/events playback.action",
        &command.payload,
    );
    let application_command = playback_v2::application_command(request)?;
    let context = context
        .cloned()
        .ok_or_else(|| "playback action requires an interaction context".to_owned())?;
    let result = match socket {
        Some(socket) => playback_service::execute_for_socket(
            state,
            Some(session),
            Some(&session.desk),
            socket,
            context,
            application_command,
        ),
        None => playback_service::execute(
            state,
            Some(session),
            Some(&session.desk),
            context,
            application_command,
        ),
    }
    .map_err(|error| error.message)?;
    serde_json::to_value(playback_v2::action_outcome(result)).map_err(|error| error.to_string())
}
