#[tokio::test]
async fn priority_snapshot_and_action_are_exact_user_shared_sparse_and_replay_safe() {
    let scenario = CommandHttpScenario::new().await;
    let user_id = scenario.session.user.id.0;
    let initial = scenario
        .priority_snapshot_for(user_id, Some(&scenario.token))
        .await;
    assert_eq!(initial.status(), StatusCode::OK);
    assert_eq!(initial.headers()[header::ETAG], "\"0\"");
    let initial: light_wire::v2::programmer_priority::ProgrammerPrioritySnapshot =
        serde_json::from_value(json(initial).await).unwrap();
    assert_eq!(initial.projection.user_id, user_id);
    assert_eq!(initial.projection.priority, 100);

    assert_eq!(
        scenario
            .priority_snapshot_for(Uuid::new_v4(), Some(&scenario.token))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        scenario.priority_snapshot_for(user_id, None).await.status(),
        StatusCode::UNAUTHORIZED
    );

    let second_desk = scenario
        .state
        .desk
        .lock()
        .add_desk("Priority peer", "priority-peer")
        .unwrap();
    let (second_token, second_user) = login_on_desk(&scenario, "Operator", second_desk.id).await;
    assert_eq!(second_user, user_id);
    let request = serde_json::json!({
        "request_id":"priority-http-1",
        "expected_revision":0,
        "priority":75,
    });
    let compatibility_before = compatibility_event_count(&scenario.state);
    let activation = scenario.state.activation_lock.clone().lock_owned().await;
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        scenario.priority_action_for(user_id, &scenario.token, request.clone()),
    )
    .await
    .expect("user-owned priority must remain available while the active Show changes");
    drop(activation);
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let changed: light_wire::v2::programmer_priority::ProgrammerPriorityActionOutcome =
        serde_json::from_value(json(response).await).unwrap();
    assert_eq!(changed.projection.revision, 1);
    assert_eq!(changed.projection.priority, 75);
    assert!(matches!(
        changed.outcome,
        light_wire::v2::programmer_priority::ProgrammerPriorityActionState::Changed { .. }
    ));
    assert_eq!(compatibility_event_count(&scenario.state), compatibility_before);

    let peer = scenario
        .priority_snapshot_for(second_user, Some(&second_token))
        .await;
    let peer: light_wire::v2::programmer_priority::ProgrammerPrioritySnapshot =
        serde_json::from_value(json(peer).await).unwrap();
    assert_eq!(peer.projection, changed.projection);

    let event_count = priority_event_count(&scenario.state, user_id);
    assert_eq!(event_count, 1);
    let replay = scenario
        .priority_action_for(user_id, &scenario.token, request)
        .await;
    let replay: light_wire::v2::programmer_priority::ProgrammerPriorityActionOutcome =
        serde_json::from_value(json(replay).await).unwrap();
    assert!(replay.replayed);
    assert_eq!(priority_event_count(&scenario.state, user_id), event_count);
    assert_eq!(compatibility_event_count(&scenario.state), compatibility_before);

    let no_change = scenario
        .priority_action_for(
            user_id,
            &second_token,
            serde_json::json!({
                "request_id":"priority-http-no-change",
                "expected_revision":1,
                "priority":75,
            }),
        )
        .await;
    let no_change: light_wire::v2::programmer_priority::ProgrammerPriorityActionOutcome =
        serde_json::from_value(json(no_change).await).unwrap();
    assert!(matches!(
        no_change.outcome,
        light_wire::v2::programmer_priority::ProgrammerPriorityActionState::NoChange
    ));
    assert_eq!(priority_event_count(&scenario.state, user_id), event_count);

    let conflict = scenario
        .priority_action_for(
            user_id,
            &scenario.token,
            serde_json::json!({
                "request_id":"priority-http-conflict",
                "expected_revision":0,
                "priority":30,
            }),
        )
        .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(json(conflict).await["current_revision"], 1);
    assert_eq!(priority_event_count(&scenario.state, user_id), event_count);

    let foreign = scenario
        .priority_action_for(
            Uuid::new_v4(),
            &scenario.token,
            serde_json::json!({
                "request_id":"priority-http-foreign",
                "expected_revision":1,
                "priority":20,
            }),
        )
        .await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preset_recall_uses_one_portable_show_graph_and_one_values_event() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Preset recall route").await;
    let selected = [light_core::FixtureId::new(), light_core::FixtureId::new()];
    let group = light_programmer::GroupDefinition {
        id: "5".into(),
        name: "Document group".into(),
        fixtures: vec![selected[0]],
        ..Default::default()
    };
    assert_eq!(
        scenario
            .put_active_object(
                &show_id,
                "group",
                "5",
                0,
                serde_json::to_value(group).unwrap(),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let preset = light_programmer::Preset {
        name: "Document look".into(),
        family: light_programmer::PresetFamily::Intensity,
        number: 1,
        values: HashMap::new(),
        group_values: HashMap::from([(
            "5".into(),
            HashMap::from([(
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Normalized(0.65),
            )]),
        )]),
    };
    let mut preset_body = serde_json::to_value(preset).unwrap();
    preset_body["future_extension"] = serde_json::json!({"retained":true});
    assert_eq!(
        scenario
            .put_active_object(&show_id, "preset", "1.1", 0, preset_body)
            .await
            .status(),
        StatusCode::OK
    );
    let show = scenario.state.active_show.read().clone().unwrap();
    let show_revision = ShowStore::open(&show.path)
        .unwrap()
        .portable_revision()
        .unwrap()
        .value();
    let selection_revision = scenario
        .state
        .programmers
        .select(scenario.session.id, [selected[1], selected[0]]);

    // Deliberately contradict the portable Group graph. Recall must derive Group membership from
    // the exact same portable document and revision as the Preset, not this runtime projection.
    let engine_revision = scenario.state.engine.snapshot().revision;
    scenario
        .state
        .engine
        .replace_snapshot(EngineSnapshot {
            groups: vec![light_programmer::GroupDefinition {
                id: "5".into(),
                fixtures: vec![selected[1]],
                ..Default::default()
            }],
            revision: engine_revision + 1,
            ..EngineSnapshot::default()
        })
        .unwrap();

    let request = preset_recall_request(
        "preset-recall-http",
        show_revision,
        selection_revision,
        0,
    );
    assert_eq!(
        scenario
            .preset_recall_action(&show_id, None, request.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        scenario
            .preset_recall_action(
                &Uuid::new_v4().to_string(),
                Some(&scenario.token),
                request.clone(),
            )
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let mut forged = request.clone();
    forged["values"] = serde_json::json!({"forged":true});
    assert_eq!(
        scenario
            .preset_recall_action(&show_id, Some(&scenario.token), forged)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let baseline = scenario.state.application_events.latest_sequence();
    let compatibility_before = compatibility_event_count(&scenario.state);
    let response = scenario
        .preset_recall_action(&show_id, Some(&scenario.token), request.clone())
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let changed: light_wire::v2::preset_recall::PresetRecallOutcome =
        serde_json::from_value(json(response).await).unwrap();
    assert_eq!(changed.programmer_revision, 1);
    assert_eq!(changed.show_revision, show_revision);
    assert_eq!(changed.preset.revision, 1);
    assert_eq!(changed.preset.body["future_extension"]["retained"], true);
    let light_wire::v2::preset_recall::PresetRecallActionState::Changed {
        projection: Some(projection),
        event_sequence: Some(event_sequence),
    } = changed.outcome
    else {
        panic!("Preset recall should return one authoritative values projection")
    };
    assert_eq!(event_sequence, baseline + 1);
    assert_eq!(projection.fixture_values.len(), 1);
    assert_eq!(projection.fixture_values[0].fixture_id, selected[0].0);
    assert_eq!(values_event_count(&scenario.state, scenario.session.user.id.0), 1);
    assert_eq!(compatibility_event_count(&scenario.state), compatibility_before);

    let replay = scenario
        .preset_recall_action(&show_id, Some(&scenario.token), request)
        .await;
    let replay: light_wire::v2::preset_recall::PresetRecallOutcome =
        serde_json::from_value(json(replay).await).unwrap();
    assert!(replay.replayed);
    assert_eq!(values_event_count(&scenario.state, scenario.session.user.id.0), 1);

    let no_change = scenario
        .preset_recall_action(
            &show_id,
            Some(&scenario.token),
            preset_recall_request(
                "preset-recall-no-change",
                show_revision,
                selection_revision,
                1,
            ),
        )
        .await;
    let no_change: light_wire::v2::preset_recall::PresetRecallOutcome =
        serde_json::from_value(json(no_change).await).unwrap();
    assert!(matches!(
        no_change.outcome,
        light_wire::v2::preset_recall::PresetRecallActionState::NoChange
    ));
    assert_eq!(values_event_count(&scenario.state, scenario.session.user.id.0), 1);

    let conflict = scenario
        .preset_recall_action(
            &show_id,
            Some(&scenario.token),
            preset_recall_request(
                "preset-recall-conflict",
                show_revision,
                selection_revision,
                0,
            ),
        )
        .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(json(conflict).await["current_revision"], 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preset_v1_recall_is_rejected_before_show_reads_while_activation_is_locked() {
    let scenario = CommandHttpScenario::new().await;
    let command = WsCommand {
        protocol_version: 1,
        request_id: "preset-v1-show-changing".into(),
        session_id: scenario.session.id,
        expected_revision: None,
        command: "preset.apply".into(),
        payload: serde_json::json!({"family":"Intensity","number":1}),
    };
    let sequence_before = scenario.state.application_events.latest_sequence();
    let generation_before = scenario
        .state
        .programmers
        .normal_values_generation(scenario.session.id);
    let activation = scenario.state.activation_lock.clone().lock_owned().await;

    let rejected = dispatch_ws_command(&scenario.state, &scenario.session, command);

    assert!(!rejected.ok);
    assert_eq!(
        rejected.error.as_deref(),
        Some("the active show is changing; retry the Programmer action")
    );
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        sequence_before
    );
    assert_eq!(
        scenario
            .state
            .programmers
            .normal_values_generation(scenario.session.id),
        generation_before
    );
    drop(activation);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn priority_and_preset_v1_compatibility_reuse_typed_services_and_replay_quietly() {
    let scenario = CommandHttpScenario::new().await;
    let priority = || WsCommand {
        protocol_version: 1,
        request_id: "priority-v1-typed".into(),
        session_id: scenario.session.id,
        expected_revision: None,
        command: "programmer.priority".into(),
        payload: serde_json::json!({"priority":55}),
    };
    let commands_before = audit_kind_count(&scenario.state, "command_applied");
    let changed_before = audit_kind_count(&scenario.state, "programmer_changed");
    let first = dispatch_ws_command(&scenario.state, &scenario.session, priority());
    assert!(first.ok, "{:?}", first.error);
    let payload = first.payload.unwrap();
    assert_eq!(payload.as_object().unwrap().keys().collect::<Vec<_>>(), vec!["programmer"]);
    assert_eq!(payload["programmer"]["priority"], 55);
    assert_eq!(audit_kind_count(&scenario.state, "command_applied"), commands_before + 1);
    assert_eq!(audit_kind_count(&scenario.state, "programmer_changed"), changed_before);
    assert_eq!(priority_event_count(&scenario.state, scenario.session.user.id.0), 1);

    let replay = dispatch_ws_command(&scenario.state, &scenario.session, priority());
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(audit_kind_count(&scenario.state, "command_applied"), commands_before + 1);
    assert_eq!(audit_kind_count(&scenario.state, "programmer_changed"), changed_before);
    assert_eq!(priority_event_count(&scenario.state, scenario.session.user.id.0), 1);

    let show_id = scenario.create_and_open_show("Preset v1 typed service").await;
    let fixture = light_core::FixtureId::new();
    let selection_revision = scenario
        .state
        .programmers
        .select(scenario.session.id, [fixture]);
    assert!(selection_revision > 0);
    let preset = light_programmer::Preset {
        name: "V1 look".into(),
        family: light_programmer::PresetFamily::Intensity,
        number: 2,
        values: HashMap::from([(
            fixture,
            HashMap::from([(
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Normalized(0.7),
            )]),
        )]),
        group_values: HashMap::new(),
    };
    assert_eq!(
        scenario
            .put_active_object(
                &show_id,
                "preset",
                "1.2",
                0,
                serde_json::to_value(preset).unwrap(),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let recall = || WsCommand {
        protocol_version: 1,
        request_id: "preset-v1-typed".into(),
        session_id: scenario.session.id,
        expected_revision: None,
        command: "preset.apply".into(),
        payload: serde_json::json!({"family":"Intensity","number":2}),
    };
    let commands_before = audit_kind_count(&scenario.state, "command_applied");
    let changed_before = audit_kind_count(&scenario.state, "programmer_changed");
    let values_before = values_event_count(&scenario.state, scenario.session.user.id.0);
    let first = dispatch_ws_command(&scenario.state, &scenario.session, recall());
    assert!(first.ok, "{:?}", first.error);
    let payload = first.payload.unwrap();
    assert_eq!(
        payload.as_object().unwrap().keys().collect::<Vec<_>>(),
        vec!["applied", "programmer"]
    );
    assert_eq!(payload["applied"], 1);
    assert_eq!(payload["programmer"]["active_context"], "preset:1.2");
    assert_eq!(audit_kind_count(&scenario.state, "command_applied"), commands_before + 1);
    assert_eq!(audit_kind_count(&scenario.state, "programmer_changed"), changed_before + 1);
    let last = scenario
        .state
        .audit_events
        .lock()
        .iter()
        .rev()
        .find(|event| event.kind == "programmer_changed")
        .unwrap()
        .payload["changes"]
        .clone();
    assert_eq!(last, serde_json::json!(["values"]));
    assert_eq!(
        values_event_count(&scenario.state, scenario.session.user.id.0),
        values_before + 1
    );

    let replay = dispatch_ws_command(&scenario.state, &scenario.session, recall());
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(audit_kind_count(&scenario.state, "command_applied"), commands_before + 1);
    assert_eq!(audit_kind_count(&scenario.state, "programmer_changed"), changed_before + 1);
    assert_eq!(
        values_event_count(&scenario.state, scenario.session.user.id.0),
        values_before + 1
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn preset_recall_request(
    request_id: &str,
    show_revision: u64,
    selection_revision: u64,
    programmer_revision: u64,
) -> serde_json::Value {
    serde_json::json!({
        "request_id":request_id,
        "address":{"family":"intensity","number":1},
        "expected_preset_revision":1,
        "expected_show_revision":show_revision,
        "expected_programmer_revision":programmer_revision,
        "expected_capture_mode_revision":0,
        "expected_selection_revision":selection_revision,
    })
}

fn priority_event_count(state: &AppState, user_id: Uuid) -> usize {
    let filter = light_application::EventFilter::default()
        .with_object(light_application::EventObject::programming_priority(user_id));
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &filter)
    else {
        panic!("priority events should remain replayable")
    };
    events.len()
}

fn values_event_count(state: &AppState, user_id: Uuid) -> usize {
    let filter = light_application::EventFilter::default()
        .with_object(light_application::EventObject::programming_values(user_id));
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &filter)
    else {
        panic!("values events should remain replayable")
    };
    events.len()
}

fn compatibility_event_count(state: &AppState) -> usize {
    state
        .audit_events
        .lock()
        .iter()
        .filter(|event| matches!(event.kind.as_str(), "command_applied" | "programmer_changed"))
        .count()
}

fn audit_kind_count(state: &AppState, kind: &str) -> usize {
    state
        .audit_events
        .lock()
        .iter()
        .filter(|event| event.kind == kind)
        .count()
}
