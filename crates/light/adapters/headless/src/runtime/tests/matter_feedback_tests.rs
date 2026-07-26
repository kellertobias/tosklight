#[test]
fn matter_feedback_tracks_faderless_temp_and_manual_xfade_positions() {
    let (state, data_dir) = test_state();
    state.installation.update_configuration(|configuration| configuration.matter_enabled = true);
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
    ]
    .into();
    snapshot.playback_pages = vec![light_playback::PlaybackPage {
        number: 3,
        name: "Matter".into(),
        slots: HashMap::from([(1, 27), (2, 28)]),
    }]
    .into();
    state.output.replace_snapshot(snapshot).unwrap();

    let faderless_xfade = state
        .output.snapshot()
        .playbacks
        .iter()
        .find(|definition| definition.number == 28)
        .cloned()
        .unwrap();
    let rejected = dispatch_playback_action(
        &state,
        &faderless_xfade,
        "fader",
        &PoolPlaybackInput {
            value: Some(0.5),
            ..PoolPlaybackInput::default()
        },
        PlaybackDispatchContext {
            action: &light_application::ActionContext::system(
                Uuid::nil(),
                light_application::ActionSource::Osc,
            ),
            session: None,
            desk: None,
            source: "osc",
            exclusion_zones: &[],
            activation_origin: None,
        },
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
            .output.playback_runtime()
            .iter()
            .find(|playback| playback.playback_number == Some(28))
            .unwrap()
            .manual_xfade_position,
        1.0
    );

    state
        .output.execute_playback(EnginePlaybackCommand::Pool {
            number: 28,
            action: PoolPlaybackAction::Off,
        })
        .unwrap();
    let tracked_off = refresh_matter_bridge(&state);
    assert_eq!(tracked_off.lights[1].level, 0);
    assert!(!tracked_off.lights[1].on);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn matter_enablement_is_desk_persistent_and_status_is_explicit() {
    let (state, data_dir) = test_state();
    state
        .output.replace_snapshot(matter_test_snapshot())
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/configuration/update")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"request_id":"matter-enable","patch":{"matter_enabled":true}}"#,
                ))
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
            .installation.setting("server_configuration")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(persisted.matter_enabled);

    let status = app
        .oneshot(
            Request::get("/api/v2/matter/status")
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
    state.output.set_speed_group_scale_for_test(0, 0.25);

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
        state.output.speed_group_snapshot(0, 0)
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
