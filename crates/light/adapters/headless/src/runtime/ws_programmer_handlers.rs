use super::*;

pub(super) fn ws_programmer_capture_mode(
    state: &AppState,
    session: &Session,
    request: light_wire::v2::live_action::ProgrammerCaptureModeLiveActionRequest,
) -> Result<light_wire::v2::live_action::ProgrammerCaptureModeOutcome, String> {
    let request_id = request.request_id;
    state.programming.set_modes(
        session.id,
        request.blind,
        request.preview,
        None,
        request.active_context,
    );
    persist_programmer(state, session).map_err(|error| error.message)?;
    let programmer = state
        .programming
        .get(session.id)
        .ok_or_else(|| "programmer not found".to_owned())?;
    Ok(light_wire::v2::live_action::ProgrammerCaptureModeOutcome {
        request_id,
        blind: programmer.blind,
        preview: programmer.preview,
        active_context: programmer.active_context.clone(),
    })
}

pub(super) fn ws_programmer_command_line_replace(
    state: &AppState,
    command: &WsActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    #[derive(Deserialize)]
    struct Input {
        request_id: String,
        expected_revision: u64,
        text: String,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<Input>(
        "/api/v2/events programmer.command_line.replace",
        &command.payload,
    );
    if input.request_id != command.request_id {
        return Err("Command-line payload request_id must match the WebSocket request_id".into());
    }
    command_http::validate_command(&input.text).map_err(|error| error.message)?;
    let result = state
        .programming
        .handle(
            light_application::ActionEnvelope {
                context: context
                    .clone()
                    .with_expected_revision(input.expected_revision),
                command: light_application::ProgrammingCommand::ReplaceCommandLine {
                    text: input.text,
                    expected_revision: input.expected_revision,
                },
            },
            ports,
        )
        .map_err(|error| error.message)?;
    if let light_application::ProgrammingOutcome::Accepted {
        warning: Some(warning),
        ..
    } = &result.outcome
    {
        return Err(warning.clone());
    }
    let payload = serde_json::to_value(command_http::command_line_from_state(result.command_line))
        .map_err(|error| error.to_string())?;
    Ok(WsTypedProgrammingAction { payload })
}

pub(super) fn ws_programmer_selection_action(
    state: &AppState,
    command: &WsActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let request: light_wire::v2::command_line::ProgrammingSelectionActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::command_line::ProgrammingSelectionActionRequest,
    >(
        "/api/v2/events programmer.selection.action",
        &command.payload,
    );
    if request.request_id != command.request_id {
        return Err("Selection payload request_id must match the WebSocket request_id".into());
    }
    command_http::validate_selection_request(&request).map_err(|error| error.message)?;
    let request_id = request.request_id;
    let command = command_http::selection_command(request.action).map_err(|error| error.message)?;
    let result = state
        .programming
        .handle(
            light_application::ActionEnvelope {
                context: context.clone(),
                command,
            },
            ports,
        )
        .map_err(|error| error.message)?;
    let payload = serde_json::to_value(
        command_http::selection_response(request_id, result).map_err(|error| error.message)?,
    )
    .map_err(|error| error.to_string())?;
    Ok(WsTypedProgrammingAction { payload })
}

pub(super) fn ws_programmer_values_action(
    state: &AppState,
    command: &WsActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let request: light_wire::v2::programming::ProgrammingValuesActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::programming::ProgrammingValuesActionRequest,
    >("/api/v2/events programmer.values.action", &command.payload);
    if request.request_id != command.request_id {
        return Err(
            "Programmer values payload request_id must match the WebSocket request_id".into(),
        );
    }
    let request_id = request.request_id;
    let context = context
        .clone()
        .with_expected_revision(request.expected_revision);
    let colors = command_http::color_attribute_index(state);
    let command =
        command_http::values_command(request.action, &colors).map_err(|error| error.message)?;
    let result = state
        .programming
        .handle_values(
            light_application::ActionEnvelope {
                context,
                command: light_application::ProgrammingValuesRequest {
                    expected_capture_mode_revision: request.expected_capture_mode_revision,
                    command,
                },
            },
            ports,
        )
        .map_err(|error| error.message)?;
    let payload = serde_json::to_value(command_http::values_outcome(request_id, result))
        .map_err(|error| error.to_string())?;
    Ok(WsTypedProgrammingAction { payload })
}

pub(super) fn ws_programmer_priority_action(
    state: &AppState,
    command: &WsActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let request: light_wire::v2::programmer_priority::ProgrammerPriorityActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::programmer_priority::ProgrammerPriorityActionRequest,
    >(
        "/api/v2/events programmer.priority.action",
        &command.payload,
    );
    if request.request_id != command.request_id {
        return Err("Priority payload request_id must match the WebSocket request_id".into());
    }
    let result = state
        .programming
        .handle_priority(
            light_application::ActionEnvelope {
                context: context.clone(),
                command: light_application::ProgrammingPriorityRequest {
                    expected_revision:
                        light_application::ProgrammingPriorityRevisionExpectation::Exact(
                            request.expected_revision,
                        ),
                    priority: request.priority,
                },
            },
            ports,
        )
        .map_err(|error| error.message)?;
    let payload = serde_json::to_value(command_http::programmer_priority_outcome(result))
        .map_err(|error| error.to_string())?;
    Ok(WsTypedProgrammingAction { payload })
}

pub(super) struct WsControlActionResult {
    pub(super) payload: serde_json::Value,
}

pub(super) fn ws_programmer_control_action(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
) -> Result<WsControlActionResult, String> {
    #[derive(Deserialize)]
    struct Input {
        fixture_id: light_core::FixtureId,
        action_id: Uuid,
        active: bool,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let snapshot = state.output.snapshot();
    let (assignments, pulse_duration, kind) = control_action_programmer_values(
        &snapshot,
        input.fixture_id,
        input.action_id,
        input.active,
    )?;
    let transient_source = format!("fixture-control:{}:{}", input.fixture_id.0, input.action_id);
    let transient_generation = match (kind, input.active) {
        (light_fixture::ControlActionKind::Latched, _) => {
            state.programming.set_many(session.id, assignments);
            persist_programmer(state, session).map_err(|e| e.message)?;
            None
        }
        (_, true) => state.programming.set_transient_action(
            session.id,
            transient_source.clone(),
            assignments,
        ),
        (_, false) => {
            state
                .programming
                .release_transient_action(session.id, &transient_source, None);
            None
        }
    };
    if let (Some(duration_millis), Some(generation)) = (pulse_duration, transient_generation) {
        let task_state = state.clone();
        let task_session = session.clone();
        let task_source = transient_source.clone();
        let lifecycle = task_state.lifecycle.clone();
        if let Err(error) = lifecycle.schedule(async move {
            tokio::time::sleep(Duration::from_millis(duration_millis)).await;
            if task_state.programming.release_transient_action(
                task_session.id,
                &task_source,
                Some(generation),
            ) {
                emit(
                    &task_state,
                    "programmer_changed",
                    serde_json::json!({
                        "session_id":task_session.id,
                        "user_id":task_session.user.id,
                        "desk_id":task_session.desk.id,
                        "command":"programmer.control_action",
                        "changes":["transient_control"],
                        "action_id":input.action_id,
                        "active":false,
                        "timed_pulse_complete":true,
                    }),
                );
            }
            Ok(())
        }) {
            state.programming.release_transient_action(
                session.id,
                &transient_source,
                Some(generation),
            );
            return Err(error.to_string());
        }
    }
    Ok(WsControlActionResult {
        payload: serde_json::json!({
            "action_id":input.action_id,
            "active":input.active,
            "kind":kind,
            "pulse_duration_millis":pulse_duration,
            "programmer":state.programming.get(session.id),
        }),
    })
}
