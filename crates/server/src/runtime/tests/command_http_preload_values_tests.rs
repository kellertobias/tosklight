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

    let foreign = scenario
        .preload_values_snapshot_for(Uuid::new_v4(), Some(&scenario.token))
        .await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
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
    assert_preload_values_changed(&changed, "preload-batch", 1, 2);
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
    assert_preload_values_changed(&replay, "preload-batch", 1, 2);
    assert_eq!(scenario.state.application_events.latest_sequence(), 2);

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
    assert_eq!(scenario.state.application_events.latest_sequence(), 2);

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
async fn preload_values_share_one_user_across_desks_and_reject_foreign_actions() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    assert_eq!(
        scenario
            .press_key(&scenario.token, "PRE", "shared-preload-enter")
            .await
            .status(),
        StatusCode::OK
    );
    let second_desk = scenario
        .state
        .desk
        .lock()
        .add_desk("Second Preload desk", "second-preload-values")
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
    assert_preload_values_changed(&peer, "peer-preload", 1, 2);
    let snapshot = json(scenario.preload_values_snapshot().await).await;
    assert_eq!(snapshot["projection"]["revision"], 1);
    assert_eq!(snapshot["projection"]["fixture_values"].as_array().unwrap().len(), 1);

    let other_user = scenario
        .state
        .desk
        .lock()
        .add_user("Other Preload values user")
        .unwrap();
    let (other_token, logged_in_user) = login_on_desk(
        &scenario,
        "Other Preload values user",
        scenario.session.desk.id,
    )
    .await;
    assert_eq!(logged_in_user, other_user.id.0);
    assert_eq!(
        scenario
            .press_key(&other_token, "PRE", "other-preload-enter")
            .await
            .status(),
        StatusCode::OK
    );
    let other = scenario
        .preload_values_action_for(
            other_user.id.0,
            &other_token,
            preload_fixture_request("other-preload", 0, 1, fixture.0, 0.9),
        )
        .await;
    assert_eq!(other.status(), StatusCode::OK);
    let other = json(other).await;
    assert_eq!(other["projection"]["user_id"], other_user.id.0.to_string());
    assert_eq!(other["projection"]["revision"], 1);
    assert_eq!(other["projection"]["fixture_values"][0]["value"]["value"], 0.9);

    let original = json(scenario.preload_values_snapshot().await).await;
    assert_eq!(original["projection"]["revision"], 1);
    assert_eq!(
        original["projection"]["fixture_values"][0]["value"]["value"],
        0.4
    );

    let foreign = scenario
        .preload_values_action_for(
            Uuid::new_v4(),
            &scenario.token,
            preload_fixture_request("foreign-preload", 1, 1, fixture.0, 0.8),
        )
        .await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    assert_eq!(json(foreign).await["kind"], "forbidden");
    assert_only_preload_values_events(&scenario, 1);
    assert_preload_values_event_count(&scenario, other_user.id.0, 1);
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
    assert_preload_values_changed(&fixture_release, "release-preload-fixture", 2, 3);
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
    assert_preload_values_changed(&group_release, "release-preload-group", 3, 4);
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
    value: &serde_json::Value,
    request_id: &str,
    revision: u64,
    sequence: u64,
) {
    assert_eq!(value["request_id"], request_id);
    assert_eq!(value["status"], "changed");
    assert_eq!(value["revision"], revision);
    assert_eq!(value["projection"]["revision"], revision);
    assert_eq!(value["event_sequence"], sequence);
    assert!(Uuid::parse_str(value["correlation_id"].as_str().unwrap()).is_ok());
}

fn assert_only_preload_values_events(scenario: &CommandHttpScenario, expected: usize) {
    assert_preload_values_event_count(scenario, scenario.session.user.id.0, expected);
}

fn assert_preload_values_event_count(
    scenario: &CommandHttpScenario,
    user_id: Uuid,
    expected: usize,
) {
    let filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_preload_values(user_id),
    );
    let light_application::EventReplay::Events(events) =
        scenario.state.application_events.replay(0, &filter)
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
