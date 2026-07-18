#[tokio::test]
async fn active_empty_show_rename_preserves_identity_content_and_revisions() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let created = create_show(&app, &token, "New Empty Show").await;
    let show_id = created["id"].as_str().unwrap();
    let original_path = created["path"].as_str().unwrap().to_owned();
    let opened = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/open"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let reopened_desk = DeskStore::open(data_dir.join("desk.sqlite")).unwrap();
    let reopened_empty = reopened_desk.active_show().unwrap().unwrap();
    assert_eq!(reopened_empty.id.0.to_string(), show_id);
    assert_eq!(reopened_empty.name, "New Empty Show");
    assert!(FsPath::new(&reopened_empty.path).exists());
    drop(reopened_desk);
    let stored = app
        .clone()
        .oneshot(
            Request::put(format!(
                "/api/v1/shows/{show_id}/objects/user_layout/operator"
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::IF_MATCH, "0")
            .body(Body::from(r#"{"marker":"before naming"}"#))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stored.status(), StatusCode::OK);
    let revision = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/revisions"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"name":"Before naming"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revision.status(), StatusCode::CREATED);

    let renamed = app
        .clone()
        .oneshot(
            Request::put(format!("/api/v1/shows/{show_id}/rename"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"name":"Opening Night"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(renamed.status(), StatusCode::OK);
    let renamed = json(renamed).await;
    assert_eq!(renamed["id"], show_id);
    assert_eq!(renamed["name"], "Opening Night");
    let renamed_path = renamed["path"].as_str().unwrap();
    assert!(renamed_path.ends_with("Opening Night.show"));
    assert!(!FsPath::new(&original_path).exists());
    let portable = ShowStore::open(renamed_path).unwrap();
    assert_eq!(portable.id().unwrap().0.to_string(), show_id);
    assert_eq!(portable.name().unwrap(), "Opening Night");

    let objects = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/shows/{show_id}/objects/user_layout"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(objects.status(), StatusCode::OK);
    assert_eq!(json(objects).await[0]["body"]["marker"], "before naming");
    let revisions = state
        .desk
        .lock()
        .show_revisions(light_core::ShowId(Uuid::parse_str(show_id).unwrap()))
        .unwrap();
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].name, "Before naming");
    let active = state.desk.lock().active_show().unwrap().unwrap();
    assert_eq!(active.id.0.to_string(), show_id);
    assert_eq!(active.name, "Opening Night");

    let _occupied = create_show(&app, &token, "Occupied").await;
    let collision = app
        .clone()
        .oneshot(
            Request::put(format!("/api/v1/shows/{show_id}/rename"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"name":"occupied"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(collision.status(), StatusCode::CONFLICT);
    let still_active = state.desk.lock().active_show().unwrap().unwrap();
    assert_eq!(still_active.id.0.to_string(), show_id);
    assert_eq!(still_active.name, "Opening Night");
    assert!(FsPath::new(&still_active.path).exists());

    let _ = std::fs::remove_dir_all(data_dir);
}
