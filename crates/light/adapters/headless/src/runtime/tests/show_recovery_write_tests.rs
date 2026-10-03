//! Show recovery changes nothing in the show that failed to load, and serves none of it; the desk,
//! its settings, the show library and the recovery actions keep working.
use super::*;

const REFUSED: &str =
    "The active show could not be loaded; load the built-in default or a new empty show first";

async fn send(
    app: &Router,
    token: &str,
    show_id: &str,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("x-tosk-show", show_id);
    if body.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let body = body.map_or_else(Body::empty, |body| Body::from(body.to_string()));
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

/// Representative show-content writes: direct-store intents (Schedules, Cue previews) and
/// unit-of-work writes (zones, Stage layout, Timecodes). The zones write expects the seeded
/// revision 1.
fn writes(round: &str) -> Vec<(&'static str, serde_json::Value)> {
    vec![
        (
            "/api/v2/virtual-playback-exclusion-zones/update",
            serde_json::json!({"request_id": format!("zones-{round}"), "expected_revision": 1,
                "zones": [{"id": "right", "name": "Right", "playback_numbers": [1003, 1004]}]}),
        ),
        (
            "/api/v2/schedules/create",
            serde_json::json!({"request_id": format!("schedule-{round}"), "definition": {
                "name": "Nightly", "enabled": false,
                "trigger": {"type": "interval", "every_seconds": 300,
                    "enabled_at": "2000-01-01T00:00:00Z"},
                "target": {"type": "playback", "page": 1, "slot": 1, "playback_number": 1,
                    "action": "on", "master_transition": null}}}),
        ),
        (
            "/api/v2/cues/thumbnails/update",
            serde_json::json!({"request_id": format!("thumbs-{round}"), "thumbnails": [{
                "cue_id": Uuid::new_v4(), "state_hash": "h", "image_base64": "AAAA",
                "width": 1, "height": 1}]}),
        ),
        (
            "/api/v2/stage-layout/actions",
            serde_json::json!({"request_id": format!("stage-{round}"), "action": {
                "type": "regenerate_2d", "projection": "top_to_bottom"}}),
        ),
        (
            "/api/v2/timecodes/actions",
            serde_json::json!({"request_id": format!("timecode-{round}"), "action": {
                "type": "create", "definition": {"id": Uuid::new_v4(), "number": 70,
                    "name": "Song", "duration_frame": 1000, "transport_offset_frame": 0,
                    "auto_start": false, "markers": [], "lanes": []}}}),
        ),
    ]
}

#[tokio::test]
async fn show_recovery_refuses_show_content_writes_and_keeps_the_desk_working() {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, data_dir) = test_state_with_clock(clock);
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Recovery writes").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, &show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let schedule_id = Uuid::new_v4();
    for (path, body) in [
        (
            format!("/api/v2/test/shows/{show_id}/objects/schedule/{schedule_id}"),
            serde_json::json!({"expected_revision": 0, "action": {"type": "put", "body": {
                "id": schedule_id, "name": "Stored", "enabled": false,
                "trigger": {"type": "interval", "every_seconds": 300,
                    "enabled_at": "2000-01-01T00:00:00Z"},
                "target": {"type": "playback", "page": 1, "slot": 1, "playback_number": 1,
                    "action": "on", "master_transition": null}}}}),
        ),
        (
            "/api/v2/virtual-playback-exclusion-zones/update".to_owned(),
            serde_json::json!({"request_id": "seed-zones", "expected_revision": 0, "zones": [
                {"id": "left", "name": "Left", "playback_numbers": [1001, 1002]}]}),
        ),
    ] {
        let (status, body) = send(&app, &token, &show_id, "POST", &path, Some(body)).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
    }
    let (_, schedules) = send(&app, &token, &show_id, "GET", "/api/v2/schedules", None).await;
    assert_eq!(schedules["schedules"].as_array().unwrap().len(), 1);
    let stored_revision = schedules["show_revision"].as_u64().unwrap();
    assert!(stored_revision > 0);

    state
        .active_show
        .set_error(Some("The active show could not be loaded".into()));

    // Reads answer as for an empty show.
    let (status, schedules) = send(&app, &token, &show_id, "GET", "/api/v2/schedules", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(schedules["schedules"], serde_json::json!([]));
    assert_eq!(schedules["show_revision"], 0);
    let (status, thumbnails) = send(
        &app,
        &token,
        &show_id,
        "GET",
        "/api/v2/cues/thumbnails",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(thumbnails["entries"], serde_json::json!([]));
    let (status, psn) = send(&app, &token, &show_id, "GET", "/api/v2/psn", None).await;
    assert_eq!(status, StatusCode::OK, "{psn}");
    assert_eq!(psn["revision"], 0);
    assert_eq!(psn["macros"], serde_json::json!([]));
    let (status, attributes) = send(
        &app,
        &token,
        &show_id,
        "GET",
        "/api/v2/attribute-configuration",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{attributes}");
    assert_eq!(attributes["show_revision"], 0);

    // Every show-content write is refused with the actionable message.
    for (path, body) in writes("recovery") {
        let (status, refused) = send(&app, &token, &show_id, "POST", path, Some(body)).await;
        assert_eq!(status, StatusCode::CONFLICT, "{path}: {refused}");
        let message = refused["error"]
            .as_str()
            .or_else(|| refused["message"].as_str())
            .unwrap_or_default();
        assert_eq!(message, REFUSED, "{path}: {refused}");
    }
    // The application-service boundary refuses on its own, whatever transport reached it.
    {
        use light_application::ActiveShowUnitOfWork;
        let mut unit = ServerActiveShowUnitOfWork::begin(
            &state,
            light_core::ShowId(Uuid::parse_str(&show_id).unwrap()),
            ActiveShowBackupKind::ShowObjects,
        )
        .unwrap();
        let mut transaction = light_show::PortableShowTransaction::new(unit.document().revision());
        transaction.put("group", "9", serde_json::json!({"id": "9", "name": "Late"}));
        let refused = unit.commit(transaction).unwrap_err();
        assert_eq!(refused.kind, light_application::ActionErrorKind::Conflict);
        assert_eq!(refused.message, REFUSED);
        let refused = unit
            .backup(&light_application::BackupIdentity {
                show_id: unit.document().id(),
                correlation_id: Uuid::new_v4(),
                request_id: "recovery-backup".into(),
            })
            .unwrap_err();
        assert_eq!(refused.message, REFUSED);
    }

    // Desk-level settings, sessions and the show library keep working.
    let (status, body) = send(
        &app,
        &token,
        &show_id,
        "POST",
        "/api/v2/configuration/update",
        Some(serde_json::json!({"request_id": "recovery-autosave",
            "patch": {"autosave_interval_seconds": 120}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        state.installation.configuration().autosave_interval_seconds,
        120
    );
    let (second_token, _) = login(&app, "Second").await;
    let library = app
        .clone()
        .oneshot(show_snapshot_request(&second_token))
        .await
        .unwrap();
    assert_eq!(library.status(), StatusCode::OK);
    // A library action that rewrites the failed show in place is refused; on another show it runs.
    let other = create_show(&app, &token, "Other library show").await;
    for (target, expected) in [
        (show_id.as_str(), StatusCode::CONFLICT),
        (other["id"].as_str().unwrap(), StatusCode::OK),
    ] {
        let described = app
            .clone()
            .oneshot(show_action_request(
                &token,
                serde_json::json!({"type": "set_description", "show_id": target,
                    "description": "Edited in recovery"}),
            ))
            .await
            .unwrap();
        assert_eq!(described.status(), expected, "{target}");
    }

    // The failed show is exactly as it was.
    state.active_show.set_error(None);
    let (_, schedules) = send(&app, &token, &show_id, "GET", "/api/v2/schedules", None).await;
    assert_eq!(schedules["show_revision"], stored_revision);
    assert_eq!(schedules["schedules"].as_array().unwrap().len(), 1);
    let (_, zones) = send(
        &app,
        &token,
        &show_id,
        "GET",
        "/api/v2/virtual-playback-exclusion-zones",
        None,
    )
    .await;
    assert_eq!(zones["revision"], 1);
    assert_eq!(zones["zones"][0]["id"], "left");
    let (_, timecodes) = send(&app, &token, &show_id, "GET", "/api/v2/timecodes", None).await;
    assert_eq!(timecodes["objects"], serde_json::json!([]));

    // A recovery action (a new empty show) leaves recovery, and its content is writable.
    state
        .active_show
        .set_error(Some("The active show could not be loaded".into()));
    let fresh = create_show(&app, &token, "Fresh after recovery").await;
    let fresh_id = fresh["id"].as_str().unwrap().to_owned();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, &fresh_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    assert!(state.active_show.error().is_none());
    let (path, body) = writes("fresh").swap_remove(1);
    let (status, body) = send(&app, &token, &fresh_id, "POST", path, Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Negative control: the same writes reach a running show (none answers the recovery refusal).
#[tokio::test]
async fn a_running_show_accepts_the_writes_show_recovery_refuses() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Running writes").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, &show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let seed = serde_json::json!({"request_id": "seed-zones", "expected_revision": 0, "zones": [
        {"id": "left", "name": "Left", "playback_numbers": [1001, 1002]}]});
    let (status, _) = send(
        &app,
        &token,
        &show_id,
        "POST",
        "/api/v2/virtual-playback-exclusion-zones/update",
        Some(seed),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for (path, body) in writes("running") {
        let (status, answered) = send(&app, &token, &show_id, "POST", path, Some(body)).await;
        assert_eq!(status, StatusCode::OK, "{path}: {answered}");
    }
    let _ = std::fs::remove_dir_all(data_dir);
}
