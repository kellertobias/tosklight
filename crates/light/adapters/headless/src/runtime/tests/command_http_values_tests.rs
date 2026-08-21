#[tokio::test]
async fn programmer_values_snapshot_returns_authenticated_projection_and_safe_cursor() {
    let scenario = CommandHttpScenario::new().await;
    let fixture_id = scenario.install_direct_fixture();
    let response = scenario
        .execute("values-snapshot", Some("GROUP 1 AT 50"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let expected_cursor = scenario.state.events.latest_sequence();
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
            .programming
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
    assert_eq!(
        snapshot.cursor.sequence,
        scenario.state.events.latest_sequence()
    );
    assert_eq!(snapshot.projection.user_id, scenario.session.user.id.0);
    assert_eq!(snapshot.projection.revision, 1);
    assert!(snapshot.projection.blind);
    assert!(!snapshot.projection.preview);
    assert!(snapshot.projection.preload_capture_programmer);

    let second_desk = scenario
        .state
        .installation.add_desk("Second capture desk", "second-capture")
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
        scenario.state.events.replay(0, &filter)
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
    let values_filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::programming_values(scenario.session.user.id.0),
    );
    let light_application::EventReplay::Events(values_events) =
        scenario.state.events.replay(0, &values_filter)
    else {
        panic!("the focused Programmer values history should remain replayable")
    };
    assert!(values_events.is_empty());
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
    assert_eq!(scenario.state.events.latest_sequence(), 3);

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
    assert_eq!(scenario.state.events.latest_sequence(), 6);

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
        .events.audit_events()
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
        .installation.add_desk("Second desk", "second-values")
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

    let other_user = scenario.state.installation.add_user("Other values user").unwrap();
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
        .installation.add_desk("Lifecycle peer", "lifecycle-peer")
        .unwrap();
    let (second_token, second_user) = login_on_desk(&scenario, "Operator", second_desk.id).await;
    assert_eq!(second_user, scenario.session.user.id.0);
    let second_session = scenario
        .state
        .sessions.sessions().into_iter()
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
        .programming
        .get(scenario.session.id)
        .unwrap()
        .id;
    let cursor = scenario.state.events.latest_sequence();

    let response = scenario
        .app
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/v2/programmers/{}/clear",
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
    assert_eq!(scenario.state.programming.normal_values_revision(user_id), 2);
    assert_eq!(scenario.state.programming.capture_mode_revision(user_id), 2);
    assert_eq!(scenario.state.programming.priority_revision(user_id), 1);
    for session_id in [scenario.session.id, second_session] {
        let programmer = scenario.state.programming.get(session_id).unwrap();
        assert!(programmer.values.is_empty());
        assert!(programmer.group_values.is_empty());
        assert_eq!(
            scenario.state.programming.capture_mode(session_id),
            Some(Default::default())
        );
    }
    let light_application::EventReplay::Events(events) = scenario
        .state
        .events
        .replay(cursor, &light_application::EventFilter::default())
    else {
        panic!("the lifecycle events should remain replayable")
    };
    assert_eq!(events.len(), 4);
    assert!(events.iter().all(|event| event.desk_id.is_none()));
    let mut values_events = 0;
    let mut capture_events = 0;
    let mut lifecycle_events = 0;
    let mut priority_events = 0;
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
            light_application::ApplicationEvent::Programming(
                light_application::ProgrammingEvent::PriorityChanged(
                    light_application::ProgrammingPriorityChange::Upsert { projection },
                ),
            ) => {
                priority_events += 1;
                assert_eq!(projection.user_id, user_id);
                assert_eq!(projection.revision, 1);
                assert_eq!(projection.priority, 100);
            }
            _ => panic!("unexpected Programmer lifecycle event"),
        }
    }
    assert_eq!((values_events, capture_events, lifecycle_events), (1, 1, 1));
    assert_eq!(priority_events, 1);
    assert_eq!(
        scenario
            .state
            .events.audit_events()
            .iter()
            .filter(|event| event.kind == "programmer_cleared")
            .count(),
        1
    );

    let lifecycle_cursor = scenario.state.events.latest_sequence();
    let stale = scenario.values_action(old_request).await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let stale = json(stale).await;
    assert_eq!(stale["current_revision"], 2);
    assert_eq!(stale["retryable"], false);
    assert_eq!(
        scenario.state.events.latest_sequence(),
        lifecycle_cursor
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn programmer_values_wire_accepts_unknown_fields_and_validates_known_fields() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    let response = scenario
        .values_action(serde_json::json!({
            "request_id": "forged-preload",
            "expected_revision": 0,
            "expected_capture_mode_revision": 0,
            "future_root": "accepted",
            "action": {
                "type": "set_fixture",
                "fixture_id": fixture.0,
                "attribute": "intensity",
                "value": {"kind": "normalized", "value": 0.5},
                "mode": "preload",
                "timing": {
                    "fade": false,
                    "future_timing": true
                }
            }
        }))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_values_changed(&response, "forged-preload", 1, 2);
    assert_eq!(
        response["projection"]["fixture_values"][0]["value"]["value"],
        0.5
    );

    let response = scenario
        .values_action(serde_json::json!({
            "request_id": "wrong-known-field",
            "expected_revision": "one",
            "expected_capture_mode_revision": 0,
            "action": {"type": "clear"}
        }))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = json(response).await;
    assert_eq!(response["kind"], "invalid");
    assert!(response["error"].as_str().unwrap().contains("expected_revision"));
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
            Request::post("/api/v2/sessions")
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
        .events
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

fn color_range_fixture(number: u32, attributes: &[&str]) -> light_fixture::PatchedFixture {
    let (template, _, _) = schema_v2_direct_fixture();
    let mut fixture = template;
    fixture.fixture_id = light_core::FixtureId::new();
    fixture.fixture_number = Some(number);
    fixture.address = Some(1 + (number as u16 - 1) * 8);
    fixture.definition.heads = vec![light_fixture::LogicalHead {
        index: 0,
        name: "Main".into(),
        shared: true,
        parameters: attributes
            .iter()
            .map(|attribute| light_fixture::Parameter {
                attribute: light_core::AttributeKey((*attribute).into()),
                components: vec![],
                default: 0.0,
                virtual_dimmer: false,
                metadata: Default::default(),
                capabilities: vec![],
            })
            .collect(),
    }];
    fixture
}

fn native_hsi_color_range_fixture(number: u32) -> light_fixture::PatchedFixture {
    let mut profile = light_fixture::FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Native HSI".into();
    profile.short_name = "HSI".into();
    let mode = &mut profile.modes[0];
    let mode_id = mode.id;
    let head_id = mode.heads[0].id;
    mode.splits[0].footprint = 3;
    mode.channels = [
        ("fixture.hue", "color.hue"),
        ("fixture.saturation", "color.saturation"),
        ("fixture.brightness", "color.brightness"),
    ]
    .into_iter()
    .map(|(fixture_attribute, attribute)| light_fixture::FixtureChannel {
        id: Uuid::new_v4(),
        head_id,
        split: 1,
        fixture_attribute: light_core::AttributeKey(fixture_attribute.into()),
        attribute: light_core::AttributeKey(attribute.into()),
        canonical_transform: light_fixture::CanonicalTransform::Identity,
        resolution: light_fixture::ChannelResolution::U8,
        secondary_slots: vec![],
        default_raw: 0,
        highlight_raw: 255,
        physical_min: None,
        physical_max: None,
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        reacts_to_sequence_master: false,
        reacts_to_group_master: false,
        reacts_to_grand_master: false,
        behavior: light_fixture::ChannelBehavior::Controlled,
        functions: vec![],
    })
    .collect();
    mode.color_systems = vec![light_fixture::HeadColorSystem {
        head_id,
        correction_matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        system: light_fixture::ColorSystem::HueSaturation {
            hue_channel_id: mode.channels[0].id,
            saturation_channel_id: mode.channels[1].id,
            intensity_channel_id: Some(mode.channels[2].id),
        },
    }];
    let mut fixture = schema_v2_direct_fixture().0;
    fixture.fixture_number = Some(number);
    fixture.name = "Native HSI".into();
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    fixture.address = Some(1 + (number as u16 - 1) * 3);
    fixture
}

fn programmer_color_values(
    scenario: &CommandHttpScenario,
) -> impl Fn(light_core::FixtureId, &str) -> Option<f32> + use<> {
    let programmer = scenario.state.programming.get(scenario.session.id).unwrap();
    move |fixture: light_core::FixtureId, attribute: &str| {
        programmer
            .values
            .iter()
            .find(|value| value.fixture_id == fixture && value.attribute.0 == attribute)
            .and_then(|value| value.value.normalized())
    }
}

#[tokio::test]
async fn color_range_resolves_rgb_and_cmy_channels_server_side_in_selection_order() {
    let scenario = CommandHttpScenario::new().await;
    let rgb_first = color_range_fixture(1, &["color.red", "color.green", "color.blue"]);
    let cmy_middle = color_range_fixture(2, &["color.cyan", "color.magenta", "color.yellow"]);
    let rgb_last = color_range_fixture(3, &["color.red", "color.green", "color.blue"]);
    let ids = [
        rgb_first.fixture_id,
        cmy_middle.fixture_id,
        rgb_last.fixture_id,
    ];
    scenario
        .state
        .output.replace_snapshot(EngineSnapshot {
            fixtures: vec![rgb_first, cmy_middle, rgb_last].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();

    // Red → blue the short way; the CMY fixture sits mid-arc on green and an id outside the
    // patch is skipped without failing the request.
    let absent = light_core::FixtureId::new();
    let action = serde_json::json!({
        "request_id": "color-range",
        "expected_revision": 0,
        "expected_capture_mode_revision": 0,
        "action": {
            "type": "set_selection_color_range",
            "fixture_ids": [ids[0].0, ids[1].0, ids[2].0, absent.0],
            "start": {"hue": 0.0, "saturation": 1.0},
            "end": {"hue": 0.5, "saturation": 1.0},
            "hue_travel": 0.5,
            "brightness": 1.0,
            "timing": {"fade": false}
        }
    });
    let response = scenario.values_action(action).await;
    assert_eq!(response.status(), StatusCode::OK);

    let value = programmer_color_values(&scenario);
    // 4 selected ids → ratio thirds: red, green (1/3), cyan-ish? No — endpoints pin over the
    // 4-strong selection; the absent id occupies the last slot, so the patched fixtures sit at
    // ratios 0, 1/3, 2/3 → red, green, cyan-blue boundary.
    assert_eq!(value(ids[0], "color.red"), Some(1.0));
    assert_eq!(value(ids[0], "color.green"), Some(0.0));
    assert_eq!(value(ids[0], "color.blue"), Some(0.0));
    assert_eq!(value(ids[0], "color.cyan"), None);
    // Hue 1/6 at full saturation → RGB (1, 1, 0) → CMY (0, 0, 1).
    assert_eq!(value(ids[1], "color.cyan"), Some(0.0));
    assert_eq!(value(ids[1], "color.magenta"), Some(0.0));
    assert_eq!(value(ids[1], "color.yellow"), Some(1.0));
    assert_eq!(value(ids[1], "color.red"), None);
    // Hue 1/3 → green.
    assert_eq!(value(ids[2], "color.red"), Some(0.0));
    assert_eq!(value(ids[2], "color.green"), Some(1.0));
    assert_eq!(value(ids[2], "color.blue"), Some(0.0));
    assert_eq!(value(absent, "color.red"), None);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn color_picker_persists_canonical_color_and_renders_native_hsi_channels() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = native_hsi_color_range_fixture(1);
    let fixture_id = fixture.fixture_id;
    scenario
        .state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();

    let response = scenario
        .values_action(serde_json::json!({
            "request_id": "native-hsi",
            "expected_revision": 0,
            "expected_capture_mode_revision": 0,
            "action": {
                "type": "set_selection_color_range",
                "fixture_ids": [fixture_id.0],
                "start": {"hue": 1.0 / 3.0, "saturation": 1.0},
                "end": {"hue": 1.0 / 3.0, "saturation": 1.0},
                "hue_travel": 0.0,
                "brightness": 0.5,
                "timing": {"fade": false}
            }
        }))
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let programmer = scenario.state.programming.get(scenario.session.id).unwrap();
    assert_eq!(programmer.values.len(), 1);
    assert_eq!(programmer.values[0].attribute.0, "color");
    let light_core::AttributeValue::ColorXyz(stored) = programmer.values[0].value else {
        panic!("whole-color picker must persist one canonical XYZ value")
    };
    let expected = light_fixture::srgb_to_xyz(0.0, 0.5, 0.0);
    assert!((stored.x - expected.x).abs() < 0.000_001);
    assert!((stored.y - expected.y).abs() < 0.000_001);
    assert!((stored.z - expected.z).abs() < 0.000_001);

    let rendered = scenario
        .state
        .output
        .render(RenderOptions::default())
        .unwrap();
    assert_eq!(&rendered.universes[&1][0..3], &[85, 255, 128]);
    let blacked_out = scenario
        .state
        .output
        .render(RenderOptions {
            blackout: true,
            ..RenderOptions::default()
        })
        .unwrap();
    assert_eq!(
        blacked_out.universes[&1][2], 0,
        "HSB brightness must reach its physical off endpoint during blackout"
    );
    let visual = scenario
        .state
        .output
        .profile_visualization_values(
            &scenario.state.output.resolved_values(),
            RenderOptions::default(),
        )
        .unwrap();
    let Some(light_core::AttributeValue::ColorXyz(projected)) =
        visual.get(&(fixture_id, light_core::AttributeKey("color".into())))
    else {
        panic!("native HSI output must project back to canonical visible color")
    };
    let Some(light_core::AttributeValue::Normalized(projected_intensity)) =
        visual.get(&(fixture_id, light_core::AttributeKey::intensity()))
    else {
        panic!("native HSI output must project its visible brightness")
    };
    assert!((projected.x * projected_intensity - expected.x).abs() < 0.01);
    assert!((projected.y * projected_intensity - expected.y).abs() < 0.01);
    assert!((projected.z * projected_intensity - expected.z).abs() < 0.01);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn color_range_supports_a_full_revolution_back_to_the_start_color() {
    let scenario = CommandHttpScenario::new().await;
    let fixtures: Vec<_> = (1..=3)
        .map(|number| color_range_fixture(number, &["color.red", "color.green", "color.blue"]))
        .collect();
    let ids: Vec<_> = fixtures.iter().map(|fixture| fixture.fixture_id).collect();
    scenario
        .state
        .output.replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();

    // Maintainer-pinned: red → red with one full revolution distributes the wheel once —
    // hues 0°, 120°, 240° → red, green, blue.
    let action = serde_json::json!({
        "request_id": "color-revolution",
        "expected_revision": 0,
        "expected_capture_mode_revision": 0,
        "action": {
            "type": "set_selection_color_range",
            "fixture_ids": [ids[0].0, ids[1].0, ids[2].0],
            "start": {"hue": 0.0, "saturation": 1.0},
            "end": {"hue": 0.0, "saturation": 1.0},
            "hue_travel": 1.0,
            "brightness": 1.0,
            "timing": {"fade": false}
        }
    });
    let response = scenario.values_action(action).await;
    assert_eq!(response.status(), StatusCode::OK);

    let value = programmer_color_values(&scenario);
    assert_eq!(value(ids[0], "color.red"), Some(1.0));
    assert_eq!(value(ids[0], "color.green"), Some(0.0));
    assert_eq!(value(ids[1], "color.green"), Some(1.0));
    assert_eq!(value(ids[1], "color.red"), Some(0.0));
    assert_eq!(value(ids[2], "color.blue"), Some(1.0));
    assert_eq!(value(ids[2], "color.red"), Some(0.0));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn color_range_rejects_out_of_range_payloads_without_mutation() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = color_range_fixture(1, &["color.red", "color.green", "color.blue"]);
    let id = fixture.fixture_id;
    scenario
        .state
        .output.replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let action = serde_json::json!({
        "request_id": "color-invalid",
        "expected_revision": 0,
        "expected_capture_mode_revision": 0,
        "action": {
            "type": "set_selection_color_range",
            "fixture_ids": [id.0],
            "start": {"hue": 1.5, "saturation": 1.0},
            "end": {"hue": 0.0, "saturation": 1.0},
            "hue_travel": 0.0,
            "brightness": 1.0,
            "timing": {"fade": false}
        }
    });
    let response = scenario.values_action(action).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let value = programmer_color_values(&scenario);
    assert_eq!(value(id, "color.red"), None);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn set_selection_resolves_the_spread_server_side_in_selection_order() {
    let scenario = CommandHttpScenario::new().await;
    let (template, _, _) = schema_v2_direct_fixture();
    let mut fixtures = Vec::new();
    let mut ids = Vec::new();
    for number in 0..3_u32 {
        let mut fixture = template.clone();
        fixture.fixture_id = light_core::FixtureId::new();
        fixture.fixture_number = Some(number + 1);
        fixture.address = Some(1 + number as u16 * 8);
        ids.push(fixture.fixture_id);
        fixtures.push(fixture);
    }
    scenario
        .state
        .output.replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();

    // Deliberately not id-sorted: the interpolation must follow the request's selection order,
    // matching the command line's `AT 0 THRU 50` distribution.
    let ordered = [ids[2], ids[0], ids[1]];
    let action = serde_json::json!({
        "request_id": "selection-spread",
        "expected_revision": 0,
        "expected_capture_mode_revision": 0,
        "action": {"type": "batch", "mutations": [{
            "type": "set_selection",
            "fixture_ids": [ordered[0].0, ordered[1].0, ordered[2].0],
            "attribute": "intensity",
            "value": {"kind": "spread", "value": [0.0, 0.5]},
            "timing": {"fade": false}
        }]}
    });
    let response = scenario.values_action(action).await;
    assert_eq!(response.status(), StatusCode::OK);

    let programmer = scenario.state.programming.get(scenario.session.id).unwrap();
    let value_of = |fixture: light_core::FixtureId| {
        programmer
            .values
            .iter()
            .find(|value| value.fixture_id == fixture)
            .and_then(|value| value.value.normalized())
            .unwrap()
    };
    assert_eq!(value_of(ordered[0]), 0.0);
    assert_eq!(value_of(ordered[1]), 0.25);
    assert_eq!(value_of(ordered[2]), 0.5);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn indexed_preset_uses_each_embedded_profile_raw_value_immediately() {
    let scenario = CommandHttpScenario::new().await;
    let first = schema_v2_direct_fixture().0;
    let first_profile_revision = first.definition.profile_snapshot.as_ref().unwrap().revision;
    let first_function_id = first.definition.profile_snapshot.as_ref().unwrap().modes[0].channels
        [0]
    .functions[0]
        .id;
    let mut second = first.clone();
    second.fixture_id = light_core::FixtureId::new();
    second.fixture_number = Some(2);
    second.address = Some(3);
    let second_profile = second.definition.profile_snapshot.as_mut().unwrap();
    let light_fixture::ChannelFunctionBehavior::Indexed { raw_value, .. } =
        &mut second_profile.modes[0].channels[0].functions[0].behavior
    else {
        panic!("schema-v2 test fixture should expose an indexed Gobo function");
    };
    *raw_value = 41;
    let second_profile_revision = second_profile.revision;
    let second_function_id = second_profile.modes[0].channels[0].functions[0].id;
    let ids = [first.fixture_id, second.fixture_id];
    scenario
        .state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![first, second].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let selection_revision = scenario
        .state
        .programming
        .select(scenario.session.id, ids);

    let response = scenario
        .values_action(serde_json::json!({
            "request_id": "indexed-preset",
            "expected_revision": 0,
            "expected_capture_mode_revision": 0,
            "action": {
                "type": "apply_indexed_preset",
                "expected_selection_revision": selection_revision,
                "attribute": "gobo.1",
                "targets": [
                    {
                        "fixture_id": ids[0].0,
                        "function_id": first_function_id,
                        "expected_profile_revision": first_profile_revision
                    },
                    {
                        "fixture_id": ids[1].0,
                        "function_id": second_function_id,
                        "expected_profile_revision": second_profile_revision
                    }
                ]
            }
        }))
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let programmer = scenario.state.programming.get(scenario.session.id).unwrap();
    assert_eq!(programmer.values.len(), 2);
    for value in programmer.values.iter() {
        assert_eq!(
            value.value,
            light_core::AttributeValue::Discrete("gobo.dots".into())
        );
        assert!(!value.fade);
        assert_eq!(value.fade_millis, None);
    }
    let frame = scenario
        .state
        .output
        .render(RenderOptions::default())
        .unwrap()
        .universes[&1];
    assert_eq!(frame[0], 93);
    assert_eq!(frame[2], 41);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn indexed_preset_rejects_a_stale_selection_without_mutating_the_programmer() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    let profile_revision = fixture.definition.profile_snapshot.as_ref().unwrap().revision;
    let function_id = fixture.definition.profile_snapshot.as_ref().unwrap().modes[0].channels[0]
        .functions[0]
        .id;
    scenario
        .state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let stale_revision = scenario
        .state
        .programming
        .select(scenario.session.id, [fixture_id]);
    scenario
        .state
        .programming
        .select(scenario.session.id, []);

    let response = scenario
        .values_action(serde_json::json!({
            "request_id": "stale-indexed-preset",
            "expected_revision": 0,
            "expected_capture_mode_revision": 0,
            "action": {
                "type": "apply_indexed_preset",
                "expected_selection_revision": stale_revision,
                "attribute": "gobo.1",
                "targets": [{
                    "fixture_id": fixture_id.0,
                    "function_id": function_id,
                    "expected_profile_revision": profile_revision
                }]
            }
        }))
        .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .values
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn indexed_preset_rejects_a_stale_embedded_profile_revision() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    let profile = fixture.definition.profile_snapshot.as_ref().unwrap();
    let function_id = profile.modes[0].channels[0].functions[0].id;
    let stale_profile_revision = profile.revision.saturating_sub(1);
    scenario
        .state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let selection_revision = scenario
        .state
        .programming
        .select(scenario.session.id, [fixture_id]);

    let response = scenario
        .values_action(serde_json::json!({
            "request_id": "stale-profile-indexed-preset",
            "expected_revision": 0,
            "expected_capture_mode_revision": 0,
            "action": {
                "type": "apply_indexed_preset",
                "expected_selection_revision": selection_revision,
                "attribute": "gobo.1",
                "targets": [{
                    "fixture_id": fixture_id.0,
                    "function_id": function_id,
                    "expected_profile_revision": stale_profile_revision
                }]
            }
        }))
        .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .values
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
