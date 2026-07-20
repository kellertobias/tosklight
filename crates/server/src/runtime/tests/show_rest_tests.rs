#[tokio::test]
async fn rest_session_show_and_revision_flow() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"username":"Operator"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let session = json(response).await;
    let token = session["token"].as_str().unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/shows")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    r#"{"name":"Tour","data_base64":null,"overwrite":false}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let show = json(response).await;
    let show_id = show["id"].as_str().unwrap();
    let uri = format!("/api/v1/shows/{show_id}/objects/group/front");
    let response = app
        .clone()
        .oneshot(
            Request::put(&uri)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "0")
                .body(Body::from(r#"{"name":"Front","fixtures":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let conflict = app
        .clone()
        .oneshot(
            Request::put(&uri)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "0")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let objects = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/shows/{show_id}/objects/group"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(objects.status(), StatusCode::OK);
    assert_eq!(objects.headers()[header::ETAG], "\"2\"");
    assert_eq!(json(objects).await.as_array().unwrap().len(), 1);
    let exact = app
        .clone()
        .oneshot(
            Request::get(&uri)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exact.status(), StatusCode::OK);
    assert_eq!(exact.headers()["x-light-show-revision"], "\"2\"");
    let missing = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v1/shows/{show_id}/objects/group/missing"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(missing.headers()["x-light-show-revision"], "\"2\"");
    assert!(
        std::fs::read_dir(data_dir.join("backups"))
            .unwrap()
            .next()
            .is_some()
    );
    let configuration=app.clone().oneshot(Request::put("/api/v1/configuration").header(header::CONTENT_TYPE,"application/json").header(header::AUTHORIZATION,format!("Bearer {token}")).body(Body::from(r#"{"frame_rate_hz":40,"output_bind_ip":"0.0.0.0","osc_bind":null,"art_timecode_bind":null,"backup_retention":5,"speed_groups_bpm":[101,102,103,104],"programmer_fade_millis":1250,"sequence_master_fade_millis":2500}"#)).unwrap()).await.unwrap();
    assert_eq!(configuration.status(), StatusCode::OK);
    assert_eq!(state.output_rate.load(Ordering::Relaxed), 40);
    assert_eq!(
        state.configuration.read().speed_groups_bpm,
        [101.0, 102.0, 103.0, 104.0, 15.0]
    );
    assert_eq!(state.configuration.read().programmer_fade_millis, 1_250);
    assert_eq!(
        state.configuration.read().sequence_master_fade_millis,
        2_500
    );
    let user = app
        .clone()
        .oneshot(
            Request::post("/api/v1/users")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"name":"Video","enabled":true}"#))
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
        .desk
        .lock()
        .show(light_core::ShowId(Uuid::parse_str(show_id).unwrap()))
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
        .oneshot(
            Request::get(format!(
                "/api/v1/shows/{show_id}/objects/future/wanted"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["x-light-show-revision"],
        format!("\"{expected_show_revision}\"")
    );
    assert_eq!(json(response).await["body"], serde_json::json!({"value": 1}));
    let missing = app
        .oneshot(
            Request::get(format!(
                "/api/v1/shows/{show_id}/objects/future/missing"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        missing.headers()["x-light-show-revision"],
        format!("\"{expected_show_revision}\"")
    );
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
        .desk
        .lock()
        .show(light_core::ShowId(Uuid::parse_str(show_id).unwrap()))
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

    let response = app
        .oneshot(
            Request::get(format!(
                "/api/v1/shows/{show_id}/objects/group/derived"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["body"]["fixtures"], serde_json::json!(fixtures));
    let _ = std::fs::remove_dir_all(data_dir);
}
