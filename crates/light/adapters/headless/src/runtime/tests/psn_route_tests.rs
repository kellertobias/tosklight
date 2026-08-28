//! The PSN routes as a client meets them.
//!
//! What is being checked here is not arithmetic — that is tested against real packets beside the
//! receiver — but the contract an operator's tab depends on: a show that has never heard of
//! tracking reads as off, an edit carries only what changed, a refusal says what is wrong in the
//! words that were typed, and a resent edit does not bind a tracker twice.

use super::*;

async fn open_show_for(app: &Router, token: &str, show_id: &str) {
    let response = app
        .clone()
        .oneshot(open_show_request(token, show_id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

async fn read_psn(app: &Router, token: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v2/psn")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn update_psn(app: &Router, token: &str, body: &serde_json::Value) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/v2/psn/update")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn open_desk(app: &Router, name: &str) -> (String, String) {
    let (token, _) = login(app, name).await;
    let show = create_show(app, &token, "Tracking").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    open_show_for(app, &token, &show_id).await;
    (token, show_id)
}

#[tokio::test]
async fn a_show_that_has_never_heard_of_tracking_reads_as_off() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;

    let snapshot = read_psn(&app, &token).await;

    assert_eq!(snapshot["configuration"]["enabled"], false);
    assert_eq!(snapshot["configuration"]["group"], "236.10.10.10");
    assert_eq!(snapshot["configuration"]["port"], 56565);
    assert!(
        snapshot["configuration"]["bindings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(snapshot["revision"], 0);
    // Nothing is being listened to, so there is no health to report and nothing is wrong.
    assert!(snapshot["status"]["health"].is_null());
    assert!(snapshot["status"]["error"].is_null());
}

#[tokio::test]
async fn binding_a_tracker_to_a_point_is_stored_and_read_back() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;
    let binding = Uuid::new_v4();
    let point = Uuid::new_v4();

    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({
            "request_id": "bind-1",
            "enabled": true,
            "bindings": [{
                "id": binding,
                "tracker_id": 3,
                "point_fixture_id": point,
                "enabled": true,
            }],
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["unchanged"], false);

    let snapshot = read_psn(&app, &token).await;
    assert_eq!(snapshot["configuration"]["enabled"], true);
    assert_eq!(snapshot["configuration"]["bindings"][0]["tracker_id"], 3);
    assert_eq!(
        snapshot["configuration"]["bindings"][0]["point_fixture_id"],
        serde_json::json!(point)
    );
    // The running receiver was told, rather than finding out when the show is next read.
    assert!(state.psn.configuration().enabled);
    assert_eq!(state.psn.configuration().bindings[0].tracker_id, 3);
}

#[tokio::test]
async fn an_edit_carries_only_what_changed() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;
    let binding = Uuid::new_v4();
    update_psn(
        &app,
        &token,
        &serde_json::json!({
            "request_id": "bind-1",
            "enabled": true,
            "bindings": [{
                "id": binding,
                "tracker_id": 3,
                "point_fixture_id": Uuid::new_v4(),
                "enabled": true,
            }],
        }),
    )
    .await;

    // Turning the source off must not forget which tracker was which point.
    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "off-1", "enabled": false}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let snapshot = read_psn(&app, &token).await;
    assert_eq!(snapshot["configuration"]["enabled"], false);
    assert_eq!(snapshot["configuration"]["bindings"][0]["tracker_id"], 3);
}

#[tokio::test]
async fn an_address_that_is_not_a_multicast_group_is_refused_in_the_words_that_were_typed() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;

    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "bad-1", "group": "10.0.0.4"}),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error = json(response).await["error"].as_str().unwrap().to_owned();
    assert!(error.contains("10.0.0.4"), "{error}");
    assert!(error.contains("multicast"), "{error}");
    // Nothing was stored, so the desk keeps listening where it was.
    assert_eq!(
        read_psn(&app, &token).await["configuration"]["group"],
        "236.10.10.10"
    );
}

#[tokio::test]
async fn a_resent_edit_is_answered_from_the_replay_window_rather_than_applied_twice() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;
    let edit = serde_json::json!({
        "request_id": "enable-1",
        "enabled": true,
    });

    let first = json(update_psn(&app, &token, &edit).await).await;
    let second = json(update_psn(&app, &token, &edit).await).await;

    assert_eq!(first["replayed"], false);
    assert_eq!(second["replayed"], true);
    assert_eq!(first["revision"], second["revision"]);

    // The same identity carrying a different edit is a client bug, and is refused rather than
    // silently applied.
    let conflicting = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "enable-1", "enabled": false}),
    )
    .await;
    assert_eq!(conflicting.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn asking_for_what_is_already_stored_changes_no_revision() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;
    let first = json(
        update_psn(
            &app,
            &token,
            &serde_json::json!({"request_id": "enable-1", "enabled": true}),
        )
        .await,
    )
    .await;

    let again = json(
        update_psn(
            &app,
            &token,
            &serde_json::json!({"request_id": "enable-2", "enabled": true}),
        )
        .await,
    )
    .await;

    assert_eq!(again["unchanged"], true);
    assert_eq!(again["revision"], first["revision"]);
}

#[tokio::test]
async fn a_zone_with_its_corners_the_wrong_way_round_is_refused() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;

    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({
            "request_id": "zone-1",
            "zones": [{
                "id": Uuid::new_v4(),
                "name": "Downstage",
                "min_metres": [2.0, 0.0, 0.0],
                "max_metres": [-2.0, 3.0, 3.0],
                "tracker_ids": [],
                "dwell_millis": 250,
            }],
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        json(response).await["error"]
            .as_str()
            .unwrap()
            .contains("low corner above its high corner")
    );
}

#[tokio::test]
async fn an_unknown_field_is_accepted_rather_than_refused() {
    // api-rules §5: a client from a later version must not be turned away by the desk.
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;

    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({
            "request_id": "future-1",
            "enabled": true,
            "beam_tracking_mode": "wide",
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        read_psn(&app, &token).await["configuration"]["enabled"],
        true
    );
}
