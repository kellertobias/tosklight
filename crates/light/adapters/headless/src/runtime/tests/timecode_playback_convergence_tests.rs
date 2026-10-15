use super::*;

#[tokio::test]
async fn test_bench_clock_advances_timecode_on_step_and_scheduler_ticks() {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, data_dir) = test_state_with_clock(clock.clone());
    let id = light_playback::TimecodeId(Uuid::new_v4());
    state
        .timecodes
        .install(
            light_playback::TimecodeDefinition {
                id,
                number: 1,
                name: "Controlled clock".into(),
                duration: Some(light_playback::TimecodeFrame(440)),
                transport_offset: light_playback::TimecodeFrame::ZERO,
                auto_start: false,
                audio: None,
                markers: Vec::new(),
                lanes: Vec::new(),
            },
            None,
        )
        .unwrap();
    state
        .timecodes
        .handle(id, light_playback::TimecodeTransportAction::Go)
        .unwrap();
    let app = router(state.clone());
    let response = app
        .oneshot(
            Request::post("/api/v2/test/clock/advance")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"millis":1000}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    // This isolated test state intentionally has no network sender. Rendering and transport
    // still advance before the delivery boundary reports that precise setup limitation.
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        json(response).await["error"],
        "network output is unavailable"
    );
    assert_eq!(
        state.timecodes.snapshot(id).unwrap().frame,
        light_playback::TimecodeFrame(44)
    );
    // The bounded free-run path calls the same render entry after advancing application time.
    clock.advance_millis(500);
    assert_eq!(
        output_scheduler::render_test_tick(state.clone())
            .await
            .unwrap_err()
            .to_string(),
        "network output is unavailable"
    );
    assert_eq!(
        state.timecodes.snapshot(id).unwrap().frame,
        light_playback::TimecodeFrame(66)
    );
    // A render without advancing the shared clock cannot move the transport.
    assert_eq!(
        output_scheduler::render_test_tick(state.clone())
            .await
            .unwrap_err()
            .to_string(),
        "network output is unavailable"
    );
    assert_eq!(
        state.timecodes.snapshot(id).unwrap().frame,
        light_playback::TimecodeFrame(66)
    );
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[tokio::test]
async fn http_and_ws_page_aliases_control_one_timecode_runtime() {
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
                    "address": {"kind": "playback", "page": 2, "position": 1},
                    "action": {"type": "pause", "pressed": true},
                    "control_source": {"surface_id": "pane:timecode", "control_id": 1}
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
        .execute_playback(EnginePlaybackCommand::Playback {
            address: light_playback::PlaybackAddress::new(1, 8).unwrap(),
            action: light_engine::PlaybackAction::Go,
            activation_origin: None,
            control_source: light_playback::PlaybackControlSource {
                surface_id: "main".into(),
                control_id: 8,
            },
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
    let definition = |button_count| light_playback::PlaybackDefinition {
        name: "Timecode 70".into(),
        target: target.clone(),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        button_count,
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
    let mut pane_definition = definition(1);
    pane_definition.buttons[1] = light_playback::PlaybackButtonAction::None;
    pane_definition.buttons[2] = light_playback::PlaybackButtonAction::None;
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
        pool_number: None,
        legacy_pool_aliases: Vec::new(),
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
            playback_pages: vec![
                light_playback::PlaybackPage {
                    number: 1,
                    name: "Main".into(),
                    assignments: std::collections::HashMap::from([
                        (7, definition(3)),
                        (8, cue_playback),
                    ]),
                },
                light_playback::PlaybackPage {
                    number: 2,
                    name: "Timecode pane".into(),
                    assignments: std::collections::HashMap::from([(1, pane_definition)]),
                },
            ]
            .into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
}

fn playback_action_request(
    token: &str,
    show_id: light_core::ShowId,
    desk_id: Uuid,
    _request_label: &str,
) -> Request<Body> {
    Request::post("/api/v2/playback-actions")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-tosk-show", show_id.0.to_string())
        .header("x-tosk-desk", desk_id.to_string())
        .body(Body::from(
            serde_json::json!({
                "address": {"kind": "playback", "page": 1, "position": 7},
                "action": {"type": "go", "pressed": true},
                "control_source": {"surface_id": "main", "control_id": 7}
            })
            .to_string(),
        ))
        .unwrap()
}

#[test]
fn timecode_wire_snapshot_preserves_runtime_arming() {
    let definition = light_playback::TimecodeDefinition {
        id: light_playback::TimecodeId(Uuid::new_v4()),
        number: 1,
        name: "External transport".into(),
        duration: Some(light_playback::TimecodeFrame(440)),
        transport_offset: light_playback::TimecodeFrame::ZERO,
        auto_start: false,
        audio: None,
        markers: Vec::new(),
        lanes: Vec::new(),
    };
    for external_armed in [false, true] {
        let snapshot = light_application::timeline::TimecodeRuntimeSnapshot {
            timecode_id: definition.id,
            revision: 7,
            transport: light_playback::TimecodeTransportState::Paused,
            external_armed,
            frame: light_playback::TimecodeFrame(44),
            duration: light_playback::TimecodeFrame(440),
            reconstructed: definition.state_at(light_playback::TimecodeFrame(44)),
            cue_list_clips: Vec::new(),
            audio_linked: false,
        };
        let wire = super::super::timecode_v2::wire_snapshot(snapshot);
        assert_eq!(wire.external_armed, external_armed);
        assert_eq!(
            wire.state,
            light_wire::v2::timecode::TimecodeTransportState::Paused
        );
    }
}

#[tokio::test]
async fn automatic_timecode_stop_releases_owned_cue_in_the_same_captured_dmx_frame() {
    assert_automatic_timecode_same_frame(false).await;
}

#[tokio::test]
async fn isolated_transport_settler_releases_real_timecode_owned_cue_before_render() {
    assert_automatic_timecode_same_frame(true).await;
}

async fn assert_automatic_timecode_same_frame(isolated_proof: bool) {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, data_dir) = test_state_with_clock(clock.clone());
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Automatic Timecode same-frame release").await;
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, show["id"].as_str().unwrap()))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let fixture = light_core::FixtureId::new();
    let owned = operational_cue_list(light_core::CueListId::new(), fixture);
    let owned_id = owned.id;
    let owned_cue = owned.cues[0].id;
    let timecode_id = light_playback::TimecodeId(Uuid::new_v4());
    let mut first = light_playback::Cue::new(cue("1"));
    first.fade_millis = 0;
    let mut second = light_playback::Cue::new(cue("2"));
    second.fade_millis = 0;
    second.trigger = light_playback::CueTrigger::Follow { delay_millis: 10 };
    second
        .actions
        .push(light_playback::CueAction::TimecodeStop { timecode_id });
    let trigger: light_playback::CueList = serde_json::from_value(serde_json::json!({
        "id":light_core::CueListId::new(), "name":"Stop timeline", "priority":0,
        "mode":"sequence", "looped":false, "cues":[first,second]
    }))
    .unwrap();
    let trigger_id = trigger.id;
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![operational_fixture(fixture)].into(),
            cue_lists: vec![owned, trigger].into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Main".into(),
                assignments: std::collections::HashMap::from([(
                    1,
                    light_playback::PlaybackDefinition::new_cue_list("Stop timeline", trigger_id),
                )]),
            }]
            .into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    state
        .timecodes
        .install(
            light_playback::TimecodeDefinition {
                id: timecode_id,
                number: 1,
                name: "Own dimmer Cue".into(),
                duration: Some(light_playback::TimecodeFrame(440)),
                transport_offset: light_playback::TimecodeFrame::ZERO,
                auto_start: false,
                audio: None,
                markers: Vec::new(),
                lanes: vec![light_playback::TimecodeLane {
                    id: light_playback::TimecodeLaneId(Uuid::new_v4()),
                    name: "Dimmer".into(),
                    content: light_playback::TimecodeLaneContent::CueList {
                        cue_list_id: owned_id,
                        clips: vec![light_playback::TimecodeCueListClip {
                            id: light_playback::TimecodeClipId(Uuid::new_v4()),
                            start_frame: light_playback::TimecodeFrame::ZERO,
                            end_frame: light_playback::TimecodeFrame(440),
                            start_cue_id: owned_cue,
                            end_cue_id: owned_cue,
                            start_behavior: light_playback::TimecodeClipStart::State,
                            end_behavior: light_playback::TimecodeClipEnd::Release,
                            cue_starts: Vec::new(),
                            in_fade_frames: 0,
                            out_fade_frames: 0,
                        }],
                    },
                }],
            },
            None,
        )
        .unwrap();
    state
        .timecodes
        .handle(timecode_id, light_playback::TimecodeTransportAction::Go)
        .unwrap();
    state.timecodes.reconcile_cue_lists(state.output.engine());
    let _ = output_scheduler::render_test_tick(state.clone()).await;
    assert_eq!(
        state.output.dmx_snapshot().0.universes[0].slots[0],
        255,
        "the real Timecode clip must own the patched dimmer before the automatic action"
    );
    state
        .output
        .execute_playback(EnginePlaybackCommand::Playback {
            address: light_playback::PlaybackAddress::new(1, 1).unwrap(),
            action: light_engine::PlaybackAction::Go,
            activation_origin: None,
            control_source: light_playback::PlaybackControlSource {
                surface_id: "main".into(),
                control_id: 1,
            },
        })
        .unwrap();
    let _ = output_scheduler::render_test_tick(state.clone()).await;
    clock.advance_millis(20);
    let captured = if isolated_proof {
        // Test-only proof: use the real transport, owned Cue and physical Engine frame,
        // without changing either production capture callback or test-bench dispatch.
        let engine = state.output.engine();
        let prepared = engine.prepare_output_frame(state.output.render_options());
        let sampled_at = prepared.sampled_at();
        assert_eq!(prepared.automatic_playback_transitions().len(), 1);
        let settled = output_scheduler::timecode_capture::settle(
            engine,
            prepared,
            |action| {
                super::super::timecode_v2::apply_installed_cue_action(
                    &state.timecodes,
                    &state.active_show.current().unwrap().path,
                    action,
                )
                .map_err(|error| error.message)
            },
            || {
                state.timecodes.reconcile_cue_lists(engine);
                Ok(())
            },
        );
        let accepted = settled.accepted.unwrap();
        assert_eq!(accepted.automatic_playback_transitions().len(), 1);
        assert!(output_scheduler::claim_automatic_cue_action_batches(&accepted).is_empty());
        let refreshed = settled.frame.unwrap();
        assert_eq!(refreshed.sampled_at(), sampled_at);
        assert!(refreshed.automatic_playback_transitions().is_empty());
        engine.render_prepared(&refreshed, &[]).unwrap().universes[&1][0]
    } else {
        let _ = output_scheduler::render_test_tick(state.clone()).await;
        state.output.dmx_snapshot().0.universes[0].slots[0]
    };
    let transport = state.timecodes.snapshot(timecode_id).unwrap().transport;
    let retained_owner = state
        .output
        .playback_runtime_status_for_cue_list(owned_id)
        .is_some_and(|runtime| runtime.playback.enabled);
    assert_eq!(
        (captured, transport, retained_owner),
        (0, light_playback::TimecodeTransportState::Stopped, false),
        "automatic Stop, owned Cue release and captured DMX must settle in one frame"
    );
    std::fs::remove_dir_all(data_dir).unwrap();
}

fn asymmetric_timeline_cues(fixtures: [light_core::FixtureId; 2]) -> light_playback::CueList {
    let mut list = operational_cue_list(light_core::CueListId::new(), fixtures[0]);
    list.cues.clear();
    for (index, levels) in [[0.2, 0.8], [0.8, 0.2]].into_iter().enumerate() {
        let mut item = light_playback::Cue::new(cue(if index == 0 { "1" } else { "2" }));
        item.fade_millis = if index == 0 { 0 } else { 2_000 };
        item.delay_millis = if index == 0 { 0 } else { 500 };
        if index == 0 {
            item.out_fade_millis = Some(1_000);
            item.out_delay_millis = Some(250);
        }
        for (fixture, level) in fixtures.into_iter().zip(levels) {
            let mut change = light_playback::CueChange::set(
                fixture,
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Normalized(level),
            );
            if index == 0 {
                change.fade_millis = Some(0);
                change.delay_millis = Some(0);
            }
            item.changes.push(change);
        }
        list.cues.push(item);
    }
    list
}

fn asymmetric_timeline_definition(
    list: &light_playback::CueList,
) -> light_playback::TimecodeDefinition {
    use light_playback::*;
    TimecodeDefinition {
        id: TimecodeId(Uuid::new_v4()),
        number: 1,
        name: "Asymmetric seek".into(),
        duration: Some(TimecodeFrame(440)),
        transport_offset: TimecodeFrame::ZERO,
        auto_start: false,
        audio: None,
        markers: Vec::new(),
        lanes: vec![TimecodeLane {
            id: TimecodeLaneId(Uuid::new_v4()),
            name: "Manual Cues".into(),
            content: TimecodeLaneContent::CueList {
                cue_list_id: list.id,
                clips: vec![TimecodeCueListClip {
                    id: TimecodeClipId(Uuid::new_v4()),
                    start_frame: TimecodeFrame::ZERO,
                    end_frame: TimecodeFrame(264),
                    start_cue_id: list.cues[0].id,
                    end_cue_id: list.cues[1].id,
                    start_behavior: TimecodeClipStart::State,
                    end_behavior: TimecodeClipEnd::Hold,
                    cue_starts: vec![TimecodeCueStart {
                        cue_id: list.cues[1].id,
                        offset_frame: TimecodeFrame(44),
                    }],
                    in_fade_frames: 0,
                    out_fade_frames: 0,
                }],
            },
        }],
    }
}

#[tokio::test]
async fn timecode_paused_seek_reconstructs_asymmetric_predecessor_output() {
    use light_playback::{TimecodeFrame, TimecodeTransportAction};
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, data_dir) = test_state_with_clock(clock);
    let fixtures = [light_core::FixtureId::new(), light_core::FixtureId::new()];
    let list = asymmetric_timeline_cues(fixtures);
    let definition = asymmetric_timeline_definition(&list);
    let mut second = operational_fixture(fixtures[1]);
    second.address = Some(2);
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![operational_fixture(fixtures[0]), second].into(),
            cue_lists: vec![list.clone()].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    state.timecodes.install(definition.clone(), None).unwrap();
    state
        .timecodes
        .handle(definition.id, TimecodeTransportAction::Go)
        .unwrap();
    state
        .timecodes
        .handle(definition.id, TimecodeTransportAction::Pause)
        .unwrap();
    // Frozen native oracle: Cue 2 enters at 1s, rising .2->.8 after .5s over2s;
    // falling .8->.2 uses Cue 1 Out delay .25s/fade1s. No captured bytes define this.
    for (frame, expected) in [
        (77, [70u8, 128]),
        (110, [128, 51]),
        (154, [204, 51]),
        (77, [70, 128]),
    ] {
        state
            .timecodes
            .handle(
                definition.id,
                TimecodeTransportAction::Seek {
                    frame: TimecodeFrame(frame),
                },
            )
            .unwrap();
        state.timecodes.reconcile_cue_lists(state.output.engine());
        let _ = output_scheduler::render_test_tick(state.clone()).await;
        let snapshot = state.output.dmx_snapshot().0;
        let actual = &snapshot.universes[0].slots[..2];
        for (got, want) in actual.iter().zip(expected) {
            assert!(
                got.abs_diff(want) <= 1,
                "frame {frame}: actual {actual:?}, expected {expected:?}"
            );
        }
        let status = state
            .output
            .engine()
            .playback_runtime_status_for_cue_list(list.id)
            .unwrap();
        assert!(status.playback.discrete_cue_actions_suppressed);
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn timecode_seek_resolves_overlap_and_nonfirst_clip_entry_from_actual_schedule() {
    use light_playback::{TimecodeFrame, TimecodeLaneContent, TimecodeTransportAction};
    for (overlapping, expected) in [(true, [48u8, 77]), (false, [26, 6])] {
        let clock = Arc::new(ManualClock::new(fixed_test_time()));
        let (state, data_dir) = test_state_with_clock(clock);
        let fixtures = [light_core::FixtureId::new(), light_core::FixtureId::new()];
        let mut list = asymmetric_timeline_cues(fixtures);
        let mut definition = asymmetric_timeline_definition(&list);
        if overlapping {
            for change in &mut list.cues[0].changes {
                change.fade_millis = Some(2_000);
            }
        } else {
            let TimecodeLaneContent::CueList { clips, .. } = &mut definition.lanes[0].content
            else {
                unreachable!()
            };
            clips[0].start_cue_id = list.cues[1].id;
            clips[0].start_frame = TimecodeFrame(44);
            clips[0].cue_starts.clear();
        }
        let mut second = operational_fixture(fixtures[1]);
        second.address = Some(2);
        state
            .output
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![operational_fixture(fixtures[0]), second].into(),
                cue_lists: vec![list.clone()].into(),
                ..EngineSnapshot::default()
            })
            .unwrap();
        state.timecodes.install(definition.clone(), None).unwrap();
        state
            .timecodes
            .handle(definition.id, TimecodeTransportAction::Go)
            .unwrap();
        state
            .timecodes
            .handle(definition.id, TimecodeTransportAction::Pause)
            .unwrap();
        state
            .timecodes
            .handle(
                definition.id,
                TimecodeTransportAction::Seek {
                    frame: TimecodeFrame(77),
                },
            )
            .unwrap();
        state.timecodes.reconcile_cue_lists(state.output.engine());
        let _ = output_scheduler::render_test_tick(state.clone()).await;
        let snapshot = state.output.dmx_snapshot().0;
        let actual = &snapshot.universes[0].slots[..2];
        for (got, want) in actual.iter().zip(expected) {
            assert!(
                got.abs_diff(want) <= 1,
                "overlapping={overlapping}: actual {actual:?}, expected {expected:?}"
            );
        }
        assert!(
            state
                .output
                .engine()
                .playback_runtime_status_for_cue_list(list.id)
                .unwrap()
                .playback
                .discrete_cue_actions_suppressed
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

#[tokio::test]
async fn tl795_assigned_go_resumes_paused_frame_and_scheduled_cue_history() {
    use light_playback::{TimecodeFrame, TimecodeTransportAction, TimecodeTransportState};
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, data_dir) = test_state_with_clock(clock.clone());
    let app = router(state.clone());
    let (token, _) = login(&app, "Assigned Timecode resume").await;
    let show = create_show(&app, &token, "Assigned resume").await;
    assert_eq!(
        app.clone()
            .oneshot(open_show_request(&token, show["id"].as_str().unwrap()))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let session = state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap();
    let fixtures = [light_core::FixtureId::new(), light_core::FixtureId::new()];
    let list = asymmetric_timeline_cues(fixtures);
    let definition = asymmetric_timeline_definition(&list);
    let assignment: light_playback::PlaybackDefinition =
        serde_json::from_value(serde_json::json!({
            "name":"Shared Timecode","target":{"type":"timecode","timecode_id":definition.id},
            "has_fader":false,"buttons":["go","pause","off"]
        }))
        .unwrap();
    let mut second = operational_fixture(fixtures[1]);
    second.address = Some(2);
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![operational_fixture(fixtures[0]), second].into(),
            cue_lists: vec![list.clone()].into(),
            playback_pages: vec![
                light_playback::PlaybackPage {
                    number: 1,
                    name: "Main".into(),
                    assignments: std::collections::HashMap::from([(7, assignment.clone())]),
                },
                light_playback::PlaybackPage {
                    number: 2,
                    name: "Pane".into(),
                    assignments: std::collections::HashMap::from([(1, assignment)]),
                },
            ]
            .into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    state.timecodes.install(definition.clone(), None).unwrap();
    state
        .timecodes
        .handle(definition.id, TimecodeTransportAction::Go)
        .unwrap();
    state
        .timecodes
        .handle(definition.id, TimecodeTransportAction::Pause)
        .unwrap();
    state
        .timecodes
        .handle(
            definition.id,
            TimecodeTransportAction::Seek {
                frame: TimecodeFrame(200),
            },
        )
        .unwrap();
    state.timecodes.reconcile_cue_lists(state.output.engine());
    let _ = output_scheduler::render_test_tick(state.clone()).await;
    let held = state.output.dmx_snapshot().0;
    assert_eq!(
        state
            .output
            .engine()
            .playback_runtime_status_for_cue_list(list.id)
            .unwrap()
            .playback
            .current_cue_id,
        Some(list.cues[1].id)
    );
    let response = app
        .clone()
        .oneshot(playback_action_request(
            &token,
            state.active_show.current().unwrap().id,
            session.desk.id,
            "resume-paused-main",
        ))
        .await
        .unwrap();
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let resumed = state.timecodes.snapshot(definition.id).unwrap();
    assert_eq!(resumed.transport, TimecodeTransportState::Playing);
    assert_eq!(
        resumed.frame,
        TimecodeFrame(200),
        "assigned GO must resume the held Timecode, not restart at0"
    );
    let _ = output_scheduler::render_test_tick(state.clone()).await;
    assert_eq!(
        state.output.dmx_snapshot().0.universes[0].slots[..2],
        held.universes[0].slots[..2]
    );
    assert_eq!(
        state
            .output
            .engine()
            .playback_runtime_status_for_cue_list(list.id)
            .unwrap()
            .playback
            .current_cue_id,
        Some(list.cues[1].id)
    );
    clock.advance_millis(250);
    let _ = output_scheduler::render_test_tick(state.clone()).await;
    assert_eq!(
        state.timecodes.snapshot(definition.id).unwrap().frame,
        TimecodeFrame(211)
    );
    state
        .timecodes
        .handle(definition.id, TimecodeTransportAction::Pause)
        .unwrap();
    let pane=dispatch_live_action(&state,&session,live_action_frame(&session,"resume-paused-pane",serde_json::from_value(serde_json::json!({
        "type":"playback","request":{"address":{"kind":"playback","page":2,"position":1},
        "action":{"type":"go","pressed":true},"control_source":{"surface_id":"pane:timecode","control_id":1}}
    })).unwrap()));
    assert!(pane.ok, "{:?}", pane.error);
    assert_eq!(
        state.timecodes.snapshot(definition.id).unwrap().frame,
        TimecodeFrame(211)
    );
    // An already-playing assigned GO retains its existing restart behavior; direct editor Go too.
    let response = app
        .clone()
        .oneshot(playback_action_request(
            &token,
            state.active_show.current().unwrap().id,
            session.desk.id,
            "restart-playing-main",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        state.timecodes.snapshot(definition.id).unwrap().frame,
        TimecodeFrame::ZERO
    );
    state
        .timecodes
        .handle(definition.id, TimecodeTransportAction::Pause)
        .unwrap();
    state
        .timecodes
        .handle(
            definition.id,
            TimecodeTransportAction::Seek {
                frame: TimecodeFrame(200),
            },
        )
        .unwrap();
    state
        .timecodes
        .handle(definition.id, TimecodeTransportAction::Go)
        .unwrap();
    assert_eq!(
        state.timecodes.snapshot(definition.id).unwrap().frame,
        TimecodeFrame::ZERO,
        "direct editor Play still restarts"
    );
    state
        .timecodes
        .handle(definition.id, TimecodeTransportAction::Stop)
        .unwrap();
    let response = app
        .oneshot(playback_action_request(
            &token,
            state.active_show.current().unwrap().id,
            session.desk.id,
            "start-stopped-main",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        state.timecodes.snapshot(definition.id).unwrap().transport,
        TimecodeTransportState::Playing
    );
    assert_eq!(
        state.timecodes.snapshot(definition.id).unwrap().frame,
        TimecodeFrame::ZERO
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
