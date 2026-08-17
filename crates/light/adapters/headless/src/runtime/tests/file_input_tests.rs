#[tokio::test]
async fn file_input_stays_owned_until_session_close() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, session_id) = login(&app, "Operator").await;
    let session = state
        .sessions.sessions().into_iter()
        .find(|session| session.token == token)
        .unwrap();
    state
        .programming
        .set_command_line(session.id, "COPY".into());

    let claimed = app
        .clone()
        .oneshot(
            Request::post("/api/v2/files/input-context/claim")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"claim-owner",
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

    assert!(
        state
            .sessions
            .file_input_context(session.desk.id)
            .is_some()
    );

    let competing = app
        .clone()
        .oneshot(
            Request::post("/api/v2/files/input-context/claim")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"claim-competitor",
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
            Request::delete(format!("/api/v2/sessions/{session_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disconnected.status(), StatusCode::NO_CONTENT);
    assert_eq!(state.sessions.file_input_context_count(), 0);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn losing_file_input_claim_does_not_consume_the_pending_command() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = state
        .sessions.sessions().into_iter()
        .find(|session| session.token == token)
        .unwrap();
    state
        .programming
        .set_command_line(session.id, "COPY".into());

    let winner = app
        .clone()
        .oneshot(
            Request::post("/api/v2/files/input-context/claim")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"claim-winner",
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
            Request::post("/api/v2/files/input-context/claim")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"claim-loser",
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
        state.programming.get(session.id).unwrap().command_line,
        "COPY"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn login_to_speed_group_desk(app: &Router, desk_id: Uuid) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/sessions")
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
            Request::post("/api/v2/speed-groups/A/observations")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(observation.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn update_speed_group_source(
    app: &Router,
    token: &str,
    group: &str,
    request_id: &str,
    source: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::post(format!(
                "/api/v2/speed-groups/{group}/settings/update"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "request_id": request_id,
                    "source": source,
                    "configuration": SoundToLightConfig::default(),
                })
                .to_string(),
            ))
            .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn speed_group_sources_follow_directed_chains_and_reject_cycles() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;

    let linked = update_speed_group_source(
        &app,
        &token,
        "A",
        "link-a-to-b",
        serde_json::json!({"type":"speed_group","group":"B"}),
    )
    .await;
    assert_eq!(linked.status(), StatusCode::OK);
    let linked = json(linked).await;
    assert_eq!(linked["source"], serde_json::json!({"type":"speed_group","group":"B"}));
    assert_eq!(linked["snapshot"]["effective_bpm"], 90.0);
    assert_eq!(linked["replayed"], false);

    let replayed = update_speed_group_source(
        &app,
        &token,
        "A",
        "link-a-to-b",
        serde_json::json!({"type":"speed_group","group":"B"}),
    )
    .await;
    assert_eq!(replayed.status(), StatusCode::OK);
    assert_eq!(json(replayed).await["replayed"], true);

    assert_eq!(
        update_speed_group_source(
            &app,
            &token,
            "B",
            "link-b-to-c",
            serde_json::json!({"type":"speed_group","group":"C"}),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        update_speed_group_source(
            &app,
            &token,
            "C",
            "link-c-to-a",
            serde_json::json!({"type":"speed_group","group":"A"}),
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );

    let conflicting_replay = update_speed_group_source(
        &app,
        &token,
        "A",
        "link-a-to-b",
        serde_json::json!({"type":"manual"}),
    )
    .await;
    assert_eq!(conflicting_replay.status(), StatusCode::CONFLICT);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn sound_to_light_is_authoritative_per_speed_group_and_capture_is_desk_scoped() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let primary_desk = state
        .sessions.sessions().into_iter()
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
            Request::post("/api/v2/speed-groups/A/settings/update")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"sound-settings-1",
                        "source":{"type":"sound_to_light"},
                        "configuration":enabled
                    })
                    .to_string(),
                ))
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

    let other_desk = state.installation.add_desk("Other", "other").unwrap();
    let other_token = login_to_speed_group_desk(&app, other_desk.id).await;
    let contested = post_sound_observation(&app, &other_token, &observation).await;
    assert_eq!(contested.status(), StatusCode::CONFLICT);

    // A direct/manual value from any attached surface takes ownership and remains the stable
    // fallback instead of silently retaining Sound mode.
    let direct_response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/speed-groups/A/actions")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"action":"set_bpm","bpm":111}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(direct_response.status(), StatusCode::OK);
    let current = app
        .oneshot(
            Request::get("/api/v2/speed-groups/A")
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
    assert!(state.output.sound_capture_owner(0).is_none());

    let persisted: DeskConfiguration = serde_json::from_str(
        &state
            .installation.setting("server_configuration")
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
    state
        .output
        .set_speed_group_sound_config(0, enabled.clone())
        .unwrap();
    state.installation.update_configuration(|configuration| {
        configuration.speed_group_sound_to_light[0] = enabled;
    });
    state.output.set_sound_capture_owner(
        0,
        Some(SoundCaptureOwner {
            desk_id: Uuid::new_v4(),
            last_seen_millis: 1,
        }),
    );

    handle_timing_osc(
        &state,
        "/light/main/speed-group/1/button",
        &[OscArgument::Bool(true)],
    );

    assert!(!state.output.speed_group_sound_config(0).enabled);
    assert!(!state.installation.configuration().speed_group_sound_to_light[0].enabled);
    assert!(state.output.sound_capture_owner(0).is_none());
    let event = state.events.audit_events().last().cloned().unwrap();
    assert_eq!(event.kind, "speed_group_action");
    assert_eq!(event.payload["source"], "osc");
    assert_eq!(event.payload["action"], "learn");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn osc_release_fade_updates_and_persists_the_desk_timing() {
    let (state, data_dir) = test_state();

    handle_timing_osc(
        &state,
        "/light/main/programmer/release-fade",
        &[OscArgument::Float(0.25)],
    );

    assert_eq!(state.installation.configuration().release_fade_millis, 15_000);
    let persisted = state
        .installation
        .setting("server_configuration")
        .unwrap()
        .unwrap();
    let persisted: serde_json::Value = serde_json::from_str(&persisted).unwrap();
    assert_eq!(persisted["release_fade_millis"], 15_000);
    let _ = std::fs::remove_dir_all(data_dir);
}
