use super::*;

fn jump_to(cue: &mut Cue, destination: Uuid, count: u32) {
    cue.actions.push(CueAction::Jump {
        cue_id: destination,
        count,
    });
}

#[test]
fn cue_jump_count_exhausts_then_continues_normally() {
    let one = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    let mut two = Cue::new(crate::CueNumber::try_from_legacy_f64(2.0).unwrap());
    let three = Cue::new(crate::CueNumber::try_from_legacy_f64(3.0).unwrap());
    jump_to(&mut two, one.id, 2);
    let cue_list = list(vec![one, two, three]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();

    engine.on(1).unwrap();
    assert_eq!(
        engine.runtime()[0].current_cue_number,
        Some(cue_number(1.0))
    );
    for expected in [2.0, 1.0, 2.0, 1.0, 2.0, 3.0] {
        engine.go_playback(1).unwrap();
        assert_eq!(
            engine.runtime()[0].current_cue_number,
            Some(cue_number(expected))
        );
    }
    assert_eq!(
        engine.jump_counts.values().copied().collect::<Vec<_>>(),
        [3]
    );
}

#[test]
fn each_jump_point_keeps_an_independent_runtime_count() {
    let mut one = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    let mut two = Cue::new(crate::CueNumber::try_from_legacy_f64(2.0).unwrap());
    let three = Cue::new(crate::CueNumber::try_from_legacy_f64(3.0).unwrap());
    let one_destination = one.id;
    jump_to(&mut one, one_destination, 1);
    jump_to(&mut two, three.id, 1);
    let one_id = one.id;
    let two_id = two.id;
    let cue_list = list(vec![one, two, three]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();

    engine.on(1).unwrap();
    for expected in [1.0, 2.0, 3.0] {
        engine.go_playback(1).unwrap();
        assert_eq!(
            engine.runtime()[0].current_cue_number,
            Some(cue_number(expected))
        );
    }
    assert_eq!(engine.jump_counts[&(id, one_id)], 2);
    assert_eq!(engine.jump_counts[&(id, two_id)], 1);
}

#[test]
fn off_and_restart_reset_every_jump_count() {
    let one = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    let mut two = Cue::new(crate::CueNumber::try_from_legacy_f64(2.0).unwrap());
    let three = Cue::new(crate::CueNumber::try_from_legacy_f64(3.0).unwrap());
    jump_to(&mut two, one.id, 2);
    let cue_list = list(vec![one, two, three]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();

    engine.on(1).unwrap();
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();
    assert_eq!(engine.jump_counts.values().next(), Some(&1));
    engine.off(1).unwrap();
    assert!(engine.jump_counts.is_empty());
    engine.on(1).unwrap();
    for expected in [2.0, 1.0, 2.0, 1.0] {
        engine.go_playback(1).unwrap();
        assert_eq!(
            engine.runtime()[0].current_cue_number,
            Some(cue_number(expected))
        );
    }
}

#[test]
fn fast_forward_skips_and_resets_a_jump_point() {
    let one = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    let mut two = Cue::new(crate::CueNumber::try_from_legacy_f64(2.0).unwrap());
    let three = Cue::new(crate::CueNumber::try_from_legacy_f64(3.0).unwrap());
    jump_to(&mut two, one.id, 3);
    let cue_list = list(vec![one, two, three]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();

    engine.on(1).unwrap();
    engine.go_playback(1).unwrap();
    engine.fast_forward_playback(1).unwrap();
    assert_eq!(
        engine.runtime()[0].current_cue_number,
        Some(cue_number(3.0))
    );
    assert!(engine.jump_counts.is_empty());
}

#[test]
fn goto_resets_jump_points_that_it_bypasses() {
    let one = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    let mut two = Cue::new(crate::CueNumber::try_from_legacy_f64(2.0).unwrap());
    let three = Cue::new(crate::CueNumber::try_from_legacy_f64(3.0).unwrap());
    jump_to(&mut two, one.id, 3);
    let cue_list = list(vec![one, two, three]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();

    engine.on(1).unwrap();
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();
    assert_eq!(engine.jump_counts.values().next(), Some(&1));
    engine.go_playback(1).unwrap();
    engine.goto_playback(1, cue_number(3.0)).unwrap();
    assert!(engine.jump_counts.is_empty());
}

#[test]
fn fast_navigation_bypasses_only_the_current_transition_timing() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let fixture = FixtureId::new();
    let mut one = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    one.changes.push(value(fixture, "pan", 0.2));
    let mut two = Cue::new(crate::CueNumber::try_from_legacy_f64(2.0).unwrap());
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
    let mut cue = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
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
    let identity = PlaybackIdentity::physical(1).unwrap();
    let runtime = engine.runtime_status_at(identity).unwrap();
    let runtime = &runtime.playback;
    assert_eq!(runtime.fader_position, 0.6);
    assert!(runtime.fader_pickup_required);
    assert_eq!(runtime.fader_pickup_target, Some(0.0));

    engine.set_master(1, 0.9).unwrap();
    assert!(!engine.runtime()[0].enabled);
    assert_eq!(engine.runtime()[0].master, 1.0);
    assert_eq!(
        engine
            .runtime_status_at(identity)
            .unwrap()
            .playback
            .fader_pickup_target,
        Some(0.0)
    );
    engine.set_master(1, 0.0).unwrap();
    assert!(
        !engine
            .runtime_status_at(identity)
            .unwrap()
            .playback
            .fader_pickup_required
    );
    assert_eq!(
        engine
            .runtime_status_at(identity)
            .unwrap()
            .playback
            .fader_pickup_target,
        None
    );
    assert!(!engine.runtime()[0].enabled);
    engine.set_master(1, 0.4).unwrap();
    assert!(!engine.runtime()[0].enabled);
    assert_eq!(engine.runtime()[0].master, 0.4);
}

#[test]
fn explicit_fader_activation_ignores_cuelist_zero_policy() {
    let cue_list = list(vec![Cue::new(
        crate::CueNumber::try_from_legacy_f64(1.0).unwrap(),
    )]);
    let cue_list_id = cue_list.id;
    let mut playback = definition(1, cue_list_id);
    playback.go_activates = false;
    playback.auto_off = false;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(playback).unwrap();

    engine
        .set_master_with_explicit_activation_mutation(1, 0.4)
        .unwrap();
    assert!(engine.runtime()[0].enabled);
    assert_eq!(engine.runtime()[0].master, 0.4);

    engine
        .set_master_with_explicit_activation_mutation(1, 0.0)
        .unwrap();
    assert!(!engine.runtime()[0].enabled);
    assert_eq!(engine.runtime()[0].master, 0.0);
}

#[test]
fn cuelist_zero_auto_off_resumes_only_its_own_current_cue() {
    let fixture = FixtureId::new();
    let mut one = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    one.changes.push(value(fixture, "intensity", 0.5));
    let mut two = Cue::new(crate::CueNumber::try_from_legacy_f64(2.0).unwrap());
    two.changes.push(value(fixture, "intensity", 1.0));
    let mut cue_list = list(vec![one, two]);
    cue_list.auto_off_at_zero = true;
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    engine.goto_playback(1, cue_number(2.0)).unwrap();
    let cue_id = engine.runtime()[0].current_cue_id;

    engine.set_master(1, 1.0).unwrap();
    engine.set_master(1, 0.0).unwrap();
    assert!(!engine.runtime()[0].enabled);
    assert!(engine.runtime()[0].fader_zero_auto_off_armed);
    engine.set_master(1, 0.6).unwrap();

    let runtime = &engine.runtime()[0];
    assert!(runtime.enabled);
    assert_eq!(runtime.master, 0.6);
    assert_eq!(runtime.current_cue_id, cue_id);
    assert_eq!(runtime.current_cue_number, Some(cue_number(2.0)));
    assert!(!runtime.fader_zero_auto_off_armed);
}

#[test]
fn cuelist_flash_auto_off_is_armed_only_by_the_flash_that_started_it() {
    let mut cue = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    cue.changes.push(value(FixtureId::new(), "intensity", 1.0));
    let mut cue_list = list(vec![cue]);
    cue_list.auto_off_flash_release = true;
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();

    engine.set_flash(1, true).unwrap();
    engine.set_flash(1, false).unwrap();
    assert!(engine.runtime().is_empty());

    engine.on(1).unwrap();
    engine.set_flash(1, true).unwrap();
    engine.set_flash(1, false).unwrap();
    assert!(engine.runtime()[0].enabled);
}

#[test]
fn cuelist_flash_and_swap_share_keep_running_or_release_all_policy() {
    let build = |release_all| {
        let mut cue = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
        cue.changes.push(value(FixtureId::new(), "intensity", 1.0));
        let mut cue_list = list(vec![cue]);
        cue_list.auto_off_flash_release = release_all;
        let cue_list_id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine
            .register_definition(definition(1, cue_list_id))
            .unwrap();
        engine
    };

    // A Flash the operator never turned on lets go of the whole look when it is released. Swap
    // keeps its held playback running at zero master so the swapped-away state can return.
    let mut flashed = build(false);
    PlaybackEngine::set_flash(&mut flashed, 1, true).unwrap();
    PlaybackEngine::set_flash(&mut flashed, 1, false).unwrap();
    assert!(flashed.runtime().iter().all(|runtime| !runtime.enabled));

    let mut swapped = build(false);
    PlaybackEngine::set_swap(&mut swapped, 1, true).unwrap();
    PlaybackEngine::set_swap(&mut swapped, 1, false).unwrap();
    let runtime = &swapped.runtime()[0];
    assert!(runtime.enabled);
    assert_eq!(runtime.master, 0.0);

    for release in [
        PlaybackEngine::set_flash as fn(&mut PlaybackEngine, u16, bool) -> Result<(), String>,
        PlaybackEngine::set_swap,
    ] {
        let mut release_all = build(true);
        release(&mut release_all, 1, true).unwrap();
        release(&mut release_all, 1, false).unwrap();
        assert!(release_all.runtime().iter().all(|runtime| !runtime.enabled));
    }

    let mut combined_hold = build(false);
    combined_hold.set_flash(1, true).unwrap();
    combined_hold.set_swap(1, true).unwrap();
    combined_hold.set_flash(1, false).unwrap();
    assert!(combined_hold.active.is_empty());
    combined_hold.set_swap(1, false).unwrap();
    assert!(combined_hold.active.values().any(|runtime| runtime.enabled));
}

#[test]
fn physical_shared_runtime_keeps_legacy_serializable_number_provenance() {
    let mut cue = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    cue.changes.push(value(FixtureId::new(), "intensity", 1.0));
    let cue_list = list(vec![cue]);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();

    engine.on(1).unwrap();
    let runtime = engine.runtime_status();

    assert_eq!(runtime[0].playback.playback_number, Some(1));
    assert_eq!(runtime[0].playback.playback_identity, None);
    serde_json::to_value(runtime).unwrap();
}

#[test]
fn physical_faders_take_over_one_shared_master_independently() {
    let fixture = FixtureId::new();
    let mut cue = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    cue.changes.push(value(fixture, "intensity", 1.0));
    let cue_list = list(vec![cue]);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    engine
        .register_definition(definition(2, cue_list_id))
        .unwrap();

    engine.set_virtual_master(1, 0.5).unwrap();
    assert_eq!(engine.runtime()[0].master, 0.5);
    engine.set_master(1, 0.6).unwrap();
    assert_eq!(engine.runtime()[0].master, 0.5);
    engine.set_master(1, 0.4).unwrap();
    assert_eq!(engine.runtime()[0].master, 0.4);

    engine.set_master(2, 0.8).unwrap();
    assert_eq!(engine.runtime()[0].master, 0.4);
    engine.set_master(2, 0.3).unwrap();
    assert_eq!(engine.runtime()[0].master, 0.3);

    engine.set_virtual_master(1, 0.7).unwrap();
    assert_eq!(engine.runtime()[0].master, 0.7);
    let one = engine
        .runtime_status_at(PlaybackIdentity::physical(1).unwrap())
        .unwrap()
        .playback;
    let two = engine
        .runtime_status_at(PlaybackIdentity::physical(2).unwrap())
        .unwrap()
        .playback;
    assert_eq!(one.fader_position, 0.4);
    assert_eq!(two.fader_position, 0.3);
    assert_eq!(one.fader_pickup_target, Some(0.7));
    assert_eq!(two.fader_pickup_target, Some(0.7));
}

#[test]
fn restored_shared_master_requires_each_physical_fader_to_reacquire() {
    let cue_list = list(vec![Cue::new(
        crate::CueNumber::try_from_legacy_f64(1.0).unwrap(),
    )]);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list.clone()).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    engine
        .register_definition(definition(2, cue_list_id))
        .unwrap();
    engine.set_virtual_master(1, 0.6).unwrap();
    let persisted = engine.runtime();

    let mut restored = PlaybackEngine::default();
    restored.register(cue_list).unwrap();
    restored
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    restored
        .register_definition(definition(2, cue_list_id))
        .unwrap();
    restored.restore_active(persisted);
    for number in [1, 2] {
        let status = restored
            .runtime_status_at(PlaybackIdentity::physical(number).unwrap())
            .unwrap();
        assert!(status.playback.fader_pickup_required);
        assert_eq!(status.playback.fader_pickup_target, Some(0.6));
    }

    restored.set_master(1, 0.7).unwrap();
    assert_eq!(restored.runtime()[0].master, 0.6);
    restored.set_master(1, 0.5).unwrap();
    assert_eq!(restored.runtime()[0].master, 0.5);
    assert!(
        restored
            .runtime_status_at(PlaybackIdentity::physical(2).unwrap())
            .unwrap()
            .playback
            .fader_pickup_required
    );
}

#[test]
fn temp_is_a_separate_entry_and_never_auto_offs_the_underlying_playback() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let fixture = FixtureId::new();
    let mut a = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    a.changes.push(value(fixture, "pan", 0.2));
    let mut b = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
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
fn keep_running_release_and_swap_protection_preserve_normal_runtime() {
    let fixture_a = FixtureId::new();
    let fixture_b = FixtureId::new();
    let fixture_c = FixtureId::new();
    let make = |fixture, level| {
        let mut cue = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
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
    engine.register_definition(definition(2, b_id)).unwrap();
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
    // The Flash was the only thing running this playback, so releasing it lets go of the whole
    // look rather than leaving its non-intensity values behind at zero master.
    assert!(
        engine
            .runtime()
            .into_iter()
            .all(|runtime| runtime.playback_number != Some(2) || !runtime.enabled)
    );
    assert!(
        engine
            .contributions()
            .into_iter()
            .all(|value| value.fixture_id != fixture_b)
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
            let mut cue = Cue::new(CueNumber::from(index + 1));
            cue.fade_millis = 30_000;
            cue.delay_millis = 10_000;
            cue.out_fade_millis = Some(60_000);
            cue.out_delay_millis = Some(60_000);
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
    assert_eq!(
        engine.runtime()[0].current_cue_number,
        Some(cue_number(2.0))
    );
    assert_eq!(
        resolve(engine.contributions())[&(fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(1.0)
    );
    assert_eq!(
        engine.runtime()[0].manual_xfade_direction,
        ManualXFadeDirection::TowardsLow
    );
    engine.set_master(1, 1.0).unwrap();
    assert_eq!(
        engine.runtime()[0].current_cue_number,
        Some(cue_number(2.0))
    );
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
    assert_eq!(
        restored.runtime()[0].current_cue_number,
        Some(cue_number(3.0))
    );
}

#[test]
fn pause_dynamics_does_not_freeze_ordinary_cue_transitions() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let fixture = FixtureId::new();
    let mut cue = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
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
    assert!((level(&engine) - 1.0).abs() < 0.001);
    engine.set_dynamics_paused(false);
    clock.advance_millis(250);
    assert!((level(&engine) - 1.0).abs() < 0.001);
}

fn dynamic_playback_definition(
    number: u16,
    fader_mode: DynamicPlaybackFaderMode,
    targetless: bool,
) -> PlaybackDefinition {
    let dynamic_id = Uuid::new_v4();
    let definition = light_dynamics::DynamicDefinition {
        id: dynamic_id,
        pool_number: number,
        revision: 1,
        name: format!("Dynamic {number}"),
        color: None,
        icon: None,
        overall_speed_multiplier: light_dynamics::Rational::ONE,
        target_binding: if targetless {
            light_dynamics::DynamicTargetBinding::Targetless
        } else {
            light_dynamics::DynamicTargetBinding::FrozenTargets {
                targets: vec![FixtureId::new()],
            }
        },
        lanes: Vec::new(),
        random_groups: Vec::new(),
        phase_spread_mode: light_dynamics::DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: light_dynamics::DynamicSpatialMappingOverride::default(),
        phase: light_dynamics::PhaseDistribution {
            ordering: light_dynamics::PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 360.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: Vec::new(),
        },
        speed: light_dynamics::DynamicSpeed::Fixed {
            duration_millis: 1_000,
        },
        run_mode: light_dynamics::DynamicRunMode::Loop,
        default_activation: light_dynamics::ActivationPolicy::StartNow,
        activation_boundary: light_dynamics::ActivationBoundary::Beat,
    };
    let target = PlaybackTarget::Dynamic {
        assignment: DynamicPlaybackAssignment {
            dynamic: light_dynamics::DynamicReference {
                dynamic_id: Some(dynamic_id),
                last_known_pool_number: number,
                embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                    definition: Arc::new(definition),
                },
            },
            revision: 1,
            target_scope: targetless.then(|| DynamicPlaybackTargetScope::FrozenTargets {
                targets: vec![FixtureId::new()],
            }),
            fader_mode,
            priority: 0,
            activation_override: None,
            resume_policy: DynamicPlaybackResumePolicy::FollowDynamic,
            local_speed_multiplier: light_dynamics::Rational::ONE,
            learned_duration_millis: None,
            crossfade_non_intensity: false,
            auto_off_at_zero: true,
            auto_off_flash_release: true,
            auto_off_full_control: true,
        },
    };
    PlaybackDefinition {
        number,
        name: format!("Dynamic Playback {number}"),
        buttons: PlaybackDefinition::default_buttons(&target),
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
        footprint: PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
        target,
    }
}

#[test]
fn explicit_fader_activation_ignores_dynamic_zero_policy() {
    let mut definition =
        dynamic_playback_definition(17, DynamicPlaybackFaderMode::SizeAndMaster, false);
    let PlaybackTarget::Dynamic { assignment } = &mut definition.target else {
        unreachable!()
    };
    assignment.auto_off_at_zero = false;
    definition.go_activates = false;
    definition.auto_off = false;
    let mut engine = PlaybackEngine::default();
    engine.register_definition(definition).unwrap();

    engine
        .set_master_with_explicit_activation_mutation(17, 0.6)
        .unwrap();
    assert!(engine.active_dynamic_playbacks()[0].enabled);
    assert_eq!(engine.active_dynamic_playbacks()[0].fader_value, 0.6);

    engine
        .set_master_with_explicit_activation_mutation(17, 0.0)
        .unwrap();
    assert!(!engine.active_dynamic_playbacks()[0].enabled);
    assert_eq!(engine.active_dynamic_playbacks()[0].fader_value, 0.0);
}

#[test]
fn dynamic_playback_fader_pause_speed_flash_and_restore_are_authoritative() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let definition =
        dynamic_playback_definition(17, DynamicPlaybackFaderMode::SizeAndMaster, false);
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register_definition(definition.clone()).unwrap();

    engine.set_master(17, 0.5).unwrap();
    let active = &engine.active_dynamic_playbacks()[0];
    assert!(active.enabled);
    assert_eq!(
        (active.fader_value, active.size, active.master),
        (0.5, 0.5, 0.5)
    );

    engine.toggle_dynamic_pause_mutation(17).unwrap();
    assert!(engine.active_dynamic_playbacks()[0].paused);
    engine.scale_dynamic_speed_mutation(17, true).unwrap();
    assert_eq!(
        engine.active_dynamic_playbacks()[0]
            .local_speed_multiplier
            .factor(),
        2.0
    );
    clock.advance_millis(25);
    engine.restart_dynamic_mutation(17).unwrap();
    assert_eq!(
        engine.active_dynamic_playbacks()[0].activated_at,
        started + chrono::Duration::milliseconds(25)
    );
    assert!(!engine.active_dynamic_playbacks()[0].paused);

    engine.off(17).unwrap();
    engine.set_dynamic_flash_mutation(17, true).unwrap();
    assert!(engine.active_dynamic_playbacks()[0].enabled);
    assert!(engine.active_dynamic_playbacks()[0].flash);
    engine.set_dynamic_flash_mutation(17, false).unwrap();
    assert!(!engine.active_dynamic_playbacks()[0].enabled);
    assert!(!engine.active_dynamic_playbacks()[0].flash);

    engine.on(17).unwrap();
    engine.set_master(17, 0.0).unwrap();
    assert!(!engine.active_dynamic_playbacks()[0].enabled);

    engine.on(17).unwrap();
    engine.set_master(17, 0.4).unwrap();
    let persisted = engine.active_dynamic_playbacks();
    let mut restored = PlaybackEngine::with_clock(clock);
    restored.register_definition(definition).unwrap();
    restored.restore_active_dynamics(persisted.clone());
    assert_eq!(restored.active_dynamic_playbacks(), persisted);
}

#[test]
fn target_bound_dynamic_runtime_is_shared_across_physical_and_virtual_assignments() {
    let first = dynamic_playback_definition(21, DynamicPlaybackFaderMode::SizeAndMaster, false);
    let mut second = first.clone();
    second.number = 22;
    second.name = "Second Dynamic control".into();
    let mut virtual_definition = first.clone();
    virtual_definition.number = 1_001;
    virtual_definition.name = "Virtual Dynamic control".into();
    let surviving_second = second.clone();
    let surviving_virtual = virtual_definition.clone();
    let virtual_address = VirtualPlaybackAddress::new(1, 1_001).unwrap();
    let target_id = match &first.target {
        PlaybackTarget::Dynamic { assignment } => assignment.target_id(),
        _ => unreachable!(),
    };
    let mut engine = PlaybackEngine::default();
    engine.register_definition(first).unwrap();
    engine.register_definition(second).unwrap();
    engine
        .register_virtual_definition(virtual_address, virtual_definition)
        .unwrap();

    engine.on(21).unwrap();
    engine
        .toggle_dynamic_pause_at_mutation(PlaybackIdentity::physical(22).unwrap())
        .unwrap();
    engine
        .set_dynamic_fader_at_mutation(PlaybackIdentity::Virtual(virtual_address), 0.35)
        .unwrap();

    let active = engine.active_dynamic_playbacks();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].dynamic_id, Some(target_id));
    assert!(active[0].paused);
    assert_eq!(
        (active[0].fader_value, active[0].size, active[0].master),
        (0.35, 0.35, 0.35)
    );
    for identity in [
        PlaybackIdentity::physical(21).unwrap(),
        PlaybackIdentity::physical(22).unwrap(),
        PlaybackIdentity::Virtual(virtual_address),
    ] {
        let projected = engine.active_dynamic_playback_at(identity).unwrap();
        assert_eq!(projected.dynamic_id, Some(target_id));
        assert!(projected.paused);
        assert_eq!(projected.master, 0.35);
        assert_eq!(
            projected.fader_pickup_required,
            matches!(identity, PlaybackIdentity::Physical(_))
        );
        assert_eq!(
            projected.fader_pickup_target,
            matches!(identity, PlaybackIdentity::Physical(_)).then_some(0.35)
        );
    }

    assert!(engine.off_dynamic_mutation(22).unwrap().value);
    assert!(!engine.active_dynamic_playbacks()[0].enabled);

    engine.on(21).unwrap();
    let mut next = PlaybackEngine::default();
    next.register_definition(surviving_second).unwrap();
    next.register_virtual_definition(virtual_address, surviving_virtual)
        .unwrap();
    let preserved = engine.active_dynamics_for_snapshot(&next);
    assert_eq!(preserved.len(), 1);
    assert_eq!(preserved[0].playback_number, 22);
    assert_eq!(preserved[0].playback_identity, None);
    next.restore_active_dynamics(preserved);
    assert!(
        next.active_dynamic_playback_at(PlaybackIdentity::Virtual(virtual_address))
            .is_some_and(|active| active.enabled)
    );
    assert!(
        next.active_dynamics_for_snapshot(&PlaybackEngine::default())
            .is_empty()
    );
}

#[test]
fn shared_dynamic_physical_faders_pick_up_independently_and_software_bypasses_pickup() {
    let first = dynamic_playback_definition(41, DynamicPlaybackFaderMode::Master, false);
    let mut second = first.clone();
    second.number = 42;
    let mut engine = PlaybackEngine::default();
    engine.register_definition(first).unwrap();
    engine.register_definition(second).unwrap();

    engine.set_virtual_master_mutation(41, 0.6).unwrap();
    engine.set_master_mutation(41, 0.2).unwrap();
    assert_eq!(engine.active_dynamic_playbacks()[0].master, 0.6);
    assert_eq!(
        engine
            .active_dynamic_playback_at(PlaybackIdentity::physical(41).unwrap())
            .unwrap()
            .fader_pickup_target,
        Some(0.6)
    );

    engine.set_master_mutation(41, 0.7).unwrap();
    assert_eq!(engine.active_dynamic_playbacks()[0].master, 0.7);
    assert!(
        !engine
            .active_dynamic_playback_at(PlaybackIdentity::physical(41).unwrap())
            .unwrap()
            .fader_pickup_required
    );
    assert_eq!(
        engine
            .active_dynamic_playback_at(PlaybackIdentity::physical(42).unwrap())
            .unwrap()
            .fader_pickup_target,
        Some(0.7)
    );

    engine.set_virtual_master_mutation(42, 0.4).unwrap();
    assert_eq!(engine.active_dynamic_playbacks()[0].master, 0.4);
    for number in [41, 42] {
        assert_eq!(
            engine
                .active_dynamic_playback_at(PlaybackIdentity::physical(number).unwrap())
                .unwrap()
                .fader_pickup_target,
            Some(0.4)
        );
    }
    engine.set_master_mutation(42, 0.4).unwrap();
    assert!(
        !engine
            .active_dynamic_playback_at(PlaybackIdentity::physical(42).unwrap())
            .unwrap()
            .fader_pickup_required
    );
}

#[test]
fn shared_dynamic_flash_holds_remain_assignment_local_until_the_last_release() {
    let first = dynamic_playback_definition(51, DynamicPlaybackFaderMode::Master, false);
    let mut second = first.clone();
    second.number = 52;
    let mut engine = PlaybackEngine::default();
    engine.register_definition(first).unwrap();
    engine.register_definition(second).unwrap();

    engine.set_dynamic_flash_mutation(51, true).unwrap();
    engine.set_dynamic_flash_mutation(52, true).unwrap();
    assert_eq!(engine.active_dynamic_playbacks().len(), 1);
    assert!(engine.active_dynamic_playbacks()[0].enabled);
    for number in [51, 52] {
        let projected = engine
            .active_dynamic_playback_at(PlaybackIdentity::physical(number).unwrap())
            .unwrap();
        assert!(projected.flash);
        assert!(projected.flash_restore_off);
    }
    let persisted = engine.active_dynamic_playbacks_for_persistence();
    assert_eq!(persisted.len(), 1);
    assert!(!persisted[0].enabled);
    assert!(!persisted[0].flash);
    assert!(!persisted[0].flash_restore_off);

    engine.set_dynamic_flash_mutation(51, false).unwrap();
    assert!(engine.active_dynamic_playbacks()[0].enabled);
    assert!(
        !engine
            .active_dynamic_playback_at(PlaybackIdentity::physical(51).unwrap())
            .unwrap()
            .flash
    );
    assert!(
        engine
            .active_dynamic_playback_at(PlaybackIdentity::physical(52).unwrap())
            .unwrap()
            .flash
    );

    engine.set_dynamic_flash_mutation(52, false).unwrap();
    let active = &engine.active_dynamic_playbacks()[0];
    assert!(!active.enabled);
    assert!(!active.flash);
    assert!(!active.flash_restore_off);
}

#[test]
fn dynamic_flash_release_without_auto_off_promotes_the_shared_target_on() {
    let mut definition = dynamic_playback_definition(53, DynamicPlaybackFaderMode::Master, false);
    let PlaybackTarget::Dynamic { assignment } = &mut definition.target else {
        unreachable!()
    };
    assignment.auto_off_flash_release = false;
    let mut engine = PlaybackEngine::default();
    engine.register_definition(definition).unwrap();

    engine.set_dynamic_flash_mutation(53, true).unwrap();
    engine.set_dynamic_flash_mutation(53, false).unwrap();

    let active = &engine.active_dynamic_playbacks()[0];
    assert!(active.enabled);
    assert!(!active.flash);
    assert!(!active.flash_restore_off);
}

#[test]
fn dynamic_flash_auto_off_is_armed_only_when_that_flash_started_the_target() {
    let definition = dynamic_playback_definition(54, DynamicPlaybackFaderMode::Master, false);
    let mut engine = PlaybackEngine::default();
    engine.register_definition(definition).unwrap();

    engine.on(54).unwrap();
    engine.set_dynamic_flash_mutation(54, true).unwrap();
    engine.set_dynamic_flash_mutation(54, false).unwrap();
    let active = &engine.active_dynamic_playbacks()[0];
    assert!(active.enabled);
    assert!(!active.flash);

    engine.off(54).unwrap();
    engine.set_dynamic_flash_mutation(54, true).unwrap();
    engine.set_dynamic_flash_mutation(54, false).unwrap();
    let active = &engine.active_dynamic_playbacks()[0];
    assert!(!active.enabled);
    assert!(!active.flash);
}

#[test]
fn legacy_duplicate_dynamic_runtime_rows_restore_deterministically_by_latest_activation() {
    let first = dynamic_playback_definition(31, DynamicPlaybackFaderMode::Master, false);
    let mut second = first.clone();
    second.number = 32;
    let mut source = PlaybackEngine::default();
    source.register_definition(first.clone()).unwrap();
    source.register_definition(second.clone()).unwrap();
    source.on(31).unwrap();
    let mut older = source.active_dynamic_playbacks()[0].clone();
    older.dynamic_id = None;
    older.playback_number = 31;
    older.master = 0.2;
    let mut newer = older.clone();
    newer.playback_number = 32;
    newer.master = 0.8;
    newer.activated_at += chrono::Duration::milliseconds(1);

    for rows in [
        vec![older.clone(), newer.clone()],
        vec![newer.clone(), older.clone()],
    ] {
        let mut restored = PlaybackEngine::default();
        restored.register_definition(first.clone()).unwrap();
        restored.register_definition(second.clone()).unwrap();
        restored.restore_active_dynamics(rows);
        let active = restored.active_dynamic_playbacks();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].master, 0.8);
        assert_eq!(active[0].playback_number, 32);
        assert!(active[0].dynamic_id.is_some());
    }
}

#[test]
fn scheduled_master_transitions_are_advanced_by_the_playback_tick() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let fixture = FixtureId::new();
    let mut cue = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
    cue.changes.push(value(fixture, "intensity", 1.0));
    let cue_list = list(vec![cue]);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(cue_list).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    engine
        .register_definition(definition(2, cue_list_id))
        .unwrap();
    engine.set_master(1, 0.2).unwrap();
    engine.set_master(2, 0.2).unwrap();

    engine
        .set_master_transition_mutation(1, 0.8, 1_000)
        .unwrap();
    assert_eq!(engine.runtime()[0].master, 0.2);
    clock.advance_millis(500);
    engine.tick(started + chrono::Duration::milliseconds(500), None);
    assert!((engine.runtime()[0].master - 0.5).abs() < 0.001);
    assert_eq!(
        engine
            .runtime_status_at(PlaybackIdentity::physical(2).unwrap())
            .unwrap()
            .playback
            .fader_pickup_target,
        Some(0.5)
    );
    clock.advance_millis(500);
    engine.tick(started + chrono::Duration::milliseconds(1_000), None);
    assert_eq!(engine.runtime()[0].master, 0.8);
    assert_eq!(
        engine
            .runtime_status_at(PlaybackIdentity::physical(2).unwrap())
            .unwrap()
            .playback
            .fader_pickup_target,
        Some(0.8)
    );
    assert!(engine.runtime()[0].master_transition.is_none());
}

#[test]
fn dynamic_master_transition_does_not_rewrite_the_physical_fader_assignment() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let definition = dynamic_playback_definition(17, DynamicPlaybackFaderMode::Size, false);
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register_definition(definition).unwrap();
    engine.set_master(17, 0.4).unwrap();
    let before = engine.active_dynamic_playbacks()[0].clone();

    engine
        .set_master_transition_mutation(17, 0.2, 1_000)
        .unwrap();
    clock.advance_millis(500);
    engine.tick(started + chrono::Duration::milliseconds(500), None);
    let active = &engine.active_dynamic_playbacks()[0];
    assert!((active.master - 0.6).abs() < 0.001);
    assert_eq!(active.fader_value, before.fader_value);
    assert_eq!(active.size, before.size);
}

#[test]
fn targetless_dynamic_cannot_be_assigned_directly_to_a_playback() {
    let definition = dynamic_playback_definition(18, DynamicPlaybackFaderMode::Master, true);
    assert_eq!(
        definition.validate().unwrap_err(),
        "targetless Dynamics cannot be assigned directly to a Playback"
    );
}

#[test]
fn dynamic_playback_tap_learns_local_cycle_and_double_half_adjust_it() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let definition = dynamic_playback_definition(19, DynamicPlaybackFaderMode::Master, false);
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register_definition(definition).unwrap();

    assert_eq!(engine.tap_dynamic_speed_mutation(19).unwrap().value, None);
    clock.advance_millis(800);
    assert_eq!(
        engine.tap_dynamic_speed_mutation(19).unwrap().value,
        Some(800)
    );
    clock.advance_millis(1_000);
    assert_eq!(
        engine.tap_dynamic_speed_mutation(19).unwrap().value,
        Some(900)
    );
    assert_eq!(
        engine.active_dynamic_playbacks()[0].learned_duration_millis,
        Some(900)
    );

    engine.scale_dynamic_speed_mutation(19, true).unwrap();
    assert_eq!(
        engine.active_dynamic_playbacks()[0].learned_duration_millis,
        Some(450)
    );
    engine.scale_dynamic_speed_mutation(19, false).unwrap();
    assert_eq!(
        engine.active_dynamic_playbacks()[0].learned_duration_millis,
        Some(900)
    );
    engine.clear_dynamic_learned_speed_mutation(19).unwrap();
    assert_eq!(
        engine.active_dynamic_playbacks()[0].learned_duration_millis,
        None
    );
}
