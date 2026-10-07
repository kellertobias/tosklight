use super::*;
use light_core::ApplicationClock;

fn manual_engine(
    levels: &[f32],
    fade: u64,
) -> (
    PlaybackEngine,
    Arc<light_core::ManualClock>,
    CueListId,
    Vec<Uuid>,
) {
    let fixture = FixtureId::new();
    let cues: Vec<_> = levels
        .iter()
        .enumerate()
        .map(|(index, level)| {
            focus(
                (index + 1) as f64,
                fixture,
                *level,
                if index == 0 { 0 } else { fade },
            )
        })
        .collect();
    let ids = cues.iter().map(|cue| cue.id).collect();
    let cue_list = list(cues);
    let id = cue_list.id;
    let clock = Arc::new(light_core::ManualClock::new(Utc::now()));
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(cue_list).unwrap();
    let mut def = definition(1, id);
    def.fader = PlaybackFaderMode::XFade;
    engine.register_definition(def).unwrap();
    (engine, clock, id, ids)
}

#[test]
fn manual_leg_uses_compiled_endpoints_and_keeps_one_occurrence_through_reversal_and_completion() {
    let (mut engine, clock, id, cues) = manual_engine(&[0.0, 0.8, 0.2], 1_000);
    let at = clock.now();
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();
    let arbitration = engine.active()[0].transition_ordinal;
    clock.advance_millis(500);
    assert_eq!(
        sample(&engine, clock.now()).value.value,
        AttributeValue::Normalized(0.4)
    );
    engine.set_manual_xfade(1, 0.5).unwrap();
    let middle = evidence(&engine, clock.now());
    assert!(
        (sample(&engine, clock.now())
            .value
            .value
            .normalized()
            .unwrap()
            - 0.5)
            .abs()
            < 0.0001
    );
    assert_eq!(
        middle
            .entries()
            .iter()
            .map(|entry| entry.occurrence.authored_cue_id)
            .collect::<Vec<_>>(),
        vec![Some(cues[1]), Some(cues[2])]
    );
    assert_eq!(middle.entries()[0].occurrence.action_changed_at, at);
    assert_eq!(
        middle.entries()[1].occurrence.action_changed_at,
        clock.now()
    );
    let leg = middle.entries()[1].occurrence.clone();
    engine.set_manual_xfade(1, 0.0).unwrap();
    assert_eq!(
        evidence(&engine, clock.now()).entries(),
        &middle.entries()[..1]
    );
    engine.set_manual_xfade(1, 0.8).unwrap();
    assert!(Arc::ptr_eq(&middle, &evidence(&engine, clock.now())));
    engine.pause(id).unwrap();
    clock.advance_millis(2_000);
    engine.set_manual_xfade(1, 0.6).unwrap();
    assert!(Arc::ptr_eq(&middle, &evidence(&engine, clock.now())));
    engine.resume(id).unwrap();
    engine.set_manual_xfade(1, 1.0).unwrap();
    assert_eq!(engine.active()[0].transition_ordinal, arbitration);
    assert_eq!(engine.active()[0].activated_at, clock.now());
    assert_eq!(evidence(&engine, clock.now()).entries()[0].occurrence, leg);
}

#[test]
fn manual_equal_endpoints_and_cancellation_keep_distinct_action_identity() {
    let (mut engine, clock, id, _) = manual_engine(&[0.4, 0.4, 0.6], 1_000);
    engine.go_playback(1).unwrap();
    let original = evidence(&engine, clock.now());
    engine.set_manual_xfade(1, 0.5).unwrap();
    let manual = evidence(&engine, clock.now());
    assert_eq!(manual.entries().len(), 2);
    assert_ne!(
        manual.entries()[0].occurrence.action_ordinal,
        manual.entries()[1].occurrence.action_ordinal
    );
    engine.pause(id).unwrap();
    clock.advance_millis(1_000);
    engine.go_playback(1).unwrap(); // The existing resumed-GO branch cancels the manual route.
    assert!(engine.active()[0].manual_xfade_from_index.is_none());
    assert!(Arc::ptr_eq(&original, &evidence(&engine, clock.now())));
    engine.set_manual_xfade(1, 0.5).unwrap();
    let second_manual = evidence(&engine, clock.now());
    engine.go_playback(1).unwrap(); // Running GO captures the actual manual interior.
    let captured = evidence(&engine, clock.now());
    assert!(Arc::ptr_eq(&captured, &second_manual));
    clock.advance_millis(500);
    assert_eq!(evidence(&engine, clock.now()).entries().len(), 3);
}

#[test]
fn manual_completion_before_observation_and_restore_never_reconstruct_old_identity() {
    let (mut engine, clock, id, _) = manual_engine(&[0.2, 0.8, 0.4], 1_000);
    engine.go_playback(1).unwrap();
    engine.set_manual_xfade(1, 1.0).unwrap();
    let completed = evidence(&engine, clock.now());
    assert_eq!(completed.entries().len(), 1);
    engine.set_manual_xfade(1, 0.5).unwrap();
    let leg_ordinal = engine.active()[0]
        .source_history
        .as_ref()
        .unwrap()
        .action_ordinal();
    let expected_value = sample(&engine, clock.now()).value.value;
    let state: Vec<ActivePlayback> =
        serde_json::from_value(serde_json::to_value(engine.runtime()).unwrap()).unwrap();
    let mut restored = PlaybackEngine::with_clock(clock.clone());
    restored.register(engine.cue_lists[&id].clone()).unwrap();
    restored
        .register_definition(engine.definitions[&1].clone())
        .unwrap();
    restored.restore_active(state);
    assert_eq!(sample(&restored, clock.now()).value.value, expected_value);
    assert!(sample(&restored, clock.now()).family_evidence.is_none());
    restored.set_manual_xfade(1, 0.0).unwrap();
    assert!(sample(&restored, clock.now()).family_evidence.is_none());
    restored.go_playback(1).unwrap();
    assert!(
        restored.active()[0]
            .source_history
            .as_ref()
            .unwrap()
            .action_ordinal()
            > leg_ordinal
    );
}

#[test]
fn temporary_replay_has_its_own_boundary_and_idempotent_controls_keep_it() {
    let (mut engine, clock, _, _) = manual_engine(&[0.2, 0.8], 1_000);
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();
    let arbitration = engine.active()[0].transition_ordinal;
    let next_arbitration = engine.next_transition_ordinal;
    clock.advance_millis(500);
    engine.set_temp_button(1, true).unwrap();
    let identity = PlaybackIdentity::physical(1).unwrap();
    let key = (identity, TemporaryPlaybackKind::TempButton);
    let first_action = engine.temporary[&key]
        .source_history
        .as_ref()
        .unwrap()
        .action_ordinal();
    engine.set_temp_button(1, true).unwrap();
    assert_eq!(
        engine.temporary[&key]
            .source_history
            .as_ref()
            .unwrap()
            .action_ordinal(),
        first_action
    );
    assert_eq!(engine.next_transition_ordinal, next_arbitration);
    assert_eq!(
        engine.active[&PlaybackKey::CueList(engine.temporary[&key].cue_list_id)].transition_ordinal,
        arbitration
    );
    let now = clock.now() + ChronoDuration::milliseconds(500);
    let temp = engine
        .contributions_with_context(now, None)
        .into_iter()
        .find(|row| row.source.temporary)
        .unwrap();
    let entries = temp.family_evidence.unwrap();
    assert_eq!(entries.entries().len(), 2);
    assert!(!entries.entries()[0].occurrence.source.temporary);
    assert!(entries.entries()[1].occurrence.source.temporary);
    assert_eq!(entries.entries()[1].occurrence.action_ordinal, first_action);
    assert_eq!(
        entries.entries()[1].occurrence.action_changed_at,
        clock.now()
    );
    engine.set_temp_button(1, false).unwrap();
    engine.set_temp_button(1, true).unwrap();
    assert!(
        engine.temporary[&key]
            .source_history
            .as_ref()
            .unwrap()
            .action_ordinal()
            > first_action
    );
}

#[test]
fn temporary_automatic_transition_uses_stored_endpoint_sources_and_preserves_arbitration() {
    let fixture = FixtureId::new();
    let mut first = focus(1.0, fixture, 0.4, 0);
    first.fade_millis = 1_000;
    let mut second = focus(2.0, fixture, 0.8, 1_000);
    second.trigger = CueTrigger::Wait { delay_millis: 200 };
    let cue_list = list(vec![first, second]);
    let id = cue_list.id;
    let clock = Arc::new(light_core::ManualClock::new(Utc::now()));
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.set_temp_button(1, true).unwrap();
    let key = (
        PlaybackIdentity::physical(1).unwrap(),
        TemporaryPlaybackKind::TempButton,
    );
    let action = engine.temporary[&key]
        .source_history
        .as_ref()
        .unwrap()
        .action_ordinal();
    let arbitration = engine.temporary[&key].transition_ordinal;
    clock.advance_millis(201);
    engine.tick(clock.now(), None);
    clock.advance_millis(500);
    let middle = sample(&engine, clock.now());
    assert!((middle.value.value.normalized().unwrap() - 0.6).abs() < 0.0001);
    let evidence = middle.family_evidence.unwrap();
    assert_eq!(evidence.entries().len(), 2);
    assert_eq!(evidence.entries()[0].occurrence.action_ordinal, action);
    assert!(evidence.entries()[1].occurrence.action_ordinal > action);
    assert_eq!(engine.temporary[&key].transition_ordinal, arbitration);
}

#[test]
fn provenance_watermark_is_monotonic_and_exhaustion_never_changes_values_or_arbitration() {
    let (mut engine, clock, _, _) = manual_engine(&[0.2, 0.8], 1_000);
    engine.reserve_source_occurrence_watermark(u64::MAX - 1);
    engine.reserve_source_occurrence_watermark(1);
    assert_eq!(engine.source_occurrence_watermark(), u64::MAX - 1);
    engine.go_playback(1).unwrap();
    assert_eq!(
        evidence(&engine, clock.now()).entries()[0]
            .occurrence
            .action_ordinal,
        u64::MAX
    );
    assert_eq!(engine.source_occurrence_watermark(), u64::MAX);
    engine.reserve_source_occurrence_watermark(0);
    engine.set_manual_xfade(1, 0.5).unwrap();
    assert_eq!(
        sample(&engine, clock.now()).value.value,
        AttributeValue::Normalized(0.5)
    );
    assert!(sample(&engine, clock.now()).family_evidence.is_none());
    assert_eq!(engine.active()[0].transition_ordinal, 1);
    engine.go_playback(1).unwrap();
    assert_eq!(engine.active()[0].transition_ordinal, 2);
    engine.set_temp_button(1, true).unwrap();
    assert!(
        engine
            .temporary
            .values()
            .all(|playback| playback.source_history.is_none())
    );
    assert_eq!(engine.source_occurrence_watermark(), u64::MAX);
}

#[test]
fn manual_tracking_wrap_completion_recovers_only_proven_outgoing_rows() {
    let fixture_a = FixtureId::new();
    let fixture_b = FixtureId::new();
    let mut cue_list = list(vec![
        focus(1.0, fixture_a, 0.2, 0),
        focus(2.0, fixture_b, 0.8, 0),
    ]);
    cue_list.wrap_mode = Some(WrapMode::Tracking);
    let id = cue_list.id;
    let clock = Arc::new(light_core::ManualClock::new(Utc::now()));
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(cue_list).unwrap();
    let mut def = definition(1, id);
    def.fader = PlaybackFaderMode::XFade;
    engine.register_definition(def).unwrap();
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();
    let outgoing = engine
        .contributions_with_context(clock.now(), None)
        .into_iter()
        .find(|row| row.value.fixture_id == fixture_b)
        .unwrap()
        .family_evidence
        .unwrap();
    engine.set_manual_xfade(1, 0.5).unwrap();
    let middle = engine
        .contributions_with_context(clock.now(), None)
        .into_iter()
        .find(|row| row.value.fixture_id == fixture_b)
        .unwrap();
    assert_eq!(middle.value.value, AttributeValue::Normalized(0.4));
    engine.set_manual_xfade(1, 1.0).unwrap();
    let completed = engine
        .contributions_with_context(clock.now(), None)
        .into_iter()
        .find(|row| row.value.fixture_id == fixture_b)
        .unwrap();
    assert_eq!(completed.value.value, AttributeValue::Normalized(0.8));
    assert!(Arc::ptr_eq(
        &outgoing,
        completed.family_evidence.as_ref().unwrap()
    ));
}

#[test]
fn manual_snap_override_selects_target_even_when_fader_reverses_to_zero() {
    let (mut engine, clock, _, cues) = manual_engine(&[0.2, 0.8], 1_000);
    engine.go_playback(1).unwrap();
    engine.set_manual_xfade(1, 0.5).unwrap();
    engine.set_manual_xfade(1, 0.0).unwrap();
    let snapped = engine
        .contributions_with_context_at(clock.now(), |_, _| true)
        .pop()
        .unwrap();
    assert_eq!(snapped.value.value, AttributeValue::Normalized(0.8));
    let proof = snapped.family_evidence.unwrap();
    assert_eq!(proof.entries().len(), 1);
    assert_eq!(proof.entries()[0].occurrence.authored_cue_id, Some(cues[1]));
}

#[test]
fn temporary_deleted_hold_and_release_promotion_keep_actual_historical_sources() {
    let (mut engine, clock, id, _) = manual_engine(&[0.2, 0.8, 0.4], 1_000);
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();
    clock.advance_millis(500);
    let original = evidence(&engine, clock.now());
    let mut replacement = engine.cue_lists[&id].clone();
    replacement.cues.remove(1);
    let retained = engine.active_for_snapshot(&[replacement.clone()], clock.now());
    engine.active.clear();
    engine.register(replacement).unwrap();
    engine.restore_active(retained);
    engine.set_temp_button(1, true).unwrap();
    let held = engine
        .contributions_with_context(clock.now(), None)
        .into_iter()
        .find(|row| row.source.temporary)
        .unwrap();
    assert_eq!(held.value.value, AttributeValue::Normalized(0.5));
    assert!(Arc::ptr_eq(
        &original,
        held.family_evidence.as_ref().unwrap()
    ));

    let (mut engine, clock, id, _) = manual_engine(&[0.4], 0);
    engine.definitions.get_mut(&1).unwrap().flash_release = FlashReleaseMode::ReleaseIntensityOnly;
    engine.set_flash(1, true).unwrap();
    let flashed = evidence(&engine, clock.now());
    assert!(flashed.entries()[0].occurrence.source.temporary);
    engine.set_flash(1, false).unwrap();
    assert!(!engine.active[&PlaybackKey::CueList(id)].temporary);
    assert!(Arc::ptr_eq(&flashed, &evidence(&engine, clock.now())));
}

#[test]
fn temporary_peers_and_preview_actions_have_isolated_occurrences_and_counters() {
    let (mut engine, clock, _, _) = manual_engine(&[0.4, 0.6], 1_000);
    engine.go_playback(1).unwrap();
    let live_watermark = engine.source_occurrence_watermark();
    let mut preview = engine.fork_for_preview(clock.now());
    preview.set_manual_xfade(1, 0.5).unwrap();
    preview.set_temp_button(1, true).unwrap();
    assert_eq!(engine.source_occurrence_watermark(), live_watermark);
    assert!(engine.temporary.is_empty());
    assert!(engine.active()[0].manual_xfade_from_index.is_none());
    engine.set_flash(1, true).unwrap();
    engine.set_temp_button(1, true).unwrap();
    let ordinals: HashSet<_> = engine
        .temporary
        .values()
        .map(|playback| playback.source_history.as_ref().unwrap().action_ordinal())
        .collect();
    assert_eq!(ordinals.len(), 2);
    engine.set_temp_fader(1, 0.3).unwrap();
    let key = (
        PlaybackIdentity::physical(1).unwrap(),
        TemporaryPlaybackKind::TempFader,
    );
    let fader = engine.temporary[&key]
        .source_history
        .as_ref()
        .unwrap()
        .action_ordinal();
    engine.set_temp_fader(1, 0.8).unwrap();
    assert_eq!(
        engine.temporary[&key]
            .source_history
            .as_ref()
            .unwrap()
            .action_ordinal(),
        fader
    );
}

#[test]
fn removing_a_captured_route_does_not_reuse_its_warmed_phase_proof() {
    let (mut engine, clock, _, _) = manual_engine(&[0.0, 1.0, 0.0], 1_000);
    engine.definitions.get_mut(&1).unwrap().fader = PlaybackFaderMode::Master;
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();
    clock.advance_millis(250);
    engine.go_playback(1).unwrap();
    clock.advance_millis(250);
    assert_eq!(evidence(&engine, clock.now()).entries().len(), 3);
    engine.set_virtual_master(1, 0.5).unwrap();
    engine.on(1).unwrap();
    let resumed = sample(&engine, clock.now());
    assert_eq!(resumed.value.value, AttributeValue::Normalized(0.75));
    assert!(resumed.family_evidence.is_none());
    clock.advance_millis(750);
    assert_eq!(evidence(&engine, clock.now()).entries().len(), 1);
}

#[test]
fn chaser_retiming_at_last_cue_cannot_reuse_a_different_previous_route() {
    let (mut engine, clock, id, _) = manual_engine(&[0.0, 1.0, 0.0], 1_000);
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();
    clock.advance_millis(250);
    engine.go_playback(1).unwrap();
    let _ = evidence(&engine, clock.now());
    let cue_list = engine.cue_lists.get_mut(&id).unwrap();
    cue_list.mode = CueListMode::Chaser;
    cue_list.speed_group = Some("A".into());
    clock.advance_millis(10_000);
    engine.set_control_timing([60.0, 90.0, 60.0, 30.0, 15.0], 0, 0);
    assert_eq!(
        sample(&engine, clock.now()).value.value,
        AttributeValue::Normalized(0.0)
    );
    assert!(sample(&engine, clock.now()).family_evidence.is_none());
}
