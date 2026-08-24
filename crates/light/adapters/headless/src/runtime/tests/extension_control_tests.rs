use super::*;
use light_extensions_contract::{
    CanonicalControlIntent, ControlInput, ControlInputEvent, FeedbackControlState,
    HighlightControlAction, LampState, ModifierKey, NavigationAction, PlaybackControl,
    ProgrammerKey,
};
use light_extensions_host::{BoundControlInput, HostControlContext, TimecodeEnvelope};

#[test]
fn canonical_extension_highlight_feedback_tracks_active_navigation_state() {
    let active = light_programmer::HighlightState {
        active: true,
        mode: light_programmer::HighlightMode::Selection,
        output_enabled: true,
        capture_only: false,
        remembered: Vec::new(),
        active_index: None,
        active_fixture: None,
        can_previous: true,
        can_next: true,
        owner_user_id: None,
        owner_user_name: None,
        message: None,
    };
    for (action, expected_enabled, expected_selected, expected_lamp) in [
        (HighlightControlAction::Toggle, true, true, LampState::On),
        (HighlightControlAction::Previous, true, false, LampState::On),
        (HighlightControlAction::Next, true, false, LampState::On),
        (HighlightControlAction::All, true, true, LampState::On),
    ] {
        let mut feedback = FeedbackControlState::default();
        extensions_runtime::apply_highlight_feedback(&mut feedback, Some(&active), action);
        assert!(feedback.available);
        assert_eq!(feedback.enabled, expected_enabled, "{action:?}");
        assert_eq!(feedback.selected, expected_selected, "{action:?}");
        assert_eq!(feedback.lamp, expected_lamp, "{action:?}");
    }

    let inactive = light_programmer::HighlightState {
        active: false,
        output_enabled: false,
        ..active
    };
    for action in [
        HighlightControlAction::Previous,
        HighlightControlAction::Next,
        HighlightControlAction::All,
    ] {
        let mut feedback = FeedbackControlState::default();
        extensions_runtime::apply_highlight_feedback(&mut feedback, Some(&inactive), action);
        assert!(!feedback.enabled, "{action:?} is active-only");
        assert!(!feedback.selected, "{action:?}");
        assert_eq!(feedback.lamp, LampState::Off, "{action:?}");
    }
}

#[test]
fn canonical_extension_controls_use_authoritative_output_and_playback_services() {
    let (state, data_dir) = test_state();
    let desk = state.installation.add_desk("Main").unwrap();
    let host = HostControlContext {
        extension_id: "de.tosklight.surface".into(),
        extension_instance_id: "main-surface".into(),
        desk_id: desk.id.to_string(),
        source: "native_extension",
    };

    extensions_runtime::apply_bound_control(
        &state,
        &host,
        &BoundControlInput {
            input: ControlInputEvent {
                input_id: 1,
                occurred_at_micros: 100,
                control: ControlInput::Absolute {
                    control_id: "grand-master".into(),
                    value: 0.42,
                },
            },
            intent: CanonicalControlIntent::GrandMaster,
        },
    )
    .unwrap();
    assert_eq!(state.output.control_projection().grand_master, 0.42);

    extensions_runtime::apply_bound_control(
        &state,
        &host,
        &BoundControlInput {
            input: ControlInputEvent {
                input_id: 2,
                occurred_at_micros: 101,
                control: ControlInput::Button {
                    control_id: "blackout".into(),
                    pressed: true,
                },
            },
            intent: CanonicalControlIntent::Blackout,
        },
    )
    .unwrap();
    assert!(state.output.control_projection().blackout);

    let cue_list = light_playback::CueList {
        id: light_core::CueListId::new(),
        name: "Extension CueList".into(),
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
    let cue_list_id = cue_list.id;
    let playback =
        light_playback::PlaybackDefinition::new_cue_list(17, "Extension playback", cue_list_id);
    let page = light_playback::PlaybackPage {
        number: 2,
        name: "Extension page".into(),
        slots: HashMap::from([(3, 17)]),
        virtual_playbacks: HashMap::new(),
    };
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list].into(),
            playbacks: vec![playback].into(),
            playback_pages: vec![page].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    extensions_runtime::apply_bound_control(
        &state,
        &host,
        &BoundControlInput {
            input: ControlInputEvent {
                input_id: 3,
                occurred_at_micros: 102,
                control: ControlInput::Button {
                    control_id: "page-two-go".into(),
                    pressed: true,
                },
            },
            intent: CanonicalControlIntent::PlaybackExplicit {
                page: 2,
                slot: 3,
                control: PlaybackControl::ButtonTwo,
            },
        },
    )
    .unwrap();
    assert_eq!(
        state.output.active_playbacks()[0].current_cue_number,
        Some(cue("1"))
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn canonical_extension_surface_events_match_the_attached_hardware_desk_action_shape() {
    let (state, data_dir) = test_state();
    let desk = state.installation.add_desk("Main").unwrap();
    let host = HostControlContext {
        extension_id: "de.tosklight.surface".into(),
        extension_instance_id: "main-surface".into(),
        desk_id: desk.id.to_string(),
        source: "native_extension",
    };
    let apply = |input_id, control, intent| {
        extensions_runtime::apply_bound_control(
            &state,
            &host,
            &BoundControlInput {
                input: ControlInputEvent {
                    input_id,
                    occurred_at_micros: 100 + input_id,
                    control,
                },
                intent,
            },
        )
        .unwrap();
    };

    apply(
        1,
        ControlInput::Button {
            control_id: "nav-down".into(),
            pressed: true,
        },
        CanonicalControlIntent::Navigation {
            action: NavigationAction::Down,
        },
    );
    apply(
        2,
        ControlInput::Relative {
            control_id: "encoder-two".into(),
            delta: 1,
        },
        CanonicalControlIntent::Encoder { index: 2 },
    );
    let events = state
        .events
        .audit_events()
        .into_iter()
        .filter(|event| event.kind == "desk_action")
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].payload["control"], "nav");
    assert_eq!(events[0].payload["value"], "down");
    assert_eq!(events[1].payload["control"], "encode/2");
    assert_eq!(events[1].payload["value"], "up");
    for event in events {
        assert_eq!(event.payload["path"], "desk");
        assert_eq!(event.payload["desk_id"], serde_json::json!(desk.id));
        assert_eq!(event.payload["source"], "extension");
        assert!(
            event.payload["value"].is_null() || event.payload["value"].is_string(),
            "DeskActionNotification only accepts string values"
        );
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn canonical_extension_programmer_highlight_and_speed_controls_use_server_authority() {
    let (state, data_dir) = test_state();
    let desk = state.installation.add_desk("Main").unwrap();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: light_core::SessionId::new(),
        user: user.clone(),
        token: "extension-controls".into(),
        connected: true,
        desk: desk.clone(),
    };
    state.programming.start(session.id, user.id);
    state.sessions.insert_session(session.clone());
    let host = HostControlContext {
        extension_id: "de.tosklight.surface".into(),
        extension_instance_id: "main-surface".into(),
        desk_id: desk.id.to_string(),
        source: "native_extension",
    };
    let apply = |input_id, control, intent| {
        extensions_runtime::apply_bound_control(
            &state,
            &host,
            &BoundControlInput {
                input: ControlInputEvent {
                    input_id,
                    occurred_at_micros: 200 + input_id,
                    control,
                },
                intent,
            },
        )
        .unwrap();
    };

    apply(
        0,
        ControlInput::Button {
            control_id: "set".into(),
            pressed: true,
        },
        CanonicalControlIntent::ProgrammerKey {
            key: ProgrammerKey::Set,
        },
    );
    assert!(state.events.audit_events().iter().any(|event| {
        event.kind == "desk_action"
            && event.payload["action"] == "set"
            && event.payload["source"] == "extension"
    }));
    apply(
        1,
        ControlInput::Button {
            control_id: "one".into(),
            pressed: true,
        },
        CanonicalControlIntent::ProgrammerKey {
            key: ProgrammerKey::One,
        },
    );
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "F1"
    );
    apply(
        2,
        ControlInput::Button {
            control_id: "shift".into(),
            pressed: true,
        },
        CanonicalControlIntent::Modifier {
            modifier: ModifierKey::Shift,
        },
    );
    apply(
        3,
        ControlInput::Button {
            control_id: "clear".into(),
            pressed: true,
        },
        CanonicalControlIntent::ProgrammerKey {
            key: ProgrammerKey::Clear,
        },
    );
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "FREEZE"
    );
    apply(
        4,
        ControlInput::Button {
            control_id: "clear".into(),
            pressed: true,
        },
        CanonicalControlIntent::ProgrammerKey {
            key: ProgrammerKey::Clear,
        },
    );
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "UNFREEZE"
    );
    apply(
        5,
        ControlInput::Button {
            control_id: "shift".into(),
            pressed: false,
        },
        CanonicalControlIntent::Modifier {
            modifier: ModifierKey::Shift,
        },
    );
    let key_phases = state
        .events
        .audit_events()
        .into_iter()
        .filter(|event| {
            event.kind == "command_key_phase"
                && event.payload["source"] == "extension"
                && event.payload["key"] == "SHIFT"
        })
        .map(|event| event.payload["phase"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(key_phases, ["press", "release"]);
    apply(
        6,
        ControlInput::Button {
            control_id: "clear".into(),
            pressed: true,
        },
        CanonicalControlIntent::ProgrammerKey {
            key: ProgrammerKey::Clear,
        },
    );
    assert_eq!(state.programming.get(session.id).unwrap().command_line, "");

    for (offset, action) in [
        HighlightControlAction::Toggle,
        HighlightControlAction::Next,
        HighlightControlAction::Previous,
        HighlightControlAction::All,
    ]
    .into_iter()
    .enumerate()
    {
        let input_id = 4 + (offset as u64 * 2);
        let control_id = format!("highlight-{action:?}").to_ascii_lowercase();
        apply(
            input_id,
            ControlInput::Button {
                control_id: control_id.clone(),
                pressed: true,
            },
            CanonicalControlIntent::Highlight { action },
        );
        apply(
            input_id + 1,
            ControlInput::Button {
                control_id,
                pressed: false,
            },
            CanonicalControlIntent::Highlight { action },
        );
    }
    let highlight_actions = state
        .events
        .audit_events()
        .into_iter()
        .filter(|event| event.kind == "highlight_changed" && event.payload["source"] == "extension")
        .map(|event| event.payload["action"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        highlight_actions,
        ["toggle", "next", "previous", "all"],
        "only pressed edges route the four canonical Highlight actions"
    );

    apply(
        12,
        ControlInput::Absolute {
            control_id: "speed-a-level".into(),
            value: 0.25,
        },
        CanonicalControlIntent::SpeedGroup {
            group: 'A',
            control: light_extensions_contract::SpeedGroupControl::Level,
        },
    );
    assert_eq!(
        state
            .output
            .speed_group_snapshot(0, application_millis(&state))
            .speed_master_scale,
        0.25
    );
    assert!(state.events.audit_events().iter().any(|event| {
        event.kind == "speed_group_action"
            && event.payload["source"] == "extension"
            && event.payload["action"] == "level"
    }));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn extension_programmer_keys_do_not_cross_an_active_show_transition() {
    let (state, data_dir) = test_state();
    let desk = state.installation.add_desk("Main").unwrap();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: light_core::SessionId::new(),
        user: user.clone(),
        token: "extension-transition".into(),
        connected: true,
        desk: desk.clone(),
    };
    state.programming.start(session.id, user.id);
    state.sessions.insert_session(session.clone());
    let host = HostControlContext {
        extension_id: "de.tosklight.surface".into(),
        extension_instance_id: "main-surface".into(),
        desk_id: desk.id.to_string(),
        source: "native_extension",
    };
    let transition = state.active_show.acquire().await;

    let error = extensions_runtime::apply_bound_control(
        &state,
        &host,
        &BoundControlInput {
            input: ControlInputEvent {
                input_id: 1,
                occurred_at_micros: 200,
                control: ControlInput::Button {
                    control_id: "one".into(),
                    pressed: true,
                },
            },
            intent: CanonicalControlIntent::ProgrammerKey {
                key: ProgrammerKey::One,
            },
        },
    )
    .unwrap_err();

    assert!(error.detail.contains("active show is changing"));
    assert_eq!(state.programming.get(session.id).unwrap().command_line, "");
    drop(transition);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn extension_timecode_uses_the_exact_selected_authoritative_source() {
    let (state, data_dir) = test_state();
    let mut configuration = DeskConfiguration::default();
    configuration.timecode_source = TimecodeSourceSelection::External {
        source: "extension:de.tosklight.timecode:mtc-one".into(),
    };
    configuration.timecode_frame_rate = Some(DeskTimecodeFrameRate {
        numerator: 25,
        denominator: 1,
        drop_frame: false,
    });
    state
        .output
        .configure_timecode(configuration.timecode_router_config());
    extensions_runtime::ingest_extension_timecode(
        &state,
        &TimecodeEnvelope {
            extension_id: "de.tosklight.timecode".into(),
            extension_instance_id: "mtc-one".into(),
            received_at_micros: 1_800_000_000_000_000,
            sample: light_extensions_contract::TimecodeSample {
                sample_id: 1,
                observed_at_micros: 100,
                hours: 1,
                minutes: 2,
                seconds: 3,
                frames: 4,
                rate: light_extensions_contract::TimecodeRate::Fps25,
                drop_frame: false,
            },
        },
    )
    .unwrap();

    let (source, timecode) = state.output.timecode_status();
    assert_eq!(
        source.as_deref(),
        Some("extension:de.tosklight.timecode:mtc-one")
    );
    assert_eq!(timecode.unwrap().frames, 4);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn extension_feedback_context_isolates_desks_and_carries_show_generation() {
    let (state, data_dir) = test_state();
    let front = state.installation.add_desk("Front").unwrap();
    let wing = state.installation.add_desk("Wing").unwrap();
    let show = ShowEntry {
        id: light_core::ShowId::new(),
        name: "Feedback show".into(),
        path: data_dir.join("feedback.show").display().to_string(),
        revision: 19,
        updated_at: String::new(),
        revision_copy: None,
    };
    state.active_show.replace_current(Some(show.clone()));
    let context = |desk: &ControlDesk| HostControlContext {
        extension_id: "de.tosklight.surface".into(),
        extension_instance_id: format!("{}-surface", desk.id),
        desk_id: desk.id.to_string(),
        source: "native_extension",
    };

    let front_context = extensions_runtime::feedback_context(Some(&state), &context(&front));
    let wing_context = extensions_runtime::feedback_context(Some(&state), &context(&wing));
    assert_eq!(front_context.desk_id, front.id.to_string());
    assert_eq!(wing_context.desk_id, wing.id.to_string());
    assert_ne!(front_context.desk_id, wing_context.desk_id);
    let show_id = show.id.0.to_string();
    for feedback in [front_context, wing_context] {
        assert_eq!(feedback.show_id.as_deref(), Some(show_id.as_str()));
        assert_eq!(feedback.show_generation, 19);
    }
    let _ = std::fs::remove_dir_all(data_dir);
}
