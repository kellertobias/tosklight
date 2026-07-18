#[tokio::test]
async fn event_socket_disconnect_keeps_file_input_owned_until_session_close() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, session_id) = login(&app, "Operator").await;
    let session = state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .cloned()
        .unwrap();
    state
        .programmers
        .set_command_line(session.id, "COPY".into());
    state.ws_connections.lock().insert(session.id, 1);

    let claimed = app
        .clone()
        .oneshot(
            Request::post("/api/v1/files/input-context")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "instance_id":"acceptance-file-manager",
                        "action":"copy",
                        "origin":"pending"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(claimed.status(), StatusCode::OK);

    // ApiDriver commands use a short-lived event socket. Its asynchronous
    // close must not release a claim made immediately afterwards by the
    // still-authenticated Desk session.
    finish_event_socket(&state, &session);
    assert!(!state.ws_connections.lock().contains_key(&session.id));
    assert!(
        state
            .file_input_contexts
            .lock()
            .contains_key(&session.desk.id)
    );

    let competing = app
        .clone()
        .oneshot(
            Request::post("/api/v1/files/input-context")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "instance_id":"another-pane",
                        "action":"copy",
                        "origin":"toolbar"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(competing.status(), StatusCode::CONFLICT);

    let disconnected = app
        .oneshot(
            Request::delete(format!("/api/v1/sessions/{session_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disconnected.status(), StatusCode::NO_CONTENT);
    assert!(state.file_input_contexts.lock().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn losing_file_input_claim_does_not_consume_the_pending_command() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .cloned()
        .unwrap();
    state
        .programmers
        .set_command_line(session.id, "COPY".into());

    let winner = app
        .clone()
        .oneshot(
            Request::post("/api/v1/files/input-context")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "instance_id":"winning-toolbar",
                        "action":"copy",
                        "origin":"toolbar"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(winner.status(), StatusCode::OK);

    let loser = app
        .oneshot(
            Request::post("/api/v1/files/input-context")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "instance_id":"losing-pending-pane",
                        "action":"copy",
                        "origin":"pending"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(loser.status(), StatusCode::CONFLICT);
    assert_eq!(
        state.programmers.get(session.id).unwrap().command_line,
        "COPY"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn login_to_speed_group_desk(app: &Router, desk_id: Uuid) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"username":"Operator","desk_id":desk_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    json(response).await["token"].as_str().unwrap().to_owned()
}

async fn post_sound_observation(
    app: &Router,
    token: &str,
    observation: &serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/v1/speed-groups/A/observation")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(observation.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn sound_to_light_is_authoritative_per_speed_group_and_capture_is_desk_scoped() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let primary_desk = state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .unwrap()
        .desk
        .id;

    let enabled = SoundToLightConfig {
        enabled: true,
        smoothing: 0.0,
        ..SoundToLightConfig::default()
    };
    let updated = app
        .clone()
        .oneshot(
            Request::put("/api/v1/speed-groups/A")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&enabled).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);

    let observation = serde_json::json!({
        "captured_at_millis": 1,
        "source_available": true,
        "usable_signal": true,
        "level": 0.8,
        "selected_band_level": 0.7,
        "detected_bpm": 120.0,
        "confidence": 0.95
    });
    let observed = post_sound_observation(&app, &token, &observation).await;
    assert_eq!(observed.status(), StatusCode::OK);
    let observed = json(observed).await;
    assert_eq!(observed["snapshot"]["source"], "sound");
    assert_eq!(observed["snapshot"]["effective_bpm"], 120.0);

    // Two browser sessions attached to one desk are alternate surfaces of that same desk and
    // may therefore feed the same analyzer lease.
    let same_desk_token = login_to_speed_group_desk(&app, primary_desk).await;
    let same_desk_observation =
        post_sound_observation(&app, &same_desk_token, &observation).await;
    assert_eq!(same_desk_observation.status(), StatusCode::OK);

    let other_desk = state.desk.lock().add_desk("Other", "other").unwrap();
    let other_token = login_to_speed_group_desk(&app, other_desk.id).await;
    let contested = post_sound_observation(&app, &other_token, &observation).await;
    assert_eq!(contested.status(), StatusCode::CONFLICT);

    // A direct/manual value from any attached surface takes ownership and remains the stable
    // fallback instead of silently retaining Sound mode.
    let mut direct = state.configuration.read().clone();
    direct.speed_groups_bpm[0] = 111.0;
    let direct_response = app
        .clone()
        .oneshot(
            Request::put("/api/v1/configuration")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&direct).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(direct_response.status(), StatusCode::OK);
    let current = app
        .oneshot(
            Request::get("/api/v1/speed-groups/A")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let current = json(current).await;
    assert_eq!(current["snapshot"]["source"], "manual");
    assert_eq!(current["snapshot"]["effective_bpm"], 111.0);
    assert_eq!(current["configuration"]["enabled"], false);
    assert!(state.sound_capture_owners.lock()[0].is_none());

    let persisted: DeskConfiguration = serde_json::from_str(
        &state
            .desk
            .lock()
            .setting("server_configuration")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(persisted.speed_groups_bpm[0], 111.0);
    assert!(!persisted.speed_group_sound_to_light[0].enabled);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn osc_speed_group_feedback_uses_effective_sound_rate_and_pause_state() {
    let mut controller = SpeedGroupController::new(
        96.0,
        SoundToLightConfig {
            enabled: true,
            smoothing: 0.0,
            multiplier: 2.0,
            ..SoundToLightConfig::default()
        },
    )
    .unwrap();
    controller.observe_sound(SoundObservation::tempo(1_000, 120.0, 0.95));

    let running = speed_group_osc_feedback(controller.snapshot(1_000));
    assert_eq!(running[0], OscArgument::Int(240));
    assert_eq!(running[4], OscArgument::String("on".into()));

    controller.set_paused(true);
    let paused = speed_group_osc_feedback(controller.snapshot(1_001));
    assert_eq!(paused[0], OscArgument::Int(240));
    assert_eq!(paused[4], OscArgument::String("off".into()));
}

#[test]
fn osc_speed_group_button_performs_the_authoritative_learn_action() {
    let (state, data_dir) = test_state();
    let enabled = SoundToLightConfig {
        enabled: true,
        ..SoundToLightConfig::default()
    };
    state.speed_groups.lock()[0]
        .set_sound_config(enabled.clone())
        .unwrap();
    state.configuration.write().speed_group_sound_to_light[0] = enabled;
    state.sound_capture_owners.lock()[0] = Some(SoundCaptureOwner {
        desk_id: Uuid::new_v4(),
        last_seen_millis: 1,
    });

    handle_timing_osc(
        &state,
        "/light/main/speed-group/1/button",
        &[OscArgument::Bool(true)],
    );

    assert!(!state.speed_groups.lock()[0].sound_config().enabled);
    assert!(!state.configuration.read().speed_group_sound_to_light[0].enabled);
    assert!(state.sound_capture_owners.lock()[0].is_none());
    let event = state.audit_events.lock().back().cloned().unwrap();
    assert_eq!(event.kind, "speed_group_action");
    assert_eq!(event.payload["source"], "osc");
    assert_eq!(event.payload["action"], "learn");
    let _ = std::fs::remove_dir_all(data_dir);
}
