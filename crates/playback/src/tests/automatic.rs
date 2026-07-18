use super::*;

#[test]
fn follow_chaser_and_timecode_advance_without_manual_go() {
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(fixture, "intensity", 0.2));
    let mut second = Cue::new(2.0);
    second.trigger = CueTrigger::Follow { delay_millis: 100 };
    second.changes.push(value(fixture, "intensity", 0.8));
    let mut third = Cue::new(3.0);
    third.trigger = CueTrigger::Timecode { frame: 250 };
    let mut cue_list = list(vec![first, second, third]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list.clone()).unwrap();
    let started = Utc::now();
    engine.go_at(id, started).unwrap();
    engine.tick(started + ChronoDuration::milliseconds(99), None);
    assert_eq!(engine.active()[0].cue_index, 0);
    engine.tick(started + ChronoDuration::milliseconds(100), None);
    assert_eq!(engine.active()[0].cue_index, 1);
    engine.tick(started + ChronoDuration::milliseconds(100), Some(250));
    assert_eq!(engine.active()[0].cue_index, 2);
    engine.release(id);
    cue_list.mode = CueListMode::Chaser;
    cue_list.chaser_step_millis = 50;
    let mut chaser = PlaybackEngine::default();
    chaser.register(cue_list).unwrap();
    chaser.go_at(id, started).unwrap();
    chaser.tick(started + ChronoDuration::milliseconds(50), None);
    assert_eq!(chaser.active()[0].cue_index, 1);
    chaser.jump_at(id, 1.0, started).unwrap();
    chaser.tick(started, Some(250));
    assert_eq!(chaser.active()[0].cue_index, 2);
}

#[test]
fn legacy_looped_lists_migrate_to_tracking_wrap_defaults() {
    let mut encoded = serde_json::to_value(list(vec![Cue::new(1.0)])).unwrap();
    let object = encoded.as_object_mut().unwrap();
    for field in [
        "intensity_priority_mode",
        "wrap_mode",
        "restart_mode",
        "force_cue_timing",
        "disable_cue_timing",
        "chaser_xfade_millis",
        "chaser_xfade_percent",
        "speed_multiplier",
    ] {
        object.remove(field);
    }
    object.insert("looped".into(), true.into());
    let migrated: CueList = serde_json::from_value(encoded).unwrap();
    assert_eq!(migrated.effective_wrap_mode(), WrapMode::Tracking);
    assert_eq!(migrated.restart_mode, RestartMode::FirstCue);
    assert_eq!(migrated.intensity_priority_mode, IntensityPriorityMode::Htp);
    assert_eq!(migrated.speed_multiplier, 1.0);
}

#[test]
fn legacy_chaser_xfade_migrates_once_to_stable_integer_percent() {
    let mut legacy = list(vec![Cue::new(1.0)]);
    legacy.mode = CueListMode::Chaser;
    legacy.chaser_step_millis = 1_000;
    legacy.chaser_xfade_millis = 255;
    legacy.chaser_xfade_percent = None;
    let mut encoded = serde_json::to_value(&legacy).unwrap();
    encoded
        .as_object_mut()
        .unwrap()
        .remove("chaser_xfade_percent");

    let mut migrated: CueList = serde_json::from_value(encoded).unwrap();
    migrated.migrate_legacy_chaser_xfade(&[120.0, 90.0, 60.0, 30.0, 15.0]);
    assert_eq!(migrated.chaser_xfade_percent, Some(26));
    assert_eq!(migrated.chaser_xfade_millis, 0);
    let normalized = serde_json::to_value(&migrated).unwrap();
    assert_eq!(normalized["chaser_xfade_percent"], 26);
    assert!(normalized.get("chaser_xfade_millis").is_none());
    let reloaded: CueList = serde_json::from_value(normalized).unwrap();
    assert_eq!(reloaded.chaser_xfade_percent, Some(26));
}

#[test]
fn chaser_xfade_percent_tracks_live_step_duration_exactly() {
    let mut chaser = list(vec![Cue::new(1.0)]);
    chaser.mode = CueListMode::Chaser;
    chaser.speed_group = Some("A".into());
    chaser.chaser_xfade_percent = Some(50);
    assert_eq!(effective_chaser_xfade_millis(&chaser, &[120.0; 5]), 250);
    assert_eq!(effective_chaser_xfade_millis(&chaser, &[60.0; 5]), 500);
    chaser.speed_multiplier = 2.0;
    assert_eq!(effective_chaser_xfade_millis(&chaser, &[120.0; 5]), 125);
    chaser.chaser_xfade_percent = Some(0);
    assert_eq!(effective_chaser_xfade_millis(&chaser, &[120.0; 5]), 0);
    chaser.chaser_xfade_percent = Some(100);
    assert_eq!(effective_chaser_xfade_millis(&chaser, &[120.0; 5]), 250);
    chaser.disable_cue_timing = true;
    assert_eq!(effective_chaser_xfade_millis(&chaser, &[120.0; 5]), 0);
    assert_eq!(chaser.chaser_xfade_percent, Some(100));
}

#[test]
fn tracking_wrap_keeps_final_state_while_reset_wrap_releases_it() {
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(fixture, "intensity", 0.2));
    let mut second = Cue::new(2.0);
    second.changes.push(value(fixture, "pan", 0.7));
    let mut tracking = list(vec![first.clone(), second.clone()]);
    tracking.wrap_mode = Some(WrapMode::Tracking);
    let id = tracking.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(tracking).unwrap();
    engine.go_at(id, started).unwrap();
    engine.go_at(id, started).unwrap();
    engine.go_at(id, started).unwrap();
    assert!(
        engine
            .contributions_at(started)
            .iter()
            .any(|value| value.attribute.0 == "pan")
    );

    let mut reset = list(vec![first, second]);
    reset.wrap_mode = Some(WrapMode::Reset);
    let reset_id = reset.id;
    let mut reset_engine = PlaybackEngine::default();
    reset_engine.register(reset).unwrap();
    reset_engine.go_at(reset_id, started).unwrap();
    reset_engine.go_at(reset_id, started).unwrap();
    reset_engine.go_at(reset_id, started).unwrap();
    assert!(
        !reset_engine
            .contributions_at(started)
            .iter()
            .any(|value| value.attribute.0 == "pan")
    );
}

#[test]
fn deleting_the_active_cue_holds_output_and_anchors_navigation() {
    let fixture = FixtureId::new();
    let mut one = Cue::new(1.0);
    one.changes.push(value(fixture, "intensity", 0.1));
    let mut two = Cue::new(2.0);
    two.changes.push(value(fixture, "intensity", 0.6));
    let mut three = Cue::new(3.0);
    three.fade_millis = 1_000;
    three.changes.push(value(fixture, "intensity", 0.9));
    let original = list(vec![one.clone(), two, three.clone()]);
    let id = original.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(original).unwrap();
    engine.go_at(id, started).unwrap();
    engine.go_at(id, started).unwrap();
    let mut replacement = list(vec![one, three]);
    replacement.id = id;
    let active = engine.active_for_snapshot(&[replacement.clone()], started);
    assert_eq!(
        active[0].deleted_cue_hold.as_ref().unwrap().deleted_number,
        2.0
    );
    let mut replaced = PlaybackEngine::default();
    replaced.register(replacement).unwrap();
    replaced.restore_active(active);
    assert_eq!(contribution_level(&replaced, started, fixture), 0.6);
    replaced.go_at(id, started).unwrap();
    assert_eq!(replaced.active()[0].current_cue_number, Some(3.0));
    assert_eq!(contribution_level(&replaced, started, fixture), 0.6);
    assert!(
        (contribution_level(
            &replaced,
            started + ChronoDuration::milliseconds(500),
            fixture
        ) - 0.75)
            .abs()
            < 0.001
    );
    assert_eq!(
        contribution_level(
            &replaced,
            started + ChronoDuration::milliseconds(1_000),
            fixture
        ),
        0.9
    );
    replaced
        .back_at(id, started + ChronoDuration::milliseconds(1_000))
        .unwrap();
    assert_eq!(replaced.active()[0].current_cue_number, Some(1.0));
}

#[test]
fn go_activate_honors_restart_mode_when_playback_is_off() {
    for (restart_mode, expected_index) in [
        (RestartMode::FirstCue, 0),
        (RestartMode::ContinueCurrentCue, 1),
    ] {
        let mut cue_list = list(vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)]);
        cue_list.restart_mode = restart_mode;
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.go_playback(1).unwrap();
        engine.go_playback(1).unwrap();
        assert_eq!(engine.active()[0].cue_index, 1);
        engine.off(1).unwrap();
        engine.go_playback(1).unwrap();
        assert_eq!(engine.active()[0].cue_index, expected_index);
    }
}

#[test]
fn continue_restart_falls_back_to_first_if_remembered_cue_was_deleted_while_off() {
    let mut original = list(vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)]);
    original.restart_mode = RestartMode::ContinueCurrentCue;
    let id = original.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(original).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();
    engine.off(1).unwrap();

    let mut replacement = list(vec![Cue::new(1.0), Cue::new(3.0)]);
    replacement.id = id;
    replacement.restart_mode = RestartMode::ContinueCurrentCue;
    let active = engine.active_for_snapshot(&[replacement.clone()], started);
    assert!(active[0].deleted_cue_hold.is_none());
    assert!(active[0].current_cue_id.is_none());

    let mut replaced = PlaybackEngine::default();
    replaced.register(replacement).unwrap();
    replaced.register_definition(definition(1, id)).unwrap();
    replaced.restore_active(active);
    replaced.on(1).unwrap();
    assert_eq!(replaced.active()[0].cue_index, 0);
    assert_eq!(replaced.active()[0].current_cue_number, Some(1.0));
}

#[test]
fn deleting_an_inactive_earlier_cue_preserves_current_identity() {
    let original = list(vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)]);
    let id = original.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(original).unwrap();
    engine.go_at(id, started).unwrap();
    engine.go_at(id, started).unwrap();
    engine.go_at(id, started).unwrap();
    let mut replacement = list(vec![Cue::new(2.0), Cue::new(3.0)]);
    replacement.id = id;
    let active = engine.active_for_snapshot(&[replacement.clone()], started);
    let mut replaced = PlaybackEngine::default();
    replaced.register(replacement).unwrap();
    replaced.restore_active(active);
    assert_eq!(replaced.active()[0].cue_index, 1);
    assert_eq!(replaced.active()[0].current_cue_number, Some(3.0));
}

#[test]
fn restart_and_timing_settings_have_contract_precedence() {
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.fade_millis = 1_000;
    first.delay_millis = 500;
    let mut change = value(fixture, "intensity", 1.0);
    change.fade_millis = Some(2_000);
    change.delay_millis = Some(100);
    first.changes.push(change);
    let mut second = Cue::new(2.0);
    second.trigger = CueTrigger::Wait {
        delay_millis: 4_000,
    };
    second.changes.push(value(fixture, "intensity", 0.2));
    let mut cue_list = list(vec![first, second]);
    cue_list.force_cue_timing = true;
    let id = cue_list.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list.clone()).unwrap();
    engine.go_at(id, started).unwrap();
    assert!(
        (contribution_level(
            &engine,
            started + ChronoDuration::milliseconds(1_000),
            fixture
        ) - 0.5)
            .abs()
            < 0.01
    );
    engine.go_at(id, started).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.off(1).unwrap();
    engine.on(1).unwrap();
    assert_eq!(engine.active()[0].cue_index, 0);

    cue_list.disable_cue_timing = true;
    let immediate_id = cue_list.id;
    let mut immediate = PlaybackEngine::default();
    immediate.register(cue_list).unwrap();
    immediate.go_at(immediate_id, started).unwrap();
    assert_eq!(contribution_level(&immediate, started, fixture), 1.0);
    immediate.tick(started, None);
    assert_eq!(immediate.active()[0].cue_index, 1);
}
