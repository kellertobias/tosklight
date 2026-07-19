#[test]
fn osc_exposes_time_minus_and_latched_shift_shortcuts() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "osc-test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    state.sessions.write().insert(session.id, session.clone());
    let source: SocketAddr = "127.0.0.1:9010".parse().unwrap();
    state.osc_subscribers.lock().insert(
        "test".into(),
        OscSubscriber {
            desk_alias: "main".into(),
            target: source,
            command_source: source,
            session_id: session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
    let pressed = [OscArgument::Bool(true)];
    handle_programmer_osc(
        &state,
        "/light/main/programmer/set",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    assert_eq!(state.programmers.get(session.id).unwrap().command_line, "");
    assert!(state.audit_events.lock().iter().any(|event| {
        event.kind == "desk_action"
            && event.payload["action"] == "set"
            && event.payload["session_id"] == serde_json::json!(session.id)
    }));
    state
        .programmers
        .set_command_line(session.id, "COPY".into());
    handle_programmer_osc(
        &state,
        "/light/main/programmer/set",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    assert_eq!(
        state.programmers.get(session.id).unwrap().command_line,
        "COPY SET "
    );
    state
        .programmers
        .set_command_line(session.id, String::new());
    handle_programmer_osc(
        &state,
        "/light/main/programmer/time",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    handle_programmer_osc(
        &state,
        "/light/main/programmer/minus",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    assert_eq!(
        state.programmers.get(session.id).unwrap().command_line,
        "TIME - "
    );
    handle_programmer_osc(
        &state,
        "/light/main/programmer/shift",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    assert!(state.osc_subscribers.lock()["test"].shifted);
    handle_programmer_osc(
        &state,
        "/light/main/programmer/digit-1",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    assert!(!state.osc_subscribers.lock()["test"].shifted);
    assert_eq!(
        state.programmers.get(session.id).unwrap().command_line,
        "TIME - "
    );
    handle_programmer_osc(
        &state,
        "/light/main/programmer/shift",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    handle_programmer_osc(
        &state,
        "/light/main/programmer/clear",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    assert!(!state.osc_subscribers.lock()["test"].shifted);
    assert_eq!(
        state.programmers.get(session.id).unwrap().command_line,
        "TIME - "
    );
    let events = state.audit_events.lock();
    let shifted_clear = events.back().unwrap();
    assert_eq!(shifted_clear.kind, "desk_action");
    assert_eq!(shifted_clear.payload["action"], "shift-clear");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn osc_playback_source_cannot_cross_its_subscribed_desk_alias() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user,
        token: "osc-alias-isolation".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.sessions.write().insert(session.id, session.clone());
    let source: SocketAddr = "127.0.0.1:9011".parse().unwrap();
    state.osc_subscribers.lock().insert(
        "cross-desk".into(),
        OscSubscriber {
            desk_alias: "other-desk".into(),
            target: source,
            command_source: source,
            session_id: session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );

    assert!(
        osc_playback_session(
            &state,
            Some("127.0.0.1:9011"),
            "other-desk",
            Some(&session.desk),
        )
        .is_err()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn held_shift_record_short_double_and_long_gestures_are_mutually_distinct() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "osc-update-test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    state.sessions.write().insert(session.id, session.clone());
    let source: SocketAddr = "127.0.0.1:9011".parse().unwrap();
    state.osc_subscribers.lock().insert(
        "update-test".into(),
        OscSubscriber {
            desk_alias: "main".into(),
            target: source,
            command_source: source,
            session_id: session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
    let pressed = [OscArgument::Bool(true)];
    let released = [OscArgument::Bool(false)];
    let send = |action: &str, arguments: &[OscArgument]| {
        handle_programmer_osc(
            &state,
            &format!("/light/main/programmer/{action}"),
            arguments,
            Some("127.0.0.1:9011"),
        );
    };

    send("shift", &pressed);
    send("record", &pressed);
    send("record", &released);
    assert_eq!(
        state.programmers.get(session.id).unwrap().command_line,
        "UPDATE"
    );

    send("record", &pressed);
    send("record", &released);

    send("record", &pressed);
    state
        .osc_subscribers
        .lock()
        .get_mut("update-test")
        .unwrap()
        .update_record_started = Some(Instant::now() - Duration::from_millis(700));
    send("record", &released);
    assert_eq!(state.programmers.get(session.id).unwrap().command_line, "");

    let kinds = state
        .audit_events
        .lock()
        .iter()
        .map(|event| event.kind.clone())
        .filter(|kind| kind.starts_with("update_"))
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            "update_armed".to_string(),
            "update_targets_requested".to_string(),
            "update_settings_requested".to_string()
        ]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn software_update_armed_state_is_shared_only_with_the_same_desk() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let front = test_control_desk();
    let mut wing = test_control_desk();
    wing.id = Uuid::new_v4();
    wing.osc_alias = "wing".into();
    let first = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "update-front-one".into(),
        connected: true,
        desk: front.clone(),
    };
    let second = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "update-front-two".into(),
        connected: true,
        desk: front,
    };
    let other = Session {
        id: SessionId::new(),
        user,
        token: "update-wing".into(),
        connected: true,
        desk: wing,
    };
    for session in [&first, &second, &other] {
        state.programmers.start(session.id, session.user.id);
        attach_session_command_context(&state, session);
        state.sessions.write().insert(session.id, session.clone());
    }

    let armed = dispatch_ws_command(
        &state,
        &first,
        WsCommand {
            protocol_version: 1,
            request_id: "arm-update".into(),
            session_id: first.id,
            expected_revision: None,
            command: "programmer.command_line".into(),
            payload: serde_json::json!({"value":"UPDATE "}),
        },
    );
    assert!(armed.ok);
    assert_eq!(
        state.programmers.get(second.id).unwrap().command_line,
        "UPDATE "
    );
    assert!(
        state
            .programmers
            .get(other.id)
            .unwrap()
            .command_line
            .is_empty()
    );
    let event = state
        .audit_events
        .lock()
        .iter()
        .rev()
        .find(|event| event.kind == "update_armed")
        .cloned()
        .unwrap();
    assert_eq!(event.payload["desk_id"], first.desk.id.to_string());
    assert_eq!(event.payload["armed"], true);

    let disarmed = dispatch_ws_command(
        &state,
        &second,
        WsCommand {
            protocol_version: 1,
            request_id: "disarm-update".into(),
            session_id: second.id,
            expected_revision: None,
            command: "programmer.command_line".into(),
            payload: serde_json::json!({"value":""}),
        },
    );
    assert!(disarmed.ok);
    assert!(
        state
            .programmers
            .get(first.id)
            .unwrap()
            .command_line
            .is_empty()
    );
    let events = state
        .audit_events
        .lock()
        .iter()
        .filter(|event| event.kind == "update_armed")
        .map(|event| event.payload["armed"].as_bool())
        .collect::<Vec<_>>();
    assert_eq!(events, vec![Some(true), Some(false)]);
    let _ = std::fs::remove_dir_all(data_dir);
}
