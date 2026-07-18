#[tokio::test(start_paused = true)]
async fn timed_control_action_is_transient_and_reveals_latched_fan_value_at_deadline() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "timed-control-action".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.write().insert(session.id, session.clone());

    let (mut fixture, action_id, channel_ids) = schema_v2_direct_fixture();
    fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].control_actions[0].kind =
        light_fixture::ControlActionKind::TimedPulse;
    fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].control_actions[0]
        .duration_millis = Some(750);
    fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].control_actions[0].semantic =
        light_fixture::ControlActionSemantic::LampOn;
    let fan_action_id = Uuid::new_v4();
    fixture.definition.profile_snapshot.as_mut().unwrap().modes[0]
        .control_actions
        .push(light_fixture::ControlAction {
            id: fan_action_id,
            name: "Fan Max".into(),
            semantic: light_fixture::ControlActionSemantic::FanMax,
            kind: light_fixture::ControlActionKind::Latched,
            duration_millis: None,
            assignments: vec![light_fixture::ControlActionAssignment {
                channel_id: channel_ids[0],
                active_raw: 180,
                inactive_raw: 0,
            }],
        });
    let fixture_id = fixture.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            ..EngineSnapshot::default()
        })
        .unwrap();

    let fan_response = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "fan-max".into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.control_action".into(),
            payload: serde_json::json!({
                "fixture_id":fixture_id,
                "action_id":fan_action_id,
                "active":true,
            }),
        },
    );
    assert!(fan_response.ok, "{:?}", fan_response.error);

    let response = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "timed-pulse".into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.control_action".into(),
            payload: serde_json::json!({
                "fixture_id":fixture_id,
                "action_id":action_id,
                "active":true,
            }),
        },
    );
    assert!(response.ok, "{:?}", response.error);
    assert_eq!(
        response.payload.as_ref().unwrap()["pulse_duration_millis"],
        750
    );

    let action_attributes = channel_ids.map(light_fixture::FixtureMode::control_action_attribute);
    let expected_active = HashMap::from([
        (action_attributes[0].clone(), 201),
        (action_attributes[1].clone(), 255),
    ]);
    let expected_fan_max = HashMap::from([(action_attributes[0].clone(), 180)]);
    let programmer = state.programmers.get(session.id).unwrap();
    assert_eq!(transient_raw_values(&programmer), expected_active);
    assert_eq!(persistent_raw_values(&programmer), expected_fan_max);
    assert_eq!(
        persistent_raw_values(&persisted_programmer(&state, session.id)),
        expected_fan_max
    );
    assert!(persisted_programmer(&state, session.id).transient_values.is_empty());
    assert_eq!(
        state
            .audit_events
            .lock()
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        vec![
            "command_applied",
            "programmer_changed",
            "command_applied",
            "programmer_changed"
        ]
    );

    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(749)).await;
    tokio::task::yield_now().await;
    assert_eq!(
        persistent_raw_values(&persisted_programmer(&state, session.id)),
        expected_fan_max
    );

    tokio::time::advance(Duration::from_millis(1)).await;
    tokio::task::yield_now().await;
    let programmer = state.programmers.get(session.id).unwrap();
    assert!(transient_raw_values(&programmer).is_empty());
    assert_eq!(persistent_raw_values(&programmer), expected_fan_max);
    assert_eq!(
        persistent_raw_values(&persisted_programmer(&state, session.id)),
        expected_fan_max
    );
    let events = state.audit_events.lock();
    assert_eq!(events.len(), 5);
    assert_eq!(events[4].kind, "programmer_changed");
    assert_eq!(events[4].payload["action_id"], action_id.to_string());
    assert_eq!(events[4].payload["active"], false);
    assert_eq!(events[4].payload["timed_pulse_complete"], true);
    drop(events);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn explicit_profile_preset_generation_writes_portable_show_objects() {
    let (state, data_dir) = test_state();
    let (fixture, _, _) = schema_v2_direct_fixture();
    let fixture_id = fixture.fixture_id;
    let show_path = data_dir.join("shows/generated-presets.show");
    let show_id = initialise_show(&show_path, "Generated presets").unwrap();
    let entry = ShowEntry {
        id: show_id,
        name: "Generated presets".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };
    let store = ShowStore::open(&show_path).unwrap();
    store
        .put_object(
            "patched_fixture",
            &fixture_id.0.to_string(),
            &serde_json::to_value(fixture).unwrap(),
            0,
        )
        .unwrap();
    *state.active_show.write() = Some(entry.clone());
    state
        .engine
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();
    assert!(store.objects("preset").unwrap().is_empty());
    store
        .put_object(
            "preset",
            "2.1",
            &serde_json::to_value(light_programmer::Preset {
                name: "Red".into(),
                family: light_programmer::PresetFamily::Color,
                number: 1,
                ..Default::default()
            })
            .unwrap(),
            0,
        )
        .unwrap();

    let response = generate_profile_presets(&state, vec![fixture_id]).unwrap();

    assert_eq!(response["created"][0]["name"], "Dots");
    assert_eq!(response["created"][0]["address"]["family"], "Beam");
    assert_eq!(response["created"][0]["address"]["number"], 1);
    let stored = ShowStore::open(&show_path)
        .unwrap()
        .objects("preset")
        .unwrap();
    assert_eq!(stored.len(), 2);
    assert!(stored.iter().any(|object| object.id == "2.1"
        && object.body["family"] == "Color"
        && object.body["number"] == 1));
    let generated = stored.iter().find(|object| object.id == "4.1").unwrap();
    assert_eq!(generated.body["family"], "Beam");
    assert_eq!(generated.body["number"], 1);
    assert_eq!(
        generated.body["generated_from_fixture_profile"]["semantic_id"],
        "gobo.dots"
    );
    let preset: light_programmer::Preset = serde_json::from_value(generated.body.clone()).unwrap();
    assert_eq!(
        preset.values[&fixture_id][&light_core::AttributeKey("gobo.1".into())],
        light_core::AttributeValue::Discrete("gobo.dots".into())
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn blind_and_preload_transitions_synchronously_suppress_live_highlight() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "highlight-safety".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.write().insert(session.id, session.clone());
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            ..EngineSnapshot::default()
        })
        .unwrap();
    state.programmers.select(session.id, [fixture_id]);
    let fixtures = highlight_fixture_summaries(&state.engine.snapshot().fixtures);
    let groups = HashMap::new();
    let selection = state.programmers.selection(session.id).unwrap();
    state
        .highlight
        .action(
            session.desk.id,
            user.id,
            Some(&user.name),
            HighlightAction::On,
            &selection,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    sync_highlight_output(&state);
    assert_eq!(state.engine.highlighted_fixtures(), vec![fixture_id]);

    let blind = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "blind".into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.mode".into(),
            payload: serde_json::json!({"blind":true}),
        },
    );
    assert!(blind.ok, "{:?}", blind.error);
    assert!(state.engine.highlighted_fixtures().is_empty());

    state
        .programmers
        .set_modes(session.id, Some(false), None, None, None);
    state
        .highlight
        .action(
            session.desk.id,
            user.id,
            Some(&user.name),
            HighlightAction::On,
            &selection,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    sync_highlight_output(&state);
    assert_eq!(state.engine.highlighted_fixtures(), vec![fixture_id]);

    let preview = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "preview".into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.mode".into(),
            payload: serde_json::json!({"preview":true}),
        },
    );
    assert!(preview.ok, "{:?}", preview.error);
    assert!(state.engine.highlighted_fixtures().is_empty());
    let preview_state = current_highlight_transition(&state, &session).unwrap();
    assert!(preview_state.state.active);
    assert!(preview_state.state.capture_only);
    assert!(!preview_state.state.output_enabled);

    let leave_preview = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "leave-preview".into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.mode".into(),
            payload: serde_json::json!({"preview":false}),
        },
    );
    assert!(leave_preview.ok, "{:?}", leave_preview.error);
    assert_eq!(state.engine.highlighted_fixtures(), vec![fixture_id]);

    let preload = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "preload".into(),
            session_id: session.id,
            expected_revision: None,
            command: "preload.enter".into(),
            payload: serde_json::json!({}),
        },
    );
    assert!(preload.ok, "{:?}", preload.error);
    assert!(state.engine.highlighted_fixtures().is_empty());
    let state_after_preload = state.highlight.status(
        session.desk.id,
        user.id,
        Some(&user.name),
        &selection,
        &fixtures,
        &groups,
        true,
    );
    assert!(state_after_preload.state.active);
    assert!(state_after_preload.state.capture_only);
    assert!(!state_after_preload.state.output_enabled);
    let _ = std::fs::remove_dir_all(data_dir);
}
