use super::*;

#[tokio::test]
async fn v2_patch_snapshot_authenticates_and_returns_the_show_revision_etag() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let denied = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v2/shows/{}/patch", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let denied_body = json(denied).await;
    assert_eq!(denied_body["retryable"], false);
    assert!(denied_body["current_revision"].is_null());

    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "V2 Patch snapshot").await;
    let show_id = show["id"].as_str().unwrap();
    open_show_for_patch_test(&app, &token, show_id).await;

    let response = app
        .oneshot(
            Request::get(format!("/api/v2/shows/{show_id}/patch"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let snapshot = json(response).await;
    assert_eq!(snapshot["show_id"], show_id);
    assert_eq!(snapshot["show_revision"], 1);
    assert_eq!(snapshot["patch_revision"], 0);
    assert_eq!(snapshot["cursor"]["sequence"], 0);
    assert_eq!(snapshot["fixtures"], serde_json::json!([]));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_patch_mutation_returns_typed_revision_conflicts() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "V2 Patch conflict").await;
    let show_id = show["id"].as_str().unwrap();
    open_show_for_patch_test(&app, &token, show_id).await;

    let response = app
        .oneshot(
            Request::post(format!("/api/v2/shows/{show_id}/patch/fixtures"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, "2")
                .body(Body::from(valid_patch_request().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let error = json(response).await;
    assert_eq!(error["error"], "stale show revision");
    assert_eq!(error["current_revision"], 1);
    assert_eq!(error["retryable"], false);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_patch_requires_if_match_and_rejects_invalid_batches_without_side_effects() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "V2 Patch validation").await;
    let show_id = show["id"].as_str().unwrap();
    open_show_for_patch_test(&app, &token, show_id).await;

    let missing_precondition = post_patch(
        &app,
        &token,
        show_id,
        None,
        serde_json::json!({"request_id":"missing-if-match","remove_fixture_ids":[Uuid::new_v4()]}),
    )
    .await;
    assert_eq!(missing_precondition.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json(missing_precondition).await["error"],
        "If-Match revision is required"
    );

    let invalid = post_patch(
        &app,
        &token,
        show_id,
        Some(1),
        serde_json::json!({"request_id":"empty-batch"}),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json(invalid).await["retryable"], false);

    let unchanged = get_patch(&app, &token, show_id).await;
    assert_eq!(unchanged.headers()[header::ETAG], "\"1\"");
    let unchanged = json(unchanged).await;
    assert_eq!(unchanged["show_revision"], 1);
    assert_eq!(unchanged["patch_revision"], 0);
    assert_eq!(unchanged["cursor"]["sequence"], 0);

    let successful_request = valid_patch_request_for(profile_id, mode_id, "successful-route-test");
    let success = post_patch(&app, &token, show_id, Some(1), successful_request.clone()).await;
    assert_eq!(success.status(), StatusCode::OK);
    assert_eq!(success.headers()[header::ETAG], "\"2\"");
    let success = json(success).await;
    assert_eq!(success["changed"], true);
    assert_eq!(success["show_revision"], 2);
    assert_eq!(success["patch_revision"], 1);
    assert_eq!(success["event_sequence"], 1);
    assert_eq!(success["fixtures"].as_array().unwrap().len(), 1);

    let replay = post_patch(&app, &token, show_id, Some(1), successful_request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(replay.headers()[header::ETAG], "\"2\"");
    let replay = json(replay).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["event_sequence"], 1);
    assert_eq!(patch_backup_count(&data_dir), 1);

    let committed = json(get_patch(&app, &token, show_id).await).await;
    assert_eq!(committed["show_revision"], 2);
    assert_eq!(committed["patch_revision"], 1);
    assert_eq!(committed["cursor"]["sequence"], 1);
    assert_eq!(committed["fixtures"].as_array().unwrap().len(), 1);

    open_show_for_patch_test(&app, &token, show_id).await;
    assert_eq!(state.engine.snapshot().fixtures.len(), 1);
    let reopened = json(get_patch(&app, &token, show_id).await).await;
    assert_eq!(reopened["show_revision"], 2);
    assert_eq!(reopened["patch_revision"], 1);
    assert_eq!(reopened["fixtures"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn patch_backup_count(data_dir: &FsPath) -> usize {
    std::fs::read_dir(data_dir.join("backups"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains("-patch-"))
        .count()
}

async fn open_show_for_patch_test(app: &Router, token: &str, show_id: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/open"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

fn valid_patch_request() -> serde_json::Value {
    valid_patch_request_for(Uuid::new_v4(), Uuid::new_v4(), "stale-route-test")
}

fn valid_patch_request_for(profile_id: Uuid, mode_id: Uuid, request_id: &str) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "fixtures": [{
            "fixture_id": Uuid::new_v4(),
            "fixture_number": 1,
            "virtual_fixture_number": null,
            "name": "Route test",
            "profile_id": profile_id,
            "profile_revision": 1,
            "mode_id": mode_id,
            "split_patches": [{"split": 1, "universe": null, "address": null}],
            "layer_id": "default",
            "direct_control": null,
            "location": {"x": 0, "y": 0, "z": 0},
            "rotation": {"x": 0.0, "y": 0.0, "z": 0.0},
            "multipatch": [],
            "move_in_black_enabled": true,
            "move_in_black_delay_millis": 0,
            "highlight_overrides": []
        }]
    })
}

fn install_patch_route_profile(state: &AppState) -> (Uuid, Uuid) {
    let mut profile = light_fixture::FixtureProfile::blank();
    profile.manufacturer = "Route Test".into();
    profile.name = "Patch Fixture".into();
    profile.short_name = "Patch".into();
    let profile = state
        .fixture_library
        .lock()
        .save_profile(profile, 0)
        .unwrap();
    (profile.id.0, profile.modes[0].id)
}

async fn get_patch(app: &Router, token: &str, show_id: &str) -> Response {
    app.clone()
        .oneshot(
            Request::get(format!("/api/v2/shows/{show_id}/patch"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn post_patch(
    app: &Router,
    token: &str,
    show_id: &str,
    revision: Option<u64>,
    body: serde_json::Value,
) -> Response {
    let mut request = Request::post(format!("/api/v2/shows/{show_id}/patch/fixtures"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(revision) = revision {
        request = request.header(header::IF_MATCH, revision.to_string());
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}
