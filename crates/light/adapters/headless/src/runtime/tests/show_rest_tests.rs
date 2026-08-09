#[tokio::test]
async fn rest_session_show_and_revision_flow() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"username":"Operator"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let session = json(response).await;
    let token = session["token"].as_str().unwrap();
    let show = create_show(&app, token, "Tour").await;
    let show_id = show["id"].as_str().unwrap();
    let response = seed_show_object(
        &state,
        token,
        show_id,
        "group",
        "front",
        0,
        serde_json::json!({"name":"Front","fixtures":[]}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let conflict =
        seed_show_object(&state, token, show_id, "group", "front", 0, serde_json::json!({}))
            .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let opened = app
        .clone()
        .oneshot(open_show_request(token, show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let objects = app
        .clone()
        .oneshot(v2_show_object_get(token, show_id, "group", None))
        .await
        .unwrap();
    assert_eq!(objects.status(), StatusCode::OK);
    let objects = json(objects).await;
    assert_eq!(objects["show_revision"], 3);
    assert_eq!(objects["objects"].as_array().unwrap().len(), 1);
    let exact = app
        .clone()
        .oneshot(
            v2_show_object_get(token, show_id, "group", Some("front")),
        )
        .await
        .unwrap();
    assert_eq!(exact.status(), StatusCode::OK);
    assert_eq!(json(exact).await["show_revision"], 3);
    let missing = app
        .clone()
        .oneshot(
            v2_show_object_get(token, show_id, "group", Some("missing")),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::OK);
    let missing = json(missing).await;
    assert_eq!(missing["show_revision"], 3);
    assert!(missing["object"].is_null());
    assert!(
        std::fs::read_dir(data_dir.join("backups"))
            .unwrap()
            .next()
            .is_some()
    );
    let configuration=app.clone().oneshot(Request::post("/api/v2/configuration/update").header(header::CONTENT_TYPE,"application/json").header(header::AUTHORIZATION,format!("Bearer {token}")).body(Body::from(r#"{"request_id":"configuration-test","patch":{"frame_rate_hz":40,"output_bind_ip":"0.0.0.0","osc_bind":null,"art_timecode_bind":null,"backup_retention":5,"programmer_fade_millis":1250,"command_line_at_uses_programmer_fade":false,"sequence_master_fade_millis":2500}}"#)).unwrap()).await.unwrap();
    assert_eq!(configuration.status(), StatusCode::OK);
    assert_eq!(state.output.frame_rate_hz(), 40);
    for (index, bpm) in [101, 102, 103, 104].into_iter().enumerate() {
        let group = char::from(b'A' + index as u8);
        let response = app.clone().oneshot(
            Request::post(format!("/api/v2/speed-groups/{group}/actions"))
                .header(header::CONTENT_TYPE,"application/json")
                .header(header::AUTHORIZATION,format!("Bearer {token}"))
                .body(Body::from(format!(r#"{{"action":"set_bpm","bpm":{bpm}}}"#)))
                .unwrap()
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    assert_eq!(state.installation.configuration().speed_groups_bpm, [101.0, 102.0, 103.0, 104.0, 15.0]);
    assert_eq!(state.installation.configuration().programmer_fade_millis, 1_250);
    assert!(!state
        .installation.configuration()
        .command_line_at_uses_programmer_fade);
    assert_eq!(
        state.installation.configuration().sequence_master_fade_millis,
        2_500
    );
    let user = app
        .clone()
        .oneshot(
            Request::post("/api/v2/users/create")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"request_id":"create-video","name":"Video","enabled":true}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(user.status(), StatusCode::CREATED);
    assert!(authenticate_token(&state, "not-a-session-token").is_err());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn exact_non_group_read_does_not_deserialize_its_collection() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Exact object").await;
    let show_id = show["id"].as_str().unwrap();
    let entry = state
        .installation.show(light_core::ShowId(Uuid::parse_str(show_id).unwrap()))
        .unwrap()
        .unwrap();
    let store = ShowStore::open(&entry.path).unwrap();
    store
        .put_object("future", "wanted", &serde_json::json!({"value": 1}), 0)
        .unwrap();
    store
        .put_object("future", "sibling", &serde_json::json!({"value": 2}), 0)
        .unwrap();
    let expected_show_revision = store.portable_revision().unwrap().value();
    drop(store);
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let connection = rusqlite::Connection::open(&entry.path).unwrap();
    connection
        .execute(
            "UPDATE objects SET body_json=?1 WHERE kind=?2 AND id=?3",
            rusqlite::params!["not-json", "future", "sibling"],
        )
        .unwrap();
    drop(connection);

    let response = app
        .clone()
        .oneshot(v2_show_object_get(
            &token,
            show_id,
            "future",
            Some("wanted"),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["show_revision"], expected_show_revision);
    assert_eq!(
        response["object"]["body"],
        serde_json::json!({"value": 1})
    );
    let missing = app
        .oneshot(
            v2_show_object_get(&token, show_id, "future", Some("missing")),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::OK);
    let missing = json(missing).await;
    assert_eq!(missing["show_revision"], expected_show_revision);
    assert!(missing["object"].is_null());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn exact_group_read_keeps_derived_membership_materialization() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Derived group").await;
    let show_id = show["id"].as_str().unwrap();
    let entry = state
        .installation.show(light_core::ShowId(Uuid::parse_str(show_id).unwrap()))
        .unwrap()
        .unwrap();
    let fixtures = [light_core::FixtureId::new(), light_core::FixtureId::new()];
    let store = ShowStore::open(entry.path).unwrap();
    let source = light_programmer::GroupDefinition {
        id: "source".into(),
        fixtures: fixtures.to_vec(),
        ..Default::default()
    };
    let derived = light_programmer::GroupDefinition {
        id: "derived".into(),
        derived_from: Some(light_programmer::DerivedGroup {
            source_group_id: "source".into(),
            rule: light_programmer::SelectionRule::All,
        }),
        ..Default::default()
    };
    store
        .put_object("group", "source", &serde_json::to_value(source).unwrap(), 0)
        .unwrap();
    store
        .put_object(
            "group",
            "derived",
            &serde_json::to_value(derived).unwrap(),
            0,
        )
        .unwrap();
    drop(store);

    let opened = app
        .clone()
        .oneshot(open_show_request(&token, show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let response = app
        .oneshot(
            v2_show_object_get(&token, show_id, "group", Some("derived")),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json(response).await["object"]["body"]["fixtures"],
        serde_json::json!(fixtures)
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
