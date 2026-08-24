#[tokio::test(start_paused = true)]
async fn timed_control_action_is_transient_and_reveals_latched_fan_value_at_deadline() {
    let (state, data_dir) = test_state();
    let task_cancellation = CancellationToken::new();
    let task_receiver = state.lifecycle.take_task_receiver().unwrap();
    let task_driver = tokio::spawn(
        super::capabilities::runtime::supervisor::drive_owned_tasks(
            task_cancellation.clone(),
            task_receiver,
        ),
    );
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "timed-control-action".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());

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
        .output.replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();

    let fan_response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "fan-max",
            light_wire::v2::live_action::LiveAction::FixtureControl(
                light_wire::v2::live_action::FixtureControlLiveActionRequest {
                    request_id: "fan-max".into(),
                    fixture_id: fixture_id.0,
                    action_id: fan_action_id,
                    active: true,
                },
            ),
        ),
    );
    assert!(fan_response.ok, "{:?}", fan_response.error);

    let response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "timed-pulse",
            light_wire::v2::live_action::LiveAction::FixtureControl(
                light_wire::v2::live_action::FixtureControlLiveActionRequest {
                    request_id: "timed-pulse".into(),
                    fixture_id: fixture_id.0,
                    action_id,
                    active: true,
                },
            ),
        ),
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
    let programmer = state.programming.get(session.id).unwrap();
    assert_eq!(transient_raw_values(&programmer), expected_active);
    assert_eq!(persistent_raw_values(&programmer), expected_fan_max);
    assert_eq!(
        persistent_raw_values(&persisted_programmer(&state, session.id)),
        expected_fan_max
    );
    assert!(persisted_programmer(&state, session.id).transient_values.is_empty());
    let values_filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_values(),
    );
    let light_application::EventReplay::Events(values_events) =
        state.events.replay(0, &values_filter)
    else {
        panic!("expected replayable typed Programmer values events");
    };
    assert_eq!(values_events.len(), 1);
    assert!(values_events.iter().all(|event| matches!(
        event.payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::ValuesChanged(_)
        )
    )));
    assert!(state.events.audit_events().is_empty());

    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(749)).await;
    tokio::task::yield_now().await;
    assert_eq!(
        persistent_raw_values(&persisted_programmer(&state, session.id)),
        expected_fan_max
    );

    tokio::time::advance(Duration::from_millis(1)).await;
    tokio::task::yield_now().await;
    let programmer = state.programming.get(session.id).unwrap();
    assert!(transient_raw_values(&programmer).is_empty());
    assert_eq!(persistent_raw_values(&programmer), expected_fan_max);
    assert_eq!(
        persistent_raw_values(&persisted_programmer(&state, session.id)),
        expected_fan_max
    );
    let events = state.events.audit_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "programmer_changed");
    assert_eq!(events[0].payload["action_id"], action_id.to_string());
    assert_eq!(events[0].payload["active"], false);
    assert_eq!(events[0].payload["timed_pulse_complete"], true);
    assert_eq!(
        events[0].payload["changes"],
        serde_json::json!(["transient_control"])
    );
    drop(events);

    task_cancellation.cancel();
    task_driver.await.unwrap().unwrap();
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
    state.active_show.replace_current(Some(entry.clone()));
    state
        .output.replace_snapshot(load_engine_snapshot(&entry).unwrap())
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

    let response = generate_profile_presets_action(
        &state,
        vec![fixture_id],
        light_application::ActionContext::system(
            Uuid::nil(),
            light_application::ActionSource::Http,
        )
        .with_request_id("generate-profile-presets")
        .with_expected_revision(0),
        show_id,
    )
    .unwrap();

    assert_eq!(response.created[0].name, "Dots");
    assert_eq!(
        response.created[0].address.family,
        light_wire::v2::preset_recording::PresetRecordingFamily::Beam
    );
    assert_eq!(response.created[0].address.number, 1);
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
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "highlight-safety".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    state
        .output.replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    state.programming.select(session.id, [fixture_id]);
    let fixtures = highlight_fixture_summaries(&state.output.snapshot().fixtures);
    let groups = HashMap::new();
    let selection = state.programming.selection(session.id).unwrap();
    state
        .highlight
        .apply_action(
                        HighlightAction::On,
            &selection,
            &fixtures,
            &groups,
            false,
        );
    sync_highlight_output(&state);
    assert_eq!(state.output.highlighted_fixtures(), vec![fixture_id]);

    let blind = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "blind",
            light_wire::v2::live_action::LiveAction::ProgrammerCaptureMode(
                light_wire::v2::live_action::ProgrammerCaptureModeLiveActionRequest {
                    request_id: "blind".into(),
                    blind: Some(true),
                    preview: None,
                    active_context: None,
                },
            ),
        ),
    );
    assert!(blind.ok, "{:?}", blind.error);
    assert!(state.output.highlighted_fixtures().is_empty());

    state
        .programming
        .set_modes(session.id, Some(false), None, None, None);
    state
        .highlight
        .apply_action(
                        HighlightAction::On,
            &selection,
            &fixtures,
            &groups,
            false,
        );
    sync_highlight_output(&state);
    assert_eq!(state.output.highlighted_fixtures(), vec![fixture_id]);

    let preview = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "preview",
            light_wire::v2::live_action::LiveAction::ProgrammerCaptureMode(
                light_wire::v2::live_action::ProgrammerCaptureModeLiveActionRequest {
                    request_id: "preview".into(),
                    blind: None,
                    preview: Some(true),
                    active_context: None,
                },
            ),
        ),
    );
    assert!(preview.ok, "{:?}", preview.error);
    assert!(state.output.highlighted_fixtures().is_empty());
    let preview_state = current_highlight_transition(&state, &session).unwrap();
    assert!(preview_state.state.active);
    assert!(preview_state.state.capture_only);
    assert!(!preview_state.state.output_enabled);

    let leave_preview = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "leave-preview",
            light_wire::v2::live_action::LiveAction::ProgrammerCaptureMode(
                light_wire::v2::live_action::ProgrammerCaptureModeLiveActionRequest {
                    request_id: "leave-preview".into(),
                    blind: None,
                    preview: Some(false),
                    active_context: None,
                },
            ),
        ),
    );
    assert!(leave_preview.ok, "{:?}", leave_preview.error);
    assert_eq!(state.output.highlighted_fixtures(), vec![fixture_id]);

    let preload = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "preload",
            light_wire::v2::live_action::LiveAction::ProgrammerPreloadLifecycle(
                light_wire::v2::preload_lifecycle::ProgrammingPreloadLifecycleRequest {
                    request_id: "preload".into(),
                    expected_capture_mode_revision: state
                        .programming
                        .capture_mode_revision(),
                    expected_values_revision: state.programming.preload_values_revision(),
                    expected_queue_revision: state
                        .programming
                        .preload_playback_queue_revision(),
                    expected_selection_revision: state
                        .programming
                        .selection(session.id)
                        .unwrap()
                        .revision,
                    action: light_wire::v2::preload_lifecycle::ProgrammingPreloadLifecycleAction::Enter {},
                },
            ),
        ),
    );
    assert!(preload.ok, "{:?}", preload.error);
    assert!(state.output.highlighted_fixtures().is_empty());
    let state_after_preload = state.highlight.transition(
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
