#[test]
fn focused_macro_editor_routes_attached_keypad_input_without_mutating_command_line() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "macro-editor-osc-test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id, user.id);
    state.sessions.insert_session(session.clone());
    let source: SocketAddr = "127.0.0.1:9018".parse().unwrap();
    state.integrations.register_osc_subscriber(
        "macro-editor-test".into(),
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
    state
        .programming
        .set_command_line(session.id, "FIXTURE 99".into());
    file_manager::try_claim_input_context(
        &state,
        file_manager::FileInputContext {
            instance_id: "macro-editor:acceptance".into(),
            action: file_manager::FileInputAction::MacroEdit,
            session_id: session.id,
            desk_id: session.desk.id,
            expires_at: Instant::now() + Duration::from_secs(120),
        },
        || Ok(()),
    )
    .unwrap();

    assert!(handle_programmer_osc(
        &state,
        "/light/main/programmer/digit-7",
        &[OscArgument::Bool(true)],
        Some("127.0.0.1:9018"),
    ));
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "FIXTURE 99"
    );
    assert!(state.events.audit_events().iter().any(|event| {
        event.kind == "file_input_action"
            && event.payload["operation"] == "macro_edit"
            && event.payload["action"] == "digit-7"
            && event.payload["instance_id"] == "macro-editor:acceptance"
            && event.payload["source"] == "osc"
    }));

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn osc_exposes_time_minus_and_latched_shift_shortcuts() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "osc-test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id, user.id);
    state.sessions.insert_session(session.clone());
    let source: SocketAddr = "127.0.0.1:9010".parse().unwrap();
    state.integrations.register_osc_subscriber(
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
    handle_control_event(
        &state,
        ControlEvent::Osc {
            address: "/light/main/programmer/set".into(),
            arguments: vec![
                OscArgument::Bool(true),
                OscArgument::String("hardware-set-1".into()),
            ],
            source: Some("127.0.0.1:9010".into()),
        },
    );
    let timing = state.action_timing.snapshot();
    assert_eq!(timing.len(), 1);
    assert_eq!(timing[0].source, "osc");
    assert_eq!(timing[0].request_id, "hardware-set-1");
    assert!(!timing[0].requires_output_frame);
    assert!(timing[0].acknowledgement_within_budget);
    assert!(state.integrations.captured_osc_feedback().iter().any(
        |(_, address, arguments)| {
            address == "/light/main/feedback/action"
                && arguments.first()
                    == Some(&OscArgument::String("hardware-set-1".into()))
        }
    ));
    assert_eq!(state.programming.get(session.id).unwrap().command_line, "");
    assert!(state.events.audit_events().iter().any(|event| {
        event.kind == "desk_action"
            && event.payload["action"] == "set"
            && event.payload["session_id"] == serde_json::json!(session.id)
            && event.payload["request_id"] == "hardware-set-1"
    }));
    handle_control_event(
        &state,
        ControlEvent::Osc {
            address: "/light/main/encode/2".into(),
            arguments: vec![
                OscArgument::String("up".into()),
                OscArgument::String("hardware-encoder-1".into()),
            ],
            source: Some("127.0.0.1:9010".into()),
        },
    );
    assert_eq!(
        state.action_timing.snapshot().len(),
        1,
        "encoder timing remains pending until its correlated WebSocket action"
    );
    assert!(state.events.audit_events().iter().any(|event| {
        event.kind == "desk_action"
            && event.payload["control"] == "encode/2"
            && event.payload["request_id"] == "hardware-encoder-1"
            && event.payload["session_id"] == serde_json::json!(session.id)
            && event.payload["desk_id"] == serde_json::json!(session.desk.id)
    }));
    handle_control_event(
        &state,
        ControlEvent::Osc {
            address: "/light/main/programmer/align".into(),
            arguments: vec![
                OscArgument::Bool(true),
                OscArgument::String("hardware-align-1".into()),
            ],
            source: Some("127.0.0.1:9010".into()),
        },
    );
    assert!(state.events.audit_events().iter().any(|event| {
        event.kind == "desk_action"
            && event.payload["action"] == "align"
            && event.payload["request_id"] == "hardware-align-1"
            && event.payload["session_id"] == serde_json::json!(session.id)
    }));
    let timings_before_release = state.action_timing.snapshot().len();
    handle_control_event(
        &state,
        ControlEvent::Osc {
            address: "/light/main/programmer/set".into(),
            arguments: vec![
                OscArgument::Bool(false),
                OscArgument::String("hardware-set-1".into()),
            ],
            source: Some("127.0.0.1:9010".into()),
        },
    );
    assert_eq!(
        state.action_timing.snapshot().len(),
        timings_before_release,
        "Programmer key release is not measured as another action"
    );
    state
        .programming
        .set_command_line(session.id, "COPY".into());
    handle_programmer_osc(
        &state,
        "/light/main/programmer/set",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "COPY SET"
    );
    state
        .programming
        .set_command_line(session.id, String::new());
    handle_programmer_osc(
        &state,
        "/light/main/programmer/time",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    let interaction_event = state
        .events.audit_events()
        .iter()
        .rev()
        .find(|event| event.kind == "programmer_changed")
        .cloned()
        .unwrap();
    assert_eq!(interaction_event.payload["command"], "programmer.command_line");
    assert_eq!(interaction_event.payload["changes"], serde_json::json!(["interaction"]));
    state
        .programming
        .set_command_line(session.id, String::new());
    handle_programmer_osc(
        &state,
        "/light/main/programmer/record",
        &pressed,
        Some("127.0.0.1:9010"),
    );
	handle_programmer_osc(
		&state,
		"/light/main/programmer/record",
		&[OscArgument::Bool(false)],
		Some("127.0.0.1:9010"),
	);
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "RECORD "
    );
    assert!(!state.events.audit_events().iter().any(|event| {
        event.kind == "desk_action"
            && event.payload["action"] == "record"
            && event.payload["session_id"] == serde_json::json!(session.id)
    }));
	handle_programmer_osc(
		&state,
		"/light/main/programmer/record",
		&pressed,
		Some("127.0.0.1:9010"),
	);
	state.integrations.set_osc_unshifted_record_started(
		"test",
		Instant::now() - Duration::from_millis(3000),
	);
	handle_programmer_osc(
		&state,
		"/light/main/programmer/record",
		&[OscArgument::Bool(false)],
		Some("127.0.0.1:9010"),
	);
	assert!(state.events.audit_events().iter().any(|event| {
		event.kind == "desk_action"
			&& event.payload["action"] == "record-settings"
			&& event.payload["session_id"] == serde_json::json!(session.id)
	}));
    state
        .programming
        .set_command_line(session.id, "TIME".into());
    handle_programmer_osc(
        &state,
        "/light/main/programmer/minus",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "TIME -"
    );
    handle_programmer_osc(
        &state,
        "/light/main/programmer/shift",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    assert!(state.integrations.osc_subscriber("test").unwrap().shifted);
    handle_programmer_osc(
        &state,
        "/light/main/programmer/digit-1",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    assert!(!state.integrations.osc_subscriber("test").unwrap().shifted);
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "TIME -"
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
    assert!(!state.integrations.osc_subscriber("test").unwrap().shifted);
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "FREEZE"
    );
    handle_programmer_osc(
        &state,
        "/light/main/programmer/clear",
        &pressed,
        Some("127.0.0.1:9010"),
    );
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "UNFREEZE"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn shifted_all_previous_and_next_are_unassigned_without_leaking_highlight_actions() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "osc-grid-test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id, user.id);
    state.sessions.insert_session(session.clone());
    let source: SocketAddr = "127.0.0.1:9014".parse().unwrap();
    state.integrations.register_osc_subscriber(
        "grid-test".into(),
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
    let send_programmer = |action: &str, pressed: bool| {
        handle_programmer_osc(
            &state,
            &format!("/light/main/programmer/{action}"),
            &[OscArgument::Bool(pressed)],
            Some("127.0.0.1:9014"),
        );
    };
    let send_highlight = |action: &str, pressed: bool| {
        handle_highlight_osc(
            &state,
            &format!("/light/main/highlight/{action}"),
            &[OscArgument::Bool(pressed)],
            Some("127.0.0.1:9014"),
        );
    };
    let ordinary_all_count = || {
        state
            .events
            .audit_events()
            .iter()
            .filter(|event| {
                event.kind == "highlight_changed" && event.payload["action"] == "all"
            })
            .count()
    };

    for action in ["all", "next", "prev"] {
        send_programmer("shift", true);
        send_programmer("shift", false);
        assert!(state.integrations.osc_subscriber("grid-test").unwrap().shifted);
        send_highlight(action, true);
        send_highlight(action, false);
        assert!(!state.integrations.osc_subscriber("grid-test").unwrap().shifted);
    }
    assert_eq!(ordinary_all_count(), 0);
    assert!(!state.events.audit_events().iter().any(|event| {
        event.kind == "desk_action"
            && event.payload["action"].as_str().is_some_and(|action| action.contains("grid"))
    }));

    // Unshifted ALL retains the original Highlight behavior.
    send_highlight("all", true);
    assert_eq!(ordinary_all_count(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn osc_playback_source_cannot_cross_its_subscribed_desk_alias() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user,
        token: "osc-alias-isolation".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.sessions.insert_session(session.clone());
    let source: SocketAddr = "127.0.0.1:9011".parse().unwrap();
    state.integrations.register_osc_subscriber(
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
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "osc-update-test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id, user.id);
    state.sessions.insert_session(session.clone());
    let source: SocketAddr = "127.0.0.1:9011".parse().unwrap();
    state.integrations.register_osc_subscriber(
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
        state.programming.get(session.id).unwrap().command_line,
        "UPDATE"
    );

    send("record", &pressed);
    send("record", &released);

    send("record", &pressed);
    state.integrations.set_osc_record_started(
        "update-test",
		Instant::now() - Duration::from_millis(3000),
    );
    send("record", &released);
    assert_eq!(state.programming.get(session.id).unwrap().command_line, "");

    let kinds = state
        .events.audit_events()
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
    let user = state.installation.users().unwrap().remove(0);
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
        state.programming.start(session.id, session.user.id);
        attach_session_command_context(&state, session);
        state.sessions.insert_session(session.clone());
    }

    let armed = dispatch_live_action(
        &state,
        &first,
        live_action_frame(
            &first,
            "arm-update",
            light_wire::v2::live_action::LiveAction::CommandLineSet(
                light_wire::v2::live_action::CommandLineSetLiveActionRequest {
                    value: "UPDATE ".into(),
                },
            ),
        ),
    );
    assert!(armed.ok);
    assert_eq!(
        state.programming.get(second.id).unwrap().command_line,
        "UPDATE "
    );
    assert!(
        state
            .programming
            .get(other.id)
            .unwrap()
            .command_line
            .is_empty()
    );
    let event = state
        .events.audit_events()
        .iter()
        .rev()
        .find(|event| event.kind == "update_armed")
        .cloned()
        .unwrap();
    assert_eq!(event.payload["desk_id"], first.desk.id.to_string());
    assert_eq!(event.payload["armed"], true);

    let disarmed = dispatch_live_action(
        &state,
        &second,
        live_action_frame(
            &second,
            "disarm-update",
            light_wire::v2::live_action::LiveAction::CommandLineSet(
                light_wire::v2::live_action::CommandLineSetLiveActionRequest {
                    value: String::new(),
                },
            ),
        ),
    );
    assert!(disarmed.ok);
    assert!(
        state
            .programming
            .get(first.id)
            .unwrap()
            .command_line
            .is_empty()
    );
    let events = state
        .events.audit_events()
        .iter()
        .filter(|event| event.kind == "update_armed")
        .map(|event| event.payload["armed"].as_bool())
        .collect::<Vec<_>>();
    assert_eq!(events, vec![Some(true), Some(false)]);
    let _ = std::fs::remove_dir_all(data_dir);
}
