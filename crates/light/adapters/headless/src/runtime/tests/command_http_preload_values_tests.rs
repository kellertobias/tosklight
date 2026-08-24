#[tokio::test]
async fn preload_values_snapshot_is_exact_user_owned_and_empty_before_capture() {
    let scenario = CommandHttpScenario::new().await;
    let response = scenario.preload_values_snapshot().await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"0\"");
    let snapshot: light_wire::v2::preload_values::ProgrammingPreloadValuesSnapshot =
        serde_json::from_value(json(response).await).unwrap();
    assert_eq!(snapshot.projection.user_id, scenario.session.user.id.0);
    assert_eq!(snapshot.projection.revision, 0);
    assert!(snapshot.projection.fixture_values.is_empty());
    assert!(snapshot.projection.group_values.is_empty());

    // A URL naming an identity from before the collapse reads the desk's own Programmer.
    let legacy = scenario
        .preload_values_snapshot_for(Uuid::new_v4(), Some(&scenario.token))
        .await;
    assert_eq!(legacy.status(), StatusCode::OK);
    let missing = scenario
        .preload_values_snapshot_for(scenario.session.user.id.0, None)
        .await;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preload_values_batch_is_atomic_revisioned_replay_safe_and_sparse_on_no_op() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    let rejected = scenario
        .preload_values_action(preload_fixture_request(
            "before-capture",
            0,
            0,
            fixture.0,
            0.2,
        ))
        .await;
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    assert_eq!(json(rejected).await["current_capture_mode_revision"], 0);

    assert_eq!(
        scenario
            .press_key(&scenario.token, "PRE", "enter-preload-values")
            .await
            .status(),
        StatusCode::OK
    );
    let batch = serde_json::json!({
        "request_id": "preload-batch",
        "expected_revision": 0,
        "expected_capture_mode_revision": 1,
        "action": {
            "type": "batch",
            "mutations": [
                {
                    "type": "set_fixture",
                    "fixture_id": fixture.0,
                    "attribute": "intensity",
                    "value": {"kind": "normalized", "value": 0.5},
                    "timing": {"fade": true, "fade_millis": 1000, "delay_millis": 250}
                },
                {
                    "type": "set_group",
                    "group_id": "1",
                    "attribute": "pan",
                    "value": {"kind": "spread", "value": [0.1, 0.9]}
                }
            ]
        }
    });
    let response = scenario.preload_values_action(batch.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let changed = json(response).await;
    assert_preload_values_changed(&scenario, &changed, "preload-batch", 1);
    assert_eq!(changed["capture_mode_revision"], 1);
    assert_eq!(changed["projection"]["fixture_values"][0]["fade"], true);
    assert_eq!(
        changed["projection"]["fixture_values"][0]["delay_millis"],
        250
    );
    assert_eq!(
        changed["projection"]["group_values"][0]["value"]["kind"],
        "spread"
    );

    let replay = json(scenario.preload_values_action(batch).await).await;
    assert_eq!(replay["replayed"], true);
    assert_preload_values_changed(&scenario, &replay, "preload-batch", 1);
    assert_only_preload_values_events(&scenario, 1);

    let exact = json(
        scenario
            .preload_values_action(serde_json::json!({
                "request_id": "preload-exact",
                "expected_revision": 1,
                "expected_capture_mode_revision": 1,
                "action": {
                    "type": "set_fixture",
                    "fixture_id": fixture.0,
                    "attribute": "intensity",
                    "value": {"kind": "normalized", "value": 0.5},
                    "timing": {"fade": true, "fade_millis": 1000, "delay_millis": 250}
                }
            }))
            .await,
    )
    .await;
    assert_eq!(exact["status"], "no_change");
    assert_eq!(exact["revision"], 1);
    assert!(exact.get("projection").is_none());
    assert!(exact.get("event_sequence").is_none());
    assert_only_preload_values_events(&scenario, 1);

    let conflict = scenario
        .preload_values_action(preload_fixture_request(
            "preload-stale",
            0,
            1,
            fixture.0,
            0.7,
        ))
        .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(json(conflict).await["current_revision"], 1);
    assert_only_preload_values_events(&scenario, 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preload_values_are_the_desks_whichever_surface_prepares_them() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    assert_eq!(
        scenario
            .press_key(&scenario.token, "PRE", "shared-preload-enter")
            .await
            .status(),
        StatusCode::OK
    );
    // A legacy desk record still exists in older installations; a session logging in on one
    // prepares the desk's Preload rather than one of its own.
    let second_desk = scenario
        .state
        .installation
        .add_desk("Second Preload desk")
        .unwrap();
    let (second_token, second_user) = login_on_desk(&scenario, "Operator", second_desk.id).await;
    assert_eq!(second_user, scenario.session.user.id.0);

    let peer = scenario
        .preload_values_action_for(
            second_user,
            &second_token,
            preload_fixture_request("peer-preload", 0, 1, fixture.0, 0.4),
        )
        .await;
    assert_eq!(peer.status(), StatusCode::OK);
    let peer = json(peer).await;
    assert_preload_values_changed(&scenario, &peer, "peer-preload", 1);
    let snapshot = json(scenario.preload_values_snapshot().await).await;
    assert_eq!(snapshot["projection"]["revision"], 1);
    assert_eq!(
        snapshot["projection"]["fixture_values"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // A surface arriving under an identity from before the collapse writes into the same Preload,
    // so it must supply the desk's current revision rather than starting again from zero.
    let legacy_user = scenario
        .state
        .installation
        .add_user("Other Preload values user")
        .unwrap();
    let (legacy_token, logged_in_user) = login_on_desk(
        &scenario,
        "Other Preload values user",
        scenario.session.desk.id,
    )
    .await;
    assert_eq!(logged_in_user, legacy_user.id.0);
    let legacy = scenario
        .preload_values_action_for(
            legacy_user.id.0,
            &legacy_token,
            preload_fixture_request("legacy-preload", 1, 1, fixture.0, 0.9),
        )
        .await;
    assert_eq!(legacy.status(), StatusCode::OK);
    let legacy = json(legacy).await;
    assert_eq!(
        legacy["projection"]["user_id"],
        scenario.session.user.id.0.to_string(),
        "the desk reports its own Programmer, not the name the caller asked under"
    );
    assert_eq!(legacy["projection"]["revision"], 2);
    assert_eq!(
        legacy["projection"]["fixture_values"][0]["value"]["value"],
        0.9
    );

    // The desk's own snapshot shows the same value: there is one set of pending values.
    let original = json(scenario.preload_values_snapshot().await).await;
    assert_eq!(original["projection"]["revision"], 2);
    assert_eq!(
        original["projection"]["fixture_values"][0]["value"]["value"],
        0.9
    );

    // A stale revision is still refused, which is what protects concurrent surfaces.
    let stale = scenario
        .preload_values_action_for(
            scenario.session.user.id.0,
            &scenario.token,
            preload_fixture_request("stale-preload", 1, 1, fixture.0, 0.8),
        )
        .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preload_values_fixture_and_group_releases_are_individual_atomic_actions() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    assert_eq!(
        scenario
            .press_key(&scenario.token, "PRE", "release-preload-enter")
            .await
            .status(),
        StatusCode::OK
    );
    let seeded = scenario
        .preload_values_action(serde_json::json!({
            "request_id": "release-preload-seed",
            "expected_revision": 0,
            "expected_capture_mode_revision": 1,
            "action": {
                "type": "batch",
                "mutations": [
                    {
                        "type": "set_fixture",
                        "fixture_id": fixture.0,
                        "attribute": "intensity",
                        "value": {"kind": "normalized", "value": 0.5}
                    },
                    {
                        "type": "set_group",
                        "group_id": "1",
                        "attribute": "pan",
                        "value": {"kind": "spread", "value": [0.2, 0.8]}
                    }
                ]
            }
        }))
        .await;
    assert_eq!(seeded.status(), StatusCode::OK);

    let fixture_release = json(
        scenario
            .preload_values_action(serde_json::json!({
                "request_id": "release-preload-fixture",
                "expected_revision": 1,
                "expected_capture_mode_revision": 1,
                "action": {
                    "type": "release_fixture",
                    "fixture_id": fixture.0,
                    "attribute": "intensity"
                }
            }))
            .await,
    )
    .await;
    assert_preload_values_changed(
        &scenario,
        &fixture_release,
        "release-preload-fixture",
        2,
    );
    assert!(
        fixture_release["projection"]["fixture_values"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fixture_release["projection"]["group_values"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let group_release = json(
        scenario
            .preload_values_action(serde_json::json!({
                "request_id": "release-preload-group",
                "expected_revision": 2,
                "expected_capture_mode_revision": 1,
                "action": {
                    "type": "release_group",
                    "group_id": "1",
                    "attribute": "pan"
                }
            }))
            .await,
    )
    .await;
    assert_preload_values_changed(&scenario, &group_release, "release-preload-group", 3);
    assert!(
        group_release["projection"]["group_values"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_only_preload_values_events(&scenario, 3);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn preload_fixture_request(
    request_id: &str,
    expected_revision: u64,
    expected_capture_mode_revision: u64,
    fixture_id: Uuid,
    value: f32,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "expected_revision": expected_revision,
        "expected_capture_mode_revision": expected_capture_mode_revision,
        "action": {
            "type": "set_fixture",
            "fixture_id": fixture_id,
            "attribute": "intensity",
            "value": {"kind": "normalized", "value": value}
        }
    })
}

fn assert_preload_values_changed(
    scenario: &CommandHttpScenario,
    value: &serde_json::Value,
    request_id: &str,
    revision: u64,
) {
    assert_eq!(value["request_id"], request_id);
    assert_eq!(value["status"], "changed");
    assert_eq!(value["revision"], revision);
    assert_eq!(value["projection"]["revision"], revision);
    let filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_preload_values(),
    );
    let light_application::EventReplay::Events(events) =
        scenario.state.events.replay(0, &filter)
    else {
        panic!("the focused Preload values event history should remain replayable")
    };
    let event = events
        .iter()
        .find(|event| {
            matches!(
                &event.payload,
                light_application::ApplicationEvent::Programming(
                    light_application::ProgrammingEvent::PreloadValuesChanged(change)
                ) if change.projection.revision == revision
            )
        })
        .expect("the response revision must have a focused Preload values event");
    assert_eq!(value["event_sequence"], event.sequence);
    assert!(Uuid::parse_str(value["correlation_id"].as_str().unwrap()).is_ok());
}

fn assert_only_preload_values_events(scenario: &CommandHttpScenario, expected: usize) {
    assert_preload_values_event_count(scenario, expected);
}

fn assert_preload_values_event_count(scenario: &CommandHttpScenario, expected: usize) {
    let filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_preload_values(),
    );
    let light_application::EventReplay::Events(events) =
        scenario.state.events.replay(0, &filter)
    else {
        panic!("the focused Preload values event history should remain replayable")
    };
    assert_eq!(events.len(), expected);
    assert!(events.iter().all(|event| matches!(
        event.payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::PreloadValuesChanged(_)
        )
    )));
}
