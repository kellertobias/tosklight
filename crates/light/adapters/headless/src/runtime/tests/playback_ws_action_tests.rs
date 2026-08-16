use super::*;

#[tokio::test]
async fn playback_action_ws_frame_uses_typed_service_and_ui_source_once() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap();
    open_test_show(&app, &token).await;
    install_playback(&state);
    let cursor = state.events.latest_sequence();
    let request_id = "ws-playback-action";

    let response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            request_id,
            serde_json::from_value(serde_json::json!({
                "type": "playback",
                "request": {
                    "request_id": request_id,
                    "address": {
                        "kind": "playback",
                        "playback_number": 1,
                        "future_address_hint": true
                    },
                    "action": {"type": "go", "pressed": true, "velocity": 3},
                    "surface": "virtual",
                    "future_transport_hint": "accepted"
                }
            }))
            .unwrap(),
        ),
    );

    assert!(response.ok, "{:?}", response.error);
    let payload = response.payload.unwrap();
    assert_eq!(payload["request_id"], request_id);
    assert_eq!(payload["outcome"]["status"], "applied");
    assert_eq!(payload["resolved"]["playback_number"], 1);
    assert_eq!(payload["projection"]["runtime"]["current"]["number"], 1.0);
    assert_eq!(payload["replayed"], false);
    let light_application::EventReplay::Events(events) = state.events.replay(
        cursor,
        &light_application::EventFilter::for_desk(session.desk.id),
    ) else {
        panic!("playback event should be retained");
    };
    let playback_events = events
        .iter()
        .filter(|event| {
            matches!(
                event.payload,
                light_application::ApplicationEvent::Playback(_)
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(playback_events.len(), 1);
    assert_eq!(
        playback_events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::UserInterface)
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn playback_action_ws_frame_rejects_mismatched_request_identity_without_mutation() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap();
    open_test_show(&app, &token).await;
    install_playback(&state);
    let cursor = state.events.latest_sequence();

    let response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "ws-envelope",
            serde_json::from_value(serde_json::json!({
                "type": "playback",
                "request": {
                    "request_id": "different-payload",
                    "address": {"kind": "playback", "playback_number": 1},
                    "action": {"type": "go", "pressed": true},
                    "surface": "virtual"
                }
            }))
            .unwrap(),
        ),
    );

    assert!(!response.ok);
    assert!(
        response
            .error
            .unwrap()
            .contains("action payload request_id must match the frame request_id")
    );
    assert_eq!(state.events.latest_sequence(), cursor);
    assert!(state.output.playback_runtime().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn open_test_show(app: &Router, token: &str) {
    let show = create_show(app, token, "Playback WS show").await;
    let response = app
        .clone()
        .oneshot(open_show_request(token, show["id"].as_str().unwrap()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

fn install_playback(state: &AppState) {
    let cue_list = light_playback::CueList {
        id: light_core::CueListId::new(),
        name: "Main".into(),
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
        cues: vec![light_playback::Cue::new(cue("1"))],
    };
    let target = light_playback::PlaybackTarget::CueList {
        cue_list_id: cue_list.id,
    };
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list].into(),
            playbacks: vec![light_playback::PlaybackDefinition {
                number: 1,
                name: "Playback 1".into(),
                buttons: light_playback::PlaybackDefinition::default_buttons(&target),
                button_count: 3,
                fader: light_playback::PlaybackDefinition::default_fader(&target),
                has_fader: true,
                footprint: light_playback::PlaybackFootprint::Normal,
                go_activates: true,
                auto_off: true,
                xfade_millis: 0,
                color: "#20c997".into(),
                flash_release: light_playback::FlashReleaseMode::default(),
                protect_from_swap: false,
                presentation_icon: None,
                presentation_image: None,
                target,
            }]
            .into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
}
