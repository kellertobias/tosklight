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
fn locked_desk_can_preview_update_but_cannot_apply_it() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "locked-update-preview".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.write().insert(session.id, session.clone());
    let fixture = light_core::FixtureId::new();
    state.programmers.select(session.id, [fixture]);

    let show_path = data_dir.join("shows/locked-update-preview.show");
    let show_id = initialise_show(&show_path, "Locked Update preview").unwrap();
    *state.active_show.write() = Some(ShowEntry {
        id: show_id,
        name: "Locked Update preview".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    });
    ShowStore::open(&show_path)
        .unwrap()
        .put_object(
            "group",
            "982",
            &serde_json::to_value(light_programmer::GroupDefinition {
                id: "982".into(),
                name: "Locked preview".into(),
                ..Default::default()
            })
            .unwrap(),
            0,
        )
        .unwrap();
    write_desk_lock(
        &state,
        session.desk.id,
        &DeskLockConfiguration {
            locked: true,
            ..Default::default()
        },
    )
    .unwrap();
    let request = UpdateApiRequest {
        target: UpdateApiTarget {
            family: UpdateApiTargetFamily::Group,
            object_id: Some("982".into()),
            playback_number: None,
            cue_id: None,
            cue_number: None,
            validate_active_context: false,
        },
        mode: update::UpdateMode::ExistingContent(update::ExistingContentMode::AddNew),
        expected_revision: None,
        expected_programmer_revision: None,
        expected_show_revision: None,
    };

    let preview = preview_update_request(&state, &session, &request).unwrap();
    assert_eq!(preview.preview.changed_count(), 1);
    let error = perform_update(
        &state,
        &session,
        &UpdateApiRequest {
            expected_revision: Some(preview.revision),
            expected_programmer_revision: Some(preview.programmer_revision),
            ..request
        },
    )
    .unwrap_err();
    assert_eq!(error.status, StatusCode::CONFLICT);
    assert_eq!(error.message, "desk is locked");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn update_targets_endpoint_keeps_the_legacy_shape_over_one_application_query() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "typed-update-targets".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.write().insert(session.id, session.clone());
    let fixture = light_core::FixtureId::new();
    state.programmers.set(
        session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.8),
    );
    assert!(state.programmers.set_modes(
        session.id,
        None,
        None,
        None,
        Some(Some("preset:1.1".into())),
    ));

    let show_path = data_dir.join("shows/typed-update-targets.show");
    let show_id = initialise_show(&show_path, "Typed Update targets").unwrap();
    *state.active_show.write() = Some(ShowEntry {
        id: show_id,
        name: "Typed Update targets".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    });
    let revision = ShowStore::open(&show_path)
        .unwrap()
        .put_object(
            "preset",
            "1.1",
            &serde_json::to_value(light_programmer::Preset {
                name: "Intensity 1".into(),
                family: light_programmer::PresetFamily::Intensity,
                number: 1,
                values: HashMap::from([(
                    fixture,
                    HashMap::from([(
                        light_core::AttributeKey::intensity(),
                        light_core::AttributeValue::Normalized(0.2),
                    )]),
                )]),
                group_values: HashMap::new(),
            })
            .unwrap(),
            0,
        )
        .unwrap();

    let response = router(state)
        .oneshot(
            Request::get("/api/v1/update/targets?filter=show_all_active")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", session.token),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    let entries = body.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["target"]["family"]["type"], "preset");
    assert_eq!(entries[0]["target"]["object_id"], "1.1");
    assert_eq!(entries[0]["revision"], revision);
    assert_eq!(entries[0]["existing_preview"]["revision"], revision);
    assert_eq!(entries[0]["add_new_preview"]["revision"], revision);
    assert_eq!(
        entries[0]["existing_preview"]["programmer_revision"],
        entries[0]["add_new_preview"]["programmer_revision"]
    );
    assert!(entries[0]["existing_preview"]["show_revision"].is_number());
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
