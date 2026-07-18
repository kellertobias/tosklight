use super::*;

pub(super) fn ws_master_set(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    let input: MasterInput =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    lock_live_input(state, session, "desk:master".into())?;
    let mut control = state.output_control.lock();
    if let Some(level) = input.grand_master {
        if !level.is_finite() || !(0.0..=1.0).contains(&level) {
            return Err("grand_master must be within 0-1".into());
        }
        control.options.grand_master = level;
    }
    if let Some(blackout) = input.blackout {
        control.options.blackout = blackout;
    }
    let result = serde_json::json!({"grand_master":control.options.grand_master,"blackout":control.options.blackout});
    drop(control);
    persist_output_runtime(state).map_err(|error| error.message)?;
    Ok(result)
}

pub(super) fn ws_group_master_set(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        group_id: String,
        value: f32,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    if !input.value.is_finite() || !(0.0..=1.0).contains(&input.value) {
        return Err("group master must be within 0-1".into());
    }
    lock_live_input(state, session, format!("group-master:{}", input.group_id))?;
    let mut snapshot = (*state.engine.snapshot()).clone();
    let group = snapshot
        .groups
        .iter_mut()
        .find(|group| group.id == input.group_id)
        .ok_or("group does not exist")?;
    group.master = input.value;
    state
        .engine
        .replace_snapshot(snapshot)
        .map_err(|error| error.to_string())?;
    persist_output_runtime(state).map_err(|error| error.message)?;
    Ok(serde_json::json!({"group_id":input.group_id,"master":input.value}))
}

pub(super) fn ws_group_master_flash(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        group_id: String,
        value: f32,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    if !input.value.is_finite() || !(0.0..=1.0).contains(&input.value) {
        return Err("group flash must be within 0-1".into());
    }
    if !state
        .engine
        .snapshot()
        .groups
        .iter()
        .any(|group| group.id == input.group_id)
    {
        return Err("group does not exist".into());
    }
    lock_live_input(state, session, format!("group-flash:{}", input.group_id))?;
    state
        .engine
        .set_group_master_flash(input.group_id.clone(), input.value);
    Ok(serde_json::json!({"group_id":input.group_id,"flash":input.value}))
}

pub(super) fn ws_playback_go(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        cue_list_id: light_core::CueListId,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    playback_service::websocket_payload(
        state,
        session,
        &command.command,
        input.cue_list_id,
        &command.request_id,
    )
}
