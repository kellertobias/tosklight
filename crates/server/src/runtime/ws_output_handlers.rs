use super::*;

pub(super) fn ws_master_set(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    let input: MasterInput =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    lock_live_input(state, session, "desk:master".into())?;
    let output = output_runtime_service::command(input.grand_master, input.blackout)
        .map_err(|error| error.message)?;
    let context = light_application::ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        light_application::ActionSource::UserInterface,
    )
    .with_request_id(&command.request_id);
    let outcome =
        output_runtime_service::execute_while_show_stable(state, Some(session), context, output)
            .map_err(|error| error.message)?;
    Ok(serde_json::json!({
        "grand_master":outcome.projection.grand_master,
        "blackout":outcome.projection.blackout
    }))
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
