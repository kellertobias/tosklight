#[tokio::test]
async fn priority_snapshot_and_action_are_desk_shared_sparse_and_replay_safe() {
    let scenario = CommandHttpScenario::new().await;
    let user_id = scenario.session.user.id.0;
    let initial = scenario
        .priority_snapshot_for(user_id, Some(&scenario.token))
        .await;
    assert_eq!(initial.status(), StatusCode::OK);
    assert_eq!(initial.headers()[header::ETAG], "\"0\"");
    let initial: light_wire::v2::programmer_priority::ProgrammerPrioritySnapshot =
        serde_json::from_value(json(initial).await).unwrap();
    assert_eq!(initial.projection.priority, 100);

    // A URL naming an identity from before the collapse reads the desk's own Programmer.
    assert_eq!(
        scenario
            .priority_snapshot_for(Uuid::new_v4(), Some(&scenario.token))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario.priority_snapshot_for(user_id, None).await.status(),
        StatusCode::UNAUTHORIZED
    );

    let second_desk = scenario
        .state
        .installation.add_desk("Priority peer")
        .unwrap();
    let (second_token, second_user) = login_on_desk(&scenario, "Operator", second_desk.id).await;
    assert_eq!(second_user, user_id);
    let request = serde_json::json!({
        "request_id":"priority-http-1",
        "expected_revision":0,
        "priority":75,
    });
    let compatibility_before = compatibility_event_count(&scenario.state);
    let activation = scenario.state.active_show.acquire().await;
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
    assert_eq!(
        compatibility_event_count(&scenario.state),
        compatibility_before
    );

    let peer = scenario
        .priority_snapshot_for(second_user, Some(&second_token))
        .await;
    let peer: light_wire::v2::programmer_priority::ProgrammerPrioritySnapshot =
        serde_json::from_value(json(peer).await).unwrap();
    assert_eq!(peer.projection, changed.projection);

    let event_count = priority_event_count(&scenario.state);
    assert_eq!(event_count, 1);
    let replay = scenario
        .priority_action_for(user_id, &scenario.token, request)
        .await;
    let replay: light_wire::v2::programmer_priority::ProgrammerPriorityActionOutcome =
        serde_json::from_value(json(replay).await).unwrap();
    assert!(replay.replayed);
    assert_eq!(priority_event_count(&scenario.state), event_count);
    assert_eq!(
        compatibility_event_count(&scenario.state),
        compatibility_before
    );

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
    assert_eq!(priority_event_count(&scenario.state), event_count);

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
    assert_eq!(priority_event_count(&scenario.state), event_count);

    // A URL naming an identity from before the collapse sets the desk's own priority, and is
    // held to the desk's revision like any other surface.
    let legacy = scenario
        .priority_action_for(
            Uuid::new_v4(),
            &scenario.token,
            serde_json::json!({
                "request_id":"priority-http-legacy",
                "expected_revision":1,
                "priority":20,
            }),
        )
        .await;
    assert_eq!(legacy.status(), StatusCode::OK);
    assert_eq!(json(legacy).await["projection"]["priority"], 20);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preset_recall_uses_one_portable_show_graph_and_one_values_event() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Preset recall route").await;
    let selected = [light_core::FixtureId::new(), light_core::FixtureId::new()];
    let missing = light_core::FixtureId::new();
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
        values: HashMap::from([(
            missing,
            HashMap::from([(
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Normalized(0.2),
            )]),
        )]),
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
    let show = scenario.state.active_show.current().clone().unwrap();
    let show_revision = ShowStore::open(&show.path)
        .unwrap()
        .portable_revision()
        .unwrap()
        .value();
    let selection_revision = scenario.state.programming.select(scenario.session.id, []);

    // Deliberately contradict the portable Group graph. Recall must derive Group membership from
    // the exact same portable document and revision as the Preset, not this runtime projection.
    let engine_revision = scenario.state.output.snapshot().revision;
    let mut unpatched = operational_fixture(selected[0]);
    unpatched.universe = None;
    unpatched.address = None;
    scenario
        .state
        .output.replace_snapshot(EngineSnapshot {
            fixtures: vec![operational_fixture(selected[1]), unpatched].into(),
            groups: vec![light_programmer::GroupDefinition {
                id: "5".into(),
                fixtures: vec![selected[1]],
                ..Default::default()
            }]
            .into(),
            revision: engine_revision + 1,
            ..EngineSnapshot::default()
        })
        .unwrap();

    let mut request =
        preset_recall_request("preset-recall-http", show_revision, selection_revision, 0);
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
    request["values"] = serde_json::json!({"forged":true});
    let baseline = scenario.state.events.latest_sequence();
    let compatibility_before = compatibility_event_count(&scenario.state);
    let response = scenario
        .preset_recall_action(&show_id, Some(&scenario.token), request.clone())
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let changed: light_wire::v2::preset_recall::PresetRecallOutcome =
        serde_json::from_value(json(response).await).unwrap();
    assert_eq!(
        changed.disposition,
        light_wire::v2::preset_recall::PresetRecallDisposition::TargetsSelected
    );
    assert_eq!(changed.programmer_revision, 0);
    assert_eq!(changed.show_revision, show_revision);
    assert_eq!(changed.preset.revision, 1);
    assert_eq!(changed.preset.body["future_extension"]["retained"], true);
    assert!(matches!(
        changed.outcome,
        light_wire::v2::preset_recall::PresetRecallActionState::Changed {
            projection: None,
            event_sequence: None,
        }
    ));
    assert_eq!((changed.applied_fixtures, changed.selected_targets), (0, 1));
    assert_eq!(changed.active_context, None);
    assert_eq!(changed.interaction_event_sequence, Some(baseline + 1));
    assert!(
        changed
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("1 missing fixture target"))
    );
    assert_eq!(
        scenario
            .state
            .programming
            .selection(scenario.session.id)
            .unwrap()
            .selected,
        vec![selected[0]]
    );
    assert_eq!(
        values_event_count(&scenario.state),
        0
    );
    assert_eq!(
        compatibility_event_count(&scenario.state),
        compatibility_before
    );

    let recalled = scenario
        .preset_recall_action(
            &show_id,
            Some(&scenario.token),
            preset_recall_request(
                "preset-recall-second-tap",
                show_revision,
                changed.selection_revision,
                0,
            ),
        )
        .await;
    let recalled: light_wire::v2::preset_recall::PresetRecallOutcome =
        serde_json::from_value(json(recalled).await).unwrap();
    assert_eq!(
        recalled.disposition,
        light_wire::v2::preset_recall::PresetRecallDisposition::Recalled
    );
    let light_wire::v2::preset_recall::PresetRecallActionState::Changed {
        projection: Some(projection),
        event_sequence: Some(_),
    } = recalled.outcome
    else {
        panic!("second Preset tap should return one authoritative values projection")
    };
    assert_eq!(projection.fixture_values.len(), 1);
    assert_eq!(projection.fixture_values[0].fixture_id, selected[0].0);
    assert_eq!(
        values_event_count(&scenario.state),
        1
    );

    let no_change = scenario
        .preset_recall_action(
            &show_id,
            Some(&scenario.token),
            preset_recall_request(
                "preset-recall-no-change",
                show_revision,
                changed.selection_revision,
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
    assert_eq!(
        values_event_count(&scenario.state),
        1
    );

    let conflict = scenario
        .preset_recall_action(
            &show_id,
            Some(&scenario.token),
            preset_recall_request(
                "preset-recall-conflict",
                show_revision,
                changed.selection_revision,
                0,
            ),
        )
        .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(json(conflict).await["current_revision"], 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preset_recall_http_redirects_fixture_and_live_group_values_to_pending_preload() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Preload Preset recall route").await;
    let fixture = light_core::FixtureId::new();
    let intensity = light_core::AttributeKey::intensity();
    let pan = light_core::AttributeKey("pan".into());
    let group = light_programmer::GroupDefinition {
        id: "5".into(),
        name: "Live Preload group".into(),
        fixtures: vec![fixture],
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
        name: "Pending look".into(),
        family: light_programmer::PresetFamily::Intensity,
        number: 1,
        values: HashMap::from([(
            fixture,
            HashMap::from([(
                intensity,
                light_core::AttributeValue::Normalized(0.4),
            )]),
        )]),
        group_values: HashMap::from([(
            "5".into(),
            HashMap::from([(pan, light_core::AttributeValue::Normalized(0.7))]),
        )]),
    };
    assert_eq!(
        scenario
            .put_active_object(
                &show_id,
                "preset",
                "1.1",
                0,
                serde_json::to_value(preset).unwrap(),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let show = scenario.state.active_show.current().clone().unwrap();
    let show_revision = ShowStore::open(&show.path)
        .unwrap()
        .portable_revision()
        .unwrap()
        .value();
    let engine_revision = scenario.state.output.snapshot().revision;
    scenario
        .state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![operational_fixture(fixture)].into(),
            revision: engine_revision + 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let selection_revision = scenario.state.programming.select_expression(
        scenario.session.id,
        vec![fixture],
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "5".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );
    assert!(
        scenario
            .state
            .programming
            .arm_preload(scenario.session.id, true)
    );

    let mut request =
        preset_recall_request("preset-recall-preload", show_revision, selection_revision, 0);
    request["expected_preload_values_revision"] = serde_json::json!(0);
    let response = scenario
        .preset_recall_action(&show_id, Some(&scenario.token), request)
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"0\"");
    let recalled: light_wire::v2::preset_recall::PresetRecallOutcome =
        serde_json::from_value(json(response).await).unwrap();
    assert_eq!(
        recalled.target,
        Some(light_wire::v2::preset_recall::PresetRecallTarget::Preload)
    );
    assert_eq!(recalled.programmer_revision, 0);
    assert_eq!(recalled.preload_values_revision, Some(1));
    assert!(recalled.preload_event_sequence.is_some());
    assert!(matches!(
        recalled.outcome,
        light_wire::v2::preset_recall::PresetRecallActionState::Changed {
            projection: None,
            event_sequence: None,
        }
    ));
    let projection = recalled
        .preload_projection
        .expect("redirected recall returns pending Preload authority");
    assert_eq!(projection.revision, 1);
    assert_eq!(projection.fixture_values.len(), 1);
    assert_eq!(projection.group_values.len(), 1);
    let programmer = scenario
        .state
        .programming
        .get(scenario.session.id)
        .unwrap();
    assert!(programmer.values.is_empty());
    assert!(programmer.group_values.is_empty());
    assert!(programmer.preload_active.is_empty());
    assert!(programmer.preload_group_active.is_empty());
    assert_eq!(programmer.active_context, None);
    assert_eq!(
        scenario
            .state
            .programming
            .normal_values_revision(),
        0
    );
    assert_eq!(
        values_event_count(&scenario.state),
        0
    );
    assert_eq!(
        preload_values_event_count(&scenario.state),
        1
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn priority_and_preset_typed_ws_actions_keep_exact_authority_and_lock_policy() {
    let scenario = CommandHttpScenario::new().await;
    let priority = || {
        live_action_frame(
            &scenario.session,
            "priority-ws-exact",
            light_wire::v2::live_action::LiveAction::ProgrammerPriority(
                light_wire::v2::programmer_priority::ProgrammerPriorityActionRequest {
                    request_id: "priority-ws-exact".into(),
                    expected_revision: 0,
                    priority: 65,
                },
            ),
        )
    };
    let activation = scenario.state.active_show.acquire().await;
    let first = dispatch_live_action(&scenario.state, &scenario.session, priority());
    assert!(first.ok, "{:?}", first.error);
    assert_eq!(first.payload.unwrap()["projection"]["revision"], 1);
    drop(activation);
    let replay = dispatch_live_action(&scenario.state, &scenario.session, priority());
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(replay.payload.unwrap()["replayed"], true);

    let show_id = scenario.create_and_open_show("Preset typed WS").await;
    let fixture = light_core::FixtureId::new();
    let selection_revision = scenario
        .state
        .programming
        .select(scenario.session.id, [fixture]);
    let preset = light_programmer::Preset {
        name: "Typed WS look".into(),
        family: light_programmer::PresetFamily::Intensity,
        number: 3,
        values: HashMap::from([(
            fixture,
            HashMap::from([(
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Normalized(0.8),
            )]),
        )]),
        group_values: HashMap::new(),
    };
    assert_eq!(
        scenario
            .put_active_object(
                &show_id,
                "preset",
                "1.3",
                0,
                serde_json::to_value(preset).unwrap(),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let show = scenario.state.active_show.current().clone().unwrap();
    let show_revision = ShowStore::open(&show.path)
        .unwrap()
        .portable_revision()
        .unwrap()
        .value();
    let recall = || {
        live_action_frame(
            &scenario.session,
            "preset-ws-exact",
            light_wire::v2::live_action::LiveAction::PresetRecall(
                light_wire::v2::live_action::PresetRecallLiveActionRequest {
                    request_id: "preset-ws-exact".into(),
                    show_id: Uuid::parse_str(&show_id).unwrap(),
                    request: light_wire::v2::preset_recall::PresetRecallRequest {
                        address: light_wire::v2::preset_recording::PresetRecordingAddress {
                            family:
                                light_wire::v2::preset_recording::PresetRecordingFamily::Intensity,
                            number: 3,
                        },
                        expected_preset_revision: 1,
                        expected_show_revision: show_revision,
                        expected_programmer_revision: 0,
                        expected_preload_values_revision: None,
                        expected_capture_mode_revision: 0,
                        expected_selection_revision: selection_revision,
                    },
                },
            ),
        )
    };
    let first = dispatch_live_action(&scenario.state, &scenario.session, recall());
    assert!(first.ok, "{:?}", first.error);
    let payload = first.payload.unwrap();
    assert_eq!(payload["status"], "changed");
    assert_eq!(payload["disposition"], "recalled");
    assert!(payload.get("target").is_none());
    assert_eq!(payload["preset"]["id"], "1.3");
    let repeated = dispatch_live_action(&scenario.state, &scenario.session, recall());
    assert!(!repeated.ok);
    assert!(
        repeated
            .error
            .as_deref()
            .is_some_and(|error| error.contains("revision conflict"))
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn preset_recall_request(
    _request_id: &str,
    show_revision: u64,
    selection_revision: u64,
    programmer_revision: u64,
) -> serde_json::Value {
    serde_json::json!({
        "address":{"family":"intensity","number":1},
        "expected_preset_revision":1,
        "expected_show_revision":show_revision,
        "expected_programmer_revision":programmer_revision,
        "expected_capture_mode_revision":0,
        "expected_selection_revision":selection_revision,
    })
}

fn priority_event_count(state: &AppState) -> usize {
    let filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_priority(),
    );
    let light_application::EventReplay::Events(events) =
        state.events.replay(0, &filter)
    else {
        panic!("priority events should remain replayable")
    };
    events.len()
}

fn values_event_count(state: &AppState) -> usize {
    let filter = light_application::EventFilter::default()
        .with_object(light_application::EventObject::programming_values());
    let light_application::EventReplay::Events(events) =
        state.events.replay(0, &filter)
    else {
        panic!("values events should remain replayable")
    };
    events.len()
}

fn preload_values_event_count(state: &AppState) -> usize {
    let filter = light_application::EventFilter::default()
        .with_object(light_application::EventObject::programming_preload_values());
    let light_application::EventReplay::Events(events) = state.events.replay(0, &filter) else {
        panic!("Preload values events should remain replayable")
    };
    events.len()
}

fn compatibility_event_count(state: &AppState) -> usize {
    state
        .events.audit_events()
        .iter()
        .filter(|event| {
            matches!(
                event.kind.as_str(),
                "command_applied" | "programmer_changed"
            )
        })
        .count()
}
