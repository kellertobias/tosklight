use super::*;
use light_dynamics::DynamicSemanticValue;
use light_programmer::{PreloadProgrammerValueMutation, PreloadProgrammerValueTiming};

fn pending_fixture(
    registry: &ProgrammerRegistry,
    session: SessionId,
    fixture: FixtureId,
    attribute: AttributeKey,
    value: AttributeValue,
) {
    assert!(registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetFixture {
            fixture_id: fixture,
            attribute,
            value,
            timing: PreloadProgrammerValueTiming::default(),
        }],
    ));
}

#[test]
fn hypothetical_playback_drives_both_current_and_final_from_one_captured_lane() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    registry.start(session);
    let (patched, target) = fixture();
    let intensity = AttributeKey::intensity();
    let mut cues = test_cue_list(
        "Preview source",
        vec![CueChange::set(
            target,
            intensity.clone(),
            AttributeValue::Normalized(0.3),
        )],
    );
    let mut second = Cue::new(2_u16.into());
    second.changes.push(CueChange::set(
        target,
        intensity.clone(),
        AttributeValue::Normalized(0.8),
    ));
    second
        .dynamic_changes
        .push(light_playback::CueDynamicChange {
            fixture_id: target,
            attribute: AttributeKey("pan".into()),
            value: DynamicSemanticValue::FixAt {
                value: 0.7,
                timing: Default::default(),
            },
            automatic_restore: false,
        });
    cues.cues.push(second);
    let engine = Engine::new(registry.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![patched].into(),
            playbacks: vec![test_playback(1, cues.id)].into(),
            cue_lists: vec![cues].into(),
            ..Default::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    registry.queue_preload_playback_action(
        session,
        1,
        None,
        light_programmer::PreloadPlaybackQueueAction::Go,
        light_programmer::PreloadPlaybackQueueSurface::Physical,
    );
    let frame = engine.prepare_output_frame(Default::default());
    let mut hypothetical = frame
        .preload_playback_source()
        .unwrap()
        .fork_for_preview(frame.sampled_at());
    hypothetical.go_playback(1).unwrap();
    let input = engine.prepare_preload_frame(&frame, Some(&hypothetical));
    assert!(frame.cue_dynamic_values().is_empty());
    assert_eq!(input.cue_dynamic_values().len(), 1);
    assert!(input.sources().has_pending);
    // The lane is a retained projection, not a handle to either mutable runtime.
    hypothetical.off_mutation(1).unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Off);
    clock.advance_millis(500);
    let mut state = PreloadFrameState::default();
    for before_release in [true, false] {
        let current = engine.observe_prepared_preload(&input, &[], &state, before_release);
        assert_eq!(current.sampled_at(), started);
        assert_eq!(
            current.values().value(target, &intensity),
            Some(&AttributeValue::Normalized(0.8))
        );
    }
    let result = engine
        .render_prepared_preload(&input, &[], &[], &mut state)
        .unwrap();
    assert_eq!(
        result.source.values().value(target, &intensity),
        Some(&AttributeValue::Normalized(0.8))
    );
    assert_eq!(input.cue_dynamic_values().len(), 1);
    assert_eq!(
        engine
            .observe_prepared_frame(&frame, &[])
            .values()
            .value(target, &intensity),
        Some(&AttributeValue::Normalized(0.3))
    );
    assert!(!engine.playback_runtime()[0].enabled);
    assert!(frame.automatic_playback_transitions().is_empty());
}

#[test]
fn pending_static_uses_final_target_while_separate_active_go_keeps_its_fade_current() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    registry.start(session);
    let (active_fixture, active) = fixture();
    let (mut pending_fixture_patch, pending) = fixture();
    pending_fixture_patch.address = Some(10);
    let pending_root = pending_fixture_patch.fixture_id;
    let engine = Engine::new(registry.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![active_fixture, pending_fixture_patch].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    registry.arm_preload(session, true);
    pending_fixture(
        &registry,
        session,
        active,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.6),
    );
    assert!(registry.activate_preload_at_with_fade(session, started, 1_000));
    clock.set(started + ChronoDuration::milliseconds(500));
    registry.arm_preload(session, true);
    pending_fixture(
        &registry,
        session,
        pending,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.9),
    );
    let frame = engine.prepare_observer_frame(Default::default());
    let sources = engine.prepare_preload_frame(&frame, None);
    let result = engine
        .render_prepared_preload(&sources, &[], &[], &mut PreloadFrameState::default())
        .unwrap();
    assert!(
        (result
            .source
            .values()
            .value(active, &AttributeKey::intensity())
            .unwrap()
            .normalized()
            .unwrap()
            - 0.3)
            .abs()
            < 0.001
    );
    assert_eq!(
        result
            .source
            .values()
            .value(pending, &AttributeKey::intensity()),
        Some(&AttributeValue::Normalized(0.9))
    );
    assert!(
        result
            .projection
            .native_ownership
            .contains_key(&pending_root)
    );
}

#[test]
fn removing_one_pending_target_does_not_restart_the_active_go_fade() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    registry.start(session);
    let (active_patch, active) = fixture();
    let (mut other_patch, other) = fixture();
    other_patch.address = Some(10);
    let engine = Engine::new(registry.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![active_patch, other_patch].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    registry.arm_preload(session, true);
    pending_fixture(
        &registry,
        session,
        active,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.6),
    );
    assert!(registry.activate_preload_at_with_fade(session, started, 1_000));
    clock.set(started + ChronoDuration::milliseconds(500));
    registry.arm_preload(session, true);
    pending_fixture(
        &registry,
        session,
        active,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.9),
    );
    pending_fixture(
        &registry,
        session,
        other,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.4),
    );
    let first_frame = engine.prepare_observer_frame(Default::default());
    let first_sources = engine.prepare_preload_frame(&first_frame, None);
    let mut state = PreloadFrameState::default();
    let first = engine
        .render_prepared_preload(&first_sources, &[], &[], &mut state)
        .unwrap();
    assert_eq!(
        first
            .source
            .values()
            .value(active, &AttributeKey::intensity()),
        Some(&AttributeValue::Normalized(0.9))
    );

    assert!(registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::ReleaseFixture {
            fixture_id: active,
            attribute: AttributeKey::intensity(),
        }]
    ));
    clock.set(started + ChronoDuration::milliseconds(750));
    let second_frame = engine.prepare_observer_frame(Default::default());
    let second_sources = engine.prepare_preload_frame(&second_frame, None);
    assert!(
        second_sources.sources().has_pending,
        "the other pending target keeps Preload observable"
    );
    let second = engine
        .render_prepared_preload(&second_sources, &[], &[], &mut state)
        .unwrap();
    let current = second
        .source
        .values()
        .value(active, &AttributeKey::intensity())
        .unwrap()
        .normalized()
        .unwrap();
    assert!(
        (current - 0.45).abs() < 0.001,
        "active GO must stay on its original timeline: {current}"
    );
    assert_eq!(
        second
            .source
            .values()
            .value(other, &AttributeKey::intensity()),
        Some(&AttributeValue::Normalized(0.4))
    );
}

#[test]
fn same_color_pending_owns_channels_but_newer_live_shadows_it_even_when_values_match() {
    let registry = ProgrammerRegistry::with_clock(Arc::new(ManualClock::new(Utc::now())));
    let session = SessionId::new();
    registry.start(session);
    let fixture = FixtureId::new();
    let engine = Engine::new(registry.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![calibrated_visual_fixture(fixture)].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine.set_color_model(light_core::ColorProgrammingModel::Intent);
    let color = AttributeKey::color();
    let value = AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(1., 0., 1.));
    registry.arm_preload(session, true);
    pending_fixture(&registry, session, fixture, color.clone(), value.clone());
    let pending_frame = engine.prepare_observer_frame(Default::default());
    let pending_sources = engine.prepare_preload_frame(&pending_frame, None);
    let pending_result = engine
        .render_prepared_preload(
            &pending_sources,
            &[],
            &[],
            &mut PreloadFrameState::default(),
        )
        .unwrap();
    assert_eq!(
        &*pending_result.projection.native_ownership[&fixture],
        &[false, true, true, true]
    );

    registry.arm_preload(session, false);
    registry.set(session, fixture, color.clone(), value.clone());
    let shadowed_frame = engine.prepare_observer_frame(Default::default());
    let shadowed_sources = engine.prepare_preload_frame(&shadowed_frame, None);
    let shadowed = engine
        .render_prepared_preload(
            &shadowed_sources,
            &[],
            &[],
            &mut PreloadFrameState::default(),
        )
        .unwrap();
    assert_eq!(
        shadowed.source.values().value(fixture, &color),
        Some(&value)
    );
    assert_eq!(
        shadowed
            .source
            .values()
            .contribution_origin(fixture, &color)
            .unwrap()
            .source(),
        &ContributionSourceId::programmer(shadowed_frame.programmer().identity.unwrap())
    );
    assert!(!shadowed.projection.native_ownership.contains_key(&fixture));
}

#[test]
fn captured_pending_release_keeps_before_winner_and_cannot_advance_live_history() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    registry.start(session);
    let fixture = FixtureId::new();
    let engine = Engine::new(registry.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![calibrated_visual_fixture(fixture)].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine.set_color_model(light_core::ColorProgrammingModel::Intent);
    let color = AttributeKey::color();
    let value = AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(1., 0., 1.));
    registry.arm_preload(session, true);
    pending_fixture(&registry, session, fixture, color.clone(), value.clone());
    assert!(registry.apply_dynamic_values(
        session,
        &[DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: color.clone(),
            value: DynamicSemanticValue::Release,
        }],
        None
    ));
    let frame = engine.prepare_observer_frame(Default::default());
    let sources = engine.prepare_preload_frame(&frame, None);
    let mut state = PreloadFrameState::default();
    let before = engine.observe_prepared_preload(&sources, &[], &state, true);
    let after = engine.observe_prepared_preload(&sources, &[], &state, false);
    assert_eq!(before.values().value(fixture, &color), Some(&value));
    assert!(after.values().value(fixture, &color).is_none());
    let (live_revision_before, _) = engine.capture_output_continuity();
    let first = engine
        .render_prepared_preload(&sources, &[], &[], &mut state)
        .unwrap();
    assert_eq!(
        &*first.projection.native_ownership[&fixture],
        &[false, true, true, true]
    );
    let (live_revision_after, _) = engine.capture_output_continuity();
    assert_eq!(
        live_revision_before, live_revision_after,
        "preview must not commit Live history"
    );
    let live = engine.render_prepared(&frame, &[]).unwrap();
    assert!(live.resolved_values.value(fixture, &color).is_none());

    registry.arm_preload(session, false);
    registry.set(
        session,
        fixture,
        color.clone(),
        AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(0., 1., 0.)),
    );
    let shadowed_frame = engine.prepare_observer_frame(Default::default());
    let shadowed_sources = engine.prepare_preload_frame(&shadowed_frame, None);
    let shadowed = engine
        .render_prepared_preload(
            &shadowed_sources,
            &[],
            &[],
            &mut PreloadFrameState::default(),
        )
        .unwrap();
    assert!(!shadowed.projection.native_ownership.contains_key(&fixture));
    clock.set(started + ChronoDuration::seconds(1));
    let repeated = engine
        .render_prepared_preload(&sources, &[], &[], &mut PreloadFrameState::default())
        .unwrap();
    assert_eq!(repeated.source.sampled_at(), frame.sampled_at());
    assert!(repeated.source.values().value(fixture, &color).is_none());
}
