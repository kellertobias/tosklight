#[tokio::test]
async fn programmer_set_many_validates_then_applies_one_faded_undo_step() {
    let (state, data_dir) = test_state();
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();

    let response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "home",
            serde_json::from_value(serde_json::json!({
                "type":"programming_values",
                "request":{
                    "request_id":"home",
                    "expected_revision":0,
                    "expected_capture_mode_revision":0,
                    "action":{"type":"batch","mutations":[
                        {"type":"set_fixture","fixture_id":fixture_id,"attribute":"pan",
                         "value":{"kind":"normalized","value":0.25},"timing":{"fade":true}},
                        {"type":"set_fixture","fixture_id":fixture_id,"attribute":"tilt",
                         "value":{"kind":"normalized","value":0.75},"timing":{"fade":true}}
                    ]}
                }
            }))
            .unwrap(),
        ),
    );
    assert!(response.ok, "{:?}", response.error);
    let values = state.programmers.get(session.id).unwrap().values;
    assert_eq!(values.len(), 2);
    assert!(values.iter().all(|value| value.fade));
    assert_eq!(values[0].changed_at, values[1].changed_at);

    let rejected = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "invalid-home",
            serde_json::from_value(serde_json::json!({
                "type":"programming_values",
                "request":{
                    "request_id":"invalid-home",
                    "expected_revision":1,
                    "expected_capture_mode_revision":0,
                    "action":{"type":"batch","mutations":[
                        {"type":"set_fixture","fixture_id":fixture_id,"attribute":"pan",
                         "value":{"kind":"normalized","value":0.5}},
                        {"type":"set_fixture","fixture_id":light_core::FixtureId::new(),
                         "attribute":"tilt","value":{"kind":"normalized","value":0.5}}
                    ]}
                }
            }))
            .unwrap(),
        ),
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
async fn websocket_actions_are_typed_owned_and_revision_checked() {
    let (state, data_dir) = test_state();
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();

    let response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "set-1",
            serde_json::from_value(serde_json::json!({
                "type":"programming_values",
                "request":{
                    "request_id":"set-1",
                    "expected_revision":0,
                    "expected_capture_mode_revision":0,
                    "action":{"type":"set_fixture","fixture_id":fixture_id,
                        "attribute":"intensity","value":{"kind":"normalized","value":0.75}}
                }
            }))
            .unwrap(),
        ),
    );
    assert!(response.ok, "{:?}", response.error);

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
    let same_user_update = dispatch_live_action(
        &state,
        &same_user_session,
        live_action_frame(
            &same_user_session,
            "same-user",
            serde_json::from_value(serde_json::json!({
                "type":"programming_values",
                "request":{
                    "request_id":"same-user",
                    "expected_revision":1,
                    "expected_capture_mode_revision":0,
                    "action":{"type":"set_fixture","fixture_id":fixture_id,
                        "attribute":"intensity","value":{"kind":"normalized","value":0.5}}
                }
            }))
            .unwrap(),
        ),
    );
    assert!(same_user_update.ok, "one user owns authority across their sessions");

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
    let competing_update = dispatch_live_action(
        &state,
        &other_session,
        live_action_frame(
            &other_session,
            "other-user",
            serde_json::from_value(serde_json::json!({
                "type":"programming_values",
                "request":{
                    "request_id":"other-user",
                    "expected_revision":0,
                    "expected_capture_mode_revision":0,
                    "action":{"type":"set_fixture","fixture_id":fixture_id,
                        "attribute":"intensity","value":{"kind":"normalized","value":0.2}}
                }
            }))
            .unwrap(),
        ),
    );
    assert!(competing_update.ok, "different users own independent programmers");
    assert_ne!(
        state.programmers.get(session.id).unwrap().id,
        state.programmers.get(other_session.id).unwrap().id
    );

    let mut foreign = live_action_frame(
        &session,
        "foreign",
        light_wire::v2::live_action::LiveAction::ProgrammerUndo,
    );
    foreign.session_id = Uuid::new_v4();
    let foreign = dispatch_live_action(&state, &session, foreign);
    assert!(!foreign.ok);
    assert!(foreign.error.unwrap().contains("does not own"));

    let clear = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "clear",
            serde_json::from_value(serde_json::json!({
                "type":"programming_values",
                "request":{
                    "request_id":"clear",
                    "expected_revision":2,
                    "expected_capture_mode_revision":0,
                    "action":{"type":"clear"}
                }
            }))
            .unwrap(),
        ),
    );
    assert!(clear.ok, "{:?}", clear.error);

    let mut unsupported = live_action_frame(
        &session,
        "unsupported",
        light_wire::v2::live_action::LiveAction::ProgrammerUndo,
    );
    unsupported.protocol_version = 1;
    let unsupported = dispatch_live_action(&state, &session, unsupported);
    assert!(!unsupported.ok);
    assert!(unsupported.error.unwrap().contains("unsupported protocol_version"));
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
            }].into(),
            ..Default::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let action: light_wire::v2::live_action::LiveAction =
        serde_json::from_value(serde_json::json!({
            "type":"playback",
            "request":{
                "request_id":"group-master",
                "address":{"kind":"group","group_id":"front"},
                "action":{"type":"master","value":0.5},
                "surface":"virtual"
            }
        }))
        .unwrap();

    let activation = state.activation_lock.clone().lock_owned().await;
    let rejected = dispatch_live_action(
        &state,
        &session,
        live_action_frame(&session, "group-master", action.clone()),
    );
    assert!(!rejected.ok);
    assert!(
        rejected
            .error
            .as_deref()
            .is_some_and(|error| error.contains("active show is changing"))
    );
    assert_eq!(state.engine.snapshot().groups[0].master, 1.0);

    drop(activation);
    let applied = dispatch_live_action(
        &state,
        &session,
        live_action_frame(&session, "group-master", action),
    );
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
    let fixture_definition = schema_v2_direct_fixture().0;
    let fixture = fixture_definition.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture_definition].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();

    let response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "selection-event",
            serde_json::from_value(serde_json::json!({
                "type":"programming_selection",
                "request":{
                    "request_id":"selection-event",
                    "action":"replace",
                    "fixtures":[fixture],
                    "expected_revision":0
                }
            }))
            .unwrap(),
        ),
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

    let response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "command-event",
            serde_json::from_value(serde_json::json!({
                "type":"command_line_set",
                "request":{"value":"GROUP 2"}
            }))
            .unwrap(),
        ),
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
    let value_only = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "priority-only",
            serde_json::from_value(serde_json::json!({
                "type":"programmer_priority",
                "request":{
                    "request_id":"priority-only",
                    "expected_revision":0,
                    "priority":7
                }
            }))
            .unwrap(),
        ),
    );
    assert!(value_only.ok, "{:?}", value_only.error);
    assert_eq!(state.application_events.latest_sequence(), sequence + 1);
    let filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_priority(session.user.id.0),
    );
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(sequence, &filter)
    else {
        panic!("the priority event should remain replayable")
    };
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::PriorityChanged(_)
        )
    ));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn live_actions_publish_only_authoritative_changed_projections() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let fixture_definition = schema_v2_direct_fixture().0;
    let fixture = fixture_definition.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture_definition].into(),
            groups: vec![light_programmer::GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                ..Default::default()
            }]
            .into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    let frame = |request_id: &str, action: light_wire::v2::live_action::LiveAction| {
        live_action_frame(&session, request_id, action)
    };
    let values_filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_values(session.user.id.0),
    );
    let before = state.application_events.latest_sequence();
    let no_op = dispatch_live_action(
        &state,
        &session,
        frame(
            "no-op-undo",
            light_wire::v2::live_action::LiveAction::ProgrammerUndo,
        ),
    );
    assert!(no_op.ok, "{:?}", no_op.error);
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(before, &values_filter)
    else {
        panic!("the values event stream should remain replayable")
    };
    assert!(events.is_empty());

    let selection = dispatch_live_action(
        &state,
        &session,
        frame(
            "selection-change",
            serde_json::from_value(serde_json::json!({
                "type":"programming_selection",
                "request":{
                    "request_id":"selection-change",
                    "action":"replace",
                    "fixtures":[fixture],
                    "expected_revision":0
                }
            }))
            .unwrap(),
        ),
    );
    assert!(selection.ok, "{:?}", selection.error);
    let selection_filter = light_application::EventFilter::for_desk(session.desk.id).with_object(
        light_application::EventObject::programming_selection(session.desk.id),
    );
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &selection_filter)
    else {
        panic!("the selection event should remain replayable")
    };
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::InteractionChanged(_)
        )
    ));

    let values = dispatch_live_action(
        &state,
        &session,
        frame(
            "values-change",
            serde_json::from_value(serde_json::json!({
                "type":"programming_values",
                "request":{
                    "request_id":"values-change",
                    "expected_revision":0,
                    "expected_capture_mode_revision":0,
                    "action":{
                        "type":"set_fixture",
                        "fixture_id":fixture,
                        "attribute":"intensity",
                        "value":{"kind":"normalized","value":0.75}
                    }
                }
            }))
            .unwrap(),
        ),
    );
    assert!(values.ok, "{:?}", values.error);
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &values_filter)
    else {
        panic!("the values event should remain replayable")
    };
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::ValuesChanged(_)
        )
    ));

    let preload_enter = dispatch_live_action(
        &state,
        &session,
        frame(
            "preload-enter",
            serde_json::from_value(serde_json::json!({
                "type":"programmer_preload_lifecycle",
                "request":{
                    "request_id":"preload-enter",
                    "expected_capture_mode_revision":0,
                    "expected_values_revision":0,
                    "expected_queue_revision":0,
                    "expected_selection_revision":1,
                    "action":{"type":"enter"}
                }
            }))
            .unwrap(),
        ),
    );
    assert!(preload_enter.ok, "{:?}", preload_enter.error);
    let preload_values = dispatch_live_action(
        &state,
        &session,
        frame(
            "preload-values",
            serde_json::from_value(serde_json::json!({
                "type":"programmer_preload_values",
                "request":{
                    "request_id":"preload-values",
                    "expected_revision":0,
                    "expected_capture_mode_revision":1,
                    "action":{
                        "type":"set_group",
                        "group_id":"front",
                        "attribute":"intensity",
                        "value":{"kind":"normalized","value":0.5}
                    }
                }
            }))
            .unwrap(),
        ),
    );
    assert!(preload_values.ok, "{:?}", preload_values.error);
    let preload_filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_preload_values(session.user.id.0),
    );
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &preload_filter)
    else {
        panic!("the preload values event should remain replayable")
    };
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::PreloadValuesChanged(_)
        )
    ));

    let priority = dispatch_live_action(
        &state,
        &session,
        frame(
            "priority-only",
            serde_json::from_value(serde_json::json!({
                "type":"programmer_priority",
                "request":{
                    "request_id":"priority-only",
                    "expected_revision":0,
                    "priority":7
                }
            }))
            .unwrap(),
        ),
    );
    assert!(priority.ok, "{:?}", priority.error);
    let priority_filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_priority(session.user.id.0),
    );
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &priority_filter)
    else {
        panic!("the priority event should remain replayable")
    };
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::PriorityChanged(_)
        )
    ));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn transient_control_retriggers_remain_projection_quiet_and_repeated_release_is_quiet() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let (fixture, action_id, _) = schema_v2_direct_fixture();
    let fixture_id = fixture.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    let before_application_sequence = state.application_events.latest_sequence();
    for (request_id, active, expect_transient_values) in [
        ("momentary-on", true, true),
        ("momentary-retrigger", true, true),
        ("momentary-off", false, false),
        ("momentary-off-again", false, false),
    ] {
        let response = dispatch_live_action(
            &state,
            &session,
            live_action_frame(
                &session,
                request_id,
                serde_json::from_value(serde_json::json!({
                    "type":"fixture_control",
                    "request":{
                        "request_id":request_id,
                        "fixture_id":fixture_id,
                        "action_id":action_id,
                        "active":active
                    }
                }))
                .unwrap(),
            ),
        );
        assert!(response.ok, "{:?}", response.error);
        assert_eq!(
            state
                .programmers
                .get(session.id)
                .unwrap()
                .transient_values
                .is_empty(),
            !expect_transient_values
        );
    }
    let values_filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_values(session.user.id.0),
    );
    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(before_application_sequence, &values_filter)
    else {
        panic!("the values event stream should remain replayable")
    };
    assert!(
        events.is_empty(),
        "transient controls do not mutate the authoritative persisted values projection"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn direct_programmer_writes_preserve_resolved_fade_for_recording() {
    let (state, data_dir) = test_state();
    let fixture_definition = schema_v2_direct_fixture().0;
    let fixture = fixture_definition.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture_definition].into(),
            groups: vec![light_programmer::GroupDefinition {
                id: "1".into(),
                name: "Front".into(),
                ..Default::default()
            }].into(),
            ..Default::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();

    let fixture_response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "fixture-fade",
            serde_json::from_value(serde_json::json!({
                "type":"programming_values",
                "request":{
                    "request_id":"fixture-fade",
                    "expected_revision":0,
                    "expected_capture_mode_revision":0,
                    "action":{
                        "type":"set_fixture",
                        "fixture_id":fixture,
                        "attribute":"intensity",
                        "value":{"kind":"normalized","value":0.75},
                        "timing":{"fade":true,"fade_millis":3000}
                    }
                }
            }))
            .unwrap(),
        ),
    );
    assert!(fixture_response.ok);

    let group_response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "group-fade",
            serde_json::from_value(serde_json::json!({
                "type":"programming_values",
                "request":{
                    "request_id":"group-fade",
                    "expected_revision":1,
                    "expected_capture_mode_revision":0,
                    "action":{
                        "type":"set_group",
                        "group_id":"1",
                        "attribute":"intensity",
                        "value":{"kind":"normalized","value":0.5},
                        "timing":{"fade":true,"fade_millis":3000}
                    }
                }
            }))
            .unwrap(),
        ),
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
