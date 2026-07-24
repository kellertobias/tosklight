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

#[tokio::test]
async fn typed_recording_persists_default_family_preset_under_its_bare_address() {
    // Regression (HIGHLIGHT-001): storing a default-family ("Mixed"/"All") Preset through the
    // bare pool address `197` must persist and read back under object id `197`, matching legacy
    // shows and the operator's addressing rather than the internal canonical `0.197`.
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, session_id) = login(&app, "Operator").await;
    let created = create_show(&app, &token, "Bare preset address").await;
    let show_id = created["id"].as_str().unwrap();
    let fixture = light_core::FixtureId::new();
    let fixture_key = fixture.0.to_string();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    state.programmers.set(
        light_core::SessionId(Uuid::parse_str(&session_id).unwrap()),
        fixture,
        light_core::AttributeKey("pan".into()),
        light_core::AttributeValue::Normalized(0.41),
    );

    let stored = app
        .clone()
        .oneshot(
            Request::post("/api/v2/presets/record")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "bare-mixed-preset",
                        "address": {"family": "mixed", "number": 197},
                        "name": "Highlight isolation",
                        "mode": "overwrite",
                        "expected_object_revision": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stored.status(), StatusCode::OK);

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
    let objects = listed.as_array().unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0]["id"], "197");
    assert_eq!(objects[0]["body"]["family"], "Mixed");
    assert!(objects[0]["body"]["values"][&fixture_key]["pan"].is_object());

    let fetched = app
        .oneshot(
            Request::get(format!("/api/v1/shows/{show_id}/objects/preset/197"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    assert_eq!(json(fetched).await["id"], "197");

    let _ = std::fs::remove_dir_all(data_dir);
}
