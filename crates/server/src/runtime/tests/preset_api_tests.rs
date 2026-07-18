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
