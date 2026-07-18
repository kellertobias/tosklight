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
    if parts.len() != 4 || parts[0] != "light" || parts[2] != "highlight" || !osc_pressed(arguments)
    {
        return;
    }
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
    let session_id = {
        let mut subscribers = state.osc_subscribers.lock();
        let Some(subscriber) = subscribers.values_mut().find(|subscriber| {
            subscriber.command_source == source && subscriber.desk_alias == parts[1]
        }) else {
            return;
        };
        let now = Instant::now();
        if is_duplicate_osc_action(
            subscriber
                .last_highlight_action
                .as_ref()
                .map(|(previous, received_at)| (previous.as_str(), *received_at)),
            action,
            now,
        ) {
            return;
        }
        subscriber.last_highlight_action = Some((action.osc_dedupe_key().to_owned(), now));
        subscriber.session_id
    };
    let Some(session) = state.sessions.read().get(&session_id).cloned() else {
        return;
    };
    attach_session_command_context(state, &session);
    let Some(programmer) = state.programmers.get(session.id) else {
        return;
    };
    let Some(selection) = state.programmers.selection(session.id) else {
        return;
    };
    let snapshot = state.engine.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    match state.highlight.action_guarded(
        session.desk.id,
        session.user.id,
        Some(&session.user.name),
        action,
        &selection,
        &fixtures,
        &groups,
        programmer.blind || programmer.preview,
    ) {
        Ok(transition) => {
            let selection_changed = apply_highlight_selection_write(
                state,
                &session,
                transition.working_selection.as_ref(),
            )
            .unwrap_or(false);
            if selection_changed {
                emit(
                    state,
                    "programmer_changed",
                    serde_json::json!({"session_id":session.id,"source":"osc_highlight","action":action}),
                );
            }
            sync_highlight_output(state);
            emit(
                state,
                "highlight_changed",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "user_id":session.user.id,
                    "action":action,
                    "source":"osc",
                    "state":transition.state,
                }),
            );
        }
        Err(error) => emit(
            state,
            "highlight_rejected",
            serde_json::json!({
                "desk_id":session.desk.id,
                "user_id":session.user.id,
                "action":action,
                "source":"osc",
                "error":error.to_string(),
            }),
        ),
    }
}

#[derive(Clone, Copy)]
enum OscRecordGesture {
    None,
    Arm,
    Targets,
    Settings,
}

fn programmer_osc_session(
    state: &AppState,
    source: Option<SocketAddr>,
) -> Option<(OscSubscriber, Session)> {
    let subscriber = state
        .osc_subscribers
        .lock()
        .values()
        .find(|subscriber| Some(subscriber.command_source) == source)
        .cloned()?;
    let session = state.sessions.read().get(&subscriber.session_id).cloned()?;
    Some((subscriber, session))
}

fn handle_shift_osc(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    source: Option<SocketAddr>,
    pressed: bool,
) {
    let command_operation = state.programming.desk_lock(session.desk.id);
    let _command_operation_guard = command_operation.lock();
    if read_desk_lock(state, session.desk.id).locked {
        return;
    }
    if let Some(source) = source
        && let Some(target) = state
            .osc_subscribers
            .lock()
            .values_mut()
            .find(|candidate| candidate.command_source == source)
    {
        if pressed {
            target.shifted = !target.shifted;
            target.shift_held = true;
        } else {
            target.shift_held = false;
            if target.update_first_release.is_some() {
                target.shifted = false;
            }
        }
    }
    emit(
        state,
        "desk_action",
        serde_json::json!({"desk_alias":desk_alias,"desk_id":session.desk.id,"session_id":session.id,"action":if pressed { "shift-down" } else { "shift-up" },"source":"osc"}),
    );
}

fn record_gesture(target: &mut OscSubscriber, pressed: bool) -> OscRecordGesture {
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
                .programmers
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
        OscRecordGesture::Targets => emit(
            state,
            "update_targets_requested",
            serde_json::json!({"desk_id":session.desk.id,"session_id":session.id,"source":"osc"}),
        ),
        OscRecordGesture::Settings => {
            state
                .programmers
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
    let command_operation = state.programming.desk_lock(session.desk.id);
    let _command_operation_guard = command_operation.lock();
    if read_desk_lock(state, session.desk.id).locked {
        return true;
    }
    let gesture = source
        .and_then(|source| {
            state
                .osc_subscribers
                .lock()
                .values_mut()
                .find(|candidate| candidate.command_source == source)
                .map(|target| record_gesture(target, pressed))
        })
        .unwrap_or(OscRecordGesture::None);
    apply_record_gesture(state, session, gesture);
    !matches!(gesture, OscRecordGesture::None) || subscriber.shifted || subscriber.shift_held
}

fn handle_shifted_shortcut(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    action: &str,
    source: Option<SocketAddr>,
) {
    if let Some(source) = source
        && let Some(target) = state
            .osc_subscribers
            .lock()
            .values_mut()
            .find(|candidate| candidate.command_source == source)
    {
        target.shifted = false;
    }
    emit(
        state,
        "desk_action",
        serde_json::json!({"desk_alias":desk_alias,"session_id":session.id,"action":format!("shift-{}", action.strip_prefix("digit-").unwrap_or(action)),"source":"osc"}),
    );
}

fn route_programmer_osc_action(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    action: &str,
) {
    if action == "set"
        && state.programmers.get(session.id).is_some_and(|programmer| {
            matches!(programmer.command_line.trim(), "" | "FIXTURE" | "GROUP")
        })
    {
        emit(
            state,
            "desk_action",
            serde_json::json!({"desk_alias":desk_alias,"session_id":session.id,"action":"set","source":"osc"}),
        );
    } else if matches!(action, "escape" | "menu" | "prog-playback" | "record") {
        emit(
            state,
            "desk_action",
            serde_json::json!({"desk_alias":desk_alias,"session_id":session.id,"action":action,"source":"osc"}),
        );
    } else {
        command_http::route_osc_command_key(state, session, desk_alias, action);
    }
}

pub(super) fn handle_programmer_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    if parts.len() < 4 || parts[0] != "light" || parts[2] != "programmer" {
        return;
    }
    let pressed = osc_pressed(arguments);
    let source = source.and_then(|value| value.parse::<SocketAddr>().ok());
    let Some((subscriber, session)) = programmer_osc_session(state, source) else {
        return;
    };
    if read_desk_lock(state, session.desk.id).locked {
        return;
    }
    let action = parts[3];
    if action == "shift" {
        handle_shift_osc(state, &session, parts[1], source, pressed);
        return;
    }
    if action == "record" && handle_record_osc(state, &session, &subscriber, source, pressed) {
        return;
    }
    if !pressed {
        return;
    }
    if subscriber.shifted
        && (action.starts_with("digit-") || matches!(action, "clear" | "delete" | "del"))
    {
        handle_shifted_shortcut(state, &session, parts[1], action, source);
        return;
    }
    let command_operation = state.programming.desk_lock(session.desk.id);
    let _command_operation_guard = command_operation.lock();
    if read_desk_lock(state, session.desk.id).locked
        || file_manager::route_osc_input(state, &session, action)
    {
        return;
    }
    drop(_command_operation_guard);
    route_programmer_osc_action(state, &session, parts[1], action);
}
