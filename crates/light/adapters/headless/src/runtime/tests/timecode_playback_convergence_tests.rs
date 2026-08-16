use super::*;

#[tokio::test]
async fn physical_http_and_virtual_ui_ws_playbacks_control_one_timecode_runtime() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap();
    let show = create_show(&app, &token, "Timecode Playback convergence").await;
    let opened = app
        .clone()
        .oneshot(open_show_request(
            &token,
            show["id"].as_str().expect("created show id"),
        ))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let show_id = state.active_show.current().as_ref().unwrap().id;
    let timecode_id = light_playback::TimecodeId(Uuid::from_u128(70));
    let marker_id = light_playback::TimecodeMarkerId(Uuid::from_u128(700));
    let created = app
        .clone()
        .oneshot(
            Request::post("/api/v2/timecodes/actions")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-tosk-show", show_id.0.to_string())
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "create-timecode-playback-target",
                        "action": {"type": "create", "definition": {
                            "id": timecode_id.0,
                            "number": 70,
                            "name": "House timeline",
                            "duration_frame": 10000,
                            "transport_offset_frame": 0,
                            "auto_start": false,
                            "markers": [{"id": marker_id.0, "frame": 250, "name": "Cue start"}],
                            "lanes": []
                        }}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let created_status = created.status();
    assert_eq!(created_status, StatusCode::OK, "{}", json(created).await);

    for (request_id, command) in [
        ("command-timecode-arm", "TIMECODE 70 +"),
        ("command-timecode-disarm", "TIMECODE 70 -"),
        ("command-timecode-edit", "SET TIMECODE 70"),
        ("command-timecode-run", "TIMECODE 70"),
    ] {
        let response = app
            .clone()
            .oneshot(command_line_object_request(
                &token,
                session.desk.id,
                request_id,
                command,
            ))
            .await
            .unwrap();
        let status = response.status();
        let body = json(response).await;
        assert_eq!(status, StatusCode::OK, "{command}: {body}");
    }
    let stored = ActiveShowRepository::open(&state.active_show.current().unwrap().path)
        .unwrap()
        .object_with_portable_revision("timecode", &timecode_id.0.to_string())
        .unwrap()
        .1
        .unwrap();
    let stored: light_playback::TimecodeDefinition = serde_json::from_value(stored.body).unwrap();
    assert!(
        !stored.auto_start,
        "the disarm command persists autoplay off"
    );
    assert!(state.events.audit_events().iter().any(|event| {
        event.kind == "desk_action"
            && event.payload["action"] == "open-object-editor"
            && event.payload["value"] == timecode_id.0.to_string()
    }));
    assert_eq!(
        state.timecodes.snapshot(timecode_id).unwrap().transport,
        light_playback::TimecodeTransportState::Playing
    );
    let missing = app
        .clone()
        .oneshot(command_line_object_request(
            &token,
            session.desk.id,
            "command-timecode-missing",
            "TIMECODE 999",
        ))
        .await
        .unwrap();
    assert_eq!(
        missing.status(),
        StatusCode::OK,
        "command execution reports semantic rejection in its typed response"
    );
    let history = state.programming.command_history(session.desk.id);
    assert!(history.iter().any(|entry| {
        entry.command == "TIMECODE 70 +"
            && entry.status == "accepted"
            && entry.feedback.contains("Armed")
    }));
    assert!(history.iter().any(|entry| {
        entry.command == "TIMECODE 999"
            && entry.status == "rejected"
            && entry.feedback.contains("does not exist")
    }));
    install_timecode_playbacks(&state, timecode_id);

    let http = app
        .clone()
        .oneshot(playback_action_request(
            &token,
            show_id,
            session.desk.id,
            "http-timecode-go",
        ))
        .await
        .unwrap();
    let status = http.status();
    let body = json(http).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["projection"]["target"], "timecode");
    assert_eq!(body["projection"]["timecode_id"], timecode_id.0.to_string());
    assert_eq!(
        state.timecodes.snapshot(timecode_id).unwrap().transport,
        light_playback::TimecodeTransportState::Playing
    );

    let ws_id = "ui-ws-virtual-timecode-pause";
    let ws = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            ws_id,
            serde_json::from_value(serde_json::json!({
                "type": "playback",
                "request": {
                    "request_id": ws_id,
                    "address": {"kind": "virtual", "page": 1, "playback_number": 1001},
                    "action": {"type": "pause", "pressed": true},
                    "surface": "virtual"
                }
            }))
            .unwrap(),
        ),
    );
    assert!(ws.ok, "{:?}", ws.error);
    assert_eq!(
        state.timecodes.snapshot(timecode_id).unwrap().transport,
        light_playback::TimecodeTransportState::Paused
    );
    let timecode_ws = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "ui-ws-timecode-seek",
            serde_json::from_value(serde_json::json!({
                "type": "timecode",
                "request": {
                    "timecode_id": timecode_id.0,
                    "action": {"type": "seek", "frame": 333}
                }
            }))
            .unwrap(),
        ),
    );
    assert!(timecode_ws.ok, "{:?}", timecode_ws.error);
    assert_eq!(
        state.timecodes.snapshot(timecode_id).unwrap().frame,
        light_playback::TimecodeFrame(333)
    );
    timecode_v2::apply_cue_action(
        &state,
        &light_playback::CueAction::TimecodeStart {
            timecode_id,
            start: light_playback::CueTimecodeStart::Marker { marker_id },
        },
    )
    .unwrap();
    let cue_started = state.timecodes.snapshot(timecode_id).unwrap();
    assert_eq!(
        cue_started.transport,
        light_playback::TimecodeTransportState::Playing
    );
    assert!(cue_started.frame.0 >= 250);
    timecode_v2::apply_cue_action(
        &state,
        &light_playback::CueAction::TimecodeStop { timecode_id },
    )
    .unwrap();
    assert_eq!(
        state.timecodes.snapshot(timecode_id).unwrap().transport,
        light_playback::TimecodeTransportState::Stopped
    );
    state
        .output
        .execute_playback(EnginePlaybackCommand::Pool {
            number: 8,
            action: PoolPlaybackAction::Go,
        })
        .unwrap();
    let _ = output_scheduler::render_test_tick(state.clone()).await;
    let automatic = state.timecodes.snapshot(timecode_id).unwrap();
    assert_eq!(
        automatic.transport,
        light_playback::TimecodeTransportState::Playing
    );
    assert!(automatic.frame.0 >= 250);
    let expected_completion = state
        .timecodes
        .remaining_millis(timecode_id, light_playback::TimecodeFrame(250))
        .unwrap();
    assert_eq!(
        state
            .output
            .playback_runtime_status_for_cue_list(light_core::CueListId(Uuid::from_u128(701)))
            .unwrap()
            .cue_timing
            .unwrap()
            .completion_millis,
        expected_completion
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

fn command_line_object_request(
    token: &str,
    desk_id: Uuid,
    request_id: &str,
    command: &str,
) -> Request<Body> {
    Request::post("/api/v2/command-line/execute")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("x-tosk-desk", desk_id.to_string())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({"request_id": request_id, "command": command}).to_string(),
        ))
        .unwrap()
}

fn install_timecode_playbacks(state: &AppState, timecode_id: light_playback::TimecodeId) {
    let target = light_playback::PlaybackTarget::Timecode { timecode_id };
    let definition = |number| light_playback::PlaybackDefinition {
        number,
        name: "Timecode 70".into(),
        target: target.clone(),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        button_count: if number > light_playback::MAX_PLAYBACKS {
            1
        } else {
            3
        },
        fader: light_playback::PlaybackDefinition::default_fader(&target),
        has_fader: false,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#8f3541".into(),
        flash_release: light_playback::FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    let mut virtual_definition = definition(1001);
    virtual_definition.buttons[1] = light_playback::PlaybackButtonAction::None;
    virtual_definition.buttons[2] = light_playback::PlaybackButtonAction::None;
    let mut first = light_playback::Cue::new(cue("1"));
    first.name = "Before Timecode".into();
    let mut second = light_playback::Cue::new(cue("2"));
    second.name = "Start Timecode".into();
    second.trigger = light_playback::CueTrigger::Follow { delay_millis: 0 };
    second
        .actions
        .push(light_playback::CueAction::TimecodeStart {
            timecode_id,
            start: light_playback::CueTimecodeStart::Marker {
                marker_id: light_playback::TimecodeMarkerId(Uuid::from_u128(700)),
            },
        });
    let cue_list_id = light_core::CueListId(Uuid::from_u128(701));
    let cue_list = light_playback::CueList {
        id: cue_list_id,
        name: "Automatic Timecode".into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![first, second],
    };
    let cue_target = light_playback::PlaybackTarget::CueList { cue_list_id };
    let cue_playback = light_playback::PlaybackDefinition {
        number: 8,
        name: "Automatic Timecode".into(),
        target: cue_target.clone(),
        buttons: light_playback::PlaybackDefinition::default_buttons(&cue_target),
        button_count: 3,
        fader: light_playback::PlaybackDefinition::default_fader(&cue_target),
        has_fader: true,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list].into(),
            playbacks: vec![definition(7), cue_playback].into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Virtual".into(),
                slots: std::collections::HashMap::new(),
                virtual_playbacks: std::collections::HashMap::from([(1001, virtual_definition)]),
            }]
            .into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
}

fn playback_action_request(
    token: &str,
    show_id: light_core::ShowId,
    desk_id: Uuid,
    request_id: &str,
) -> Request<Body> {
    Request::post("/api/v2/playback-actions")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-tosk-show", show_id.0.to_string())
        .header("x-tosk-desk", desk_id.to_string())
        .body(Body::from(
            serde_json::json!({
                "request_id": request_id,
                "address": {"kind": "playback", "playback_number": 7},
                "action": {"type": "go", "pressed": true},
                "surface": "physical"
            })
            .to_string(),
        ))
        .unwrap()
}
