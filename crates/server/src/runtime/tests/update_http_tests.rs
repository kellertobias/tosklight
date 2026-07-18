#[tokio::test]
async fn update_settings_endpoint_persists_and_reloads_per_desk() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let front = test_control_desk();
    let mut wing = test_control_desk();
    wing.id = Uuid::new_v4();
    wing.osc_alias = "wing".into();
    let writer = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "update-settings-writer".into(),
        connected: true,
        desk: front.clone(),
    };
    let reader = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "update-settings-reader".into(),
        connected: true,
        desk: front.clone(),
    };
    let other_desk = Session {
        id: SessionId::new(),
        user,
        token: "update-settings-other-desk".into(),
        connected: true,
        desk: wing.clone(),
    };
    for session in [&writer, &reader, &other_desk] {
        state.programmers.start(session.id, session.user.id);
        attach_session_command_context(&state, session);
        state.sessions.write().insert(session.id, session.clone());
    }
    let app = router(state.clone());
    let expected = update::UpdateSettings {
        cue_mode: update::CueUpdateMode::ExistingOnly,
        preset_mode: update::ExistingContentMode::AddNew,
        group_mode: update::ExistingContentMode::AddNew,
        other_target_modes: HashMap::from([("macro".into(), update::ExistingContentMode::AddNew)]),
        show_update_modal_on_touch: false,
    };

    let saved = app
        .clone()
        .oneshot(
            Request::put("/api/v1/update/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", writer.token))
                .body(Body::from(serde_json::to_vec(&expected).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(saved.status(), StatusCode::OK);
    assert_eq!(
        serde_json::from_value::<update::UpdateSettings>(json(saved).await).unwrap(),
        expected
    );

    let persisted = state
        .desk
        .lock()
        .setting("server_configuration")
        .unwrap()
        .unwrap();
    let reloaded_configuration: DeskConfiguration = serde_json::from_str(&persisted).unwrap();
    assert_eq!(
        reloaded_configuration
            .update_settings_by_desk
            .get(&front.id),
        Some(&expected)
    );
    assert!(
        !reloaded_configuration
            .update_settings_by_desk
            .contains_key(&wing.id)
    );

    // Rebuild the HTTP surface around configuration decoded from the persisted desk setting,
    // matching the configuration boundary used by a process restart.
    let mut reloaded_state = state.clone();
    reloaded_state.configuration = Arc::new(RwLock::new(reloaded_configuration));
    let reloaded_app = router(reloaded_state);
    let same_desk = reloaded_app
        .clone()
        .oneshot(
            Request::get("/api/v1/update/settings")
                .header(header::AUTHORIZATION, format!("Bearer {}", reader.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(same_desk.status(), StatusCode::OK);
    assert_eq!(
        serde_json::from_value::<update::UpdateSettings>(json(same_desk).await).unwrap(),
        expected
    );
    let isolated = reloaded_app
        .oneshot(
            Request::get("/api/v1/update/settings")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", other_desk.token),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(isolated.status(), StatusCode::OK);
    assert_eq!(
        serde_json::from_value::<update::UpdateSettings>(json(isolated).await).unwrap(),
        update::UpdateSettings::default()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn armed_hardware_playback_touch_requests_update_without_operating_playback() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "hardware-update-target".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.write().insert(session.id, session.clone());
    state
        .programmers
        .set_command_line(session.id, "UPDATE ".into());
    let mut snapshot = matter_test_snapshot();
    snapshot.playbacks[0].buttons[0] = light_playback::PlaybackButtonAction::Go;
    state.engine.replace_snapshot(snapshot).unwrap();
    let source: SocketAddr = "127.0.0.1:19021".parse().unwrap();
    state.osc_subscribers.lock().insert(
        "hardware-update".into(),
        OscSubscriber {
            desk_alias: session.desk.osc_alias.clone(),
            target: "127.0.0.1:19022".parse().unwrap(),
            command_source: source,
            session_id: session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );

    handle_playback_osc(
        &state,
        "/light/playback/4/7/button/1",
        &[OscArgument::Bool(true)],
        Some("127.0.0.1:19021"),
    );

    assert!(
        state
            .programmers
            .get(session.id)
            .unwrap()
            .command_line
            .is_empty()
    );
    let events = state.audit_events.lock();
    let requested = events
        .iter()
        .find(|event| event.kind == "update_target_requested")
        .unwrap();
    assert_eq!(requested.payload["desk_id"], session.desk.id.to_string());
    assert_eq!(requested.payload["target"]["family"]["type"], "cue");
    assert_eq!(requested.payload["target"]["playback_number"], 25);
    assert!(
        events
            .iter()
            .any(|event| { event.kind == "update_armed" && event.payload["armed"] == false })
    );
    assert!(!events.iter().any(|event| event.kind == "playback_changed"));
    let _ = std::fs::remove_dir_all(data_dir);
}
