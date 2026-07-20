#[tokio::test]
async fn programmer_values_snapshot_returns_authenticated_projection_and_safe_cursor() {
    let scenario = CommandHttpScenario::new().await;
    let fixture_id = scenario.install_direct_fixture();
    let response = scenario
        .execute("values-snapshot", Some("GROUP 1 AT 50"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let expected_cursor = scenario.state.application_events.latest_sequence();
    let response = scenario.values_snapshot().await;
    assert_eq!(response.status(), StatusCode::OK);
    let snapshot: light_wire::v2::programming::ProgrammingValuesSnapshot =
        serde_json::from_value(json(response).await).unwrap();

    assert_eq!(snapshot.cursor.sequence, expected_cursor);
    assert_eq!(snapshot.projection.user_id, scenario.session.user.id.0);
    assert_eq!(snapshot.projection.revision, 1);
    assert!(snapshot.projection.fixture_values.is_empty());
    assert_eq!(snapshot.projection.group_values.len(), 1);
    let value = &snapshot.projection.group_values[0];
    assert_eq!(value.group_id, "1");
    assert_eq!(value.attribute, "intensity");
    assert_eq!(
        value.value,
        light_wire::v2::programming::ProgrammingAttributeValue::Normalized(0.5)
    );
    assert_eq!(
        scenario
            .state
            .programmers
            .get(scenario.session.id)
            .unwrap()
            .selected,
        vec![fixture_id]
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn programmer_values_snapshot_rejects_foreign_user_and_missing_authentication() {
    let scenario = CommandHttpScenario::new().await;

    let response = scenario
        .values_snapshot_for(Uuid::new_v4(), Some(&scenario.token))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let response = scenario
        .values_snapshot_for(scenario.session.user.id.0, None)
        .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn capture_mode_snapshot_is_user_owned_and_shared_between_the_users_desks() {
    let scenario = CommandHttpScenario::new().await;
    assert_eq!(
        scenario
            .press_key(&scenario.token, "PRE", "capture-mode-enter")
            .await
            .status(),
        StatusCode::OK
    );

    let response = scenario.capture_mode_snapshot().await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let snapshot: light_wire::v2::programming::ProgrammingCaptureModeSnapshot =
        serde_json::from_value(json(response).await).unwrap();
    assert_eq!(snapshot.cursor.sequence, 2);
    assert_eq!(snapshot.projection.user_id, scenario.session.user.id.0);
    assert_eq!(snapshot.projection.revision, 1);
    assert!(snapshot.projection.blind);
    assert!(!snapshot.projection.preview);
    assert!(snapshot.projection.preload_capture_programmer);

    let second_desk = scenario
        .state
        .desk
        .lock()
        .add_desk("Second capture desk", "second-capture")
        .unwrap();
    let (second_token, second_user) =
        login_on_desk(&scenario, "Operator", second_desk.id).await;
    let second = scenario
        .capture_mode_snapshot_for(second_user, Some(&second_token))
        .await;
    assert_eq!(second.status(), StatusCode::OK);
    let second: light_wire::v2::programming::ProgrammingCaptureModeSnapshot =
        serde_json::from_value(json(second).await).unwrap();
    assert_eq!(second.projection, snapshot.projection);

    assert_eq!(
        scenario
            .capture_mode_snapshot_for(Uuid::new_v4(), Some(&scenario.token))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        scenario
            .capture_mode_snapshot_for(scenario.session.user.id.0, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn capture_mode_event_is_user_scoped_replaceable_and_has_no_desk_scope() {
    let scenario = CommandHttpScenario::new().await;
    assert_eq!(
        scenario
            .press_key(&scenario.token, "PRE", "capture-event")
            .await
            .status(),
        StatusCode::OK
    );

    let filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_capture_mode(scenario.session.user.id.0),
    );
    let light_application::EventReplay::Events(events) =
        scenario.state.application_events.replay(0, &filter)
    else {
        panic!("the focused capture-mode event should remain replayable")
    };
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.desk_id, None);
    assert_eq!(event.delivery, light_application::DeliveryPolicy::Replaceable);
    assert_eq!(
        event.source,
        light_application::EventSource::Action(light_application::ActionSource::Http)
    );
    assert_eq!(
        event.object.as_ref().unwrap(),
        &light_application::EventObject::programming_capture_mode(scenario.session.user.id.0)
    );
    let light_application::ApplicationEvent::Programming(
        light_application::ProgrammingEvent::CaptureModeChanged(change),
    ) = &event.payload
    else {
        panic!("expected one capture-mode projection event")
    };
    assert_eq!(change.projection.revision, 1);
    assert!(change.projection.blind);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn active_preload_rejects_normal_values_without_mutation_or_values_event() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    assert_eq!(
        scenario
            .press_key(&scenario.token, "PRE", "preload-before-values")
            .await
            .status(),
        StatusCode::OK
    );

    let stale_capture = serde_json::json!({
        "request_id": "values-stale-preload-mode",
        "expected_revision": 0,
        "expected_capture_mode_revision": 0,
        "action": {
            "type": "set_fixture",
            "fixture_id": fixture.0,
            "attribute": "intensity",
            "value": {"kind": "normalized", "value": 0.5}
        }
    });
    let response = scenario.values_action(stale_capture).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error = json(response).await;
    assert_eq!(error["current_revision"], 0);
    assert_eq!(error["current_capture_mode_revision"], 1);

    let matching_capture = serde_json::json!({
        "request_id": "values-during-preload",
        "expected_revision": 0,
        "expected_capture_mode_revision": 1,
        "action": {
            "type": "set_fixture",
            "fixture_id": fixture.0,
            "attribute": "intensity",
            "value": {"kind": "normalized", "value": 0.5}
        }
    });
    let response = scenario.values_action(matching_capture).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error = json(response).await;
    assert_eq!(error["current_revision"], 0);
    assert_eq!(error["current_capture_mode_revision"], 1);

    let snapshot = json(scenario.values_snapshot().await).await;
    assert_eq!(snapshot["projection"]["revision"], 0);
    assert!(snapshot["projection"]["fixture_values"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(scenario.state.application_events.latest_sequence(), 2);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn programmer_values_actions_are_atomic_revisioned_replay_safe_and_sparse_on_no_op() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    let set = serde_json::json!({
        "request_id": "values-set",
        "expected_revision": 0,
        "expected_capture_mode_revision": 0,
        "action": {
            "type": "set_fixture",
            "fixture_id": fixture.0,
            "attribute": "intensity",
            "value": {"kind": "normalized", "value": 0.5},
            "timing": {"fade": true, "fade_millis": 1000, "delay_millis": 250}
        }
    });

    let response = scenario.values_action(set.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let first = json(response).await;
    assert_values_changed(&first, "values-set", 1, 2);
    assert_eq!(first["projection"]["fixture_values"][0]["fade"], true);
    assert_eq!(first["projection"]["fixture_values"][0]["fade_millis"], 1000);
    assert!(Uuid::parse_str(first["correlation_id"].as_str().unwrap()).is_ok());

    let replay = json(scenario.values_action(set).await).await;
    assert_eq!(replay["replayed"], true);
    assert_values_changed(&replay, "values-set", 1, 2);
    assert_eq!(scenario.state.application_events.latest_sequence(), 3);

    let batch = serde_json::json!({
        "request_id": "values-batch",
        "expected_revision": 1,
        "expected_capture_mode_revision": 0,
        "action": {
            "type": "batch",
            "mutations": [
                {"type": "release_fixture", "fixture_id": fixture.0, "attribute": "intensity"},
                {
                    "type": "set_group",
                    "group_id": "1",
                    "attribute": "pan",
                    "value": {"kind": "normalized", "value": 0.25}
                }
            ]
        }
    });
    let batch = json(scenario.values_action(batch).await).await;
    assert_values_changed(&batch, "values-batch", 2, 4);
    assert!(batch["projection"]["fixture_values"].as_array().unwrap().is_empty());
    assert_eq!(batch["projection"]["group_values"].as_array().unwrap().len(), 1);

    let clear = serde_json::json!({
        "request_id": "values-clear",
        "expected_revision": 2,
        "expected_capture_mode_revision": 0,
        "action": {"type": "clear"}
    });
    let clear = json(scenario.values_action(clear).await).await;
    assert_values_changed(&clear, "values-clear", 3, 5);

    let no_op = serde_json::json!({
        "request_id": "values-clear-no-op",
        "expected_revision": 3,
        "expected_capture_mode_revision": 0,
        "action": {"type": "clear"}
    });
    let no_op = json(scenario.values_action(no_op).await).await;
    assert_eq!(no_op["status"], "no_change");
    assert_eq!(no_op["revision"], 3);
    assert!(no_op.get("projection").is_none());
    assert!(no_op.get("event_sequence").is_none());
    assert_eq!(scenario.state.application_events.latest_sequence(), 6);

    let conflict = scenario
        .values_action(serde_json::json!({
            "request_id": "values-stale",
            "expected_revision": 2,
            "expected_capture_mode_revision": 0,
            "action": {"type": "clear"}
        }))
        .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let conflict = json(conflict).await;
    assert_eq!(conflict["kind"], "conflict");
    assert_eq!(conflict["current_revision"], 3);
    assert_eq!(conflict["retryable"], false);
    assert_only_values_events(&scenario, 3);
    assert!(!scenario
        .state
        .audit_events
        .lock()
        .iter()
        .any(|event| event.kind == "programmer_changed"));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn programmer_values_http_shares_one_user_between_desks_and_isolates_other_users() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    let second_desk = scenario
        .state
        .desk
        .lock()
        .add_desk("Second desk", "second-values")
        .unwrap();
    let (second_token, second_user) = login_on_desk(&scenario, "Operator", second_desk.id).await;
    assert_eq!(second_user, scenario.session.user.id.0);

    let first = fixture_set_request("desk-one", 0, fixture.0, 0.4);
    assert_eq!(scenario.values_action(first).await.status(), StatusCode::OK);
    let second = serde_json::json!({
        "request_id": "desk-two",
        "expected_revision": 1,
        "expected_capture_mode_revision": 0,
        "action": {
            "type": "set_group",
            "group_id": "1",
            "attribute": "pan",
            "value": {"kind": "normalized", "value": 0.7}
        }
    });
    let second = scenario
        .values_action_for(second_user, &second_token, second)
        .await;
    assert_eq!(second.status(), StatusCode::OK);
    let second = json(second).await;
    assert_values_changed(&second, "desk-two", 2, 5);
    assert_eq!(second["projection"]["fixture_values"].as_array().unwrap().len(), 1);
    assert_eq!(second["projection"]["group_values"].as_array().unwrap().len(), 1);

    let other_user = scenario.state.desk.lock().add_user("Other values user").unwrap();
    let (other_token, logged_in_user) = login_on_desk(
        &scenario,
        "Other values user",
        scenario.session.desk.id,
    )
    .await;
    assert_eq!(logged_in_user, other_user.id.0);
    let other = fixture_set_request("other-user", 0, fixture.0, 0.9);
    let other = scenario
        .values_action_for(other_user.id.0, &other_token, other)
        .await;
    assert_eq!(other.status(), StatusCode::OK);
    let other = json(other).await;
    assert_values_changed(&other, "other-user", 1, 8);
    assert_eq!(other["projection"]["group_values"].as_array().unwrap().len(), 0);

    let foreign = scenario
        .values_action_for(
            other_user.id.0,
            &scenario.token,
            serde_json::json!({
                "request_id": "forged-user",
                "expected_revision": 1,
                "expected_capture_mode_revision": 0,
                "action": {"type": "clear"}
            }),
        )
        .await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    assert_eq!(json(foreign).await["kind"], "forbidden");
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn programmer_delete_recreates_same_user_desks_with_monotonic_exact_user_authority() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    let second_desk = scenario
        .state
        .desk
        .lock()
        .add_desk("Lifecycle peer", "lifecycle-peer")
        .unwrap();
    let (second_token, second_user) = login_on_desk(&scenario, "Operator", second_desk.id).await;
    assert_eq!(second_user, scenario.session.user.id.0);
    let second_session = scenario
        .state
        .sessions
        .read()
        .values()
        .find(|session| session.token == second_token)
        .unwrap()
        .id;
    let old_request = fixture_set_request("before-delete", 0, fixture.0, 0.4);
    assert_eq!(
        scenario.values_action(old_request.clone()).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .press_key(&scenario.token, "PRE", "before-delete-preload")
            .await
            .status(),
        StatusCode::OK
    );
    let old_programmer_id = scenario
        .state
        .programmers
        .get(scenario.session.id)
        .unwrap()
        .id;
    let cursor = scenario.state.application_events.latest_sequence();

    let response = scenario
        .app
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/v1/programmers/{}/clear",
                scenario.session.id.0
            ))
            .header(
                header::AUTHORIZATION,
                format!("Bearer {second_token}"),
            )
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let user_id = scenario.session.user.id;
    assert_eq!(scenario.state.programmers.normal_values_revision(user_id), 2);
    assert_eq!(scenario.state.programmers.capture_mode_revision(user_id), 2);
    for session_id in [scenario.session.id, second_session] {
        let programmer = scenario.state.programmers.get(session_id).unwrap();
        assert!(programmer.values.is_empty());
        assert!(programmer.group_values.is_empty());
        assert_eq!(
            scenario.state.programmers.capture_mode(session_id),
            Some(Default::default())
        );
    }
    let light_application::EventReplay::Events(events) = scenario
        .state
        .application_events
        .replay(cursor, &light_application::EventFilter::default())
    else {
        panic!("the lifecycle events should remain replayable")
    };
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|event| event.desk_id.is_none()));
    let mut values_events = 0;
    let mut capture_events = 0;
    let mut lifecycle_events = 0;
    for event in &events {
        match &event.payload {
            light_application::ApplicationEvent::Programming(
                light_application::ProgrammingEvent::ValuesChanged(change),
            ) => {
                values_events += 1;
                assert_eq!(change.projection.revision, 2);
                assert!(change.projection.fixture_values.is_empty());
                assert!(change.projection.group_values.is_empty());
            }
            light_application::ApplicationEvent::Programming(
                light_application::ProgrammingEvent::CaptureModeChanged(change),
            ) => {
                capture_events += 1;
                assert_eq!(change.projection.revision, 2);
                assert_eq!(change.projection.mode(), Default::default());
            }
            light_application::ApplicationEvent::Programming(
                light_application::ProgrammingEvent::LifecycleChanged(change),
            ) => {
                lifecycle_events += 1;
                let light_application::ProgrammingLifecycleDelta::Upsert { programmer } =
                    &change.delta
                else {
                    panic!("replacement should upsert one new Programmer identity")
                };
                assert_eq!(programmer.user_id, user_id);
                assert_ne!(programmer.programmer_id, old_programmer_id);
                assert_eq!(programmer.sessions.len(), 2);
            }
            _ => panic!("unexpected Programmer lifecycle event"),
        }
    }
    assert_eq!((values_events, capture_events, lifecycle_events), (1, 1, 1));
    assert_eq!(
        scenario
            .state
            .audit_events
            .lock()
            .iter()
            .filter(|event| event.kind == "programmer_cleared")
            .count(),
        1
    );

    let lifecycle_cursor = scenario.state.application_events.latest_sequence();
    let stale = scenario.values_action(old_request).await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let stale = json(stale).await;
    assert_eq!(stale["current_revision"], 2);
    assert_eq!(stale["retryable"], false);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        lifecycle_cursor
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn programmer_values_wire_rejects_transient_or_mode_fields() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    let response = scenario
        .values_action(serde_json::json!({
            "request_id": "forged-preload",
            "expected_revision": 0,
            "expected_capture_mode_revision": 0,
            "action": {
                "type": "set_fixture",
                "fixture_id": fixture.0,
                "attribute": "intensity",
                "value": {"kind": "normalized", "value": 0.5},
                "mode": "preload"
            }
        }))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = json(response).await;
    assert_eq!(response["kind"], "invalid");
    assert!(response.get("current_revision").is_none());
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn assert_values_changed(value: &serde_json::Value, request_id: &str, revision: u64, sequence: u64) {
    assert_eq!(value["request_id"], request_id);
    assert_eq!(value["status"], "changed");
    assert_eq!(value["revision"], revision);
    assert_eq!(value["capture_mode_revision"], 0);
    assert_eq!(value["projection"]["revision"], revision);
    assert_eq!(value["event_sequence"], sequence);
}

fn fixture_set_request(
    request_id: &str,
    expected_revision: u64,
    fixture_id: Uuid,
    value: f32,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "expected_revision": expected_revision,
        "expected_capture_mode_revision": 0,
        "action": {
            "type": "set_fixture",
            "fixture_id": fixture_id,
            "attribute": "intensity",
            "value": {"kind": "normalized", "value": value}
        }
    })
}

async fn login_on_desk(
    scenario: &CommandHttpScenario,
    username: &str,
    desk_id: Uuid,
) -> (String, Uuid) {
    let response = scenario
        .app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"username": username, "desk_id": desk_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    (
        response["token"].as_str().unwrap().to_owned(),
        Uuid::parse_str(response["user"]["id"].as_str().unwrap()).unwrap(),
    )
}

fn assert_only_values_events(scenario: &CommandHttpScenario, expected: usize) {
    let filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_values(scenario.session.user.id.0),
    );
    let light_application::EventReplay::Events(events) = scenario
        .state
        .application_events
        .replay(0, &filter)
    else {
        panic!("the focused values event history should remain replayable")
    };
    assert_eq!(events.len(), expected);
    assert!(events.iter().all(|event| matches!(
        event.payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::ValuesChanged(_)
        )
    )));
}
