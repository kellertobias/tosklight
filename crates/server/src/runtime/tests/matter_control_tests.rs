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
    let five: DeskConfiguration =
        serde_json::from_value(serde_json::json!({"speed_groups_bpm":[1,2,3,4,5]})).unwrap();
    assert_eq!(five.speed_groups_bpm, [1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn matter_bridge_writes_and_tracking_feedback_use_explicit_global_addresses() {
    let (state, data_dir) = test_state();
    state.configuration.write().matter_enabled = true;
    state
        .engine
        .replace_snapshot(matter_test_snapshot())
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

    let status = apply_matter_playback_write(
        &state,
        matter::endpoint_id(4, 7).unwrap(),
        matter::MatterPlaybackWrite {
            on: None,
            level: Some(127),
        },
    )
    .unwrap();
    let runtime = state.engine.playback_runtime();
    let addressed = runtime
        .iter()
        .find(|playback| playback.playback_number == Some(25))
        .unwrap();
    assert!(addressed.enabled);
    assert!((addressed.master - 0.5).abs() < 0.001);
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
        .engine
        .execute_playback(EnginePlaybackCommand::Pool {
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
        state.audit_events.lock().back().unwrap().payload["source"],
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
    *state.active_show.write() = Some(show.clone());
    let cue_list_id = light_core::CueListId::new();
    state
        .engine
        .replace_snapshot(restored_exclusion_snapshot(cue_list_id))
        .unwrap();
    state.configuration.write().matter_enabled = true;
    let desk = state
        .desk
        .lock()
        .add_desk("Matter restart desk", "matter-restart")
        .unwrap();
    state
        .desk
        .lock()
        .set_desk_page(desk.id, show.id, 1)
        .unwrap();
    store_restart_zone(&state, &show, desk.id);
    state
        .engine
        .execute_pool_playback_with_activation(
            1,
            PoolPlaybackAction::On,
            &[vec![1, 2]],
            Some(light_playback::PlaybackActivationOrigin {
                at: state.engine.application_time(),
                desk_id: Some(desk.id),
                surface: light_playback::PlaybackActivationSurface::Virtual,
                exclusion_scope: light_playback::PlaybackExclusionScope::OriginatingDesk,
            }),
        )
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
        .engine
        .playback_runtime()
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
        .desk
        .lock()
        .setting(&active_playbacks_setting(show.id))
        .unwrap()
        .unwrap();
    let restored = serde_json::from_str(&checkpoint).unwrap();
    state
        .engine
        .execute_playback(EnginePlaybackCommand::RestoreActive(restored))
        .unwrap();

    let normalized = normalize_restored_virtual_playback_exclusions(&state).unwrap();
    assert!(!normalized.provenance_migrated);
    assert!(normalized.released_playbacks.is_empty());
    let enabled = state
        .engine
        .playback_runtime()
        .into_iter()
        .filter(|playback| playback.enabled)
        .filter_map(|playback| playback.playback_number)
        .collect::<HashSet<_>>();
    assert_eq!(enabled, HashSet::from([1, 2]));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn matter_virtual_master_controls_and_tracks_a_faderless_assignment() {
    let (state, data_dir) = test_state();
    state.configuration.write().matter_enabled = true;
    state
        .engine
        .replace_snapshot(matter_test_snapshot())
        .unwrap();
    let endpoint = matter::endpoint_id(1, 7).unwrap();
    let definition = state
        .engine
        .snapshot()
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
    let runtime = state.engine.playback_runtime();
    let active = runtime
        .iter()
        .find(|playback| playback.playback_number == Some(26))
        .unwrap();
    assert!(active.enabled);
    assert!((active.master - 0.5).abs() < 0.001);
    assert!((active.fader_position - 0.5).abs() < 0.001);
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
            .engine
            .playback_runtime()
            .iter()
            .find(|playback| playback.playback_number == Some(26))
            .unwrap()
            .master,
        1.0
    );

    state
        .engine
        .execute_playback(EnginePlaybackCommand::Pool {
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
fn matter_writes_reach_every_assignable_faderless_target_family() {
    let (state, data_dir) = test_state();
    state.configuration.write().matter_enabled = true;
    let definition = |number, target, fader| light_playback::PlaybackDefinition {
        number,
        name: format!("Matter playback {number}"),
        target,
        buttons: [light_playback::PlaybackButtonAction::None; 3],
        button_count: 3,
        fader,
        has_fader: false,
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
        .engine
        .replace_snapshot(EngineSnapshot {
            groups: vec![light_programmer::GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                master: 1.0,
                ..Default::default()
            }],
            playbacks: vec![
                definition(
                    1,
                    light_playback::PlaybackTarget::Group {
                        group_id: "front".into(),
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
            ],
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Matter".into(),
                slots: HashMap::from([(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]),
            }],
            ..Default::default()
        })
        .unwrap();

    let activation = state.activation_lock.clone().try_lock_owned().unwrap();
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
    assert_eq!(state.engine.snapshot().groups[0].master, 1.0);
    drop(activation);

    let output_cursor = state.application_events.latest_sequence();
    for playback in 1..=5 {
        apply_matter_playback_write(
            &state,
            matter::endpoint_id(1, playback).unwrap(),
            matter::MatterPlaybackWrite {
                on: None,
                level: Some(127),
            },
        )
        .unwrap();
    }

    assert!(
        (state.engine.snapshot().groups[0].master - 0.5).abs() < 0.001,
        "Group Master uses the Matter level"
    );
    let speed = state.speed_groups.lock()[0].snapshot(application_millis(&state));
    assert!((speed.manual_bpm - 150.0).abs() < 0.001);
    assert!((speed.speed_master_scale - 1.0).abs() < 0.001);
    let configuration = state.configuration.read();
    assert_eq!(configuration.programmer_fade_millis, 10_000);
    assert_eq!(configuration.sequence_master_fade_millis, 30_000);
    assert!((state.output_control.lock().options.grand_master - 0.5).abs() < 0.001);
    let light_application::EventReplay::Events(output_events) = state.application_events.replay(
        output_cursor,
        &light_application::EventFilter::default()
            .with_object(light_application::EventObject::global_output()),
    ) else {
        panic!("Matter output event should be retained");
    };
    assert_eq!(output_events.len(), 1);
    assert_eq!(
        output_events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Matter)
    );
    let light_application::ApplicationEvent::Output(
        light_application::OutputEvent::RuntimeChanged(change),
    ) = &output_events[0].payload
    else {
        panic!("expected output-runtime event");
    };
    assert_eq!(change.projection.revision, 1);
    let _ = std::fs::remove_dir_all(data_dir);
}
