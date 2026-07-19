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
    session: &Session,
    _command: &WsCommand,
) -> Result<serde_json::Value, String> {
    let capture_programmer = state.configuration.read().preload_programmer_changes;
    state
        .programmers
        .arm_preload(session.id, capture_programmer);
    persist_programmer(state, session).map_err(|e| e.message)?;
    emit(
        state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id,"preload_armed":true}),
    );
    Ok(serde_json::json!({"blind":true}))
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
    session: &Session,
    _command: &WsCommand,
) -> Result<serde_json::Value, String> {
    state.programmers.clear_preload_pending(session.id);
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(serde_json::json!({"pending_cleared":true,"active_unchanged":true}))
}

pub(super) fn ws_preload_release(
    state: &AppState,
    session: &Session,
    _command: &WsCommand,
) -> Result<serde_json::Value, String> {
    let released = state.programmers.release_preload(session.id);
    if released {
        persist_programmer(state, session).map_err(|e| e.message)?;
        emit(
            state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id,"preload_released":true}),
        );
    }
    Ok(serde_json::json!({"released":released}))
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
    match command_http::execute_existing_command(
        state,
        session,
        &input.value,
        "software",
        &context,
        command_http::ExistingCommandPolicy::Compatibility,
    ) {
        command_http::ExistingCommandOutcome::ChoiceRequired { pending_choice } => {
            Ok(serde_json::json!({
                "applied":0,
                "pending_choice":pending_choice,
                "programmer":state.programmers.get(session.id)
            }))
        }
        command_http::ExistingCommandOutcome::Accepted {
            applied,
            persistence_warning,
        } => Ok(serde_json::json!({
            "applied":applied,
            "persistence_warning":persistence_warning,
            "programmer":state.programmers.get(session.id)
        })),
        command_http::ExistingCommandOutcome::Rejected { error } => Err(error),
    }
}
