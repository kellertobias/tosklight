use super::*;

#[tokio::test]
async fn direct_manual_entry_resets_pause_scale_sound_and_capture_ownership() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let sound = SoundToLightConfig {
        enabled: true,
        ..Default::default()
    };
    {
        let mut controllers = state.speed_groups.lock();
        controllers[0].set_sound_config(sound).unwrap();
        controllers[0].set_speed_master_scale(0.5).unwrap();
        controllers[0].set_paused_at(true, 10);
    }
    state.sound_capture_owners.lock()[0] = Some(SoundCaptureOwner {
        desk_id: session.desk.id,
        last_seen_millis: 10,
    });
    let initial = speed_group_snapshot(&app, &token, session.desk.id).await;
    let cursor = state.application_events.latest_sequence();
    let attempts = persistence_attempts(&state);

    let response = post_speed_groups(
        &app,
        &token,
        session.desk.id,
        speed_request(
            "manual-takes-ownership",
            &initial,
            serde_json::json!({"type":"set_bpm","group":"A","bpm":120.0}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["status"], "changed");
    assert_eq!(response["groups"][0]["paused"], false);
    assert_eq!(response["groups"][0]["speed_master_scale"], 1.0);
    let controller = state.speed_groups.lock()[0].clone();
    assert!(!controller.sound_config().enabled);
    assert!(state.sound_capture_owners.lock()[0].is_none());
    assert!(!state.configuration.read().speed_group_sound_to_light[0].enabled);
    assert_eq!(persistence_attempts(&state), attempts + 1);
    assert_eq!(speed_group_events(&state, cursor).len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_speed_groups_are_revisioned_shared_strict_and_replay_before_desk_lock() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (front_token, _) = login(&app, "Operator").await;
    let front = authenticate_token(&state, &front_token).unwrap();

    let unauthorized = get_speed_groups(&app, None, front.desk.id).await;
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let initial = speed_group_snapshot(&app, &front_token, front.desk.id).await;
    assert_eq!(initial["projection"]["revision"], 0);
    assert_eq!(initial["projection"]["groups"].as_array().unwrap().len(), 5);
    assert_eq!(initial["projection"]["groups"][0]["group"], "A");
    let authority = initial["projection"]["authority_id"].clone();
    let cursor = state.application_events.latest_sequence();
    let attempts = persistence_attempts(&state);
    let request = speed_request(
        "speed-absolute",
        &initial,
        serde_json::json!({"type":"set_bpm","group":"A","bpm":128.5}),
    );

    let changed = post_speed_groups(&app, &front_token, front.desk.id, request.clone()).await;
    assert_eq!(changed.status(), StatusCode::OK);
    let changed = json(changed).await;
    assert_eq!(changed["status"], "changed");
    assert_eq!(changed["revision"], 1);
    assert_eq!(changed["groups"][0]["manual_bpm"], 128.5);
    assert_eq!(changed["groups"][0]["group"], "A");
    assert_eq!(changed["authority_id"], authority);
    assert_eq!(changed["durability"], "durable");
    assert_eq!(persistence_attempts(&state), attempts + 1);
    assert_eq!(speed_group_events(&state, cursor).len(), 1);
    assert_eq!(state.configuration.read().speed_groups_bpm[0], 128.5);

    write_desk_lock(
        &state,
        front.desk.id,
        &DeskLockConfiguration {
            locked: true,
            ..DeskLockConfiguration::default()
        },
    )
    .unwrap();
    let replay = post_speed_groups(&app, &front_token, front.desk.id, request).await;
    let replay_status = replay.status();
    let replay = json(replay).await;
    assert_eq!(replay_status, StatusCode::OK, "{replay}");
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["event_sequence"], changed["event_sequence"]);
    assert_eq!(persistence_attempts(&state), attempts + 1);
    assert_eq!(speed_group_events(&state, cursor).len(), 1);

    let current = speed_group_snapshot(&app, &front_token, front.desk.id).await;
    let locked = post_speed_groups(
        &app,
        &front_token,
        front.desk.id,
        speed_request(
            "speed-locked",
            &current,
            serde_json::json!({"type":"set_bpm","group":"B","bpm":130.0}),
        ),
    )
    .await;
    assert_eq!(locked.status(), StatusCode::CONFLICT);
    assert_eq!(json(locked).await["kind"], "conflict");
    write_desk_lock(&state, front.desk.id, &DeskLockConfiguration::default()).unwrap();

    let no_change = post_speed_groups(
        &app,
        &front_token,
        front.desk.id,
        speed_request(
            "speed-no-change",
            &current,
            serde_json::json!({"type":"set_bpm","group":"A","bpm":128.5}),
        ),
    )
    .await;
    assert_eq!(no_change.status(), StatusCode::OK);
    let no_change = json(no_change).await;
    assert_eq!(no_change["status"], "no_change");
    assert!(no_change.get("event_sequence").is_none());
    assert_eq!(persistence_attempts(&state), attempts + 1);

    let wing = state.desk.lock().add_desk("Wing", "wing").unwrap();
    let wing_token = login_for_speed_desk(&app, "Operator", wing.id).await;
    let wing_snapshot = speed_group_snapshot(&app, &wing_token, wing.id).await;
    assert_eq!(wing_snapshot["projection"]["revision"], 1);
    assert_eq!(
        wing_snapshot["projection"]["groups"][0]["manual_bpm"],
        128.5
    );
    let relative = post_speed_groups(
        &app,
        &wing_token,
        wing.id,
        speed_request(
            "wing-relative",
            &wing_snapshot,
            serde_json::json!({"type":"adjust_bpm","group":"A","delta_bpm":1.5}),
        ),
    )
    .await;
    assert_eq!(relative.status(), StatusCode::OK);
    assert_eq!(json(relative).await["groups"][0]["manual_bpm"], 130.0);
    let front_snapshot = speed_group_snapshot(&app, &front_token, front.desk.id).await;
    assert_eq!(front_snapshot["projection"]["revision"], 2);
    assert_eq!(
        front_snapshot["projection"]["groups"][0]["manual_bpm"],
        130.0
    );

    let foreign = get_speed_groups(&app, Some(&front_token), wing.id).await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    let foreign_action = post_speed_groups(
        &app,
        &front_token,
        wing.id,
        speed_request(
            "foreign-speed",
            &front_snapshot,
            serde_json::json!({"type":"set_bpm","group":"E","bpm":150.0}),
        ),
    )
    .await;
    assert_eq!(foreign_action.status(), StatusCode::FORBIDDEN);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn synchronization_conflicts_persistence_warning_and_authority_replacement_are_explicit() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let initial = speed_group_snapshot(&app, &token, session.desk.id).await;
    let first = speed_request(
        "set-source",
        &initial,
        serde_json::json!({"type":"set_bpm","group":"A","bpm":144.0}),
    );
    assert_eq!(
        post_speed_groups(&app, &token, session.desk.id, first.clone())
            .await
            .status(),
        StatusCode::OK
    );
    let current = speed_group_snapshot(&app, &token, session.desk.id).await;
    let cursor = state.application_events.latest_sequence();
    let attempts = persistence_attempts(&state);
    let sync = post_speed_groups(
        &app,
        &token,
        session.desk.id,
        speed_request(
            "sync-a-b",
            &current,
            serde_json::json!({"type":"synchronize","source":"A","target":"B"}),
        ),
    )
    .await;
    assert_eq!(sync.status(), StatusCode::OK);
    let sync = json(sync).await;
    assert_eq!(sync["status"], "changed");
    assert_eq!(sync["groups"].as_array().unwrap().len(), 2);
    assert_eq!(sync["groups"][0]["synchronized_with"], "B");
    assert_eq!(sync["groups"][1]["synchronized_with"], "A");
    assert_eq!(sync["groups"][1]["manual_bpm"], 144.0);
    assert_eq!(persistence_attempts(&state), attempts + 1);
    assert_eq!(speed_group_events(&state, cursor).len(), 1);

    let stale = post_speed_groups(
        &app,
        &token,
        session.desk.id,
        speed_request(
            "stale-speed",
            &initial,
            serde_json::json!({"type":"set_bpm","group":"C","bpm":120.0}),
        ),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(json(stale).await["current_revision"], 2);

    let collision = post_speed_groups(
        &app,
        &token,
        session.desk.id,
        serde_json::json!({
            "request_id":"set-source",
            "expected_authority_id":current["projection"]["authority_id"],
            "expected_revision":2,
            "action":{"type":"set_bpm","group":"A","bpm":145.0}
        }),
    )
    .await;
    assert_eq!(collision.status(), StatusCode::CONFLICT);

    let strict = post_speed_groups(
        &app,
        &token,
        session.desk.id,
        serde_json::json!({
            "request_id":"strict",
            "expected_authority_id":current["projection"]["authority_id"],
            "expected_revision":2,
            "action":{"type":"set_bpm","group":"C","bpm":120.0,"unknown":true}
        }),
    )
    .await;
    assert_eq!(strict.status(), StatusCode::BAD_REQUEST);

    state
        .speed_group_persistence_failure
        .store(true, Ordering::Relaxed);
    let current = speed_group_snapshot(&app, &token, session.desk.id).await;
    let attempts = persistence_attempts(&state);
    let pending_request = speed_request(
        "pending-speed",
        &current,
        serde_json::json!({"type":"set_bpm","group":"C","bpm":111.0}),
    );
    let pending = post_speed_groups(&app, &token, session.desk.id, pending_request.clone()).await;
    assert_eq!(pending.status(), StatusCode::OK);
    let pending = json(pending).await;
    assert_eq!(pending["durability"], "persistence_pending");
    assert!(pending["warning"].as_str().unwrap().contains("pending"));
    let replay = post_speed_groups(&app, &token, session.desk.id, pending_request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json(replay).await["replayed"], true);
    assert_eq!(persistence_attempts(&state), attempts + 1);

    let old_authority = current["projection"]["authority_id"].clone();
    let mut replaced_state = state.clone();
    replaced_state.speed_group_service =
        SpeedGroupService::new(replaced_state.application_events.clone());
    let replaced_app = router(replaced_state.clone());
    let replacement = speed_group_snapshot(&replaced_app, &token, session.desk.id).await;
    assert_eq!(replacement["projection"]["revision"], 0);
    assert_ne!(replacement["projection"]["authority_id"], old_authority);
    let old_request = post_speed_groups(
        &replaced_app,
        &token,
        session.desk.id,
        serde_json::json!({
            "request_id":"old-authority",
            "expected_authority_id":old_authority,
            "expected_revision":2,
            "action":{"type":"set_bpm","group":"D","bpm":122.0}
        }),
    )
    .await;
    assert_eq!(old_request.status(), StatusCode::CONFLICT);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn speed_request(
    request_id: &str,
    snapshot: &serde_json::Value,
    action: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "request_id":request_id,
        "expected_authority_id":snapshot["projection"]["authority_id"],
        "expected_revision":snapshot["projection"]["revision"],
        "action":action,
    })
}

async fn get_speed_groups(app: &Router, token: Option<&str>, desk_id: Uuid) -> Response {
    let mut request = Request::get(format!("/api/v2/desks/{desk_id}/speed-groups"));
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn speed_group_snapshot(app: &Router, token: &str, desk_id: Uuid) -> serde_json::Value {
    let response = get_speed_groups(app, Some(token), desk_id).await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn post_speed_groups(
    app: &Router,
    token: &str,
    desk_id: Uuid,
    payload: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::post(format!("/api/v2/desks/{desk_id}/speed-groups"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn login_for_speed_desk(app: &Router, username: &str, desk_id: Uuid) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"username":username,"desk_id":desk_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await["token"].as_str().unwrap().to_owned()
}

fn persistence_attempts(state: &AppState) -> u64 {
    state
        .speed_group_persistence_attempts
        .load(Ordering::Relaxed)
}

fn speed_group_events(state: &AppState, cursor: u64) -> Vec<Arc<light_application::EventEnvelope>> {
    let light_application::EventReplay::Events(events) = state.application_events.replay(
        cursor,
        &light_application::EventFilter::default()
            .with_capability(light_application::EventCapability::Playback)
            .with_object(light_application::EventObject::speed_groups()),
    ) else {
        panic!("Speed Group event history should be retained");
    };
    events
}
