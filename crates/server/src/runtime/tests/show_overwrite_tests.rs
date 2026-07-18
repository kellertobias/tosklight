async fn save_show_revision(app: &Router, token: &str, show_id: &str, name: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/revisions"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(serde_json::json!({"name": name}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

fn overwrite_revision_body(path: &str) -> serde_json::Value {
    ShowStore::open(path)
        .unwrap()
        .objects("user_layout")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "operator")
        .unwrap()
        .body
}

async fn prepare_overwrite_source(
    state: &AppState,
    app: &Router,
    token: &str,
) -> (String, String, String, serde_json::Value) {
    let source = create_show(app, token, "Overwrite source").await;
    let source_id = source["id"].as_str().unwrap().to_owned();
    assert_eq!(
        put_show_object(
            app,
            token,
            &source_id,
            "user_layout",
            "operator",
            serde_json::json!({"marker":"named snapshot"}),
        )
        .await
        .status(),
        StatusCode::OK
    );
    save_show_revision(app, token, &source_id, "Source Revision").await;
    let revision_path = state
        .desk
        .lock()
        .show_revision(
            light_core::ShowId(Uuid::parse_str(&source_id).unwrap()),
            1,
        )
        .unwrap()
        .unwrap()
        .path;
    let revision_body = overwrite_revision_body(&revision_path);
    assert_eq!(revision_body["marker"], "named snapshot");
    let opened = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{source_id}/revisions/1/open"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let copy_id = json(opened).await["id"].as_str().unwrap().to_owned();
    let copy_edit = app
        .clone()
        .oneshot(
            Request::put(format!(
                "/api/v1/shows/{copy_id}/objects/user_layout/operator"
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::IF_MATCH, "1")
            .body(Body::from(r#"{"marker":"copy edit"}"#))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(copy_edit.status(), StatusCode::OK);
    save_show_revision(app, token, &copy_id, "Copy private checkpoint").await;
    (source_id, copy_id, revision_path, revision_body)
}

async fn prepare_overwrite_destination(app: &Router, token: &str) -> String {
    let destination = create_show(app, token, "Destination").await;
    let destination_id = destination["id"].as_str().unwrap().to_owned();
    assert_eq!(
        put_show_object(
            app,
            token,
            &destination_id,
            "user_layout",
            "operator",
            serde_json::json!({"marker":"destination old state"}),
        )
        .await
        .status(),
        StatusCode::OK
    );
    save_show_revision(app, token, &destination_id, "Destination baseline").await;
    destination_id
}

async fn get_show_json(app: &Router, token: &str, path: String) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::get(path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    json(response).await
}

async fn overwrite_and_verify_destination(
    app: &Router,
    token: &str,
    copy_id: &str,
    destination_id: &str,
) {
    let overwritten = app
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/v1/shows/{copy_id}/overwrite/{destination_id}"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(overwritten.status(), StatusCode::OK);
    let overwritten = json(overwritten).await;
    assert_eq!(overwritten["id"], destination_id);
    assert_eq!(overwritten["name"], "Destination");
    assert!(overwritten.get("revision_copy").is_none());
    let objects = get_show_json(
        app,
        token,
        format!("/api/v1/shows/{destination_id}/objects/user_layout"),
    )
    .await;
    assert_eq!(objects[0]["body"]["marker"], "copy edit");
    let revisions = get_show_json(
        app,
        token,
        format!("/api/v1/shows/{destination_id}/revisions"),
    )
    .await;
    assert_eq!(revisions.as_array().unwrap().len(), 1);
    assert_eq!(revisions[0]["name"], "Destination baseline");
    let copy_revisions = get_show_json(
        app,
        token,
        format!("/api/v1/shows/{copy_id}/revisions"),
    )
    .await;
    assert_eq!(copy_revisions.as_array().unwrap().len(), 1);
    assert_eq!(copy_revisions[0]["name"], "Copy private checkpoint");
}

async fn verify_copy_survives_source_deletion(
    app: &Router,
    token: &str,
    source_id: &str,
    copy_id: &str,
) {
    let shows = get_show_json(app, token, "/api/v1/shows".into()).await;
    assert!(
        shows
            .as_array()
            .unwrap()
            .iter()
            .any(|show| show["id"] == copy_id)
    );
    let deleted = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/v1/shows/{source_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    let overwrite = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{copy_id}/overwrite/{source_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(overwrite.status(), StatusCode::NOT_FOUND);
    let copy = get_show_json(
        app,
        token,
        format!("/api/v1/shows/{copy_id}/objects/user_layout"),
    )
    .await;
    assert_eq!(copy[0]["body"]["marker"], "copy edit");
    let bootstrap = get_show_json(app, token, "/api/v1/bootstrap".into()).await;
    assert_eq!(bootstrap["active_show"]["id"], copy_id);
    assert_eq!(bootstrap["active_show"]["revision_copy"]["show_id"], source_id);
}

#[tokio::test]
async fn confirmed_overwrite_preserves_destination_identity_revisions_and_revision_copy() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let (source_id, copy_id, revision_path, revision_body) =
        prepare_overwrite_source(&state, &app, &token).await;
    let destination_id = prepare_overwrite_destination(&app, &token).await;
    overwrite_and_verify_destination(&app, &token, &copy_id, &destination_id).await;
    assert_eq!(overwrite_revision_body(&revision_path), revision_body);
    verify_copy_survives_source_deletion(&app, &token, &source_id, &copy_id).await;
    assert!(
        std::fs::read_dir(data_dir.join("backups"))
            .unwrap()
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().starts_with("Destination-"))
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
