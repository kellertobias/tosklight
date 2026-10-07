//! Show recovery serves none of the failed show's objects: the engine does not hold them.
use super::*;

async fn read(app: &Router, token: &str, show_id: &str, path: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::get(path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{path}");
    json(response).await
}

async fn post(app: &Router, token: &str, path: &str, body: serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{path}");
}

#[tokio::test]
async fn show_recovery_serves_no_objects_of_the_show_that_failed_to_load() {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, data_dir) = test_state_with_clock(clock);
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Recovery objects").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, &show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    post(
        &app,
        &token,
        &format!("/api/v2/test/shows/{show_id}/objects/group/1"),
        serde_json::json!({"expected_revision": 0, "action": {"type": "put", "body": {
            "id": "1", "name": "Stored Group", "fixtures": []
        }}}),
    )
    .await;
    let zones = serde_json::json!({"request_id": "zones", "expected_revision": 0, "zones": [
        {"id": "left", "name": "Left", "playback_numbers": [1001, 1002]}
    ]});
    post(
        &app,
        &token,
        "/api/v2/virtual-playback-exclusion-zones/update",
        zones,
    )
    .await;

    // Negative control: the running show serves what it stores.
    let groups = read(&app, &token, &show_id, "/api/v2/objects/group").await;
    assert_eq!(groups["objects"].as_array().unwrap().len(), 1);
    let group = read(&app, &token, &show_id, "/api/v2/objects/group/1").await;
    assert_eq!(group["object"]["body"]["name"], "Stored Group");
    let zones = read(
        &app,
        &token,
        &show_id,
        "/api/v2/virtual-playback-exclusion-zones",
    )
    .await;
    assert_eq!(zones["zones"].as_array().unwrap().len(), 1);
    let stored_revision = groups["show_revision"].as_u64().unwrap();
    assert!(stored_revision > 0);

    state
        .active_show
        .set_error(Some("The active show could not be loaded".into()));

    // The failed show stays the named active entry for the recovery dialog...
    let bootstrap = read(&app, &token, &show_id, "/api/v2/bootstrap").await;
    assert_eq!(bootstrap["active_show"]["id"], show_id);
    assert_eq!(
        bootstrap["active_show_error"],
        "The active show could not be loaded"
    );
    // ...but none of its objects run, so none are served.
    for kind in [
        "group",
        "playback",
        "playback_page",
        "preset",
        "cue_list",
        "dynamic",
    ] {
        let collection = read(&app, &token, &show_id, &format!("/api/v2/objects/{kind}")).await;
        assert_eq!(collection["show_id"], show_id, "{kind}");
        assert_eq!(collection["kind"], kind);
        assert_eq!(collection["show_revision"], 0, "{kind}");
        assert_eq!(collection["objects"], serde_json::json!([]), "{kind}");
    }
    let group = read(&app, &token, &show_id, "/api/v2/objects/group/1").await;
    assert!(group["object"].is_null());
    assert_eq!(group["object_id"], "1");
    let patch = read(&app, &token, &show_id, "/api/v2/patch").await;
    assert_eq!(patch["fixtures"], serde_json::json!([]));
    assert_eq!(patch["patch_revision"], 0);
    let zones = read(
        &app,
        &token,
        &show_id,
        "/api/v2/virtual-playback-exclusion-zones",
    )
    .await;
    assert_eq!(zones["zones"], serde_json::json!([]));
    let timecodes = read(&app, &token, &show_id, "/api/v2/timecodes").await;
    assert_eq!(timecodes["objects"], serde_json::json!([]));

    // The stored show itself is untouched: leaving recovery serves it again.
    state.active_show.set_error(None);
    let groups = read(&app, &token, &show_id, "/api/v2/objects/group").await;
    assert_eq!(groups["objects"].as_array().unwrap().len(), 1);
    assert_eq!(groups["show_revision"], stored_revision);
    let _ = std::fs::remove_dir_all(data_dir);
}
