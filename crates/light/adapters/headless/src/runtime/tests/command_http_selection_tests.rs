#[tokio::test]
async fn programming_selection_actions_are_scoped_revisioned_and_replay_safe() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    let initial_revision = scenario
        .state
        .programming
        .selection(scenario.session.id)
        .unwrap()
        .revision;
    let request = serde_json::json!({
        "request_id": "selection-1",
        "action": "replace",
        "fixtures": [fixture.0],
        "expected_revision": initial_revision,
    });

    let response = scenario.selection_action(request.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let first = json(response).await;
    assert_eq!(first["request_id"], "selection-1");
    assert_eq!(first["action"], "replaced");
    assert_eq!(first["applied"], 1);
    assert_eq!(first["selection"]["selected"], serde_json::json!([fixture.0]));
    assert_eq!(first["selection"]["gesture_open"], false);
    assert_eq!(first["replayed"], false);
    let initial_selection_events = selection_events(&scenario);
    assert_eq!(initial_selection_events.len(), 1);
    assert_eq!(
        first["event_sequence"],
        initial_selection_events[0].sequence
    );
    assert!(Uuid::parse_str(first["correlation_id"].as_str().unwrap()).is_ok());
    let selection_revision = first["selection"]["revision"].as_u64().unwrap();

    let replay = scenario.selection_action(request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["selection"]["revision"], selection_revision);
    assert_eq!(
        replay["event_sequence"],
        initial_selection_events[0].sequence
    );
    assert_eq!(selection_events(&scenario).len(), 1);

    let stale = scenario
        .selection_action(serde_json::json!({
            "request_id": "selection-stale",
            "action": "replace",
            "fixtures": [],
            "expected_revision": initial_revision,
        }))
        .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        scenario
            .state
            .programming
            .selection(scenario.session.id)
            .unwrap()
            .selected,
        vec![fixture]
    );
    assert_eq!(selection_events(&scenario).len(), 1);

    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn programming_selection_gestures_return_the_complete_authoritative_context() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();

    let gesture = scenario
        .selection_action(serde_json::json!({
            "request_id": "selection-gesture",
            "action": "gesture",
            "source": { "type": "live_group", "group_id": "1" },
            "remove": false,
        }))
        .await;
    assert_eq!(gesture.status(), StatusCode::OK);
    let gesture = json(gesture).await;
    assert_eq!(gesture["action"], "gesture_applied");
    assert_eq!(gesture["selection"]["selected"], serde_json::json!([fixture.0]));
    assert_eq!(gesture["selection"]["gesture_open"], true);
    assert_eq!(gesture["selection"]["expression"]["type"], "sources");

    let snapshot = scenario.interaction_snapshot().await;
    assert_eq!(snapshot.status(), StatusCode::OK);
    let snapshot = json(snapshot).await;
    assert_eq!(snapshot["projection"]["selection"], gesture["selection"]);
    assert_eq!(
        snapshot["cursor"]["sequence"],
        scenario.state.events.latest_sequence()
    );
    let selection_events = selection_events(&scenario);
    assert_eq!(selection_events.len(), 1);
    assert_eq!(gesture["event_sequence"], selection_events[0].sequence);

    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn canonical_multi_reference_group_selects_and_rank_spreads_through_http() {
    let scenario = CommandHttpScenario::new().await;
    let template = schema_v2_direct_fixture().0;
    let mut fixtures = (0..3_u32)
        .map(|index| {
            let mut fixture = template.clone();
            fixture.fixture_id = light_core::FixtureId::new();
            fixture.fixture_number = Some(index + 1);
            fixture.address = Some(1 + index as u16 * 8);
            fixture
        })
        .collect::<Vec<_>>();
    let ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let explicit =
        |id: &str, fixture_ids: Vec<light_core::FixtureId>| light_programmer::GroupDefinition {
            id: id.into(),
            source: Some(light_programmer::GroupFixtureSource::Explicit { fixture_ids }),
            ..Default::default()
        };
    let references = |id: &str, group_ids: &[&str]| light_programmer::GroupDefinition {
        id: id.into(),
        source: Some(light_programmer::GroupFixtureSource::References {
            references: group_ids
                .iter()
                .map(|group_id| light_programmer::GroupReference {
                    group_id: (*group_id).into(),
                    rule: light_programmer::SelectionRule::All,
                })
                .collect(),
        }),
        ..Default::default()
    };
    let mut target = references("target", &["near", "nested"]);
    target.mapping = Some(light_dynamics::SpatialSelectionMapping {
        projection: light_dynamics::SpatialProjection::from_preset(
            light_dynamics::ProjectionPreset::Top,
            light_dynamics::Position3d::default(),
        ),
        shape: light_dynamics::SpatialSelectionShape::Radial {
            center_u: 0.0,
            center_v: 0.0,
            direction: light_dynamics::RadialDirection::Outward,
        },
    });
    scenario
        .state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: std::mem::take(&mut fixtures).into(),
            groups: vec![
                explicit("near", vec![ids[0], ids[1]]),
                explicit("far", vec![ids[2]]),
                references("nested", &["far"]),
                target,
            ]
            .into(),
            dynamic_stage_positions: std::collections::HashMap::from([
                (
                    ids[0],
                    light_dynamics::SpatialPosition {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    ids[1],
                    light_dynamics::SpatialPosition {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    ids[2],
                    light_dynamics::SpatialPosition {
                        x: 2.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
            ])
            .into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();

    let selection = scenario
        .selection_action(serde_json::json!({
            "request_id": "canonical-reference-selection",
            "action": "select_group",
            "group_id": "target",
            "frozen": false,
            "rule": {"type": "all"},
            "expected_revision": 0,
        }))
        .await;
    assert_eq!(selection.status(), StatusCode::OK);
    assert_eq!(
        json(selection).await["selection"]["selected"],
        serde_json::json!([ids[0].0, ids[1].0, ids[2].0])
    );

    let spread = scenario
        .values_action(serde_json::json!({
            "request_id": "canonical-reference-spread",
            "expected_revision": 0,
            "expected_capture_mode_revision": 0,
            "action": {
                "type": "set_group",
                "group_id": "target",
                "attribute": "intensity",
                "value": {"kind": "spread", "value": [0.0, 1.0]},
                "timing": {"fade": false}
            }
        }))
        .await;
    assert_eq!(spread.status(), StatusCode::OK, "{}", json(spread).await);

    let resolved = scenario.state.output.resolved_values();
    for (fixture_id, expected) in ids.into_iter().zip([0.0, 0.5, 1.0]) {
        assert_eq!(
            resolved
                .get(&(fixture_id, light_core::AttributeKey::intensity()))
                .and_then(light_core::AttributeValue::normalized),
            Some(expected),
            "the spread follows the Group's spatial rank"
        );
    }
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn selection_events(
    scenario: &CommandHttpScenario,
) -> Vec<std::sync::Arc<light_application::EventEnvelope>> {
    let filter = light_application::EventFilter::for_desk(scenario.session.desk.id).with_object(
        light_application::EventObject::programming_selection(scenario.session.desk.id),
    );
    let light_application::EventReplay::Events(events) =
        scenario.state.events.replay(0, &filter)
    else {
        panic!("the focused Programming selection history should remain replayable")
    };
    events
}

#[tokio::test]
async fn programming_selection_contract_rejects_ambiguous_or_unsafe_requests() {
    let scenario = CommandHttpScenario::new().await;
    let revision = scenario
        .state
        .programming
        .selection(scenario.session.id)
        .unwrap()
        .revision;
    let missing_fixture = scenario
        .selection_action(serde_json::json!({
            "request_id": "selection-missing-fixture",
            "action": "replace",
            "fixtures": [Uuid::new_v4()],
            "expected_revision": revision,
        }))
        .await;
    assert_eq!(missing_fixture.status(), StatusCode::BAD_REQUEST);

    for request in [
        serde_json::json!({
            "request_id": "selection-zero-rule",
            "action": "apply_rule",
            "rule": { "type": "every_nth", "n": 0, "offset": 0 },
        }),
        serde_json::json!({
            "request_id": "selection-unsafe-rule",
            "action": "apply_rule",
            "rule": {
                "type": "every_nth",
                "n": 9_007_199_254_740_992_u64,
                "offset": 0,
            },
        }),
    ] {
        assert_eq!(
            scenario.selection_action(request).await.status(),
            StatusCode::BAD_REQUEST
        );
    }

    let malformed = scenario
        .selection_action(serde_json::json!({
            "request_id": "selection-misspelled",
            "action": "gesture",
            "source": { "type": "fixture", "fixture_id": Uuid::new_v4() },
            "removee": false,
        }))
        .await;
    assert_eq!(malformed.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let oversized = scenario
        .raw_selection_action(
            scenario.session.desk.id,
            Body::from(format!(r#"{{"padding":"{}"}}"#, "x".repeat(512 * 1024))),
        )
        .await;
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn programming_selection_request_identity_scope_and_lock_are_enforced() {
    let scenario = CommandHttpScenario::new().await;
    let first = scenario
        .selection_action(serde_json::json!({
            "request_id": "selection-reused",
            "action": "replace",
            "fixtures": [],
            "expected_revision": 0,
        }))
        .await;
    assert_eq!(first.status(), StatusCode::OK);
    let reused = scenario
        .selection_action(serde_json::json!({
            "request_id": "selection-reused",
            "action": "apply_rule",
            "rule": { "type": "all" },
        }))
        .await;
    assert_eq!(reused.status(), StatusCode::CONFLICT);

    let wrong_desk = scenario
        .selection_action_for(
            Uuid::new_v4(),
            serde_json::json!({
                "request_id": "selection-wrong-desk",
                "action": "apply_rule",
                "rule": { "type": "all" },
            }),
        )
        .await;
    assert_eq!(wrong_desk.status(), StatusCode::NOT_FOUND);

    write_desk_lock(
        &scenario.state,
        scenario.session.desk.id,
        &DeskLockConfiguration {
            locked: true,
            ..DeskLockConfiguration::default()
        },
    )
    .unwrap();
    let locked = scenario
        .selection_action(serde_json::json!({
            "request_id": "selection-locked",
            "action": "apply_rule",
            "rule": { "type": "all" },
        }))
        .await;
    assert_eq!(locked.status(), StatusCode::CONFLICT);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn accepted_selection_persistence_warning_is_replayed_without_duplicate_events() {
    let scenario = CommandHttpScenario::new().await;
    let isolated = light_show::DeskStore::open(scenario.data_dir.join("isolated-desk.sqlite"))
        .expect("isolated Desk store");
    let isolated_desk = isolated
        .add_desk("Isolated", "isolated")
        .expect("isolated control desk");
    scenario.state.installation.replace_desk_store(isolated);
    assert!(
        scenario
            .state
            .sessions
            .update_session_desk(scenario.session.id, isolated_desk.clone())
    );
    let request = serde_json::json!({
        "request_id": "selection-persistence-warning",
        "action": "replace",
        "fixtures": [],
        "expected_revision": 0,
    });

    let first = scenario
        .selection_action_for(isolated_desk.id, request.clone())
        .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first = json(first).await;
    assert!(first["warning"].as_str().is_some_and(|warning| !warning.is_empty()));
    assert_eq!(first["replayed"], false);
    let sequence = first["event_sequence"].as_u64().unwrap();

    let replay = scenario.selection_action_for(isolated_desk.id, request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["warning"], first["warning"]);
    assert_eq!(replay["event_sequence"], sequence);
    assert_eq!(replay["replayed"], true);
    assert_eq!(scenario.state.events.latest_sequence(), sequence);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn selection_waits_for_active_group_install_before_resolving_its_environment() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Selection ordering").await;
    let first = schema_v2_direct_fixture().0;
    let mut second = schema_v2_direct_fixture().0;
    second.fixture_number = Some(2);
    second.address = Some(3);
    for fixture in [&first, &second] {
        let response = scenario
            .put_active_object(
                &show_id,
                "patched_fixture",
                &fixture.fixture_id.0.to_string(),
                0,
                serde_json::to_value(fixture).unwrap(),
            )
            .await;
        if response.status() != StatusCode::OK {
            panic!("fixture install failed: {}", json(response).await);
        }
    }
    let group = scenario
        .put_active_object(
            &show_id,
            "group",
            "1",
            0,
            serde_json::json!({
                "name": "Ordered Group",
                "fixtures": [first.fixture_id.0],
            }),
        )
        .await;
    assert_eq!(group.status(), StatusCode::OK);

    scenario.state.active_show.http_lifecycle_probe().arm();
    let group_state = scenario.state.clone();
    let group_token = scenario.token.clone();
    let group_show_id = show_id.clone();
    let second_id = second.fixture_id;
    let group_update = tokio::spawn(async move {
        seed_show_object(
            &group_state,
            &group_token,
            &group_show_id,
            "group",
            "1",
            1,
            serde_json::json!({
                "name": "Ordered Group",
                "fixtures": [second_id.0],
            }),
        )
        .await
    });
    let pause = scenario.state.active_show.http_lifecycle_probe();
    tokio::task::spawn_blocking(move || pause.wait_until_started())
        .await
        .unwrap();

    let selection_app = scenario.app.clone();
    let selection_token = scenario.token.clone();
    let desk_id = scenario.session.desk.id;
    let mut selection = tokio::spawn(async move {
        selection_app
            .oneshot(
                Request::post("/api/v2/programming-selection/actions")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {selection_token}"),
                )
                .header("x-tosk-desk", desk_id.to_string())
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "selection-after-group-install",
                        "action": "gesture",
                        "source": { "type": "live_group", "group_id": "1" },
                        "remove": false,
                    })
                    .to_string(),
                ))
                .unwrap(),
            )
            .await
            .unwrap()
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut selection)
            .await
            .is_err(),
        "selection bypassed the active-show ordering gate"
    );

    scenario.state.active_show.http_lifecycle_probe().release();
    let group_update = tokio::time::timeout(Duration::from_secs(2), group_update)
        .await
        .expect("Group update deadlocked")
        .unwrap();
    assert_eq!(group_update.status(), StatusCode::OK);
    let selection = tokio::time::timeout(Duration::from_secs(2), selection)
        .await
        .expect("selection deadlocked")
        .unwrap();
    assert_eq!(selection.status(), StatusCode::OK);
    assert_eq!(
        json(selection).await["selection"]["selected"],
        serde_json::json!([second.fixture_id.0])
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
