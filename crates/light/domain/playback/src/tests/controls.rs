use super::*;

#[test]
fn fast_navigation_bypasses_only_the_current_transition_timing() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let fixture = FixtureId::new();
    let mut one = Cue::new(1.0);
    one.changes.push(value(fixture, "pan", 0.2));
    let mut two = Cue::new(2.0);
    two.fade_millis = 10_000;
    two.delay_millis = 5_000;
    let mut change = value(fixture, "pan", 0.8);
    change.fade_millis = Some(8_000);
    change.delay_millis = Some(4_000);
    two.changes.push(change);
    let cue_list = list(vec![one, two]);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(cue_list).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    engine.on(1).unwrap();
    clock.advance_millis(20_000);
    engine.fast_forward_playback(1).unwrap();
    let pan = engine
        .contributions()
        .into_iter()
        .find(|value| value.attribute.0 == "pan")
        .unwrap();
    assert_eq!(pan.value, AttributeValue::Normalized(0.8));
    let stored = &engine.cue_lists[&cue_list_id].cues[1];
    assert_eq!((stored.fade_millis, stored.delay_millis), (10_000, 5_000));
    assert_eq!(stored.changes[0].fade_millis, Some(8_000));
    assert_eq!(stored.changes[0].delay_millis, Some(4_000));

    engine.fast_rewind_playback(1).unwrap();
    let pan = engine
        .contributions()
        .into_iter()
        .find(|value| value.attribute.0 == "pan")
        .unwrap();
    assert_eq!(pan.value, AttributeValue::Normalized(0.2));
}

#[test]
fn off_requires_zero_pickup_without_moving_the_recorded_fader() {
    let fixture = FixtureId::new();
    let mut cue = Cue::new(1.0);
    cue.changes.push(value(fixture, "intensity", 1.0));
    let cue_list = list(vec![cue]);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    engine.set_master(1, 0.6).unwrap();
    engine.on(1).unwrap();
    engine.off(1).unwrap();
    let runtime = &engine.runtime()[0];
    assert_eq!(runtime.fader_position, 0.6);
    assert!(runtime.fader_pickup_required);

    engine.set_master(1, 0.9).unwrap();
    assert!(!engine.runtime()[0].enabled);
    assert_eq!(engine.runtime()[0].master, 1.0);
    engine.set_master(1, 0.0).unwrap();
    assert!(!engine.runtime()[0].fader_pickup_required);
    assert!(!engine.runtime()[0].enabled);
    engine.set_master(1, 0.4).unwrap();
    assert!(engine.runtime()[0].enabled);
    assert_eq!(engine.runtime()[0].master, 0.4);
}

#[test]
fn temp_is_a_separate_entry_and_never_auto_offs_the_underlying_playback() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let fixture = FixtureId::new();
    let mut a = Cue::new(1.0);
    a.changes.push(value(fixture, "pan", 0.2));
    let mut b = Cue::new(1.0);
    b.changes.push(value(fixture, "pan", 0.8));
    let a = list(vec![a]);
    let a_id = a.id;
    let b = list(vec![b]);
    let b_id = b.id;
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(a).unwrap();
    engine.register(b).unwrap();
    engine.register_definition(definition(1, a_id)).unwrap();
    engine.register_definition(definition(2, b_id)).unwrap();
    engine.on(1).unwrap();
    clock.advance_millis(1);
    assert!(engine.toggle_temp(2).unwrap());
    assert!(engine.runtime()[0].enabled);
    assert_eq!(
        resolve(engine.contributions())[&(fixture, AttributeKey("pan".into()))],
        AttributeValue::Normalized(0.8)
    );
    assert!(!engine.toggle_temp(2).unwrap());
    assert!(engine.runtime()[0].enabled);
    assert_eq!(
        resolve(engine.contributions())[&(fixture, AttributeKey("pan".into()))],
        AttributeValue::Normalized(0.2)
    );

    let mut temp_definition = engine.definitions[&2].clone();
    temp_definition.fader = PlaybackFaderMode::Temp;
    engine.definitions.insert(2, temp_definition);
    engine.set_master(2, 0.5).unwrap();
    assert_eq!(engine.runtime_status()[1].temporary_master, 0.5);
    engine.set_master(2, 0.0).unwrap();
    assert!(
        !engine.runtime_status().iter().any(|status| {
            status.playback.playback_number == Some(2) && status.temporary_active
        })
    );
}

#[test]
fn flash_release_modes_and_swap_protection_preserve_normal_runtime() {
    let fixture_a = FixtureId::new();
    let fixture_b = FixtureId::new();
    let fixture_c = FixtureId::new();
    let make = |fixture, level| {
        let mut cue = Cue::new(1.0);
        cue.changes.push(value(fixture, "intensity", level));
        cue.changes.push(value(fixture, "pan", level));
        list(vec![cue])
    };
    let a = make(fixture_a, 0.2);
    let a_id = a.id;
    let b = make(fixture_b, 0.8);
    let b_id = b.id;
    let c = make(fixture_c, 0.6);
    let c_id = c.id;
    let mut engine = PlaybackEngine::default();
    engine.register(a).unwrap();
    engine.register(b).unwrap();
    engine.register(c).unwrap();
    engine.register_definition(definition(1, a_id)).unwrap();
    let mut b_definition = definition(2, b_id);
    b_definition.flash_release = FlashReleaseMode::ReleaseIntensityOnly;
    engine.register_definition(b_definition).unwrap();
    let mut c_definition = definition(3, c_id);
    c_definition.protect_from_swap = true;
    engine.register_definition(c_definition).unwrap();
    engine.on(1).unwrap();
    engine.on(3).unwrap();

    engine.set_flash(2, true).unwrap();
    let flash_status = engine
        .runtime_status()
        .into_iter()
        .find(|status| status.playback.playback_number == Some(2))
        .unwrap();
    assert!(flash_status.playback.flash);
    assert!(flash_status.temporary_active);
    engine.set_flash(2, false).unwrap();
    assert!(
        engine
            .runtime_status()
            .into_iter()
            .all(|status| { status.playback.playback_number != Some(2) || !status.playback.flash })
    );
    let b_runtime = engine
        .runtime()
        .into_iter()
        .find(|runtime| runtime.playback_number == Some(2))
        .unwrap();
    assert!(b_runtime.enabled);
    assert_eq!(b_runtime.master, 0.0);
    let b_values = engine
        .contributions()
        .into_iter()
        .filter(|value| value.fixture_id == fixture_b)
        .collect::<Vec<_>>();
    assert_eq!(
        b_values
            .iter()
            .find(|value| value.attribute.is_intensity())
            .unwrap()
            .value,
        AttributeValue::Normalized(0.0)
    );
    assert_eq!(
        b_values
            .iter()
            .find(|value| value.attribute.0 == "pan")
            .unwrap()
            .value,
        AttributeValue::Normalized(0.8)
    );

    let a_before = engine
        .runtime()
        .into_iter()
        .find(|runtime| runtime.playback_number == Some(1))
        .unwrap();
    engine.set_swap(2, true).unwrap();
    let fixtures = engine
        .contributions()
        .into_iter()
        .map(|value| value.fixture_id)
        .collect::<HashSet<_>>();
    assert!(!fixtures.contains(&fixture_a));
    assert!(fixtures.contains(&fixture_b));
    assert!(fixtures.contains(&fixture_c));
    engine.set_swap(2, false).unwrap();
    let a_after = engine
        .runtime()
        .into_iter()
        .find(|runtime| runtime.playback_number == Some(1))
        .unwrap();
    assert_eq!(a_after.cue_index, a_before.cue_index);
    assert_eq!(a_after.master, a_before.master);
    assert_eq!(a_after.activated_at, a_before.activated_at);
}

#[test]
fn manual_xfade_uses_authoritative_alternating_progress_and_survives_restore() {
    let fixture = FixtureId::new();
    let cues = [0.0, 1.0, 0.5]
        .into_iter()
        .enumerate()
        .map(|(index, level)| {
            let mut cue = Cue::new(index as f64 + 1.0);
            cue.fade_millis = 30_000;
            cue.delay_millis = 10_000;
            cue.changes.push(value(fixture, "intensity", level));
            cue
        })
        .collect();
    let cue_list = list(cues);
    let cue_list_id = cue_list.id;
    let mut playback_definition = definition(1, cue_list_id);
    playback_definition.fader = PlaybackFaderMode::XFade;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine
        .register_definition(playback_definition.clone())
        .unwrap();
    engine.on(1).unwrap();
    engine.set_master(1, 0.25).unwrap();
    assert_eq!(
        resolve(engine.contributions())[&(fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(0.25)
    );
    engine.set_master(1, 1.0).unwrap();
    assert_eq!(engine.runtime()[0].current_cue_number, Some(2.0));
    assert_eq!(
        resolve(engine.contributions())[&(fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(1.0)
    );
    assert_eq!(
        engine.runtime()[0].manual_xfade_direction,
        ManualXFadeDirection::TowardsLow
    );
    engine.set_master(1, 1.0).unwrap();
    assert_eq!(engine.runtime()[0].current_cue_number, Some(2.0));
    engine.set_master(1, 0.5).unwrap();
    assert_eq!(
        resolve(engine.contributions())[&(fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(0.75)
    );

    let runtime = engine.runtime();
    let mut restored = PlaybackEngine::default();
    restored
        .register(engine.cue_lists[&cue_list_id].clone())
        .unwrap();
    restored.register_definition(playback_definition).unwrap();
    restored.restore_active(runtime);
    assert_eq!(restored.runtime()[0].manual_xfade_position, 0.5);
    assert_eq!(restored.runtime()[0].manual_xfade_progress, 0.5);
    restored.set_master(1, 0.0).unwrap();
    assert_eq!(restored.runtime()[0].current_cue_number, Some(3.0));
}

#[test]
fn pause_dynamics_freezes_and_resumes_from_the_same_phase() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let fixture = FixtureId::new();
    let mut cue = Cue::new(1.0);
    cue.fade_millis = 1_000;
    cue.changes.push(value(fixture, "intensity", 1.0));
    let cue_list = list(vec![cue]);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(cue_list).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    engine.on(1).unwrap();
    clock.advance_millis(500);
    let level = |engine: &PlaybackEngine| {
        resolve(engine.contributions())[&(fixture, AttributeKey::intensity())]
            .normalized()
            .unwrap()
    };
    assert!((level(&engine) - 0.5).abs() < 0.001);
    engine.set_dynamics_paused(true);
    clock.advance_millis(500);
    assert!((level(&engine) - 0.5).abs() < 0.001);
    engine.set_dynamics_paused(false);
    clock.advance_millis(250);
    assert!((level(&engine) - 0.75).abs() < 0.001);
}
