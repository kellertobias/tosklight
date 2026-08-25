use super::*;

pub(super) fn ws_programmer_preload_lifecycle_action(
    state: &AppState,
    command: &WsActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let request: light_wire::v2::preload_lifecycle::ProgrammingPreloadLifecycleRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::preload_lifecycle::ProgrammingPreloadLifecycleRequest,
    >(
        "/api/v2/events programmer.preload.lifecycle.action",
        &command.payload,
    );
    if request.request_id != command.request_id {
        return Err(
            "Preload lifecycle payload request_id must match the WebSocket request_id".into(),
        );
    }
    let result = state
        .programming
        .handle_preload_lifecycle(
            light_application::ActionEnvelope {
                context: context.clone(),
                command: command_http::preload_lifecycle_command(&request),
            },
            ports,
        )
        .map_err(|error| error.message)?;
    let payload = serde_json::to_value(command_http::preload_lifecycle_outcome(result))
        .map_err(|error| error.to_string())?;
    Ok(WsTypedProgrammingAction { payload })
}

pub(super) fn ws_programmer_preload_values_action(
    state: &AppState,
    command: &WsActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let request: light_wire::v2::preload_values::ProgrammingPreloadValuesActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::preload_values::ProgrammingPreloadValuesActionRequest,
    >(
        "/api/v2/events programmer.preload.values.action",
        &command.payload,
    );
    if request.request_id != command.request_id {
        return Err("Preload values payload request_id must match the WebSocket request_id".into());
    }
    let request_id = request.request_id;
    let session_id = context
        .session_id
        .map(SessionId)
        .ok_or_else(|| "Preload values require a session".to_owned())?;
    let command_action = indexed_presets::preload_action(state, session_id, request.action)
        .map_err(|error| error.message)?;
    let action = light_application::ActionEnvelope {
        context: context
            .clone()
            .with_expected_revision(request.expected_revision),
        command: light_application::ProgrammingPreloadValuesRequest {
            expected_capture_mode_revision: request.expected_capture_mode_revision,
            command: command_http::preload_values_command(command_action),
        },
    };
    let result = state
        .programming
        .handle_preload_values(action, ports)
        .map_err(|error| error.message)?;
    let payload = serde_json::to_value(command_http::preload_values_outcome(request_id, result))
        .map_err(|error| error.to_string())?;
    Ok(WsTypedProgrammingAction { payload })
}

pub(super) fn ws_programmer_command_line(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        value: String,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let was_armed = state
        .programming
        .get(session.id)
        .is_some_and(|programmer| command_line_arms_update(&programmer.command_line));
    let is_armed = command_line_arms_update(&input.value);
    state.programming.set_command_line(session.id, input.value);
    persist_programmer(state, session).map_err(|e| e.message)?;
    emit_update_armed_transition(state, session, was_armed, is_armed, "software");
    Ok(serde_json::json!({"updated":true}))
}

pub(super) fn ws_programmer_command_target(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        value: String,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    if !state
        .programming
        .set_command_target(session.id, input.value.to_ascii_uppercase())
    {
        return Err("command target must be FIXTURE or GROUP".into());
    }
    Ok(serde_json::json!({"updated":true}))
}

pub(super) fn ws_programmer_execute(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
    context: Option<&light_application::ActionContext>,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        value: String,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let context = context.cloned().unwrap_or_else(|| {
        light_application::ActionContext::operator(
            session.desk.id,
            session.id.0,
            light_application::ActionSource::UserInterface,
        )
        .with_request_id(&command.request_id)
    });
    let policy = command_http::ordered_ui_command_policy(&input.value);
    let outcome = ws_typed_recording(state, session, &input.value, &context).unwrap_or_else(|| {
        command_http::execute_existing_command(
            state,
            session,
            &input.value,
            "software",
            &context,
            policy,
        )
    });
    finish_ws_execution(state, session, &input.value, outcome)
}

fn finish_ws_execution(
    state: &AppState,
    session: &Session,
    command: &str,
    outcome: command_http::ExistingCommandOutcome,
) -> Result<serde_json::Value, String> {
    let pending_choice = match &outcome {
        command_http::ExistingCommandOutcome::ChoiceRequired { pending_choice } => {
            Some(pending_choice.clone())
        }
        command_http::ExistingCommandOutcome::Accepted { .. }
        | command_http::ExistingCommandOutcome::Rejected { .. } => None,
    };
    let replayed = matches!(
        &outcome,
        command_http::ExistingCommandOutcome::Accepted { replayed: true, .. }
    );
    let final_text = match &outcome {
        command_http::ExistingCommandOutcome::Accepted { .. } => Some(""),
        command_http::ExistingCommandOutcome::ChoiceRequired { .. }
        | command_http::ExistingCommandOutcome::Rejected { .. } => Some(command),
    };
    if !replayed {
        state
            .programming
            .complete_command_execution(session.id, final_text, pending_choice);
    }
    match outcome {
        command_http::ExistingCommandOutcome::ChoiceRequired { pending_choice } => {
            Ok(serde_json::json!({
                "applied":0,
                "pending_choice":command_http::wire_choice(pending_choice)
            }))
        }
        command_http::ExistingCommandOutcome::Accepted {
            applied,
            persistence_warning,
            ..
        } => Ok(serde_json::json!({
            "applied":applied,
            "persistence_warning":persistence_warning
        })),
        command_http::ExistingCommandOutcome::Rejected { error } => Err(error),
    }
}

fn ws_typed_recording(
    state: &AppState,
    session: &Session,
    command: &str,
    context: &light_application::ActionContext,
) -> Option<command_http::ExistingCommandOutcome> {
    let ports = command_http::ServerProgrammingPorts::new(state, session, "software", true);
    let outcome = ports.record_typed_command(
        &state.programming,
        context,
        command,
        light_application::ExecutionPolicy::Compatibility,
    )?;
    Some(match outcome {
        light_application::ProgrammingExecution::Accepted {
            applied,
            warning,
            replayed,
        } => command_http::ExistingCommandOutcome::Accepted {
            applied,
            persistence_warning: warning,
            replayed,
        },
        light_application::ProgrammingExecution::ChoiceRequired { pending_choice } => {
            command_http::ExistingCommandOutcome::ChoiceRequired { pending_choice }
        }
        light_application::ProgrammingExecution::Rejected { error } => {
            command_http::ExistingCommandOutcome::Rejected { error }
        }
    })
}
