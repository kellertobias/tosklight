#[tokio::test]
async fn rejected_command_line_does_not_poison_later_preload_execution() {
    let (state, data_dir) = test_state();
    let base = schema_v2_direct_fixture().0;
    let fixtures = (1..=3)
        .map(|number| {
            let mut fixture = base.clone();
            fixture.fixture_id = light_core::FixtureId::new();
            fixture.fixture_number = Some(number);
            fixture.address = Some(1 + (number as u16 - 1) * 3);
            fixture
        })
        .collect::<Vec<_>>();
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.clone().into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    assert!(state.programming.arm_preload(session.id, true));

    let execute = |request_id: &str, value: &str| {
        dispatch_live_action(
            &state,
            &session,
            live_action_frame(
                &session,
                request_id,
                serde_json::from_value(serde_json::json!({
                    "type": "command_line_execute",
                    "request": {"value": value}
                }))
                .unwrap(),
            ),
        )
    };

    let first = execute("preload-valid-before", "FIXTURE 1 AT 25");
    assert!(first.ok, "{:?}", first.error);

    let invalid_command = "FIXTURE 2 THRU 3 + 1 AT NOPE";
    let invalid = execute("preload-invalid-between", invalid_command);
    assert!(!invalid.ok);
    assert_eq!(invalid.error.as_deref(), Some("level must be a percentage or FULL"));
    let after_invalid = state.programming.get(session.id).unwrap();
    assert_eq!(after_invalid.command_line, invalid_command);
    assert_eq!(after_invalid.selected, vec![fixtures[0].fixture_id]);
    assert_eq!(after_invalid.preload_pending.len(), 1);

    let last = execute("preload-valid-after", "AT 75");
    assert!(last.ok, "{:?}", last.error);
    let programmer = state.programming.get(session.id).unwrap();
    assert_eq!(programmer.selected, vec![fixtures[0].fixture_id]);
    assert_eq!(programmer.preload_pending.len(), 1);
    assert_eq!(
        programmer.preload_pending[0].fixture_id,
        fixtures[0].fixture_id
    );
    assert_eq!(
        programmer.preload_pending[0].value,
        light_core::AttributeValue::Normalized(0.75)
    );
    assert!(programmer
        .preload_pending
        .iter()
        .all(|value| value.fixture_id != fixtures[1].fixture_id
            && value.fixture_id != fixtures[2].fixture_id));
    let history = state.programming.command_history(session.desk.id);
    assert!(history.iter().any(|entry| {
        entry.command == invalid_command
            && entry.status == "rejected"
            && entry.feedback == "level must be a percentage or FULL"
    }));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn programmer_set_many_validates_then_applies_one_faded_undo_step() {
    let (state, data_dir) = test_state();
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    state
        .output.replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    state.action_timing.begin_causal_origin(
        session.id.0.to_string(),
        "osc",
        "home",
        state.output.frame_rate_hz(),
        OscActionFeedback {
            path: "main".into(),
            target: "127.0.0.1:9010".parse().unwrap(),
        },
    );
    assert!(
        !state
            .integrations
            .captured_osc_feedback()
            .iter()
            .any(|(_, address, arguments)| {
                address == "/light/main/feedback/action"
                    && arguments.first() == Some(&OscArgument::String("home".into()))
            }),
        "causal encoder feedback waits for the authoritative WebSocket action"
    );

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
    let timing = response
        .action_timing
        .expect("Programmer WebSocket acknowledgement carries timing");
    assert_eq!(timing.action, "values");
    assert_eq!(timing.source, "osc");
    assert_eq!(timing.request_id, "home");
    assert_eq!(timing.budget_ticks, 2);
    assert!(timing.acknowledgement_within_budget);
    assert_eq!(timing.first_output_tick, None);
    let feedback = state
        .integrations
        .captured_osc_feedback()
        .into_iter()
        .filter(|(_, address, arguments)| {
            address == "/light/main/feedback/action"
                && arguments.first() == Some(&OscArgument::String("home".into()))
        })
        .collect::<Vec<_>>();
    assert_eq!(feedback.len(), 1);
    assert_eq!(feedback[0].0, "127.0.0.1:9010".parse().unwrap());
    let output_tick = state.action_timing.begin_output_render();
    state.action_timing.complete_output_render(output_tick);
    let completed = state.action_timing.snapshot();
    assert_eq!(completed[0].first_output_tick, Some(1));
    assert_eq!(completed[0].output_within_budget, Some(true));
    let values = state.programming.get(session.id).unwrap().values;
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
        serde_json::to_value(state.programming.get(session.id).unwrap().values).unwrap(),
        serde_json::to_value(values).unwrap()
    );
    assert!(state.programming.undo(session.id));
    assert!(state.programming.get(session.id).unwrap().values.is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn websocket_actions_are_typed_owned_and_revision_checked() {
    let (state, data_dir) = test_state();
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    state
        .output.replace_snapshot(EngineSnapshot {
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
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "same-user".into(),
        connected: true,
        desk: session.desk.clone(),
    };
    state
        .programming
        .start(same_user_session.id);
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

    let other_session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "other-user".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state
        .programming
        .start(other_session.id);
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
                    "expected_revision":2,
                    "expected_capture_mode_revision":0,
                    "action":{"type":"set_fixture","fixture_id":fixture_id,
                        "attribute":"intensity","value":{"kind":"normalized","value":0.2}}
                }
            }))
            .unwrap(),
        ),
    );
    // A second surface writes into the desk's one Programmer, so it must carry the revision that
    // Programmer is actually at rather than starting from zero.
    assert!(
        competing_update.ok,
        "every surface writes the desk's one Programmer: {:?}",
        competing_update.error
    );
    assert_eq!(
        state.programming.get(session.id).unwrap().id,
        state.programming.get(other_session.id).unwrap().id
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
                    "expected_revision":3,
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
async fn group_master_rejects_exclusive_show_change_but_not_output_read_permit() {
    let (state, data_dir) = test_state();
    let target = light_playback::PlaybackTarget::Group {
        group_id: "front".into(),
        initial_master: None,
    };
    state
        .output.replace_snapshot(EngineSnapshot {
            groups: vec![light_programmer::GroupDefinition {
                id: "front".into(),
                ..Default::default()
            }].into(),
            playbacks: vec![light_playback::PlaybackDefinition {
                number: 1,
                name: "Front master".into(),
                buttons: light_playback::PlaybackDefinition::default_buttons(&target),
                button_count: 3,
                fader: light_playback::PlaybackDefinition::default_fader(&target),
                has_fader: true,
        footprint: light_playback::PlaybackFootprint::Normal,
                go_activates: true,
                auto_off: false,
                xfade_millis: 0,
                color: "#20c997".into(),
                flash_release: light_playback::FlashReleaseMode::ReleaseAll,
                protect_from_swap: false,
                presentation_icon: None,
                presentation_image: None,
                target,
            }]
            .into(),
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

    let activation = state.active_show.acquire().await;
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
    assert_eq!(state.output.group_master("front"), Some(1.0));

    drop(activation);
    let output_read = state.active_show.acquire_shared().await;
    let applied = dispatch_live_action(
        &state,
        &session,
        live_action_frame(&session, "group-master", action),
    );
    assert!(applied.ok, "{:?}", applied.error);
    assert_eq!(state.output.group_master("front"), Some(0.5));
    drop(output_read);
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
        .output.replace_snapshot(EngineSnapshot {
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
        state.events.replay(0, &filter)
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
        state.events.replay(0, &filter)
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

    let sequence = state.events.latest_sequence();
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
    assert_eq!(state.events.latest_sequence(), sequence + 1);
    let filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_priority(),
    );
    let light_application::EventReplay::Events(events) =
        state.events.replay(sequence, &filter)
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
        .output.replace_snapshot(EngineSnapshot {
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
        light_application::EventObject::programming_values(),
    );
    let before = state.events.latest_sequence();
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
        state.events.replay(before, &values_filter)
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
        state.events.replay(0, &selection_filter)
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
        state.events.replay(0, &values_filter)
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
        light_application::EventObject::programming_preload_values(),
    );
    let light_application::EventReplay::Events(events) =
        state.events.replay(0, &preload_filter)
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
        light_application::EventObject::programming_priority(),
    );
    let light_application::EventReplay::Events(events) =
        state.events.replay(0, &priority_filter)
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
        .output.replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    let before_application_sequence = state.events.latest_sequence();
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
                .programming
                .get(session.id)
                .unwrap()
                .transient_values
                .is_empty(),
            !expect_transient_values
        );
    }
    let values_filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_values(),
    );
    let light_application::EventReplay::Events(events) = state
        .events
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
async fn combined_indexed_control_targets_share_one_transient_lifetime() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let (first, action_id, _) = schema_v2_direct_fixture();
    let mut second = first.clone();
    second.fixture_id = light_core::FixtureId::new();
    second.fixture_number = Some(2);
    second.address = Some(3);
    let fixture_ids = [first.fixture_id, second.fixture_id];
    let profile_revision = first.definition.profile_snapshot.as_ref().unwrap().revision;
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![first, second].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    let selection_revision = state.programming.select(session.id, fixture_ids);

    for (request_id, active, expected_values) in
        [("combined-control-on", true, 4), ("combined-control-off", false, 0)]
    {
        let response = dispatch_live_action(
            &state,
            &session,
            live_action_frame(
                &session,
                request_id,
                serde_json::from_value(serde_json::json!({
                    "type":"fixture_controls",
                    "request":{
                        "request_id":request_id,
                        "expected_selection_revision":selection_revision,
                        "targets":[
                            {
                                "fixture_id":fixture_ids[0],
                                "action_id":action_id,
                                "expected_profile_revision":profile_revision
                            },
                            {
                                "fixture_id":fixture_ids[1],
                                "action_id":action_id,
                                "expected_profile_revision":profile_revision
                            }
                        ],
                        "active":active
                    }
                }))
                .unwrap(),
            ),
        );
        assert!(response.ok, "{:?}", response.error);
        let programmer = state.programming.get(session.id).unwrap();
        assert!(programmer.transient_values.len() <= 1);
        assert_eq!(
            programmer
                .transient_values
                .first()
                .map_or(0, |action| action.values.len()),
            expected_values
        );
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn direct_programmer_writes_preserve_resolved_fade_for_recording() {
    let (state, data_dir) = test_state();
    state.installation.update_configuration(|configuration| {
        configuration.command_line_at_uses_programmer_fade = true;
    });
    let fixture_definition = schema_v2_direct_fixture().0;
    let fixture = fixture_definition.fixture_id;
    state
        .output.replace_snapshot(EngineSnapshot {
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

    let direct = state.programming.get(session.id).unwrap();
    assert_eq!(direct.values[0].fade_millis, Some(3_000));
    assert_eq!(
        direct.group_values["1"][&light_core::AttributeKey::intensity()].fade_millis,
        Some(3_000)
    );

    execute_programmer_command(&state, &session, "GROUP 1 AT 25").unwrap();
    let command = state.programming.get(session.id).unwrap();
    assert_eq!(
        command.group_values["1"][&light_core::AttributeKey::intensity()].fade_millis,
        Some(3_000),
        "commands without TIME resolve Programmer Fade when the value is written"
    );
    let recorded = programmer_cue(&command, cue("1"), CommandTiming::default());
    assert_eq!(recorded.changes[0].fade_millis, Some(3_000));
    assert_eq!(recorded.group_changes[0].fade_millis, Some(3_000));
    assert_eq!(
        recorded.fade_millis, 0,
        "Programmer Fade is per change, not Cue TIME"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}
