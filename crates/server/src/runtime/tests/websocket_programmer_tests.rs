#[tokio::test]
async fn programmer_set_many_validates_then_applies_one_faded_undo_step() {
    let (state, data_dir) = test_state();
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();

    let response = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "home".into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.set_many".into(),
            payload: serde_json::json!({"assignments":[
                {"fixture_id":fixture_id,"attribute":"pan","value":0.25},
                {"fixture_id":fixture_id,"attribute":"tilt","value":0.75}
            ]}),
        },
    );
    assert!(response.ok, "{:?}", response.error);
    let values = state.programmers.get(session.id).unwrap().values;
    assert_eq!(values.len(), 2);
    assert!(values.iter().all(|value| value.fade));
    assert_eq!(values[0].changed_at, values[1].changed_at);

    let rejected = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "invalid-home".into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.set_many".into(),
            payload: serde_json::json!({"assignments":[
                {"fixture_id":fixture_id,"attribute":"pan","value":0.5},
                {"fixture_id":light_core::FixtureId::new(),"attribute":"tilt","value":0.5}
            ]}),
        },
    );
    assert!(!rejected.ok);
    assert_eq!(
        serde_json::to_value(state.programmers.get(session.id).unwrap().values).unwrap(),
        serde_json::to_value(values).unwrap()
    );
    assert!(state.programmers.undo(session.id));
    assert!(state.programmers.get(session.id).unwrap().values.is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn websocket_commands_are_typed_owned_and_revision_checked() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, session_id) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let fixture = light_core::FixtureId::new();
    let response = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "set-1".into(),
            session_id: SessionId(Uuid::parse_str(&session_id).unwrap()),
            expected_revision: Some(0),
            command: "programmer.set".into(),
            payload: serde_json::json!({"fixture_id":fixture,"attribute":"intensity","value":0.75}),
        },
    );
    assert!(response.ok);
    assert_eq!(state.programmers.get(session.id).unwrap().values.len(), 1);
    let same_user_session = Session {
        id: SessionId::new(),
        user: session.user.clone(),
        token: "same-user".into(),
        connected: true,
        desk: session.desk.clone(),
    };
    state
        .programmers
        .start(same_user_session.id, same_user_session.user.id);
    let same_user_update = dispatch_ws_command(
        &state,
        &same_user_session,
        WsCommand {
            protocol_version: 1,
            request_id: "same-user".into(),
            session_id: same_user_session.id,
            expected_revision: Some(999),
            command: "programmer.set".into(),
            payload: serde_json::json!({"fixture_id":fixture,"attribute":"intensity","value":0.5}),
        },
    );
    assert!(
        same_user_update.ok,
        "one user owns the lock across all of their sessions"
    );
    let other_user = state.desk.lock().add_user("Other operator").unwrap();
    let other_session = Session {
        id: SessionId::new(),
        user: other_user,
        token: "other-user".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state
        .programmers
        .start(other_session.id, other_session.user.id);
    let competing_update = dispatch_ws_command(
        &state,
        &other_session,
        WsCommand {
            protocol_version: 1,
            request_id: "other-user".into(),
            session_id: other_session.id,
            expected_revision: None,
            command: "programmer.set".into(),
            payload: serde_json::json!({"fixture_id":fixture,"attribute":"intensity","value":0.2}),
        },
    );
    assert!(
        competing_update.ok,
        "different users own independent programmers that arbitrate in the engine"
    );
    let primary_programmer = state.programmers.get(session.id).unwrap();
    let competing_programmer = state.programmers.get(other_session.id).unwrap();
    assert_ne!(primary_programmer.id, competing_programmer.id);
    assert_eq!(primary_programmer.values.len(), 1);
    assert_eq!(competing_programmer.values.len(), 1);
    let foreign = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "foreign".into(),
            session_id: SessionId::new(),
            expected_revision: None,
            command: "programmer.clear".into(),
            payload: serde_json::Value::Null,
        },
    );
    assert!(!foreign.ok);
    assert!(foreign.error.unwrap().contains("does not own"));
    let stale = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "stale".into(),
            session_id: session.id,
            expected_revision: Some(99),
            command: "programmer.clear".into(),
            payload: serde_json::Value::Null,
        },
    );
    assert!(
        stale.ok,
        "live absolute commands ignore unrelated show revisions"
    );
    let revisioned = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "revisioned".into(),
            session_id: session.id,
            expected_revision: Some(99),
            command: "show.activate".into(),
            payload: serde_json::Value::Null,
        },
    );
    assert!(!revisioned.ok);
    assert!(revisioned.error.unwrap().contains("revision conflict"));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn group_master_set_never_replaces_a_snapshot_during_show_activation() {
    let (state, data_dir) = test_state();
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            groups: vec![light_programmer::GroupDefinition {
                id: "front".into(),
                master: 1.0,
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let command = || WsCommand {
        protocol_version: 1,
        request_id: "group-master".into(),
        session_id: session.id,
        expected_revision: None,
        command: "group.master.set".into(),
        payload: serde_json::json!({"group_id":"front","value":0.5}),
    };

    let activation = state.activation_lock.clone().lock_owned().await;
    let rejected = dispatch_ws_command(&state, &session, command());
    assert!(!rejected.ok);
    assert!(
        rejected
            .error
            .as_deref()
            .is_some_and(|error| error.contains("active show is changing"))
    );
    assert_eq!(state.engine.snapshot().groups[0].master, 1.0);

    drop(activation);
    let applied = dispatch_ws_command(&state, &session, command());
    assert!(applied.ok, "{:?}", applied.error);
    assert_eq!(state.engine.snapshot().groups[0].master, 0.5);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn compatibility_selection_publishes_one_typed_interaction_event() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let fixture = light_core::FixtureId::new();

    let response = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "selection-event".into(),
            session_id: session.id,
            expected_revision: None,
            command: "selection.set".into(),
            payload: serde_json::json!({"fixtures":[fixture]}),
        },
    );
    assert!(response.ok, "{:?}", response.error);

    let filter = light_application::EventFilter::for_desk(session.desk.id).with_object(
        light_application::EventObject::programming_selection(session.desk.id),
    );
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &filter)
    else {
        panic!("the interaction event should remain replayable")
    };
    assert_eq!(events.len(), 1);
    let light_application::ApplicationEvent::Programming(
        light_application::ProgrammingEvent::InteractionChanged(change),
    ) = &events[0].payload
    else {
        panic!("expected a typed Programming interaction event")
    };
    assert_eq!(change.selection().unwrap().selected, vec![fixture]);
    assert!(events[0].correlation_id.is_some());
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(
            light_application::ActionSource::UserInterface,
        )
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn compatibility_command_line_publishes_only_its_scoped_component() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();

    let response = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "command-event".into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.command_line".into(),
            payload: serde_json::json!({"value":"GROUP 2"}),
        },
    );
    assert!(response.ok, "{:?}", response.error);

    let filter = light_application::EventFilter::for_desk(session.desk.id).with_object(
        light_application::EventObject::programming_command_line(session.desk.id),
    );
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &filter)
    else {
        panic!("the command-line event should remain replayable")
    };
    assert_eq!(events.len(), 1);
    let light_application::ApplicationEvent::Programming(
        light_application::ProgrammingEvent::InteractionChanged(change),
    ) = &events[0].payload
    else {
        panic!("expected a typed Programming interaction event")
    };
    assert_eq!(change.command_line().unwrap().visible_text(), "GROUP 2");
    assert!(change.selection().is_none());

    let sequence = state.application_events.latest_sequence();
    let value_only = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "priority-only".into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.priority".into(),
            payload: serde_json::json!({"priority":7}),
        },
    );
    assert!(value_only.ok, "{:?}", value_only.error);
    assert_eq!(state.application_events.latest_sequence(), sequence);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn compatibility_programmer_changed_reports_only_authoritative_changed_projections() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let fixture = light_core::FixtureId::new();
    let command = |request_id: &str, command: &str, payload: serde_json::Value| WsCommand {
        protocol_version: 1,
        request_id: request_id.into(),
        session_id: session.id,
        expected_revision: None,
        command: command.into(),
        payload,
    };
    let changed_count = || {
        state
            .audit_events
            .lock()
            .iter()
            .filter(|event| event.kind == "programmer_changed")
            .count()
    };

    let before = changed_count();
    let no_op = dispatch_ws_command(
        &state,
        &session,
        command("no-op-undo", "programmer.undo", serde_json::Value::Null),
    );
    assert!(no_op.ok, "{:?}", no_op.error);
    assert_eq!(changed_count(), before);

    let selection = dispatch_ws_command(
        &state,
        &session,
        command(
            "selection-change",
            "selection.set",
            serde_json::json!({"fixtures":[fixture]}),
        ),
    );
    assert!(selection.ok, "{:?}", selection.error);
    let selection_event = state
        .audit_events
        .lock()
        .iter()
        .rev()
        .find(|event| event.kind == "programmer_changed")
        .cloned()
        .unwrap();
    assert_eq!(selection_event.payload["changes"], serde_json::json!(["interaction"]));

    let values = dispatch_ws_command(
        &state,
        &session,
        command(
            "values-change",
            "programmer.set",
            serde_json::json!({
                "fixture_id": fixture,
                "attribute": "intensity",
                "value": 0.75
            }),
        ),
    );
    assert!(values.ok, "{:?}", values.error);
    let values_event = state
        .audit_events
        .lock()
        .iter()
        .rev()
        .find(|event| event.kind == "programmer_changed")
        .cloned()
        .unwrap();
    assert_eq!(values_event.payload["changes"], serde_json::json!(["values"]));
    assert_eq!(values_event.payload["user_id"], serde_json::json!(session.user.id));
    assert_eq!(values_event.payload["desk_id"], serde_json::json!(session.desk.id));

    let preload_enter = dispatch_ws_command(
        &state,
        &session,
        command("preload-enter", "preload.enter", serde_json::Value::Null),
    );
    assert!(preload_enter.ok, "{:?}", preload_enter.error);
    let preload_values = dispatch_ws_command(
        &state,
        &session,
        command(
            "preload-values",
            "preload.group.set",
            serde_json::json!({
                "group_id": "front",
                "attribute": "intensity",
                "value": 0.5
            }),
        ),
    );
    assert!(preload_values.ok, "{:?}", preload_values.error);
    let preload_event = state
        .audit_events
        .lock()
        .iter()
        .rev()
        .find(|event| event.kind == "programmer_changed")
        .cloned()
        .unwrap();
    assert_eq!(
        preload_event.payload["changes"],
        serde_json::json!(["preload_values"])
    );
    assert_eq!(
        preload_event.payload["user_id"],
        serde_json::json!(session.user.id)
    );

    let before_priority = changed_count();
    let priority = dispatch_ws_command(
        &state,
        &session,
        command(
            "priority-only",
            "programmer.priority",
            serde_json::json!({"priority":7}),
        ),
    );
    assert!(priority.ok, "{:?}", priority.error);
    assert_eq!(changed_count(), before_priority);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn transient_control_retriggers_emit_compatibility_only_and_repeated_release_is_quiet() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let (fixture, action_id, _) = schema_v2_direct_fixture();
    let fixture_id = fixture.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            ..EngineSnapshot::default()
        })
        .unwrap();
    let changed_count = || {
        state
            .audit_events
            .lock()
            .iter()
            .filter(|event| event.kind == "programmer_changed")
            .count()
    };
    let before_changed_count = changed_count();
    let before_application_sequence = state.application_events.latest_sequence();
    let command = |request_id: &str, active: bool| WsCommand {
        protocol_version: 1,
        request_id: request_id.into(),
        session_id: session.id,
        expected_revision: None,
        command: "programmer.control_action".into(),
        payload: serde_json::json!({
            "fixture_id": fixture_id,
            "action_id": action_id,
            "active": active,
        }),
    };

    for (request_id, active, expected_changes) in [
        ("momentary-on", true, 1),
        ("momentary-retrigger", true, 2),
        ("momentary-off", false, 3),
        ("momentary-off-again", false, 3),
    ] {
        let response = dispatch_ws_command(&state, &session, command(request_id, active));
        assert!(response.ok, "{:?}", response.error);
        assert_eq!(changed_count(), before_changed_count + expected_changes);
    }
    assert_eq!(
        state.application_events.latest_sequence(),
        before_application_sequence
    );
    let changes = state
        .audit_events
        .lock()
        .iter()
        .filter(|event| event.kind == "programmer_changed")
        .skip(before_changed_count)
        .map(|event| event.payload["changes"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        changes,
        vec![serde_json::json!(["transient_control"]); 3]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn direct_programmer_writes_resolve_configured_fade_for_recording() {
    let (state, data_dir) = test_state();
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            groups: vec![light_programmer::GroupDefinition {
                id: "1".into(),
                name: "Front".into(),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, session_id) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let fixture = light_core::FixtureId::new();

    let fixture_response = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "fixture-fade".into(),
            session_id: SessionId(Uuid::parse_str(&session_id).unwrap()),
            expected_revision: None,
            command: "programmer.set".into(),
            payload: serde_json::json!({
                "fixture_id": fixture,
                "attribute": "intensity",
                "value": 0.75
            }),
        },
    );
    assert!(fixture_response.ok);

    let group_response = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "group-fade".into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.group.set".into(),
            payload: serde_json::json!({
                "group_id": "1",
                "attribute": "intensity",
                "value": 0.5
            }),
        },
    );
    assert!(group_response.ok);

    let direct = state.programmers.get(session.id).unwrap();
    assert_eq!(direct.values[0].fade_millis, Some(3_000));
    assert_eq!(
        direct.group_values["1"][&light_core::AttributeKey::intensity()].fade_millis,
        Some(3_000)
    );

    execute_programmer_command(&state, &session, "GROUP 1 AT 25").unwrap();
    let command = state.programmers.get(session.id).unwrap();
    assert_eq!(
        command.group_values["1"][&light_core::AttributeKey::intensity()].fade_millis,
        Some(3_000),
        "commands without TIME resolve Programmer Fade when the value is written"
    );
    let recorded = programmer_cue(&command, 1.0, CommandTiming::default());
    assert_eq!(recorded.changes[0].fade_millis, Some(3_000));
    assert_eq!(recorded.group_changes[0].fade_millis, Some(3_000));
    assert_eq!(
        recorded.fade_millis, 0,
        "Programmer Fade is per change, not Cue TIME"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}
