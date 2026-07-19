#[test]
fn preset_serialization_preserves_nested_extensions_but_not_deleted_values() {
    let fixture = light_core::FixtureId::new();
    let fixture_key = fixture.0.to_string();
    let original = serde_json::json!({
        "name": "Look",
        "family": "Intensity",
        "number": 1,
        "values": {
            (fixture_key.clone()): {
                "intensity": {
                    "kind": "normalized",
                    "value": 0.25,
                    "future": "removed-with-value"
                },
                "dimmer": {
                    "kind": "normalized",
                    "value": 0.4,
                    "future": {"kept": true}
                }
            }
        },
        "group_values": {}
    });
    let mut preset = serde_json::from_value::<light_programmer::Preset>(original.clone()).unwrap();
    let attributes = preset.values.get_mut(&fixture).unwrap();
    attributes.remove(&light_core::AttributeKey::intensity());
    attributes.insert(
        light_core::AttributeKey("dimmer".into()),
        light_core::AttributeValue::Normalized(0.8),
    );

    let serialized = serialize_preset_preserving_extensions(&original, &preset).unwrap();

    assert!(
        serialized["values"][&fixture_key]
            .get("intensity")
            .is_none()
    );
    assert_eq!(
        serialized["values"][&fixture_key]["dimmer"]["future"],
        serde_json::json!({"kept": true})
    );
    let value = serialized["values"][&fixture_key]["dimmer"]["value"]
        .as_f64()
        .unwrap();
    assert!((value - 0.8).abs() < 1e-6);
}

#[tokio::test]
async fn inactive_preset_merge_preserves_stored_and_requested_nested_extensions() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let created = create_show(&app, &token, "Inactive preset extensions").await;
    let show_id = created["id"].as_str().unwrap();
    assert!(state.active_show.read().is_none());
    let fixture = light_core::FixtureId::new();
    let fixture_key = fixture.0.to_string();

    let first = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/presets/1.1/store"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "0")
                .body(Body::from(
                    serde_json::json!({
                        "mode": "overwrite",
                        "preset": preset_request_value(
                            &fixture_key,
                            0.25,
                            serde_json::json!({"future_server": {"kept": true}}),
                        )
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/presets/1.1/store"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "1")
                .body(Body::from(
                    serde_json::json!({
                        "mode": "merge",
                        "preset": preset_request_value(
                            &fixture_key,
                            0.75,
                            serde_json::json!({"future_client": "accepted"}),
                        )
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);

    let stored = app
        .oneshot(
            Request::get(format!(
                "/api/v1/shows/{show_id}/objects/preset/1.1"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stored.status(), StatusCode::OK);
    let stored = json(stored).await;
    let value = &stored["body"]["values"][&fixture_key]["intensity"];
    assert_eq!(value["future_server"], serde_json::json!({"kept": true}));
    assert_eq!(value["future_client"], "accepted");
    assert!((value["value"].as_f64().unwrap() - 0.75).abs() < 1e-6);

    let _ = std::fs::remove_dir_all(data_dir);
}

fn preset_request_value(
    fixture_key: &str,
    value: f64,
    extensions: serde_json::Value,
) -> serde_json::Value {
    let mut attribute = serde_json::json!({"kind": "normalized", "value": value});
    attribute
        .as_object_mut()
        .unwrap()
        .extend(extensions.as_object().unwrap().clone());
    serde_json::json!({
        "name": "Look",
        "family": "Intensity",
        "number": 1,
        "values": {(fixture_key): {"intensity": attribute}},
        "group_values": {}
    })
}

#[tokio::test]
async fn preset_store_endpoint_merges_with_revision_control() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let created = app
        .clone()
        .oneshot(
            Request::post("/api/v1/shows")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    r#"{"name":"Preset Test","data_base64":null,"overwrite":false}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let show_id = json(created).await["id"].as_str().unwrap().to_owned();
    let fixture = light_core::FixtureId::new();
    let first = light_programmer::Preset {
        name: "Look".into(),
        family: light_programmer::PresetFamily::Intensity,
        number: 1,
        values: HashMap::from([(
            fixture,
            HashMap::from([
                (
                    light_core::AttributeKey::intensity(),
                    light_core::AttributeValue::Normalized(0.5),
                ),
                (
                    light_core::AttributeKey("pan".into()),
                    light_core::AttributeValue::Normalized(0.25),
                ),
            ]),
        )]),
        group_values: HashMap::new(),
    };
    let stored = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/presets/1.1/store"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "0")
                .body(Body::from(
                    serde_json::json!({"mode":"overwrite","preset":first}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stored.status(), StatusCode::OK);
    let stored = json(stored).await;
    assert_eq!(stored["revision"], 1);
    assert_eq!(stored["preset"]["family"], "Intensity");
    assert_eq!(
        stored["preset"]["values"][fixture.0.to_string()]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    let stale = app
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/presets/1.1/store"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "0")
                .body(Body::from(
                    serde_json::json!({"mode":"merge","preset":{"name":"","values":{}}})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn preset_object_api_uses_family_scoped_numbers() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let created = create_show(&app, &token, "Typed preset addresses").await;
    let show_id = created["id"].as_str().unwrap();

    for (storage_key, family) in [("2.1", "Color"), ("3.1", "Position")] {
        let response = put_show_object(
            &app,
            &token,
            show_id,
            "preset",
            storage_key,
            serde_json::json!({
                "name": format!("{family} one"),
                "family": family,
                "number": 1,
                "values": {},
                "group_values": {},
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(Uuid::parse_str(show_id).unwrap()))
        .unwrap()
        .unwrap();
    ShowStore::open(&entry.path)
        .unwrap()
        .put_object(
            "preset",
            "7",
            &serde_json::json!({
                "name": "Legacy Color seven",
                "family": "Color",
                "values": {},
                "group_values": {},
            }),
            0,
        )
        .unwrap();

    let listed = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/shows/{show_id}/objects/preset"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed = json(listed).await;
    assert_eq!(listed.as_array().unwrap().len(), 3);
    assert!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .any(|object| object["id"] == "2.1"
                && object["body"]["family"] == "Color"
                && object["body"]["number"] == 1)
    );
    assert!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .any(|object| object["id"] == "7"
                && object["body"]["family"] == "Color"
                && object["body"]["number"] == 7)
    );
    assert!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .any(|object| object["id"] == "3.1"
                && object["body"]["family"] == "Position"
                && object["body"]["number"] == 1)
    );

    let global_plain_id = put_show_object(
        &app,
        &token,
        show_id,
        "preset",
        "1",
        serde_json::json!({
            "name": "Ambiguous",
            "family": "Color",
            "number": 1,
            "values": {},
            "group_values": {},
        }),
    )
    .await;
    assert_eq!(global_plain_id.status(), StatusCode::BAD_REQUEST);

    let _ = std::fs::remove_dir_all(data_dir);
}
