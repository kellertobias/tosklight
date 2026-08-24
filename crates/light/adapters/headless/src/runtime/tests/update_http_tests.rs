#[tokio::test]
async fn update_settings_endpoint_persists_and_reloads_per_desk() {
    let (state, data_dir) = test_state();
    let mut front = test_control_desk();
    front.id = Uuid::new_v4();
    let mut wing = test_control_desk();
    wing.id = Uuid::new_v4();
    let writer = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "update-settings-writer".into(),
        connected: true,
        desk: front.clone(),
    };
    let reader = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "update-settings-reader".into(),
        connected: true,
        desk: front.clone(),
    };
    let other_desk = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "update-settings-other-desk".into(),
        connected: true,
        desk: wing.clone(),
    };
    for session in [&writer, &reader, &other_desk] {
        state.programming.start(session.id);
        attach_session_command_context(&state, session);
        state.sessions.insert_session(session.clone());
    }
    let app = router(state.clone());
    let expected = update::UpdateSettings {
        cue_mode: update::CueUpdateMode::ExistingOnly,
        preset_mode: update::ExistingContentMode::AddNew,
        group_mode: update::ExistingContentMode::AddNew,
        other_target_modes: HashMap::new(),
        show_update_modal_on_touch: false,
    };

    let saved = app
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/v2/desks/{}/programming-update/settings",
                front.id
            ))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", writer.token))
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"settings-persist",
                        "settings":{
                            "cue_mode":"existing_only",
                            "preset_mode":"add_new",
                            "group_mode":"add_new",
                            "show_update_modal_on_touch":false
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let saved_status = saved.status();
    let saved = json(saved).await;
    assert_eq!(saved_status, StatusCode::OK, "{saved}");
    assert_eq!(
        saved["settings"],
        serde_json::json!({
            "cue_mode":"existing_only",
            "preset_mode":"add_new",
            "group_mode":"add_new",
            "show_update_modal_on_touch":false
        })
    );

    let persisted = state
        .installation.setting("server_configuration")
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
    let reloaded_state = state.clone();
    reloaded_state
        .installation
        .replace_configuration(reloaded_configuration);
    let reloaded_app = router(reloaded_state);
    let same_desk = reloaded_app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/desks/{}/programming-update/settings",
                front.id
            ))
                .header(header::AUTHORIZATION, format!("Bearer {}", reader.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(same_desk.status(), StatusCode::OK);
    assert_eq!(
        json(same_desk).await["settings"],
        serde_json::json!({
            "cue_mode":"existing_only",
            "preset_mode":"add_new",
            "group_mode":"add_new",
            "show_update_modal_on_touch":false
        })
    );
    let isolated = reloaded_app
        .oneshot(
            Request::get(format!(
                "/api/v2/desks/{}/programming-update/settings",
                wing.id
            ))
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
        json(isolated).await["settings"],
        serde_json::json!({
            "cue_mode":"existing_in_current_cue",
            "preset_mode":"update_existing",
            "group_mode":"update_existing",
            "show_update_modal_on_touch":true
        })
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn locked_desk_can_preview_update_but_cannot_apply_it() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "locked-update-preview".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());
    let fixture = light_core::FixtureId::new();
    state.programming.select(session.id, [fixture]);

    let show_path = data_dir.join("shows/locked-update-preview.show");
    let show_id = initialise_show(&show_path, "Locked Update preview").unwrap();
    state.active_show.replace_current(Some(ShowEntry {
        id: show_id,
        name: "Locked Update preview".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    }));
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
    write_desk_lock(&state, &DeskLockConfiguration {
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

#[test]
fn armed_hardware_playback_touch_requests_update_without_operating_playback() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "hardware-update-target".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());
    state
        .programming
        .set_command_line(session.id, "UPDATE ".into());
    let mut snapshot = matter_test_snapshot();
    std::sync::Arc::make_mut(&mut snapshot.playbacks)[0].buttons[0] =
        light_playback::PlaybackButtonAction::Go;
    state.output.replace_snapshot(snapshot).unwrap();
    let source: SocketAddr = "127.0.0.1:19021".parse().unwrap();
    state.integrations.register_osc_subscriber(
        "hardware-update".into(),
        OscSubscriber {
            capability: light_core::SurfaceCapability::Programming,
            path: "desk".to_owned(),
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
            .programming
            .get(session.id)
            .unwrap()
            .command_line
            .is_empty()
    );
    let events = state.events.audit_events();
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
