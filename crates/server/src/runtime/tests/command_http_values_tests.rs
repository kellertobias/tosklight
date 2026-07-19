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
async fn programmer_values_actions_are_atomic_revisioned_replay_safe_and_sparse_on_no_op() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    let set = serde_json::json!({
        "request_id": "values-set",
        "expected_revision": 0,
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
    assert_values_changed(&first, "values-set", 1, 1);
    assert_eq!(first["projection"]["fixture_values"][0]["fade"], true);
    assert_eq!(first["projection"]["fixture_values"][0]["fade_millis"], 1000);
    assert!(Uuid::parse_str(first["correlation_id"].as_str().unwrap()).is_ok());

    let replay = json(scenario.values_action(set).await).await;
    assert_eq!(replay["replayed"], true);
    assert_values_changed(&replay, "values-set", 1, 1);
    assert_eq!(scenario.state.application_events.latest_sequence(), 1);

    let batch = serde_json::json!({
        "request_id": "values-batch",
        "expected_revision": 1,
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
    assert_values_changed(&batch, "values-batch", 2, 2);
    assert!(batch["projection"]["fixture_values"].as_array().unwrap().is_empty());
    assert_eq!(batch["projection"]["group_values"].as_array().unwrap().len(), 1);

    let clear = serde_json::json!({
        "request_id": "values-clear",
        "expected_revision": 2,
        "action": {"type": "clear"}
    });
    let clear = json(scenario.values_action(clear).await).await;
    assert_values_changed(&clear, "values-clear", 3, 3);

    let no_op = serde_json::json!({
        "request_id": "values-clear-no-op",
        "expected_revision": 3,
        "action": {"type": "clear"}
    });
    let no_op = json(scenario.values_action(no_op).await).await;
    assert_eq!(no_op["status"], "no_change");
    assert_eq!(no_op["revision"], 3);
    assert!(no_op.get("projection").is_none());
    assert!(no_op.get("event_sequence").is_none());
    assert_eq!(scenario.state.application_events.latest_sequence(), 3);

    let conflict = scenario
        .values_action(serde_json::json!({
            "request_id": "values-stale",
            "expected_revision": 2,
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
    assert_values_changed(&second, "desk-two", 2, 2);
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
    assert_values_changed(&other, "other-user", 1, 3);
    assert_eq!(other["projection"]["group_values"].as_array().unwrap().len(), 0);

    let foreign = scenario
        .values_action_for(
            other_user.id.0,
            &scenario.token,
            serde_json::json!({
                "request_id": "forged-user",
                "expected_revision": 1,
                "action": {"type": "clear"}
            }),
        )
        .await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    assert_eq!(json(foreign).await["kind"], "forbidden");
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
    let light_application::EventReplay::Events(events) = scenario
        .state
        .application_events
        .replay(0, &light_application::EventFilter::default())
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
