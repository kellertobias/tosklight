use super::*;

#[test]
fn mapped_cue_action_uses_playback_service_and_publishes_one_midi_event() {
    let (state, data_dir) = test_state();
    let cue_list = mapped_test_cue_list();
    let cue_list_id = cue_list.id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list],
            control_mappings: vec![midi_mapping(ControlAction::CueGo { cue_list_id })],
            ..EngineSnapshot::default()
        })
        .unwrap();

    handle_control_event(
        &state,
        ControlEvent::Midi {
            status: 144,
            data: vec![7, 127],
        },
    );

    let active = state.engine.active_playbacks();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].current_cue_number, Some(1.0));
    let light_application::EventReplay::Events(events) = state.application_events.replay(
        0,
        &light_application::EventFilter::default()
            .with_object(light_application::EventObject::cue_list(cue_list_id.0)),
    ) else {
        panic!("mapped Cue event should be retained");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].desk_id, None);
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Midi)
    );
    assert!(matches!(
        &events[0].payload,
        light_application::ApplicationEvent::Playback(_)
    ));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mapped_global_output_respects_the_osc_desk_alias_lock() {
    let (state, data_dir) = test_state();
    let wing = state.desk.lock().add_desk("Wing", "wing").unwrap();
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            control_mappings: vec![
                light_control::ControlMapping {
                    name: "Wing blackout".into(),
                    enabled: true,
                    trigger: light_control::ControlTrigger::Osc {
                        address: "/light/wing/mapped-blackout".into(),
                    },
                    action: ControlAction::Blackout { enabled: true },
                },
                light_control::ControlMapping {
                    name: "Wing Grand Master".into(),
                    enabled: true,
                    trigger: light_control::ControlTrigger::Osc {
                        address: "/light/wing/mapped-blackout".into(),
                    },
                    action: ControlAction::GrandMaster { level: 0.35 },
                },
            ],
            ..EngineSnapshot::default()
        })
        .unwrap();
    write_desk_lock(
        &state,
        wing.id,
        &DeskLockConfiguration {
            locked: true,
            ..DeskLockConfiguration::default()
        },
    )
    .unwrap();

    let event = || ControlEvent::Osc {
        address: "/light/wing/mapped-blackout".into(),
        arguments: vec![OscArgument::Bool(true)],
        source: Some("127.0.0.1:19000".into()),
    };
    handle_control_event(&state, event());
    assert!(!state.output_control.lock().options.blackout);
    assert_eq!(state.application_events.latest_sequence(), 0);

    write_desk_lock(&state, wing.id, &DeskLockConfiguration::default()).unwrap();
    handle_control_event(&state, event());
    assert!(state.output_control.lock().options.blackout);
    assert_eq!(state.output_control.lock().options.grand_master, 0.35);
    assert_eq!(state.application_events.latest_sequence(), 1);
    let light_application::EventReplay::Events(events) = state.application_events.replay(
        0,
        &light_application::EventFilter::default()
            .with_object(light_application::EventObject::global_output()),
    ) else {
        panic!("mapped output event should be retained");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].desk_id, None);
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Osc)
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn configured_grand_master_playback_remains_a_playback_event() {
    let (state, data_dir) = test_state();
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            playbacks: vec![mapped_test_playback(
                7,
                light_playback::PlaybackTarget::GrandMaster,
            )],
            ..EngineSnapshot::default()
        })
        .unwrap();
    let context = light_application::ActionContext::system(
        Uuid::nil(),
        light_application::ActionSource::Midi,
    );
    playback_service::execute(
        &state,
        None,
        None,
        context,
        light_application::PlaybackCommand {
            address: light_application::PlaybackAddress::Pool(7),
            action: light_application::PlaybackAction::Master(
                light_application::PlaybackLevel::new(0.6),
            ),
            surface: light_application::PlaybackSurface::Physical,
        },
    )
    .unwrap();

    let light_application::EventReplay::Events(events) = state.application_events.replay(
        0,
        &light_application::EventFilter::default()
            .with_object(light_application::EventObject::playback(7)),
    ) else {
        panic!("Grand Master playback event should be retained");
    };
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0].payload,
        light_application::ApplicationEvent::Playback(_)
    ));
    let light_application::EventReplay::Events(output_events) = state.application_events.replay(
        0,
        &light_application::EventFilter::default()
            .with_object(light_application::EventObject::global_output()),
    ) else {
        panic!("complete retained history should replay");
    };
    assert!(output_events.is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

fn midi_mapping(action: ControlAction) -> light_control::ControlMapping {
    light_control::ControlMapping {
        name: "Mapped Cue".into(),
        enabled: true,
        trigger: light_control::ControlTrigger::Midi {
            status: 144,
            data1: Some(7),
        },
        action,
    }
}

fn mapped_test_cue_list() -> light_playback::CueList {
    light_playback::CueList {
        id: light_core::CueListId::new(),
        name: "Mapped".into(),
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
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![light_playback::Cue::new(1.0)],
    }
}

fn mapped_test_playback(
    number: u16,
    target: light_playback::PlaybackTarget,
) -> light_playback::PlaybackDefinition {
    light_playback::PlaybackDefinition {
        number,
        name: format!("Playback {number}"),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        button_count: 3,
        fader: light_playback::PlaybackDefinition::default_fader(&target),
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
        target,
    }
}
