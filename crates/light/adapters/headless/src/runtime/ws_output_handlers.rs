use super::*;

pub(super) struct WsTypedRuntimeAction {
    pub(super) payload: serde_json::Value,
}

pub(super) fn ws_speed_group_action(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
) -> Result<WsTypedRuntimeAction, String> {
    let request: light_wire::v2::speed_group::SpeedGroupActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::speed_group::SpeedGroupActionRequest,
    >("/api/v2/events speed_group.action", &command.payload);
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
        .active_show
        .try_acquire()
        .map_err(|_| "the active show is changing; retry the Speed Group action")?;
    state.programming.run_desk_operation(session.desk.id, || {
        let context = light_application::ActionContext::operator(
            session.desk.id,
            session.id.0,
            light_application::ActionSource::UserInterface,
        )
        .with_request_id(&request.request_id);
        let result = speed_group_service::execute_http_action(state, session, context, exact)
            .map_err(|error| error.message)?;
        let payload = serde_json::to_value(speed_group_v2::wire_outcome(result))
            .map_err(|error| error.to_string())?;
        Ok(WsTypedRuntimeAction { payload })
    })
}

pub(super) fn ws_output_runtime_action(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
) -> Result<WsTypedRuntimeAction, String> {
    let request: light_wire::v2::output_runtime::OutputRuntimeActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::output_runtime::OutputRuntimeActionRequest,
    >("/api/v2/events output_runtime.action", &command.payload);
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
        .active_show
        .try_acquire()
        .map_err(|_| "the active show is changing; retry the output action")?;
    state.programming.run_desk_operation(session.desk.id, || {
        if read_desk_lock(state).locked {
            return Err("desk is locked".into());
        }
        let context = light_application::ActionContext::operator(
            session.desk.id,
            session.id.0,
            light_application::ActionSource::UserInterface,
        )
        .with_request_id(&request.request_id);
        let result = output_runtime_service::execute_action(state, Some(session), context, exact)
            .map_err(|error| error.message)?;
        let payload = serde_json::to_value(output_runtime_v2::wire_outcome(result))
            .map_err(|error| error.to_string())?;
        Ok(WsTypedRuntimeAction { payload })
    })
}

pub(super) fn ws_dmx_override(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
) -> Result<serde_json::Value, String> {
    let input: light_wire::v2::output_control::DmxOverrideRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::output_control::DmxOverrideRequest,
    >("/api/v2/events dmx.override", &command.payload);
    if input.request_id != command.request_id {
        return Err("DMX override payload request_id must match the WebSocket request_id".into());
    }
    output_runtime_v2::validate_request_id(&input.request_id)?;
    apply_dmx_override(state, session, input)
        .map(|Json(value)| value)
        .map_err(|error| error.message)
}

pub(super) fn ws_highlight_action(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
) -> Result<serde_json::Value, String> {
    let input: light_wire::v2::output_control::HighlightActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::output_control::HighlightActionRequest,
    >("/api/v2/events highlight.action", &command.payload);
    if input.request_id != command.request_id {
        return Err("Highlight payload request_id must match the WebSocket request_id".into());
    }
    output_runtime_v2::validate_request_id(&input.request_id)?;
    let state = apply_highlight_action(
        state,
        session,
        highlight_action_from_wire(input.action),
        light_application::ActionSource::UserInterface,
    )
    .map_err(|error| error.message)?;
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub(super) fn ws_patch_preview_highlight(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
) -> Result<serde_json::Value, String> {
    let input: light_wire::v2::output_control::PatchPreviewHighlightRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::output_control::PatchPreviewHighlightRequest,
    >(
        "/api/v2/events patch_preview_highlight.action",
        &command.payload,
    );
    if input.request_id != command.request_id {
        return Err(
            "Patch Preview Highlight payload request_id must match the WebSocket request_id".into(),
        );
    }
    output_runtime_v2::validate_request_id(&input.request_id)?;
    let _activation = state
        .active_show
        .try_acquire()
        .map_err(|_| "the active show is changing; retry Patch Preview Highlight")?;
    Ok(apply_patch_preview_highlight(state, session, input))
}
