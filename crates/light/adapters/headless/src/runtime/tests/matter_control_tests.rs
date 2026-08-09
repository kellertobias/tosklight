#[test]
fn legacy_four_speed_group_configuration_gains_group_e() {
    let configuration: DeskConfiguration =
        serde_json::from_value(serde_json::json!({"speed_groups_bpm":[101,102,103,104]})).unwrap();
    assert_eq!(
        configuration.speed_groups_bpm,
        [101.0, 102.0, 103.0, 104.0, 15.0]
    );
    assert_eq!(
        configuration.speed_group_sound_to_light,
        default_sound_to_light()
    );
    assert!(!configuration.matter_enabled);
    assert!(!configuration.patch_preview_highlight_dmx);
    assert!(!configuration.file_manager_system_picker_fallback);
    assert!(configuration.file_manager_roots.is_empty());
    assert!(configuration.command_line_at_uses_programmer_fade);
    let five: DeskConfiguration =
        serde_json::from_value(serde_json::json!({"speed_groups_bpm":[1,2,3,4,5]})).unwrap();
    assert_eq!(five.speed_groups_bpm, [1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn matter_bridge_writes_and_tracking_feedback_use_explicit_global_addresses() {
    let (state, data_dir) = test_state();
    state.installation.update_configuration(|configuration| configuration.matter_enabled = true);
    state
        .output.replace_snapshot(matter_test_snapshot())
        .unwrap();

    let initial = refresh_matter_bridge(&state);
    assert_eq!(initial.lights.len(), 2);
    assert_eq!(
        initial
            .lights
            .iter()
            .map(|light| (light.page, light.playback, light.playback_number))
            .collect::<Vec<_>>(),
        vec![(1, 7, 26), (4, 7, 25)]
    );

    let physical = state
        .output
        .snapshot()
        .playbacks
        .iter()
        .find(|definition| definition.number == 25)
        .cloned()
        .unwrap();
    dispatch_playback_action(
        &state,
        &physical,
        "fader",
        &PoolPlaybackInput {
            value: Some(0.2),
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
            activation_origin: Some(light_playback::PlaybackActivationOrigin {
                at: state.output.application_time(),
                desk_id: None,
                surface: light_playback::PlaybackActivationSurface::Osc,
                exclusion_scope: light_playback::PlaybackExclusionScope::None,
            }),
        },
    )
    .unwrap();
    assert_eq!(state.output.playback_runtime()[0].master, 0.2);

    let status = apply_matter_playback_write(
        &state,
        matter::endpoint_id(4, 7).unwrap(),
        matter::MatterPlaybackWrite {
            on: None,
            level: Some(127),
        },
    )
    .unwrap();
    let runtime = state.output.playback_runtime();
    let addressed = runtime
        .iter()
        .find(|playback| playback.playback_number == Some(25))
        .unwrap();
    assert!(addressed.enabled);
    assert!((addressed.master - 0.5).abs() < 0.001);
    assert!(
        (state
            .output
            .playback_runtime_status_at(light_playback::PlaybackIdentity::physical(26).unwrap())
            .unwrap()
            .playback
            .master
            - 0.5)
            .abs()
            < 0.001
    );
    assert!(
        runtime
            .iter()
            .all(|playback| playback.playback_number != Some(26)),
        "page 4/playback 7 must not inherit page 1/playback 7"
    );
    let light = status
        .lights
        .iter()
        .find(|light| light.page == 4 && light.playback == 7)
        .unwrap();
    assert!(light.on);
    assert_eq!(light.level, 127);

    // Automatic tracking/off behavior is mirrored back to the Matter attribute snapshot.
    state
        .output.execute_playback(EnginePlaybackCommand::Pool {
            number: 25,
            action: PoolPlaybackAction::Off,
        })
        .unwrap();
    let tracked_off = refresh_matter_bridge(&state);
    let light = tracked_off
        .lights
        .iter()
        .find(|light| light.page == 4 && light.playback == 7)
        .unwrap();
    assert!(!light.on);
    assert_eq!(light.level, 0);
    assert_eq!(
        state.events.audit_events().last().unwrap().payload["source"],
        "matter"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn matter_activation_checkpoint_keeps_desk_independent_restart_scope() {
    let (state, data_dir) = test_state();
    let show = ShowEntry {
        id: light_core::ShowId::new(),
        name: "Matter restart scope".into(),
        path: data_dir.join("shows/matter-restart.show").display().to_string(),
        revision: 1,
        updated_at: String::new(),
        revision_copy: None,
    };
    state.active_show.replace_current(Some(show.clone()));
    let cue_list_id = light_core::CueListId::new();
    let mut snapshot = restored_exclusion_snapshot(cue_list_id);
    std::sync::Arc::make_mut(&mut snapshot.playbacks).push(restored_exclusion_playback(
        2,
        restored_exclusion_cue_list_id(cue_list_id, 2),
    ));
    std::sync::Arc::make_mut(&mut snapshot.playback_pages)[0]
        .slots
        .insert(2, 2);
    state.output.replace_snapshot(snapshot).unwrap();
    state.installation.update_configuration(|configuration| configuration.matter_enabled = true);
    let desk = state
        .installation.add_desk("Matter restart desk", "matter-restart")
        .unwrap();
    state
        .installation.set_desk_page(desk.id, show.id, 1)
        .unwrap();
    store_restart_zone(&state, &show, desk.id);
    let prepared = state
        .output
        .prepare_playback_batch(
            &[light_engine::PlaybackBatchCommand {
                number: 1_001,
                page: Some(1),
                action: light_engine::PlaybackBatchAction::On,
                exclusion_zones: vec![vec![1_001, 1_002]].into(),
                activation_origin: Some(light_playback::PlaybackActivationOrigin {
                    at: state.output.application_time(),
                    desk_id: Some(desk.id),
                    surface: light_playback::PlaybackActivationSurface::Virtual,
                    exclusion_scope: light_playback::PlaybackExclusionScope::Show,
                }),
            }],
            chrono::Utc::now(),
            0,
        )
        .unwrap();
    state
        .output
        .install_prepared_playback_batch(prepared)
        .unwrap();

    apply_matter_playback_write(
        &state,
        matter::endpoint_id(1, 2).unwrap(),
        matter::MatterPlaybackWrite {
            on: Some(true),
            level: None,
        },
    )
    .unwrap();

    let activation = state
        .output.playback_runtime()
        .into_iter()
        .find(|playback| playback.playback_number == Some(2))
        .unwrap()
        .activation
        .unwrap();
    assert_eq!(activation.desk_id, None);
    assert_eq!(
        activation.surface,
        light_playback::PlaybackActivationSurface::Matter
    );
    assert_eq!(
        activation.exclusion_scope,
        light_playback::PlaybackExclusionScope::None
    );
    persist_active_playbacks(&state).unwrap();
    let checkpoint = state
        .installation.setting(&active_playbacks_setting(show.id))
        .unwrap()
        .unwrap();
    let restored = serde_json::from_str(&checkpoint).unwrap();
    state
        .output.execute_playback(EnginePlaybackCommand::RestoreActive(restored))
        .unwrap();

    let normalized = normalize_restored_virtual_playback_exclusions(&state).unwrap();
    assert!(!normalized.provenance_migrated);
    assert!(normalized.released_playbacks.is_empty());
    let enabled = state
        .output.playback_runtime()
        .into_iter()
        .filter(|playback| playback.enabled)
        .filter_map(|playback| playback.playback_number)
        .collect::<HashSet<_>>();
    assert_eq!(enabled, HashSet::from([1_001, 2]));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn matter_virtual_master_controls_and_tracks_a_faderless_assignment() {
    let (state, data_dir) = test_state();
    state.installation.update_configuration(|configuration| configuration.matter_enabled = true);
    state
        .output.replace_snapshot(matter_test_snapshot())
        .unwrap();
    let endpoint = matter::endpoint_id(1, 7).unwrap();
    let definition = state
        .output.snapshot()
        .playbacks
        .iter()
        .find(|definition| definition.number == 26)
        .cloned()
        .unwrap();

    let rejected = dispatch_playback_action(
        &state,
        &definition,
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

    let status = apply_matter_playback_write(
        &state,
        endpoint,
        matter::MatterPlaybackWrite {
            on: None,
            level: Some(127),
        },
    )
    .unwrap();
    let runtime = state.output.playback_runtime();
    let active = runtime
        .iter()
        .find(|playback| playback.playback_number == Some(26))
        .unwrap();
    assert!(active.enabled);
    assert!((active.master - 0.5).abs() < 0.001);
    assert!(
        (state
            .output
            .playback_runtime_status_at(light_playback::PlaybackIdentity::physical(26).unwrap())
            .unwrap()
            .playback
            .fader_position
            - 0.5)
            .abs()
            < 0.001
    );
    let light = status
        .lights
        .iter()
        .find(|light| light.endpoint_id == endpoint)
        .unwrap();
    assert!(light.on);
    assert_eq!(light.level, 127);

    let off = apply_matter_playback_write(
        &state,
        endpoint,
        matter::MatterPlaybackWrite {
            on: Some(false),
            level: None,
        },
    )
    .unwrap();
    let light = off
        .lights
        .iter()
        .find(|light| light.endpoint_id == endpoint)
        .unwrap();
    assert!(!light.on);
    assert_eq!(light.level, 0);

    let on = apply_matter_playback_write(
        &state,
        endpoint,
        matter::MatterPlaybackWrite {
            on: Some(true),
            level: None,
        },
    )
    .unwrap();
    let light = on
        .lights
        .iter()
        .find(|light| light.endpoint_id == endpoint)
        .unwrap();
    assert!(light.on);
    assert_eq!(light.level, matter::MAX_MATTER_LEVEL);
    assert_eq!(
        state
            .output.playback_runtime()
            .iter()
            .find(|playback| playback.playback_number == Some(26))
            .unwrap()
            .master,
        1.0
    );

    state
        .output.execute_playback(EnginePlaybackCommand::Pool {
            number: 26,
            action: PoolPlaybackAction::Off,
        })
        .unwrap();
    let tracked_off = refresh_matter_bridge(&state);
    let light = tracked_off
        .lights
        .iter()
        .find(|light| light.endpoint_id == endpoint)
        .unwrap();
    assert!(!light.on);
    assert_eq!(light.level, 0);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn matter_writes_reach_eligible_faderless_groups_and_reject_ineligible_targets() {
    let (state, data_dir) = test_state();
    state.installation.update_configuration(|configuration| configuration.matter_enabled = true);
    let definition = |number, target, fader| light_playback::PlaybackDefinition {
        number,
        name: format!("Matter playback {number}"),
        target,
        buttons: [light_playback::PlaybackButtonAction::None; 3],
        button_count: 3,
        fader,
        has_fader: false,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    state
        .output.replace_snapshot(EngineSnapshot {
            groups: vec![light_programmer::GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                ..Default::default()
            }].into(),
            playbacks: vec![
                definition(
                    1,
                    light_playback::PlaybackTarget::Group {
                        group_id: "front".into(),
                        initial_master: None,
                    },
                    light_playback::PlaybackFaderMode::Master,
                ),
                definition(
                    2,
                    light_playback::PlaybackTarget::SpeedGroup { group: "A".into() },
                    light_playback::PlaybackFaderMode::DirectBpm,
                ),
                definition(
                    3,
                    light_playback::PlaybackTarget::ProgrammerFade,
                    light_playback::PlaybackFaderMode::Master,
                ),
                definition(
                    4,
                    light_playback::PlaybackTarget::CueFade,
                    light_playback::PlaybackFaderMode::Master,
                ),
                definition(
                    5,
                    light_playback::PlaybackTarget::GrandMaster,
                    light_playback::PlaybackFaderMode::Master,
                ),
            ].into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Matter".into(),
                slots: HashMap::from([(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]),
                virtual_playbacks: HashMap::new(),
            }].into(),
            ..Default::default()
        })
        .unwrap();
    let original_configuration = state.installation.configuration();

    let activation = state.active_show.acquire_blocking();
    let rejected = apply_matter_playback_write(
        &state,
        matter::endpoint_id(1, 1).unwrap(),
        matter::MatterPlaybackWrite {
            on: None,
            level: Some(127),
        },
    )
    .unwrap_err();
    assert_eq!(rejected.status, StatusCode::CONFLICT);
    assert_eq!(state.output.group_master("front"), Some(1.0));
    drop(activation);

    apply_matter_playback_write(
        &state,
        matter::endpoint_id(1, 1).unwrap(),
        matter::MatterPlaybackWrite {
            on: None,
            level: Some(127),
        },
    )
    .unwrap();
    for playback in 2..=5 {
        let rejected = apply_matter_playback_write(
            &state,
            matter::endpoint_id(1, playback).unwrap(),
            matter::MatterPlaybackWrite {
                on: None,
                level: Some(127),
            },
        )
        .unwrap_err();
        assert_eq!(rejected.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            rejected.message,
            format!("Matter endpoint {playback} is not exposed")
        );
    }

    assert!(
        (state.output.group_master("front").unwrap() - 0.5).abs() < 0.001,
        "Group Master uses the Matter level"
    );
    let matter_values = matter_playback_values(&state, &state.output.snapshot());
    assert!(
        (matter_values[&1].level - 0.5).abs() < 0.001,
        "Matter feedback uses the shared Group Master runtime level"
    );
    let configuration = state.installation.configuration();
    assert_eq!(
        configuration.programmer_fade_millis,
        original_configuration.programmer_fade_millis
    );
    assert_eq!(
        configuration.sequence_master_fade_millis,
        original_configuration.sequence_master_fade_millis
    );
    assert!((state.output.control_projection().grand_master - 1.0).abs() < 0.001);
    let _ = std::fs::remove_dir_all(data_dir);
}
