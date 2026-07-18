#[test]
fn matter_feedback_tracks_faderless_temp_and_manual_xfade_positions() {
    let (state, data_dir) = test_state();
    state.configuration.write().matter_enabled = true;
    let mut snapshot = matter_test_snapshot();
    let cue_list_id = snapshot.cue_lists[0].id;
    let definition = |number, fader, has_fader| light_playback::PlaybackDefinition {
        number,
        name: format!("Matter playback {number}"),
        target: light_playback::PlaybackTarget::CueList { cue_list_id },
        buttons: [light_playback::PlaybackButtonAction::None; 3],
        button_count: 3,
        fader,
        has_fader,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    snapshot.playbacks = vec![
        definition(27, light_playback::PlaybackFaderMode::Temp, false),
        definition(28, light_playback::PlaybackFaderMode::XFade, false),
    ];
    snapshot.playback_pages = vec![light_playback::PlaybackPage {
        number: 3,
        name: "Matter".into(),
        slots: HashMap::from([(1, 27), (2, 28)]),
    }];
    state.engine.replace_snapshot(snapshot).unwrap();

    let faderless_xfade = state
        .engine
        .snapshot()
        .playbacks
        .iter()
        .find(|definition| definition.number == 28)
        .cloned()
        .unwrap();
    let rejected = dispatch_playback_action(
        &state,
        None,
        None,
        &faderless_xfade,
        "fader",
        &PoolPlaybackInput {
            value: Some(0.5),
            ..PoolPlaybackInput::default()
        },
        "osc",
    )
    .unwrap_err();
    assert_eq!(rejected.message, "playback does not have a fader");

    for playback in 1..=2 {
        apply_matter_playback_write(
            &state,
            matter::endpoint_id(3, playback).unwrap(),
            matter::MatterPlaybackWrite {
                on: None,
                level: Some(127),
            },
        )
        .unwrap();
    }
    let status = refresh_matter_bridge(&state);
    assert_eq!(
        status
            .lights
            .iter()
            .map(|light| (light.playback_number, light.level, light.on))
            .collect::<Vec<_>>(),
        vec![(27, 127, true), (28, 127, true)]
    );

    apply_matter_playback_write(
        &state,
        matter::endpoint_id(3, 1).unwrap(),
        matter::MatterPlaybackWrite {
            on: Some(false),
            level: None,
        },
    )
    .unwrap();
    let status = refresh_matter_bridge(&state);
    assert_eq!(status.lights[0].level, 0);
    assert!(!status.lights[0].on);

    let xfade_endpoint = matter::endpoint_id(3, 2).unwrap();
    let off = apply_matter_playback_write(
        &state,
        xfade_endpoint,
        matter::MatterPlaybackWrite {
            on: Some(false),
            level: None,
        },
    )
    .unwrap();
    assert_eq!(off.lights[1].level, 0);
    assert!(!off.lights[1].on);
    let on = apply_matter_playback_write(
        &state,
        xfade_endpoint,
        matter::MatterPlaybackWrite {
            on: Some(true),
            level: None,
        },
    )
    .unwrap();
    assert_eq!(on.lights[1].level, matter::MAX_MATTER_LEVEL);
    assert!(on.lights[1].on);
    assert_eq!(
        state
            .engine
            .playback()
            .read()
            .runtime()
            .iter()
            .find(|playback| playback.playback_number == Some(28))
            .unwrap()
            .manual_xfade_position,
        1.0
    );

    state.engine.playback().write().off(28).unwrap();
    let tracked_off = refresh_matter_bridge(&state);
    assert_eq!(tracked_off.lights[1].level, 0);
    assert!(!tracked_off.lights[1].on);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn matter_enablement_is_desk_persistent_and_status_is_explicit() {
    let (state, data_dir) = test_state();
    state
        .engine
        .replace_snapshot(matter_test_snapshot())
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut configuration = state.configuration.read().clone();
    configuration.matter_enabled = true;
    let response = app
        .clone()
        .oneshot(
            Request::put("/api/v1/configuration")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&configuration).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["matter"]["enabled"], true);
    assert_eq!(response["matter"]["transport"], "adapter_ready");
    assert_eq!(response["matter"]["commissionable"], false);
    assert!(response["matter"]["limitation"].is_string());

    let persisted: DeskConfiguration = serde_json::from_str(
        &state
            .desk
            .lock()
            .setting("server_configuration")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(persisted.matter_enabled);

    let status = app
        .oneshot(
            Request::get("/api/v1/matter/status")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let status = json(status).await;
    assert_eq!(status["lights"].as_array().unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn direct_bpm_fader_reports_zero_half_and_full_authoritative_rates() {
    let (state, data_dir) = test_state();
    state.speed_groups.lock()[0]
        .set_speed_master_scale(0.25)
        .unwrap();

    let set_fader = |value| {
        apply_speed_group_playback_action(
            &state,
            "A",
            "master",
            &PoolPlaybackInput {
                value: Some(value),
                ..PoolPlaybackInput::default()
            },
            light_playback::PlaybackFaderMode::DirectBpm,
        )
        .unwrap();
        state.speed_groups.lock()[0].snapshot(0)
    };

    let half = set_fader(0.5);
    assert_eq!(half.effective_bpm, 150.0);
    assert_eq!(half.speed_master_scale, 1.0);
    assert!(!half.paused);

    let zero = set_fader(0.0);
    assert_eq!(zero.effective_bpm, 0.0);
    assert_eq!(zero.speed_master_scale, 0.0);
    assert!(zero.paused);

    let full = set_fader(1.0);
    assert_eq!(full.effective_bpm, 300.0);
    assert_eq!(full.speed_master_scale, 1.0);
    assert!(!full.paused);
    let _ = std::fs::remove_dir_all(data_dir);
}
