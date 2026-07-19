async fn put_revision_layout(
    app: &Router,
    token: &str,
    show_id: &str,
    revision: u64,
    marker: &str,
) -> Response {
    app.clone()
        .oneshot(
            Request::put(format!(
                "/api/v1/shows/{show_id}/objects/user_layout/operator"
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::IF_MATCH, revision.to_string())
            .body(Body::from(
                serde_json::json!({"marker": marker}).to_string(),
            ))
            .unwrap(),
        )
        .await
        .unwrap()
}

async fn open_named_revision(app: &Router, token: &str, show_id: &str) -> Response {
    app.clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/revisions/1/open"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn revision_layout(app: &Router, token: &str, show_id: &str) -> Response {
    app.clone()
        .oneshot(
            Request::get(format!(
                "/api/v1/shows/{show_id}/objects/user_layout"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn named_revision_load_creates_an_independent_provenanced_copy() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Revision source").await;
    let show_id = show["id"].as_str().unwrap();
    let show_uuid = Uuid::parse_str(show_id).unwrap();
    let source_entry = state
        .desk
        .lock()
        .show(light_core::ShowId(show_uuid))
        .unwrap()
        .unwrap();
    let seed_path = data_dir.join("legacy-revision-seed.show");
    default_show::initialise(&seed_path).unwrap();
    let legacy_fixture = ShowStore::open(&seed_path)
        .unwrap()
        .objects("patched_fixture")
        .unwrap()
        .remove(0);
    ShowStore::open(&source_entry.path)
        .unwrap()
        .put_object(
            "patched_fixture",
            &legacy_fixture.id,
            &legacy_fixture.body,
            0,
        )
        .unwrap();
    std::fs::remove_file(seed_path).unwrap();
    let first = put_revision_layout(&app, &token, show_id, 0, "manual").await;
    assert_eq!(first.status(), StatusCode::OK);
    let saved = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/revisions"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"name":"Before experiment"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(saved.status(), StatusCode::CREATED);
    let saved = json(saved).await;
    assert_eq!(saved["revision"], 1);
    assert_eq!(saved["name"], "Before experiment");
    assert!(saved.get("path").is_none());
    let saved_revision = state
        .desk
        .lock()
        .show_revision(light_core::ShowId(show_uuid), 1)
        .unwrap()
        .unwrap();
    let saved_source = std::fs::read(&saved_revision.path).unwrap();
    let autosaved = put_revision_layout(&app, &token, show_id, 1, "autosave").await;
    assert_eq!(autosaved.status(), StatusCode::OK);
    let opened = open_named_revision(&app, &token, show_id).await;
    assert_eq!(opened.status(), StatusCode::OK);
    let copy = json(opened).await;
    let copy_id = copy["id"].as_str().unwrap();
    assert_ne!(copy_id, show_id);
    assert!(
        copy["name"]
            .as_str()
            .unwrap()
            .starts_with("Revision source-rev-1-")
    );
    assert_eq!(copy["revision_copy"]["show_id"], show_id);
    assert_eq!(copy["revision_copy"]["show_name"], "Revision source");
    assert_eq!(copy["revision_copy"]["revision"], 1);
    assert_eq!(copy["revision_copy"]["revision_name"], "Before experiment");
    assert!(copy["revision_copy"]["copied_at"].as_str().is_some());
    assert_eq!(std::fs::read(&saved_revision.path).unwrap(), saved_source);
    let copy_entry = state
        .desk
        .lock()
        .show(light_core::ShowId(Uuid::parse_str(copy_id).unwrap()))
        .unwrap()
        .unwrap();
    let copy_fixture = ShowStore::open(&copy_entry.path)
        .unwrap()
        .objects("patched_fixture")
        .unwrap()
        .remove(0);
    assert!(
        !light_fixture::PortablePatchedFixtureRecord::decode(copy_fixture.body)
            .unwrap()
            .is_legacy_inline()
    );

    let original_objects = revision_layout(&app, &token, show_id).await;
    assert_eq!(original_objects.status(), StatusCode::OK);
    let original_objects = json(original_objects).await;
    assert_eq!(original_objects[0]["body"]["marker"], "autosave");
    let copy_objects = revision_layout(&app, &token, copy_id).await;
    assert_eq!(copy_objects.status(), StatusCode::OK);
    let copy_objects = json(copy_objects).await;
    assert_eq!(copy_objects[0]["body"]["marker"], "manual");

    let copy_edit = put_revision_layout(&app, &token, copy_id, 1, "copy edit").await;
    assert_eq!(copy_edit.status(), StatusCode::OK);
    let original_after_copy_edit = revision_layout(&app, &token, show_id).await;
    assert_eq!(
        json(original_after_copy_edit).await[0]["body"]["marker"],
        "autosave"
    );

    let opened_again = open_named_revision(&app, &token, show_id).await;
    assert_eq!(opened_again.status(), StatusCode::OK);
    let second_copy = json(opened_again).await;
    assert_ne!(second_copy["id"], copy["id"]);
    assert_ne!(second_copy["name"], copy["name"]);
    assert!(second_copy["name"].as_str().unwrap().ends_with("-2"));
    assert_eq!(std::fs::read(&saved_revision.path).unwrap(), saved_source);

    let revisions = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/shows/{show_id}/revisions"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let revisions = json(revisions).await;
    assert_eq!(revisions.as_array().unwrap().len(), 1);
    assert_eq!(revisions[0]["name"], "Before experiment");
    let _ = std::fs::remove_dir_all(data_dir);
}
async fn put_show_object(
    app: &Router,
    token: &str,
    show: &str,
    kind: &str,
    id: &str,
    body: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::put(format!("/api/v1/shows/{show}/objects/{kind}/{id}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "0")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
