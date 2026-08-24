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
    if state
        .integrations
        .consume_unassigned_shifted_highlight(source, parts[1], action)
    {
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
    Record,
    RecordSettings,
    Arm,
    Targets,
    Settings,
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
    path: &str,
    source: Option<SocketAddr>,
    pressed: bool,
) {
    state.programming.run_desk_operation(session.desk.id, || {
        if read_desk_lock(state).locked {
            return;
        }
        if let Some(source) = source {
            state.integrations.set_shift(source, pressed);
        }
        emit(
            state,
            "desk_action",
            serde_json::json!({"path":path,"desk_id":session.desk.id,"session_id":session.id,"action":if pressed { "shift-down" } else { "shift-up" },"source":"osc"}),
        );
    });
}

pub(super) fn record_gesture(target: &mut OscSubscriber, pressed: bool) -> OscRecordGesture {
    if pressed && !target.shifted && !target.shift_held {
        let now = Instant::now();
        target.update_record_started = Some(now);
        target.update_first_release = Some(now);
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
    if target.update_first_release == Some(started) {
        target.update_first_release = None;
        return if now.saturating_duration_since(started) >= Duration::from_millis(2500) {
            OscRecordGesture::RecordSettings
        } else {
            OscRecordGesture::Record
        };
    }
    if now.saturating_duration_since(started) >= Duration::from_millis(2500) {
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
        OscRecordGesture::Record | OscRecordGesture::RecordSettings => {}
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
    // The desk gate covers the lock check and the gesture transition only. Routing the Record key
    // re-enters the Programming service, which takes the user Programmer before the desk gate, so
    // it has to run once this gate is released.
    enum RecordOutcome {
        Handled(bool),
        RouteRecord,
    }
    let outcome = state.programming.run_desk_operation(session.desk.id, || {
        if read_desk_lock(state).locked {
            return RecordOutcome::Handled(true);
        }
        let gesture = source
            .map(|source| state.integrations.record_gesture(source, pressed))
            .unwrap_or(OscRecordGesture::None);
        if matches!(gesture, OscRecordGesture::Record) {
            return RecordOutcome::RouteRecord;
        }
        if matches!(gesture, OscRecordGesture::RecordSettings) {
            emit(
                state,
                "desk_action",
                serde_json::json!({"path":subscriber.path,"desk_id":session.desk.id,"session_id":session.id,"action":"record-settings","source":"osc"}),
            );
            return RecordOutcome::Handled(true);
        }
        apply_record_gesture(state, session, gesture);
        RecordOutcome::Handled(
            pressed
                || !matches!(gesture, OscRecordGesture::None)
                || subscriber.shifted
                || subscriber.shift_held,
        )
    });
    match outcome {
        RecordOutcome::Handled(handled) => handled,
        RecordOutcome::RouteRecord => {
            let _ = command_http::route_osc_command_key_outcome(
                state,
                session,
                &subscriber.path,
                "record",
                None,
            );
            true
        }
    }
}

fn handle_shifted_shortcut(
    state: &AppState,
    session: &Session,
    path: &str,
    action: &str,
    source: Option<SocketAddr>,
    request_id: Option<&str>,
) {
    // Consuming a shifted key clears the one-shot latch. A physically held Shift remains
    // represented separately by `shift_held`, so a second Clear is still shifted until release.
    if let Some(source) = source {
        state.integrations.clear_shift(source);
    }
    if matches!(action, "clear") {
        if super::fixture_freeze::advance_command_mode(state, session) {
            let _ = persist_programmer(state, session);
            emit(
                state,
                "programmer_changed",
                serde_json::json!({"path":path,"desk_id":session.desk.id,"session_id":session.id,"request_id":request_id,"source":"osc"}),
            );
        }
        return;
    }
    if let Some(digit) = action
        .strip_prefix("digit-")
        .and_then(|digit| digit.parse::<u8>().ok())
        && super::fixture_freeze::append_command_family(state, session, digit)
    {
        let _ = persist_programmer(state, session);
        emit(
            state,
            "programmer_changed",
            serde_json::json!({"path":path,"desk_id":session.desk.id,"session_id":session.id,"request_id":request_id,"source":"osc"}),
        );
        return;
    }
    if action.starts_with("digit-")
        || matches!(
            action,
            "at" | "group"
                | "grp"
                | "cue"
                | "playback"
                | "pbk"
                | "set"
                | "time"
                | "div"
                | "off"
                | "mov"
                | "move"
                | "record"
                | "clear"
        )
    {
        let current = state
            .programming
            .get(session.id)
            .map(|programmer| programmer.command_line)
            .unwrap_or_default();
        let repeated = match action {
            "group" | "grp" => current.trim_end().ends_with("FIXTURE"),
            "div" => current.trim_end().ends_with("GO TO"),
            _ => action
                .strip_prefix("digit-")
                .and_then(|digit| digit.parse::<u8>().ok())
                .and_then(|digit| {
                    [
                        "ALL",
                        "INTENSITY",
                        "COLOR",
                        "POSITION",
                        "BEAM",
                        "DYNAMICS",
                        "SHAPERS",
                        "FOCUS",
                        "CONTROL",
                        "MEDIA",
                    ]
                    .get(usize::from(digit))
                })
                .is_some_and(|family| current.trim_end().ends_with(family)),
        };
        let _ = command_http::route_osc_command_gesture_outcome(
            state,
            session,
            path,
            action,
            request_id,
            Some(light_programmer::command_line::CommandGesture {
                kind: if repeated {
                    light_programmer::command_line::CommandGestureKind::Double
                } else {
                    light_programmer::command_line::CommandGestureKind::Regular
                },
                shifted: true,
            }),
        );
        return;
    }
    emit(
        state,
        "desk_action",
        serde_json::json!({"path":path,"desk_id":session.desk.id,"session_id":session.id,"request_id":request_id,"action":format!("shift-{}", action.strip_prefix("digit-").unwrap_or(action)),"source":"osc"}),
    );
}

fn route_programmer_osc_action(
    state: &AppState,
    session: &Session,
    path: &str,
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
            serde_json::json!({"path":path,"desk_id":session.desk.id,"session_id":session.id,"request_id":request_id,"action":"set","source":"osc"}),
        );
        true
    } else if matches!(
        action,
        "align" | "escape" | "menu" | "prog-playback" | "off" | "page-up" | "page-down" | "diff"
    ) {
        emit(
            state,
            "desk_action",
            serde_json::json!({"path":path,"desk_id":session.desk.id,"session_id":session.id,"request_id":request_id,"action":action,"source":"osc"}),
        );
        true
    } else {
        command_http::route_osc_command_key_outcome(state, session, path, action, request_id)
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
    if read_desk_lock(state).locked {
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
    if matches!(action, "page-up" | "page-down") {
        emit(
            state,
            "desk_action",
            serde_json::json!({"path":parts[1],"desk_id":session.desk.id,"session_id":session.id,"request_id":request_id,"action":action,"value":if pressed { "down" } else { "up" },"source":"osc"}),
        );
        return true;
    }
    if !pressed {
        return false;
    }
    if (subscriber.shifted || subscriber.shift_held)
        && (action.starts_with("digit-")
            || matches!(
                action,
                "clear"
                    | "group"
                    | "grp"
                    | "delete"
                    | "del"
                    | "align"
                    | "cue"
                    | "playback"
                    | "escape"
                    | "enter"
                    | "preload"
                    | "mov"
                    | "move"
                    | "set"
                    | "time"
                    | "div"
                    | "off"
                    | "record"
                    | "at"
            ))
    {
        handle_shifted_shortcut(state, &session, parts[1], action, source, request_id);
        return true;
    }
    let handled = state.programming.run_desk_operation(session.desk.id, || {
        read_desk_lock(state).locked || file_manager::route_osc_input(state, &session, action)
    });
    if handled {
        return true;
    }
    route_programmer_osc_action(state, &session, parts[1], action, request_id)
}
