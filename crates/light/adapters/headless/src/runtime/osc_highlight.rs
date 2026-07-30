use super::*;

pub(super) fn osc_pressed(arguments: &[OscArgument]) -> bool {
    arguments
        .first()
        .map(|v| match v {
            OscArgument::Bool(v) => *v,
            OscArgument::Int(v) => *v != 0,
            OscArgument::Float(v) => *v > 0.0,
            OscArgument::String(v) => v != "0" && v != "false",
        })
        .unwrap_or(true)
}

pub(super) fn handle_highlight_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    if parts.len() != 4 || parts[0] != "light" || parts[2] != "highlight" {
        return;
    }
    let pressed = osc_pressed(arguments);
    let action = match parts[3] {
        "on" => HighlightAction::On,
        "off" => HighlightAction::Off,
        "toggle" => HighlightAction::Toggle,
        "next" => HighlightAction::Next,
        "previous" | "prev" => HighlightAction::Previous,
        "all" => HighlightAction::All,
        _ => return,
    };
    let Some(source) = source.and_then(|value| value.parse::<SocketAddr>().ok()) else {
        return;
    };
    let gesture = state.integrations.selection_grid_gesture(
        source,
        parts[1],
        action,
        pressed,
        Instant::now(),
    );
    if gesture != OscSelectionGridGesture::Ordinary {
        handle_selection_grid_gesture(state, parts[1], source, gesture);
        return;
    }
    if !pressed {
        return;
    }
    let Some(session_id) =
        state
            .integrations
            .accept_highlight_action(source, parts[1], action, Instant::now())
    else {
        return;
    };
    let Some(session) = state.sessions.session(session_id) else {
        return;
    };
    attach_session_command_context(state, &session);
    let Ok(_activation) = state.active_show.try_acquire() else {
        emit_highlight_osc_rejection(
            state,
            &session,
            action,
            "the active show is changing; retry Highlight",
        );
        return;
    };
    let context = programming_context(&session, light_application::ActionSource::Osc, None);
    let result = run_programming_interaction(
        state,
        &session,
        &context,
        "osc",
        ProgrammingLockPolicy::RequireUnlocked,
        || {
            execute_highlight(
                state,
                &session,
                light_application::HighlightCommand::action(action),
                light_application::ActionSource::Osc,
                true,
            )
        },
    )
    .and_then(|completed| completed.output);
    if let Err(error) = result {
        emit_highlight_osc_rejection(state, &session, action, &error.message);
    }
}

fn handle_selection_grid_gesture(
    state: &AppState,
    desk_alias: &str,
    source: SocketAddr,
    gesture: OscSelectionGridGesture,
) {
    let Some((subscriber, session)) = programmer_osc_session(state, Some(source)) else {
        return;
    };
    if subscriber.desk_alias != desk_alias || read_desk_lock(state, session.desk.id).locked {
        return;
    }
    match gesture {
        OscSelectionGridGesture::Ordinary | OscSelectionGridGesture::Pending => {}
        OscSelectionGridGesture::OpenSettings => {
            emit(
                state,
                "desk_action",
                serde_json::json!({
                    "desk_alias":desk_alias,
                    "desk_id":session.desk.id,
                    "session_id":session.id,
                    "action":"selection-grid-settings",
                    "source":"osc"
                }),
            );
        }
        OscSelectionGridGesture::CycleMethod => {
            command_http::route_osc_programming_command(
                state,
                &session,
                desk_alias,
                light_application::ProgrammingCommand::CycleSelectionGridMethod,
                None,
            );
        }
        OscSelectionGridGesture::ReorderRows => {
            command_http::route_osc_programming_command(
                state,
                &session,
                desk_alias,
                light_application::ProgrammingCommand::ReorderSelectionFromGrid {
                    axis: light_programmer::GridTraversalAxis::Rows,
                },
                None,
            );
        }
        OscSelectionGridGesture::ReorderColumns => {
            command_http::route_osc_programming_command(
                state,
                &session,
                desk_alias,
                light_application::ProgrammingCommand::ReorderSelectionFromGrid {
                    axis: light_programmer::GridTraversalAxis::Columns,
                },
                None,
            );
        }
    }
}

fn emit_highlight_osc_rejection(
    state: &AppState,
    session: &Session,
    action: HighlightAction,
    error: &str,
) {
    emit(
        state,
        "highlight_rejected",
        serde_json::json!({
            "desk_id":session.desk.id,
            "user_id":session.user.id,
            "action":action,
            "source":"osc",
            "error":error,
        }),
    );
}

#[derive(Clone, Copy)]
pub(super) enum OscRecordGesture {
    None,
    Arm,
    Targets,
    Settings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OscSelectionGridGesture {
    Ordinary,
    Pending,
    CycleMethod,
    OpenSettings,
    ReorderRows,
    ReorderColumns,
}

pub(super) fn programmer_osc_session(
    state: &AppState,
    source: Option<SocketAddr>,
) -> Option<(OscSubscriber, Session)> {
    let subscriber =
        source.and_then(|source| state.integrations.osc_subscriber_for_source(source))?;
    let session = state.sessions.session(subscriber.session_id)?;
    Some((subscriber, session))
}

fn handle_shift_osc(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    source: Option<SocketAddr>,
    pressed: bool,
) {
    state.programming.run_desk_operation(session.desk.id, || {
        if read_desk_lock(state, session.desk.id).locked {
            return;
        }
        if let Some(source) = source {
            state.integrations.set_shift(source, pressed);
        }
        emit(
            state,
            "desk_action",
            serde_json::json!({"desk_alias":desk_alias,"desk_id":session.desk.id,"session_id":session.id,"action":if pressed { "shift-down" } else { "shift-up" },"source":"osc"}),
        );
    });
}

pub(super) fn record_gesture(target: &mut OscSubscriber, pressed: bool) -> OscRecordGesture {
    if !target.shifted && !target.shift_held {
        return OscRecordGesture::None;
    }
    if pressed && !target.shift_held {
        target.shifted = false;
        target.update_record_started = None;
        target.update_first_release = None;
        return OscRecordGesture::Arm;
    }
    if pressed {
        target.update_record_started = Some(Instant::now());
        return OscRecordGesture::None;
    }
    let Some(started) = target.update_record_started.take() else {
        return OscRecordGesture::None;
    };
    let now = Instant::now();
    if now.saturating_duration_since(started) >= Duration::from_millis(650) {
        target.update_first_release = None;
        target.shifted = false;
        OscRecordGesture::Settings
    } else if target
        .update_first_release
        .is_some_and(|first| now.saturating_duration_since(first) <= Duration::from_millis(600))
    {
        target.update_first_release = None;
        OscRecordGesture::Targets
    } else {
        target.update_first_release = Some(now);
        OscRecordGesture::Arm
    }
}

fn apply_record_gesture(state: &AppState, session: &Session, gesture: OscRecordGesture) {
    match gesture {
        OscRecordGesture::Arm => {
            state
                .programming
                .set_command_line(session.id, "UPDATE".into());
            let _ = persist_programmer(state, session);
            emit(
                state,
                "update_armed",
                serde_json::json!({"desk_id":session.desk.id,"session_id":session.id,"source":"osc"}),
            );
            emit(
                state,
                "programmer_changed",
                serde_json::json!({"session_id":session.id}),
            );
        }
        OscRecordGesture::Targets => {
            emit(
                state,
                "update_targets_requested",
                serde_json::json!({"desk_id":session.desk.id,"session_id":session.id,"source":"osc"}),
            );
        }
        OscRecordGesture::Settings => {
            state
                .programming
                .set_command_line(session.id, String::new());
            let _ = persist_programmer(state, session);
            emit(
                state,
                "update_settings_requested",
                serde_json::json!({"desk_id":session.desk.id,"session_id":session.id,"source":"osc"}),
            );
            emit(
                state,
                "programmer_changed",
                serde_json::json!({"session_id":session.id}),
            );
        }
        OscRecordGesture::None => {}
    }
}

fn handle_record_osc(
    state: &AppState,
    session: &Session,
    subscriber: &OscSubscriber,
    source: Option<SocketAddr>,
    pressed: bool,
) -> bool {
    state.programming.run_desk_operation(session.desk.id, || {
        if read_desk_lock(state, session.desk.id).locked {
            return true;
        }
        let gesture = source
            .map(|source| state.integrations.record_gesture(source, pressed))
            .unwrap_or(OscRecordGesture::None);
        apply_record_gesture(state, session, gesture);
        !matches!(gesture, OscRecordGesture::None) || subscriber.shifted || subscriber.shift_held
    })
}

fn handle_shifted_shortcut(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    action: &str,
    source: Option<SocketAddr>,
    request_id: Option<&str>,
) {
    if let Some(source) = source {
        state.integrations.clear_shift(source);
    }
    emit(
        state,
        "desk_action",
        serde_json::json!({"desk_alias":desk_alias,"desk_id":session.desk.id,"session_id":session.id,"request_id":request_id,"action":format!("shift-{}", action.strip_prefix("digit-").unwrap_or(action)),"source":"osc"}),
    );
}

fn route_programmer_osc_action(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    action: &str,
    request_id: Option<&str>,
) -> bool {
    if action == "set"
        && state.programming.get(session.id).is_some_and(|programmer| {
            matches!(programmer.command_line.trim(), "" | "FIXTURE" | "GROUP")
        })
    {
        emit(
            state,
            "desk_action",
            serde_json::json!({"desk_alias":desk_alias,"desk_id":session.desk.id,"session_id":session.id,"request_id":request_id,"action":"set","source":"osc"}),
        );
        true
    } else if matches!(action, "escape" | "menu" | "prog-playback") {
        emit(
            state,
            "desk_action",
            serde_json::json!({"desk_alias":desk_alias,"desk_id":session.desk.id,"session_id":session.id,"request_id":request_id,"action":action,"source":"osc"}),
        );
        true
    } else {
        command_http::route_osc_command_key_outcome(state, session, desk_alias, action, request_id)
            .unwrap_or(false)
    }
}

pub(super) fn handle_programmer_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) -> bool {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    if parts.len() < 4 || parts[0] != "light" || parts[2] != "programmer" {
        return false;
    }
    let pressed = osc_pressed(arguments);
    let request_id = arguments.get(1).and_then(|argument| match argument {
        OscArgument::String(value) if !value.trim().is_empty() => Some(value.as_str()),
        _ => None,
    });
    let source = source.and_then(|value| value.parse::<SocketAddr>().ok());
    let Some((subscriber, session)) = programmer_osc_session(state, source) else {
        return false;
    };
    if read_desk_lock(state, session.desk.id).locked {
        return false;
    }
    let action = parts[3];
    if action == "shift" {
        handle_shift_osc(state, &session, parts[1], source, pressed);
        return true;
    }
    if action == "record" && handle_record_osc(state, &session, &subscriber, source, pressed) {
        return true;
    }
    if !pressed {
        return false;
    }
    if subscriber.shifted
        && (action.starts_with("digit-") || matches!(action, "clear" | "delete" | "del"))
    {
        handle_shifted_shortcut(state, &session, parts[1], action, source, request_id);
        return true;
    }
    if subscriber.shifted && action == "at" {
        if let Some(source) = source {
            state.integrations.clear_shift(source);
        }
        let current = state
            .programming
            .get(session.id)
            .map(|programmer| programmer.command_line)
            .unwrap_or_default();
        let current = current.trim();
        let next = if matches!(current, "" | "FIXTURE" | "GROUP") {
            "FixAT".to_owned()
        } else {
            format!("{current} FixAT")
        };
        state.programming.set_command_line(session.id, next);
        let _ = persist_programmer(state, &session);
        emit(
            state,
            "desk_action",
            serde_json::json!({"desk_alias":parts[1],"desk_id":session.desk.id,"session_id":session.id,"request_id":request_id,"action":"shift-at","source":"osc"}),
        );
        return true;
    }
    let handled = state.programming.run_desk_operation(session.desk.id, || {
        read_desk_lock(state, session.desk.id).locked
            || file_manager::route_osc_input(state, &session, action)
    });
    if handled {
        return true;
    }
    route_programmer_osc_action(state, &session, parts[1], action, request_id)
}
