use super::*;

#[tokio::test]
async fn prepared_revision_source_preserves_active_show_latest_and_named_snapshot() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Partial revision source").await;
    let show_id = show["id"].as_str().unwrap();
    let id = Uuid::parse_str(show_id).unwrap();
    let response = app
        .clone()
        .oneshot(open_show_request(&token, show_id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        put_revision_layout(&state, &token, show_id, 0, "named")
            .await
            .status(),
        StatusCode::OK
    );
    let response = app
        .clone()
        .oneshot(save_show_revision_request(&token, show_id, "Named source"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        put_revision_layout(&state, &token, show_id, 1, "latest")
            .await
            .status(),
        StatusCode::OK
    );
    let saved = state
        .installation
        .show_revision(light_core::ShowId(id), 1)
        .unwrap()
        .unwrap();
    let immutable_bytes = std::fs::read(&saved.path).unwrap();
    let original = state
        .installation
        .show(light_core::ShowId(id))
        .unwrap()
        .unwrap();
    let latest_bytes = std::fs::read(&original.path).unwrap();
    let active_before = state.active_show.current().unwrap();

    let copy = show_revision_source::prepare_named_revision_source(&state, id, 1).unwrap();
    assert_ne!(copy.id.0, id);
    assert_eq!(
        copy.revision_copy.as_ref().unwrap().revision_name,
        "Named source"
    );
    assert_eq!(copy.revision_copy.as_ref().unwrap().show_id.0, id);
    assert_eq!(state.active_show.current().unwrap().id, active_before.id);
    assert_eq!(std::fs::read(&saved.path).unwrap(), immutable_bytes);
    assert_eq!(std::fs::read(&original.path).unwrap(), latest_bytes);
    let store = ShowStore::open(&copy.path).unwrap();
    assert_eq!(
        store.objects("user_layout").unwrap()[0].body["marker"],
        "named"
    );
    store
        .put_object(
            "user_layout",
            "operator",
            &serde_json::json!({"marker":"copy edit"}),
            1,
        )
        .unwrap();
    assert_eq!(
        ShowStore::open(&original.path)
            .unwrap()
            .objects("user_layout")
            .unwrap()[0]
            .body["marker"],
        "latest"
    );
    let second = show_revision_source::prepare_named_revision_source(&state, id, 1).unwrap();
    assert_ne!(copy.id, second.id);
    assert_ne!(copy.name, second.name);

    let prepare = || {
        Request::post("/api/v2/shows")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::from(
                serde_json::json!({
                    "request_id": "prepare-named-source",
                    "action": {"type": "prepare_revision", "show_id": id, "revision": 1}
                })
                .to_string(),
            ))
            .unwrap()
    };
    let before_prepare = state.installation.show_library().unwrap().len();
    let (first, retried) = tokio::join!(
        app.clone().oneshot(prepare()),
        app.clone().oneshot(prepare())
    );
    let first = first.unwrap();
    let retried = retried.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(retried.status(), StatusCode::OK);
    let first = json(first).await;
    let retried = json(retried).await;
    assert_eq!(
        first["result"]["show"]["id"],
        retried["result"]["show"]["id"]
    );
    assert_ne!(first["replayed"], retried["replayed"]);
    assert_eq!(
        state.installation.show_library().unwrap().len(),
        before_prepare + 1
    );
    assert_eq!(state.active_show.current().unwrap().id, active_before.id);

    let url = format!("/api/v2/shows/{id}/revisions/1/download");
    let downloaded = app
        .clone()
        .oneshot(
            Request::get(&url)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(downloaded.status(), StatusCode::OK);
    assert_eq!(
        downloaded.headers()[header::CONTENT_TYPE],
        "application/vnd.light.show"
    );
    let bytes = axum::body::to_bytes(downloaded.into_body(), usize::MAX)
        .await
        .unwrap();
    let path = data_dir.join("downloaded-revision.show");
    std::fs::write(&path, bytes).unwrap();
    let downloaded_provenance = ShowStore::open(&path)
        .unwrap()
        .revision_copy_source()
        .unwrap()
        .unwrap();
    assert_eq!(downloaded_provenance.show_id.0, id);
    assert_eq!(downloaded_provenance.revision, 1);
    assert_eq!(downloaded_provenance.revision_name, "Named source");
    assert_eq!(
        ShowStore::open(&path)
            .unwrap()
            .objects("user_layout")
            .unwrap()[0]
            .body["marker"],
        "named"
    );
    assert_eq!(std::fs::read(&saved.path).unwrap(), immutable_bytes);
    assert_eq!(state.active_show.current().unwrap().id, active_before.id);

    let unauthorized = app
        .clone()
        .oneshot(Request::get(&url).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let missing = app
        .oneshot(
            Request::get(format!("/api/v2/shows/{id}/revisions/99/download"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert!(show_revision_source::prepare_named_revision_source(&state, id, 99).is_err());
    let count_before_failure = state.installation.show_library().unwrap().len();
    std::fs::write(&saved.path, b"broken sqlite snapshot").unwrap();
    assert!(show_revision_source::prepare_named_revision_source(&state, id, 1).is_err());
    assert_eq!(
        state.installation.show_library().unwrap().len(),
        count_before_failure
    );
    assert_eq!(state.active_show.current().unwrap().id, active_before.id);
    let _ = std::fs::remove_dir_all(data_dir);
}
