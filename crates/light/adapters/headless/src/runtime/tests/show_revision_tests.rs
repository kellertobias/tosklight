async fn put_revision_layout(
    state: &AppState,
    token: &str,
    show_id: &str,
    revision: u64,
    marker: &str,
) -> Response {
    seed_show_object(
        state,
        token,
        show_id,
        "user_layout",
        "operator",
        revision,
        serde_json::json!({"marker": marker}),
    )
    .await
}

async fn open_named_revision(app: &Router, token: &str, show_id: &str) -> Response {
    app.clone()
        .oneshot(open_show_revision_request(token, show_id, 1))
        .await
        .unwrap()
}

async fn revision_layout(app: &Router, token: &str, show_id: &str) -> Response {
    let opened = app
        .clone()
        .oneshot(open_show_request(token, show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    app.clone()
        .oneshot(v2_show_object_get(
            token,
            show_id,
            "user_layout",
            None,
        ))
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
        .installation.show(light_core::ShowId(show_uuid))
        .unwrap()
        .unwrap();
    let seed_path = data_dir.join("legacy-revision-seed.show");
    default_show::initialise_legacy_test_show(&seed_path).unwrap();
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
    let first = put_revision_layout(&state, &token, show_id, 0, "manual").await;
    assert_eq!(first.status(), StatusCode::OK);
    let saved = app
        .clone()
        .oneshot(save_show_revision_request(
            &token,
            show_id,
            "Before experiment",
        ))
        .await
        .unwrap();
    assert_eq!(saved.status(), StatusCode::OK);
    let saved = show_action_result(json(saved).await, "revision");
    assert_eq!(saved["revision"], 1);
    assert_eq!(saved["name"], "Before experiment");
    assert!(saved.get("path").is_none());
    let saved_revision = state
        .installation.show_revision(light_core::ShowId(show_uuid), 1)
        .unwrap()
        .unwrap();
    let saved_source = std::fs::read(&saved_revision.path).unwrap();
    let autosaved = put_revision_layout(&state, &token, show_id, 1, "autosave").await;
    assert_eq!(autosaved.status(), StatusCode::OK);
    let opened = open_named_revision(&app, &token, show_id).await;
    assert_eq!(opened.status(), StatusCode::OK);
    let copy = show_action_result(json(opened).await, "show");
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
        .installation.show(light_core::ShowId(Uuid::parse_str(copy_id).unwrap()))
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
    assert_eq!(
        original_objects["objects"][0]["body"]["marker"],
        "autosave"
    );
    let copy_objects = revision_layout(&app, &token, copy_id).await;
    assert_eq!(copy_objects.status(), StatusCode::OK);
    let copy_objects = json(copy_objects).await;
    assert_eq!(copy_objects["objects"][0]["body"]["marker"], "manual");

    let copy_edit = put_revision_layout(&state, &token, copy_id, 1, "copy edit").await;
    assert_eq!(copy_edit.status(), StatusCode::OK);
    let original_after_copy_edit = revision_layout(&app, &token, show_id).await;
    assert_eq!(
        json(original_after_copy_edit).await["objects"][0]["body"]["marker"],
        "autosave"
    );

    let opened_again = open_named_revision(&app, &token, show_id).await;
    assert_eq!(opened_again.status(), StatusCode::OK);
    let second_copy = show_action_result(json(opened_again).await, "show");
    assert_ne!(second_copy["id"], copy["id"]);
    assert_ne!(second_copy["name"], copy["name"]);
    assert!(second_copy["name"].as_str().unwrap().ends_with("-2"));
    assert_eq!(std::fs::read(&saved_revision.path).unwrap(), saved_source);

    let revisions = app
        .clone()
        .oneshot(show_snapshot_request(&token))
        .await
        .unwrap();
    let revisions = json(revisions).await["shows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|show| show["id"] == show_id)
        .unwrap()["revisions"]
        .clone();
    assert_eq!(revisions.as_array().unwrap().len(), 1);
    assert_eq!(revisions[0]["name"], "Before experiment");
    let _ = std::fs::remove_dir_all(data_dir);
}
async fn put_show_object(
    state: &AppState,
    token: &str,
    show: &str,
    kind: &str,
    id: &str,
    body: serde_json::Value,
) -> Response {
    seed_show_object(state, token, show, kind, id, 0, body).await
}

async fn seed_show_object(
    state: &AppState,
    token: &str,
    show: &str,
    kind: &str,
    id: &str,
    expected_revision: u64,
    body: serde_json::Value,
) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    let session = authenticate(state, &headers).unwrap();
    seed_object_for_test_put(
        state,
        &session,
        light_core::ShowId(Uuid::parse_str(show).unwrap()),
        kind.to_owned(),
        id.to_owned(),
        expected_revision,
        body,
    )
    .await
    .unwrap_or_else(ApiError::into_response)
}
