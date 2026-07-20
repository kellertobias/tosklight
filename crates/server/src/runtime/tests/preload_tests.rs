fn test_control_desk() -> ControlDesk {
    ControlDesk {
        id: Uuid::nil(),
        name: "Test desk".into(),
        osc_alias: "test-desk".into(),
        columns: 8,
        rows: 1,
        buttons: 3,
        playback_layout: None,
    }
}

fn preload_test_playback(
    buttons: [light_playback::PlaybackButtonAction; 3],
) -> light_playback::PlaybackDefinition {
    light_playback::PlaybackDefinition {
        number: 1,
        name: "Preload test".into(),
        target: light_playback::PlaybackTarget::CueList {
            cue_list_id: light_core::CueListId::new(),
        },
        buttons,
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}

fn preload_atomicity_test_snapshot() -> EngineSnapshot {
    let first_cue_list_id = light_core::CueListId::new();
    let second_cue_list_id = light_core::CueListId::new();
    let cue_list = |id, name: &str| light_playback::CueList {
        id,
        name: name.into(),
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
    };
    let playback = |number, target| light_playback::PlaybackDefinition {
        number,
        name: format!("Atomic Preload {number}"),
        target,
        buttons: [light_playback::PlaybackButtonAction::None; 3],
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    EngineSnapshot {
        groups: vec![light_programmer::GroupDefinition {
            id: "front".into(),
            name: "Front".into(),
            ..Default::default()
        }],
        cue_lists: vec![
            cue_list(first_cue_list_id, "Atomic Preload A"),
            cue_list(second_cue_list_id, "Atomic Preload B"),
        ],
        playbacks: vec![
            playback(
                1,
                light_playback::PlaybackTarget::CueList {
                    cue_list_id: first_cue_list_id,
                },
            ),
            playback(
                2,
                light_playback::PlaybackTarget::CueList {
                    cue_list_id: second_cue_list_id,
                },
            ),
            playback(
                3,
                light_playback::PlaybackTarget::Group {
                    group_id: "front".into(),
                },
            ),
        ],
        ..Default::default()
    }
}

fn matter_test_snapshot() -> EngineSnapshot {
    let cue_list_id = light_core::CueListId::new();
    let cue_list = light_playback::CueList {
        id: cue_list_id,
        name: "Matter look".into(),
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
    };
    let definition = |number, has_fader| light_playback::PlaybackDefinition {
        number,
        name: format!("Matter playback {number}"),
        target: light_playback::PlaybackTarget::CueList { cue_list_id },
        buttons: [light_playback::PlaybackButtonAction::None; 3],
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
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
    EngineSnapshot {
        cue_lists: vec![cue_list],
        playbacks: vec![definition(25, true), definition(26, false)],
        playback_pages: vec![
            light_playback::PlaybackPage {
                number: 1,
                name: "Main".into(),
                slots: HashMap::from([(7, 26)]),
            },
            light_playback::PlaybackPage {
                number: 4,
                name: "Matter".into(),
                slots: HashMap::from([(7, 25)]),
            },
        ],
        ..Default::default()
    }
}

#[test]
fn preload_capture_resolves_real_buttons_canonicalizes_temp_and_excludes_live_controls() {
    use light_playback::PlaybackButtonAction as Action;
    let playback = preload_test_playback([Action::Toggle, Action::Flash, Action::Go]);
    let button = |number, pressed| PoolPlaybackInput {
        button: Some(number),
        pressed: Some(pressed),
        ..PoolPlaybackInput::default()
    };
    assert_eq!(
        preload_capture_action(&playback, "button", &button(1, true)).unwrap(),
        Some("toggle")
    );
    assert_eq!(
        preload_capture_action(&playback, "button", &button(2, true)).unwrap(),
        None
    );
    assert_eq!(
        preload_capture_action(&playback, "button", &button(3, true)).unwrap(),
        Some("go")
    );
    assert_eq!(
        preload_capture_action(&playback, "button", &button(1, false)).unwrap(),
        None
    );
    let temp_playback = preload_test_playback([Action::Temp, Action::Flash, Action::Go]);
    assert_eq!(
        preload_capture_action_with_temp_state(&temp_playback, "button", &button(1, true), false,)
            .unwrap(),
        Some("temp-on")
    );
    assert_eq!(
        preload_capture_action_with_temp_state(&temp_playback, "button", &button(1, true), true,)
            .unwrap(),
        Some("temp-off")
    );
    assert_eq!(
        preload_capture_action(&playback, "temp-off", &button(1, false)).unwrap(),
        Some("temp-off")
    );
    assert_eq!(
        preload_capture_action(
            &playback,
            "master",
            &PoolPlaybackInput {
                value: Some(0.5),
                ..PoolPlaybackInput::default()
            }
        )
        .unwrap(),
        None
    );
    for (requested, retained) in [
        ("toggle", "toggle"),
        ("go", "go"),
        ("go-minus", "go-minus"),
        ("back", "go-minus"),
        ("off", "off"),
        ("on", "on"),
        ("temp-on", "temp-on"),
        ("temp-off", "temp-off"),
    ] {
        assert_eq!(
            preload_capture_action(&playback, requested, &PoolPlaybackInput::default()).unwrap(),
            Some(retained)
        );
    }
}

#[test]
fn preload_rejects_a_late_invalid_action_without_publishing_earlier_actions() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "atomic-preload".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    state
        .engine
        .replace_snapshot(preload_atomicity_test_snapshot())
        .unwrap();
    assert!(state.programmers.arm_preload(session.id, true));
    assert!(state.programmers.queue_preload_playback_action(
        session.id,
        1,
        light_programmer::PreloadPlaybackQueueAction::Go,
        light_programmer::PreloadPlaybackQueueSurface::Physical,
    ));
    assert!(state.programmers.queue_preload_playback_action(
        session.id,
        3,
        light_programmer::PreloadPlaybackQueueAction::On,
        light_programmer::PreloadPlaybackQueueSurface::Virtual,
    ));
    let programmer_before = state.programmers.get(session.id).unwrap();

    let error = commit_preload(&state, &session).unwrap_err();

    assert!(error.contains("group playback"), "{error}");
    assert!(state.engine.playback_runtime().is_empty());
    let programmer_after = state.programmers.get(session.id).unwrap();
    assert_eq!(
        programmer_after.preload_playback_pending,
        programmer_before.preload_playback_pending
    );
    assert_eq!(programmer_after.blind, programmer_before.blind);
    assert!(
        state
            .audit_events
            .lock()
            .iter()
            .all(|event| event.kind != "preload_committed")
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn committed_preload_publishes_the_exact_typed_playback_change() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "typed-preload".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    state
        .engine
        .replace_snapshot(preload_atomicity_test_snapshot())
        .unwrap();
    assert!(state.programmers.arm_preload(session.id, true));
    assert!(state.programmers.queue_preload_playback_action(
        session.id,
        1,
        light_programmer::PreloadPlaybackQueueAction::Go,
        light_programmer::PreloadPlaybackQueueSurface::Physical,
    ));

    let response = commit_preload(&state, &session).unwrap();

    assert_eq!(response["playback_event_sequences"], serde_json::json!([1]));
    let light_application::EventReplay::Events(events) = state.application_events.replay(
        0,
        &light_application::EventFilter::default(),
    ) else {
        panic!("committed Preload should retain its semantic event");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::UserInterface)
    );
    let light_application::ApplicationEvent::Playback(
        light_application::PlaybackEvent::RuntimeChanged(change),
    ) = &events[0].payload
    else {
        panic!("expected a typed Playback runtime change");
    };
    assert_eq!(change.projection.playback_number, Some(1));
    assert_eq!(
        change.transition.as_ref().map(|transition| transition.cause),
        Some(light_application::PlaybackTransitionCause::Go)
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn staged_preload_applies_exclusions_without_mutating_the_source_engine() {
    let (state, data_dir) = test_state();
    state
        .engine
        .replace_snapshot(preload_atomicity_test_snapshot())
        .unwrap();
    state
        .engine
        .execute_playback(EnginePlaybackCommand::Pool {
            number: 2,
            action: PoolPlaybackAction::On,
        })
        .unwrap();
    let pending = light_programmer::PreloadPlaybackAction {
        playback_number: 1,
        action: light_programmer::PreloadPlaybackQueueAction::On,
        surface: light_programmer::PreloadPlaybackQueueSurface::Virtual,
    };
    let source = state.engine.playback_runtime();
    let pending = vec![pending];
    let commands = preload_batch_commands(&pending).unwrap();
    let prepared = state.engine.prepare_playback_batch(
        &commands,
        chrono::Utc::now(),
        0,
        &[vec![1, 2]],
    )
    .unwrap();
    let actions = staged_preload_actions(&pending, &prepared);

    assert!(
        source
            .iter()
            .any(|runtime| { runtime.playback_number == Some(2) && runtime.enabled })
    );
    assert!(
        source
            .iter()
            .all(|runtime| runtime.playback_number != Some(1))
    );
    let unchanged = state.engine.playback_runtime();
    assert!(unchanged.iter().any(|runtime| {
        runtime.playback_number == Some(2) && runtime.enabled
    }));
    assert!(
        unchanged
            .iter()
            .all(|runtime| runtime.playback_number != Some(1))
    );
    state
        .engine
        .install_prepared_playback_batch(prepared)
        .unwrap();
    let result = state.engine.playback_runtime();
    assert!(
        result
            .iter()
            .any(|runtime| { runtime.playback_number == Some(1) && runtime.enabled })
    );
    assert!(
        result
            .iter()
            .any(|runtime| { runtime.playback_number == Some(2) && !runtime.enabled })
    );
    assert_eq!(actions[0].released_playbacks, vec![2]);
    let _ = std::fs::remove_dir_all(data_dir);
}
