fn matter_test_address(page:u8,position:u16)->light_playback::PlaybackAddress {
    light_playback::PlaybackAddress::new(page,position).unwrap()
}
fn matter_test_endpoint(state:&AppState,page:u8,position:u16)->u16 {
    refresh_matter_bridge(state).lights.iter().find(|light|light.address == matter_test_address(page,position)).unwrap().endpoint_id
}
fn matter_test_source()->light_playback::PlaybackControlSource {
    light_playback::PlaybackControlSource {surface_id:"matter-test".into(),control_id:1}
}
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
    assert!(!configuration.command_line_at_uses_programmer_fade);
    let five: DeskConfiguration =
        serde_json::from_value(serde_json::json!({"speed_groups_bpm":[1,2,3,4,5]})).unwrap();
    assert_eq!(five.speed_groups_bpm, [1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn matter_bridge_writes_and_tracking_feedback_use_explicit_canonical_addresses() {
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
            .map(|light| light.address)
            .collect::<Vec<_>>(),
        vec![matter_test_address(1,7),matter_test_address(4,7)]
    );

    let assigned = state
        .output
        .snapshot()
        .playback_pages
        .iter()
        .find(|page|page.number == 4)
        .and_then(|page|page.assignments.get(&7))
        .cloned()
        .unwrap();
    dispatch_playback_action(
        &state,
        matter_test_address(4,7),
        &assigned,
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
            control_source: &matter_test_source(),
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
        matter_test_endpoint(&state,4,7),
        matter::MatterPlaybackWrite {
            on: None,
            level: Some(127),
        color: None,
        },
    )
    .unwrap();
    let runtime = state.output.playback_runtime();
    let addressed = runtime
        .iter()
        .find(|playback| playback.playback_address == Some(matter_test_address(4,7)))
        .unwrap();
    assert!(addressed.enabled);
    assert!((addressed.master - 0.5).abs() < 0.001);
    assert!(
        (state
            .output
            .playback_runtime_status_at(matter_test_address(1,7))
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
            .all(|playback| playback.playback_address != Some(matter_test_address(1,7))),
        "page 4/playback 7 must not inherit page 1/playback 7"
    );
    let light = status
        .lights
        .iter()
        .find(|light| light.address == matter_test_address(4,7))
        .unwrap();
    assert!(light.on);
    assert_eq!(light.level, 127);

    // Automatic tracking/off behavior is mirrored back to the Matter attribute snapshot.
    state
        .output.execute_playback(EnginePlaybackCommand::Playback {
            address:matter_test_address(4,7),action:light_engine::PlaybackAction::Off,activation_origin:None,control_source:matter_test_source(),
        })
        .unwrap();
    let tracked_off = refresh_matter_bridge(&state);
    let light = tracked_off
        .lights
        .iter()
        .find(|light| light.address == matter_test_address(4,7))
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
fn matter_fader_explicitly_starts_and_stops_when_authored_policies_are_disabled() {
    let (state, data_dir) = test_state();
    state
        .installation
        .update_configuration(|configuration| configuration.matter_enabled = true);
    let mut snapshot = matter_test_snapshot();
    for page in std::sync::Arc::make_mut(&mut snapshot.playback_pages) {
      for playback in page.assignments.values_mut() { playback.go_activates=false;playback.auto_off=false; }
    }
    std::sync::Arc::make_mut(&mut snapshot.cue_lists)[0].auto_off_at_zero = false;
    state.output.replace_snapshot(snapshot).unwrap();
    let endpoint = matter_test_endpoint(&state,4,7);

    apply_matter_playback_write(
        &state,
        endpoint,
        matter::MatterPlaybackWrite {
            on: None,
            level: Some(127),
        color: None,
        },
    )
    .unwrap();
    let active = state
        .output
        .playback_runtime()
        .into_iter()
        .find(|playback| playback.playback_address == Some(matter_test_address(4,7)))
        .unwrap();
    assert!(active.enabled);
    assert!((active.master - 0.5).abs() < 0.001);

    apply_matter_playback_write(
        &state,
        endpoint,
        matter::MatterPlaybackWrite {
            on: None,
            level: Some(0),
        color: None,
        },
    )
    .unwrap();
    let stopped = state
        .output
        .playback_runtime()
        .into_iter()
        .find(|playback| playback.playback_address == Some(matter_test_address(4,7)))
        .unwrap();
    assert!(!stopped.enabled);
    assert_eq!(stopped.master, 0.0);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn matter_activation_checkpoint_restores_canonical_origin_and_general_solo_state() {
    let (state,data_dir)=test_state();
    let show=ShowEntry { is_base_show:false,id:light_core::ShowId::new(),name:"Matter restart".into(),path:data_dir.join("shows/matter-restart.show").display().to_string(),revision:1,updated_at:String::new(),created_at:None,last_loaded_at:None,revision_copy:None };
    state.active_show.replace_current(Some(show.clone()));
    let mut snapshot=matter_test_snapshot();
    let mut second=snapshot.cue_lists[0].clone(); second.id=light_core::CueListId::new();
    let mut first_definition=snapshot.playback_pages[1].assignments[&7].clone();
    first_definition.target=light_playback::PlaybackTarget::CueList{cue_list_id:snapshot.cue_lists[0].id};
    let mut second_definition=first_definition.clone(); second_definition.target=light_playback::PlaybackTarget::CueList{cue_list_id:second.id};
    std::sync::Arc::make_mut(&mut snapshot.cue_lists).push(second);
    snapshot.playback_pages=vec![light_playback::PlaybackPage {number:1,name:"Matter Solo".into(),assignments:HashMap::from([(1,first_definition),(2,second_definition)])}].into();
    snapshot.playback_solo_regions=vec![light_playback::PlaybackSoloRegion {id:Uuid::new_v4(),name:"Matter pair".into(),members:vec![matter_test_address(1,1),matter_test_address(1,2)]}].into();
    state.output.replace_snapshot(snapshot).unwrap();
    state.installation.update_configuration(|configuration|configuration.matter_enabled=true);
    state.output.execute_playback(EnginePlaybackCommand::Playback {address:matter_test_address(1,1),action:light_engine::PlaybackAction::On,activation_origin:None,control_source:matter_test_source()}).unwrap();
    apply_matter_playback_write(&state,matter_test_endpoint(&state,1,2),matter::MatterPlaybackWrite{on:Some(true),..Default::default()}).unwrap();
    let activation=state.output.playback_runtime().into_iter().find(|playback|playback.playback_address==Some(matter_test_address(1,2))).unwrap().activation.unwrap();
    assert_eq!(activation.desk_id,None);assert_eq!(activation.surface,light_playback::PlaybackActivationSurface::Matter);
    persist_active_playbacks(&state).unwrap();
    let checkpoint=state.installation.setting(&active_playbacks_setting(show.id)).unwrap().unwrap();
    state.output.execute_playback(EnginePlaybackCommand::RestoreActive(serde_json::from_str(&checkpoint).unwrap())).unwrap();
    let enabled=state.output.playback_runtime().into_iter().filter(|playback|playback.enabled).filter_map(|playback|playback.playback_address).collect::<HashSet<_>>();
    assert_eq!(enabled,HashSet::from([matter_test_address(1,2)]));
    let _=std::fs::remove_dir_all(data_dir);
}

#[test]
fn matter_master_controls_and_tracks_a_faderless_assignment() {
    let (state, data_dir) = test_state();
    state.installation.update_configuration(|configuration| configuration.matter_enabled = true);
    state
        .output.replace_snapshot(matter_test_snapshot())
        .unwrap();
    let endpoint = matter_test_endpoint(&state,1,7);
    let definition = state
        .output.snapshot()
        .playback_pages
        .iter()
        .find(|page|page.number == 1)
        .and_then(|page|page.assignments.get(&7))
        .cloned()
        .unwrap();

    let rejected = dispatch_playback_action(
        &state,
        matter_test_address(1,7),
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
            control_source: &matter_test_source(),
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
        color: None,
        },
    )
    .unwrap();
    let runtime = state.output.playback_runtime();
    let active = runtime
        .iter()
        .find(|playback| playback.playback_address == Some(matter_test_address(1,7)))
        .unwrap();
    assert!(active.enabled);
    assert!((active.master - 0.5).abs() < 0.001);
    assert!(
        (state
            .output
            .playback_runtime_status_at(matter_test_address(1,7))
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
        color: None,
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
        color: None,
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
            .find(|playback| playback.playback_address == Some(matter_test_address(1,7)))
            .unwrap()
            .master,
        1.0
    );

    state
        .output.execute_playback(EnginePlaybackCommand::Playback {
            address:matter_test_address(1,7),action:light_engine::PlaybackAction::Off,activation_origin:None,control_source:matter_test_source(),
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
            playback_pages:vec![light_playback::PlaybackPage {number:1,name:"Matter".into(),assignments:(vec![
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
            ]).into_iter().enumerate().map(|(index,definition)|((index+1) as u16,definition)).collect()}].into(),
            ..Default::default()
        })
        .unwrap();
    let original_configuration = state.installation.configuration();

    let activation = state.active_show.acquire_blocking();
    let rejected = apply_matter_playback_write(
        &state,
        matter_test_endpoint(&state,1,1),
        matter::MatterPlaybackWrite {
            on: None,
            level: Some(127),
        color: None,
        },
    )
    .unwrap_err();
    assert_eq!(rejected.status, StatusCode::CONFLICT);
    assert_eq!(state.output.group_master("front"), Some(1.0));
    drop(activation);

    apply_matter_playback_write(
        &state,
        matter_test_endpoint(&state,1,1),
        matter::MatterPlaybackWrite {
            on: None,
            level: Some(127),
        color: None,
        },
    )
    .unwrap();
    for playback in 2..=5 {
        let rejected = apply_matter_playback_write(
            &state,
            playback,
            matter::MatterPlaybackWrite {
                on: None,
                level: Some(127),
            color: None,
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
        (matter_values[&matter_test_address(1,1)].level - 0.5).abs() < 0.001,
        "Matter feedback uses the shared Group Master runtime level"
    );
    let endpoint = matter_test_endpoint(&state,1,1);
    let before_color = refresh_matter_bridge(&state);
    let color_write = matter::MatterColorWrite::ColorTemperature { mireds: 250 };
    let after_color = apply_matter_playback_write(
        &state,
        endpoint,
        matter::MatterPlaybackWrite {
            on: None,
            level: None,
            color: Some(color_write),
        },
    )
    .unwrap();
    assert_eq!(after_color.revision, before_color.revision + 1);
    assert_eq!(
        after_color
            .lights
            .iter()
            .find(|light| light.endpoint_id == endpoint)
            .unwrap()
            .color
            .unwrap()
            .mode,
        matter::MatterColorMode::ColorTemperature
    );
    let controller_color = state.output.group_color("front").unwrap();
    assert!((controller_color.x - color_write.xyz().x).abs() < 0.001);
    assert_eq!(refresh_matter_bridge(&state).revision, after_color.revision);

    let desk_color = light_core::Xyz {
        x: 0.25,
        y: 0.65,
        z: 0.1,
    };
    assert!(
        state
            .output
            .set_group_color("front", Some(desk_color))
            .unwrap()
    );
    let after_desk_color = refresh_matter_bridge(&state);
    assert_eq!(after_desk_color.revision, after_color.revision + 1);
    let mirrored = after_desk_color
        .lights
        .iter()
        .find(|light| light.endpoint_id == endpoint)
        .unwrap()
        .color
        .unwrap();
    assert_eq!(mirrored.mode, matter::MatterColorMode::ColorTemperature);
    assert_ne!(mirrored.color_temperature_mireds, 250);
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
