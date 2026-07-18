use super::*;

#[test]
fn preload_transition_uses_one_timestamp_and_programmer_fade_only_as_fallback() {
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(fixture, "intensity", 0.0));
    let mut second = Cue::new(2.0);
    second.changes.push(value(fixture, "intensity", 1.0));
    let cue_list = list(vec![first, second]);
    let id = cue_list.id;
    let started = chrono::DateTime::parse_from_rfc3339("2026-07-16T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let clock = Arc::new(light_core::ManualClock::new(started));
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.set_control_timing([120.0, 90.0, 60.0, 30.0, 15.0], 7_000);
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.go_playback(1).unwrap();

    let committed_at = started + ChronoDuration::milliseconds(750);
    clock.set(committed_at);
    let previous = engine
        .runtime()
        .into_iter()
        .find(|playback| playback.playback_number == Some(1))
        .map(|playback| (playback.enabled, playback.master));
    engine.go_playback(1).unwrap();
    engine
        .apply_preload_timing(1, "go", committed_at, 2_000, previous)
        .unwrap();

    let active = &engine.runtime()[0];
    assert_eq!(active.activated_at, committed_at);
    assert_eq!(active.transition_fade_fallback_millis, Some(2_000));
    assert!(
        (contribution_level(
            &engine,
            committed_at + ChronoDuration::milliseconds(1_000),
            fixture,
        ) - 0.5)
            .abs()
            < 0.01
    );
}

#[test]
fn explicit_cue_time_remains_authoritative_for_a_preload_transition() {
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(fixture, "intensity", 0.0));
    let mut second = Cue::new(2.0);
    second.fade_millis = 500;
    second.changes.push(value(fixture, "intensity", 1.0));
    let cue_list = list(vec![first, second]);
    let id = cue_list.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.go_playback(1).unwrap();
    let previous = engine
        .runtime()
        .into_iter()
        .find(|playback| playback.playback_number == Some(1))
        .map(|playback| (playback.enabled, playback.master));
    engine.go_playback(1).unwrap();
    engine
        .apply_preload_timing(1, "go", started, 2_000, previous)
        .unwrap();
    assert!(
        (contribution_level(
            &engine,
            started + ChronoDuration::milliseconds(250),
            fixture,
        ) - 0.5)
            .abs()
            < 0.01
    );
}

#[test]
fn pool_master_scales_intensity_without_scaling_ltp_attributes() {
    let fixture = FixtureId::new();
    let mut cue = Cue::new(1.0);
    cue.changes.push(value(fixture, "intensity", 1.0));
    cue.changes.push(value(fixture, "pan", 0.8));
    let list = list(vec![cue]);
    let id = list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.set_master(1, 0.5).unwrap();
    let values = engine.contributions();
    assert_eq!(
        values
            .iter()
            .find(|value| value.attribute.is_intensity())
            .unwrap()
            .value,
        AttributeValue::Normalized(0.5)
    );
    assert_eq!(
        values
            .iter()
            .find(|value| value.attribute.0 == "pan")
            .unwrap()
            .value,
        AttributeValue::Normalized(0.8)
    );
}

#[test]
fn virtual_master_controls_faderless_playback_without_adding_a_local_fader() {
    let fixture = FixtureId::new();
    let mut cue = Cue::new(1.0);
    cue.changes.push(value(fixture, "intensity", 1.0));
    let cue_list = list(vec![cue]);
    let cue_list_id = cue_list.id;
    let mut playback = definition(1, cue_list_id);
    playback.has_fader = false;
    playback.button_count = 1;
    playback.buttons = [
        PlaybackButtonAction::Toggle,
        PlaybackButtonAction::None,
        PlaybackButtonAction::None,
    ];
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(playback).unwrap();

    assert_eq!(
        engine.set_master(1, 0.5),
        Err("playback does not have a fader".into())
    );
    engine.set_virtual_master(1, 0.5).unwrap();
    let runtime = &engine.runtime()[0];
    assert!(runtime.enabled);
    assert_eq!(runtime.master, 0.5);
    assert_eq!(runtime.fader_position, 0.5);
    assert_eq!(
        engine.contributions()[0].value,
        AttributeValue::Normalized(0.5)
    );
}

#[test]
fn virtual_master_drives_faderless_manual_xfade_without_enabling_local_input() {
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(fixture, "intensity", 0.0));
    let mut second = Cue::new(2.0);
    second.changes.push(value(fixture, "intensity", 1.0));
    let cue_list = list(vec![first, second]);
    let cue_list_id = cue_list.id;
    let mut playback = definition(1, cue_list_id);
    playback.fader = PlaybackFaderMode::XFade;
    playback.has_fader = false;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(playback).unwrap();

    assert_eq!(
        engine.set_master(1, 0.5),
        Err("playback does not have a fader".into())
    );
    assert_eq!(
        engine.set_manual_xfade(1, 0.5),
        Err("playback is not configured for manual X-fade".into())
    );
    engine.set_virtual_master(1, 0.5).unwrap();
    let runtime = &engine.runtime()[0];
    assert!(runtime.enabled);
    assert_eq!(runtime.fader_position, 0.5);
    assert_eq!(runtime.manual_xfade_position, 0.5);
    assert_eq!(runtime.manual_xfade_progress, 0.5);
    assert_eq!(
        resolve(engine.contributions())[&(fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(0.5)
    );
}

#[test]
fn full_newer_playback_auto_offs_covered_playback_but_99_percent_does_not() {
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(fixture, "pan", 0.2));
    let mut second = Cue::new(1.0);
    second.changes.push(value(fixture, "pan", 0.8));
    let first = list(vec![first]);
    let first_id = first.id;
    let second = list(vec![second]);
    let second_id = second.id;
    let mut engine = PlaybackEngine::default();
    engine.register(first).unwrap();
    engine.register(second).unwrap();
    engine.register_definition(definition(1, first_id)).unwrap();
    engine
        .register_definition(definition(2, second_id))
        .unwrap();
    engine.on(1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    engine.set_master(2, 0.99).unwrap();
    assert_eq!(engine.active().len(), 2);
    engine.set_master(2, 1.0).unwrap();
    assert_eq!(engine.active().len(), 1);
    assert_eq!(engine.active()[0].playback_number, Some(2));
}

#[test]
fn page_and_pool_validation_enforce_public_ranges() {
    let fixture = FixtureId::new();
    let mut cue = Cue::new(1.0);
    cue.changes.push(value(fixture, "pan", 0.1));
    let list = list(vec![cue]);
    let mut invalid = definition(1001, list.id);
    assert!(invalid.validate().is_err());
    invalid.number = 1;
    assert!(invalid.validate().is_ok());
    assert!(
        PlaybackPage {
            number: 0,
            name: "Bad".into(),
            slots: HashMap::new()
        }
        .validate()
        .is_err()
    );
    assert!(
        PlaybackPage {
            number: 127,
            name: "Last".into(),
            slots: HashMap::from([(127, 1000)])
        }
        .validate()
        .is_ok()
    );
}

#[test]
fn toggle_retains_cue_and_flash_restores_off_state() {
    let fixture = FixtureId::new();
    let mut one = Cue::new(1.0);
    one.changes.push(value(fixture, "pan", 0.1));
    let mut two = Cue::new(2.0);
    two.changes.push(value(fixture, "pan", 0.2));
    let mut list = list(vec![one, two]);
    list.restart_mode = RestartMode::ContinueCurrentCue;
    let id = list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();
    assert_eq!(engine.active()[0].cue_index, 1);
    assert!(!engine.toggle(1).unwrap());
    assert!(engine.active().is_empty());
    engine.set_flash(1, true).unwrap();
    assert_eq!(engine.active()[0].cue_index, 1);
    engine.set_flash(1, false).unwrap();
    assert!(engine.active().is_empty());
    assert!(engine.toggle(1).unwrap());
    assert_eq!(engine.active()[0].cue_index, 1);
}

#[test]
fn legacy_layout_defaults_are_target_specific_and_invalid_layouts_are_rejected() {
    let cue_list_id = CueListId::new();
    let legacy = serde_json::json!({
        "number": 1,
        "name": "Legacy",
        "target": { "type": "cue_list", "cue_list_id": cue_list_id }
    });
    let definition: PlaybackDefinition = serde_json::from_value(legacy).unwrap();
    assert_eq!(
        definition.buttons,
        [
            PlaybackButtonAction::GoMinus,
            PlaybackButtonAction::Go,
            PlaybackButtonAction::Flash,
        ]
    );
    assert_eq!(definition.fader, PlaybackFaderMode::Master);
    assert_eq!(definition.button_count, 3);
    assert!(definition.has_fader);

    let mut pausable = definition.clone();
    pausable.buttons[2] = PlaybackButtonAction::Pause;
    assert!(pausable.validate().is_ok());

    let mut incompatible = definition.clone();
    incompatible.target = PlaybackTarget::GrandMaster;
    assert!(incompatible.validate().is_err());
    incompatible.reset_incompatible_layout();
    assert_eq!(
        incompatible.buttons,
        [
            PlaybackButtonAction::Blackout,
            PlaybackButtonAction::PauseDynamics,
            PlaybackButtonAction::Flash,
        ]
    );
    assert!(incompatible.validate().is_ok());

    assert_eq!(
        PlaybackDefinition::default_buttons(&PlaybackTarget::Group {
            group_id: "front".into(),
        }),
        [
            PlaybackButtonAction::Select,
            PlaybackButtonAction::SelectDereferenced,
            PlaybackButtonAction::Flash,
        ]
    );
    for target in [PlaybackTarget::ProgrammerFade, PlaybackTarget::CueFade] {
        assert_eq!(
            PlaybackDefinition::default_buttons(&target),
            [
                PlaybackButtonAction::Double,
                PlaybackButtonAction::Half,
                PlaybackButtonAction::Off,
            ]
        );
    }

    incompatible.button_count = 1;
    incompatible.buttons[1] = PlaybackButtonAction::None;
    incompatible.buttons[2] = PlaybackButtonAction::None;
    incompatible.has_fader = false;
    incompatible.presentation_icon = Some("star".into());
    assert!(incompatible.validate().is_ok());
    incompatible.presentation_image = Some("asset://background".into());
    assert!(incompatible.validate().is_err());
}
