use super::*;

pub(super) fn ws_programmer_clear(
    state: &AppState,
    session: &Session,
    _command: &WsCommand,
) -> Result<serde_json::Value, String> {
    state.programmers.clear_values(session.id);
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(serde_json::json!({"cleared":true}))
}

pub(super) fn ws_preload_enter(
    state: &AppState,
    _session: &Session,
    _command: &WsCommand,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let result = typed_preload_action(
        state,
        context,
        ports,
        light_application::ProgrammingPreloadLifecycleAction::Enter,
    )?;
    Ok(compatibility_action(
        serde_json::json!({"blind":true}),
        result,
    ))
}

pub(super) fn ws_preload_go(
    state: &AppState,
    session: &Session,
    _command: &WsCommand,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let show_id = state
        .active_show
        .read()
        .as_ref()
        .map(|show| show.id)
        .ok_or("no active show is loaded")?;
    let current = light_application::ProgrammingPreloadRevisionExpectation::Current;
    let result = typed_preload_action(
        state,
        context,
        ports,
        light_application::ProgrammingPreloadLifecycleAction::Go {
            show_id,
            expected_show_revision: current,
            expected_playback_event_sequence: current,
        },
    )?;
    let payload = compatibility_go_payload(&result)?;
    if !result.replayed
        && result.state == light_application::ProgrammingPreloadLifecycleState::Changed
    {
        emit_compatibility_go(state, session, &payload);
    }
    Ok(compatibility_action(payload, result))
}

pub(super) fn ws_preload_group_set(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        group_id: String,
        attribute: String,
        value: f32,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    if !(0.0..=1.0).contains(&input.value) {
        return Err("value must be within 0-1".into());
    }
    state.programmers.set_group(
        session.id,
        input.group_id,
        light_core::AttributeKey(input.attribute),
        light_core::AttributeValue::Normalized(input.value),
    );
    persist_programmer(state, session).map_err(|e| e.message)?;
    let programmer = state.programmers.get(session.id);
    let pending = programmer
        .as_ref()
        .is_some_and(|programmer| programmer.blind && programmer.preload_capture_programmer);
    Ok(serde_json::json!({"pending":pending,"programmer":programmer}))
}

pub(super) fn ws_preload_clear(
    state: &AppState,
    _session: &Session,
    _command: &WsCommand,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let result = typed_preload_action(
        state,
        context,
        ports,
        light_application::ProgrammingPreloadLifecycleAction::ClearPending,
    )?;
    Ok(compatibility_action(
        serde_json::json!({"pending_cleared":true,"active_unchanged":true}),
        result,
    ))
}

pub(super) fn ws_preload_release(
    state: &AppState,
    _session: &Session,
    _command: &WsCommand,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let result = typed_preload_action(
        state,
        context,
        ports,
        light_application::ProgrammingPreloadLifecycleAction::Release,
    )?;
    let released = result.state == light_application::ProgrammingPreloadLifecycleState::Changed;
    Ok(compatibility_action(
        serde_json::json!({"released":released}),
        result,
    ))
}

fn typed_preload_action(
    state: &AppState,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
    action: light_application::ProgrammingPreloadLifecycleAction,
) -> Result<light_application::ProgrammingPreloadLifecycleResult, String> {
    let current = light_application::ProgrammingPreloadRevisionExpectation::Current;
    state
        .programming
        .handle_preload_lifecycle(
            light_application::ActionEnvelope {
                context: context.clone(),
                command: light_application::ProgrammingPreloadLifecycleRequest {
                    expected_capture_mode_revision: current,
                    expected_values_revision: current,
                    expected_queue_revision: current,
                    expected_selection_revision: current,
                    action,
                },
            },
            ports,
        )
        .map_err(|error| error.message)
}

pub(super) fn ws_programmer_command_line(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        value: String,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let was_armed = state
        .programmers
        .get(session.id)
        .is_some_and(|programmer| command_line_arms_update(&programmer.command_line));
    let is_armed = command_line_arms_update(&input.value);
    state.programmers.set_command_line(session.id, input.value);
    persist_programmer(state, session).map_err(|e| e.message)?;
    emit_update_armed_transition(state, session, was_armed, is_armed, "software");
    Ok(serde_json::json!({"updated":true}))
}

pub(super) fn ws_programmer_command_target(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        value: String,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    if !state
        .programmers
        .set_command_target(session.id, input.value.to_ascii_uppercase())
    {
        return Err("command target must be FIXTURE or GROUP".into());
    }
    Ok(serde_json::json!({"updated":true}))
}

pub(super) fn ws_programmer_execute(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
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
            session.user.id.0,
            session.id.0,
            light_application::ActionSource::UserInterface,
        )
        .with_request_id(&command.request_id)
    });
    let outcome = ws_typed_recording(state, session, &input.value, &context).unwrap_or_else(|| {
        command_http::execute_existing_command(
            state,
            session,
            &input.value,
            "software",
            &context,
            command_http::ExistingCommandPolicy::Compatibility,
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
            .programmers
            .complete_command_execution(session.id, final_text, pending_choice);
    }
    match outcome {
        command_http::ExistingCommandOutcome::ChoiceRequired { pending_choice } => {
            Ok(serde_json::json!({
                "applied":0,
                "pending_choice":command_http::wire_choice(pending_choice),
                "programmer":state.programmers.get(session.id)
            }))
        }
        command_http::ExistingCommandOutcome::Accepted {
            applied,
            persistence_warning,
            ..
        } => Ok(serde_json::json!({
            "applied":applied,
            "persistence_warning":persistence_warning,
            "programmer":state.programmers.get(session.id)
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
        &state.programmers,
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
