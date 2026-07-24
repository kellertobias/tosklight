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
        .oneshot(open_show_request(&token, show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let reopened_desk = DeskStore::open(data_dir.join("desk.sqlite")).unwrap();
    let reopened_empty = reopened_desk.active_show().unwrap().unwrap();
    assert_eq!(reopened_empty.id.0.to_string(), show_id);
    assert_eq!(reopened_empty.name, "New Empty Show");
    assert!(FsPath::new(&reopened_empty.path).exists());
    drop(reopened_desk);
    let stored = seed_show_object(
        &state,
        &token,
        show_id,
        "user_layout",
        "operator",
        0,
        serde_json::json!({"marker":"before naming"}),
    )
    .await;
    assert_eq!(stored.status(), StatusCode::OK);
    let revision = app
        .clone()
        .oneshot(save_show_revision_request(&token, show_id, "Before naming"))
        .await
        .unwrap();
    assert_eq!(revision.status(), StatusCode::OK);

    let renamed = app
        .clone()
        .oneshot(rename_show_request(&token, show_id, "Opening Night"))
        .await
        .unwrap();
    assert_eq!(renamed.status(), StatusCode::OK);
    let renamed = show_action_result(json(renamed).await, "show");
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
        .oneshot(v2_show_object_get(
            &token,
            show_id,
            "user_layout",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(objects.status(), StatusCode::OK);
    assert_eq!(
        json(objects).await["objects"][0]["body"]["marker"],
        "before naming"
    );
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
        .oneshot(rename_show_request(&token, show_id, "occupied"))
        .await
        .unwrap();
    assert_eq!(collision.status(), StatusCode::CONFLICT);
    let still_active = state.desk.lock().active_show().unwrap().unwrap();
    assert_eq!(still_active.id.0.to_string(), show_id);
    assert_eq!(still_active.name, "Opening Night");
    assert!(FsPath::new(&still_active.path).exists());

    let _ = std::fs::remove_dir_all(data_dir);
}
