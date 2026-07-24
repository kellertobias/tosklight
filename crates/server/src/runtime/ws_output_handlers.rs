use super::*;

pub(super) struct WsTypedRuntimeAction {
    pub(super) payload: serde_json::Value,
    pub(super) replayed: bool,
}

pub(super) fn ws_speed_group_action(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<WsTypedRuntimeAction, String> {
    let request: light_wire::v2::speed_group::SpeedGroupActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::speed_group::SpeedGroupActionRequest,
    >("/api/v1/events speed_group.action", &command.payload);
    if request.request_id != command.request_id {
        return Err("Speed Group payload request_id must match the WebSocket request_id".into());
    }
    speed_group_v2::validate_request_id(&request.request_id)?;
    let action =
        speed_group_v2::application_action(request.action).map_err(|error| error.message)?;
    let exact = speed_group_service::exact_command(
        request.expected_authority_id,
        request.expected_revision,
        action,
    );
    let _activation = state
        .activation_lock
        .clone()
        .try_lock_owned()
        .map_err(|_| "the active show is changing; retry the Speed Group action")?;
    let desk_operation = state.programming.desk_lock(session.desk.id);
    let _desk_operation = desk_operation.lock();
    let context = light_application::ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        light_application::ActionSource::UserInterface,
    )
    .with_request_id(&request.request_id);
    let result = speed_group_service::execute_http_action(state, session, context, exact)
        .map_err(|error| error.message)?;
    let replayed = result.replayed;
    let payload = serde_json::to_value(speed_group_v2::wire_outcome(result))
        .map_err(|error| error.to_string())?;
    Ok(WsTypedRuntimeAction { payload, replayed })
}

pub(super) fn ws_output_runtime_action(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<WsTypedRuntimeAction, String> {
    let request: light_wire::v2::output_runtime::OutputRuntimeActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::output_runtime::OutputRuntimeActionRequest,
    >("/api/v1/events output_runtime.action", &command.payload);
    if request.request_id != command.request_id {
        return Err("Output runtime payload request_id must match the WebSocket request_id".into());
    }
    output_runtime_v2::validate_request_id(&request.request_id)?;
    let exact = output_runtime_service::exact_command(
        request.expected_show_id,
        request.expected_revision,
        request.grand_master,
        request.blackout,
    )
    .map_err(|error| error.message)?;
    let _activation = state
        .activation_lock
        .clone()
        .try_lock_owned()
        .map_err(|_| "the active show is changing; retry the output action")?;
    let desk_operation = state.programming.desk_lock(session.desk.id);
    let _desk_operation = desk_operation.lock();
    if read_desk_lock(state, session.desk.id).locked {
        return Err("desk is locked".into());
    }
    let context = light_application::ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        light_application::ActionSource::UserInterface,
    )
    .with_request_id(&request.request_id);
    let result = output_runtime_service::execute_action(state, Some(session), context, exact)
        .map_err(|error| error.message)?;
    let replayed = result.replayed;
    let payload = serde_json::to_value(output_runtime_v2::wire_outcome(result))
        .map_err(|error| error.to_string())?;
    Ok(WsTypedRuntimeAction { payload, replayed })
}

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
    let _activation = state
        .activation_lock
        .clone()
        .try_lock_owned()
        .map_err(|_| "the active show is changing; retry the Group master action")?;
    lock_live_input(state, session, format!("group-master:{}", input.group_id))?;
    let changed = state
        .engine
        .set_group_master(&input.group_id, input.value)
        .map_err(|error| error.to_string())?;
    if changed {
        persist_output_runtime(state).map_err(|error| error.message)?;
    }
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
