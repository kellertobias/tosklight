use super::*;

fn description_request(
    token: &str,
    show_id: &str,
    request_id: &str,
    description: &str,
) -> Request<Body> {
    Request::post("/api/v2/shows").header(header::CONTENT_TYPE,"application/json")
        .header(header::AUTHORIZATION,format!("Bearer {token}"))
        .body(Body::from(serde_json::json!({"request_id":request_id,"action":{"type":"set_description","show_id":show_id,"description":description}}).to_string())).unwrap()
}

#[tokio::test]
async fn show_description_is_portable_replay_safe_and_legacy_optional() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Described base").await;
    let show_id = show["id"].as_str().unwrap();
    let id = light_core::ShowId(Uuid::parse_str(show_id).unwrap());
    assert_eq!(
        app.clone()
            .oneshot(open_show_request(&token, show_id))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let entry = state.installation.show(id).unwrap().unwrap();
    assert_eq!(show_description::read_description(&entry.path).unwrap(), "");
    let before = ActiveShowRepository::open(&entry.path)
        .unwrap()
        .portable_revision()
        .unwrap();
    let response = app
        .clone()
        .oneshot(description_request(
            &token,
            show_id,
            "describe-show",
            " Main hall, 48 fixtures ",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        show_description::read_description(&entry.path).unwrap(),
        "Main hall, 48 fixtures"
    );
    let after = ActiveShowRepository::open(&entry.path)
        .unwrap()
        .portable_revision()
        .unwrap();
    assert!(after > before);
    let replay = app
        .clone()
        .oneshot(description_request(
            &token,
            show_id,
            "describe-show",
            " Main hall, 48 fixtures ",
        ))
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json(replay).await["replayed"], true);
    assert_eq!(state.active_show.current().unwrap().id, id);
    assert_eq!(
        ActiveShowRepository::open(&entry.path)
            .unwrap()
            .portable_revision()
            .unwrap(),
        after
    );
    let backup = data_dir.join("portable-description.show");
    ActiveShowRepository::open(&entry.path)
        .unwrap()
        .backup_to(&backup)
        .unwrap();
    assert_eq!(
        show_description::read_description(backup.to_str().unwrap()).unwrap(),
        "Main hall, 48 fixtures"
    );
    state.installation.set_show_base(id, true).unwrap();
    let copied=app.clone().oneshot(show_action_request(&token,serde_json::json!({"type":"create_from_base","show_id":show_id,"name":"Description base copy"}))).await.unwrap();
    assert_eq!(copied.status(), StatusCode::OK);
    let copied = show_action_result(json(copied).await, "show");
    assert_eq!(
        show_description::read_description(copied["path"].as_str().unwrap()).unwrap(),
        "Main hall, 48 fixtures"
    );
    let saved = app
        .clone()
        .oneshot(save_show_revision_request(
            &token,
            show_id,
            "Described revision",
        ))
        .await
        .unwrap();
    assert_eq!(saved.status(), StatusCode::OK);
    let named = show_revision_source::prepare_named_revision_source(&state, id.0, 1).unwrap();
    assert_eq!(
        show_description::read_description(&named.path).unwrap(),
        "Main hall, 48 fixtures"
    );
    assert_eq!(state.active_show.current().unwrap().id, id);
    let snapshot = app
        .clone()
        .oneshot(show_snapshot_request(&token))
        .await
        .unwrap();
    let snapshot = json(snapshot).await;
    assert_eq!(
        snapshot["shows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|show| show["id"] == show_id)
            .unwrap()["description"],
        "Main hall, 48 fixtures"
    );
    let invalid = app
        .clone()
        .oneshot(description_request(
            &token,
            show_id,
            "too-long-description",
            &"x".repeat(2001),
        ))
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let corrupt = data_dir.join("corrupt-library-entry.show");
    std::fs::write(&corrupt, b"broken sqlite").unwrap();
    state
        .installation
        .upsert_show("Recovery entry", corrupt.to_str().unwrap(), false)
        .unwrap();
    let snapshot = app.oneshot(show_snapshot_request(&token)).await.unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    let _ = std::fs::remove_dir_all(data_dir);
}
