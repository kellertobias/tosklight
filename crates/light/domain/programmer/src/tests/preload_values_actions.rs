use super::*;

fn fixture_set(
    fixture_id: FixtureId,
    attribute: &str,
    value: f32,
    timing: PreloadProgrammerValueTiming,
) -> PreloadProgrammerValueMutation {
    PreloadProgrammerValueMutation::SetFixture {
        fixture_id,
        attribute: AttributeKey(attribute.into()),
        value: AttributeValue::Normalized(value),
        timing,
    }
}

#[test]
fn off_removes_release_only_entries_with_one_undo_and_keeps_active_release() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let color = AttributeKey("color".into());
    registry.start(session);
    registry.arm_preload(session, true);
    let fixtures = [ReleaseProgrammerFixtureValue {
        fixture_id: fixture,
        attribute: color.clone(),
    }];
    let groups = [ReleaseProgrammerGroupValue {
        group_id: "front".into(),
        attribute: color.clone(),
    }];
    registry.apply_release_values(session, &fixtures, &groups);
    registry.activate_preload(session);
    registry.arm_preload(session, true);
    registry.apply_release_values(session, &fixtures, &groups);
    let before = registry.get(session).unwrap();
    let generation = registry.preload_values_generation(session).unwrap();
    let off = [
        PreloadProgrammerValueMutation::ReleaseFixture {
            fixture_id: fixture,
            attribute: color.clone(),
        },
        PreloadProgrammerValueMutation::ReleaseGroup {
            group_id: "front".into(),
            attribute: color.clone(),
        },
    ];
    assert!(registry.apply_preload_values(session, &off));
    let after = registry.get(session).unwrap();
    assert_eq!(after.undo.len(), before.undo.len() + 1);
    assert!(after.preload_dynamic_pending.is_empty());
    assert!(after.preload_group_release_pending.is_empty());
    assert_eq!(after.preload_dynamic_active, before.preload_dynamic_active);
    assert_eq!(
        after.preload_group_release_active,
        before.preload_group_release_active
    );
    assert_eq!(
        registry.preload_values_generation(session),
        Some(generation + 1)
    );
    assert!(!registry.apply_preload_values(session, &off));
    registry.undo(session);
    assert_eq!(
        registry.get(session).unwrap().preload_dynamic_pending,
        before.preload_dynamic_pending
    );
    registry.redo(session);
    assert!(
        registry
            .get(session)
            .unwrap()
            .preload_group_release_pending
            .is_empty()
    );
    assert_eq!(
        registry.preload_values_generation(session),
        Some(generation + 3)
    );
}

#[test]
fn group_release_only_undo_and_redo_publish_the_changed_lane() {
    for preload in [false, true] {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        registry.start(session);
        if preload {
            registry.arm_preload(session, true);
        }
        registry.apply_release_values(
            session,
            &[],
            &[ReleaseProgrammerGroupValue {
                group_id: "front".into(),
                attribute: AttributeKey("color".into()),
            }],
        );
        let generation = || {
            if preload {
                registry.preload_values_generation(session)
            } else {
                registry.normal_values_generation(session)
            }
        };
        assert_eq!(generation(), Some(1));
        registry.undo(session);
        assert_eq!(generation(), Some(2));
        registry.redo(session);
        assert_eq!(generation(), Some(3));
    }
}

#[test]
fn pending_batch_has_one_checkpoint_timestamp_generation_and_operator_order() {
    let entered_at = Utc::now();
    let clock = Arc::new(ManualClock::new(entered_at));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    let fixtures = [FixtureId::new(), FixtureId::new()];
    registry.start(session);
    assert!(registry.arm_preload(session, true));
    let undo_before = registry.get(session).unwrap().undo.len();
    let timing = PreloadProgrammerValueTiming {
        fade: true,
        fade_millis: Some(1_200),
        delay_millis: Some(300),
    };
    let batch = vec![
        fixture_set(fixtures[0], "intensity", 0.25, Default::default()),
        fixture_set(fixtures[1], "pan", 0.75, timing),
        PreloadProgrammerValueMutation::SetGroup {
            group_id: "front".into(),
            attribute: AttributeKey("tilt".into()),
            value: AttributeValue::Spread(vec![0.1, 0.9]),
            timing,
        },
    ];

    assert!(registry.apply_preload_values(session, &batch));
    assert_eq!(registry.preload_values_generation(session), Some(1));
    let state = registry.get(session).unwrap();
    assert_eq!(state.undo.len(), undo_before + 1);
    assert!(
        state
            .preload_pending
            .iter()
            .all(|value| value.changed_at == entered_at)
    );
    let content = registry.preload_pending_values(session).unwrap();
    assert_eq!(content.fixture_values.len(), 2);
    assert_eq!(content.group_values.len(), 1);
    assert!(
        content.fixture_values[0].programmer_order < content.fixture_values[1].programmer_order
    );
    assert!(content.fixture_values[1].programmer_order < content.group_values[0].programmer_order);
    assert_eq!(content.fixture_values[1].fade_millis, Some(1_200));
    assert_eq!(content.fixture_values[1].delay_millis, Some(300));

    assert!(!registry.apply_preload_values(session, &batch));
    assert_eq!(registry.preload_values_generation(session), Some(1));
    assert_eq!(registry.get(session).unwrap().undo.len(), undo_before + 1);
}

#[test]
fn pending_mutations_require_capture_and_lifecycle_changes_advance_generation() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session);
    let set = vec![fixture_set(fixture, "intensity", 0.5, Default::default())];

    assert!(!registry.apply_preload_values(session, &set));
    assert_eq!(registry.preload_values_generation(session), Some(0));
    assert!(registry.arm_preload(session, true));
    assert!(registry.apply_preload_values(session, &set));
    assert_eq!(registry.preload_values_generation(session), Some(1));

    assert!(registry.activate_preload(session));
    assert_eq!(registry.preload_values_generation(session), Some(2));
    assert!(
        registry
            .preload_pending_values(session)
            .unwrap()
            .fixture_values
            .is_empty()
    );
    assert!(registry.undo(session));
    assert_eq!(registry.preload_values_generation(session), Some(3));
    assert_eq!(
        registry
            .preload_pending_values(session)
            .unwrap()
            .fixture_values
            .len(),
        1
    );
    assert!(registry.clear_preload_pending(session));
    assert_eq!(registry.preload_values_generation(session), Some(4));

    assert!(registry.apply_preload_values(session, &set));
    assert_eq!(registry.preload_values_generation(session), Some(5));
    assert!(registry.release_preload(session));
    assert_eq!(registry.preload_values_generation(session), Some(6));
}

#[test]
fn maximum_pending_fixture_batch_is_applied_and_released_in_one_pass() {
    const MUTATION_LIMIT: usize = 10_000;
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    assert!(registry.arm_preload(session, true));
    let fixtures = (0..MUTATION_LIMIT)
        .map(|_| FixtureId::new())
        .collect::<Vec<_>>();
    let sets = fixtures
        .iter()
        .map(|fixture_id| fixture_set(*fixture_id, "intensity", 0.5, Default::default()))
        .collect::<Vec<_>>();

    assert!(registry.apply_preload_values(session, &sets));
    let values = registry
        .preload_pending_values(session)
        .unwrap()
        .fixture_values;
    assert_eq!(values.len(), MUTATION_LIMIT);
    assert!(
        values
            .windows(2)
            .all(|pair| pair[0].programmer_order < pair[1].programmer_order)
    );

    let releases = fixtures
        .into_iter()
        .map(
            |fixture_id| PreloadProgrammerValueMutation::ReleaseFixture {
                fixture_id,
                attribute: AttributeKey::intensity(),
            },
        )
        .collect::<Vec<_>>();
    assert!(registry.apply_preload_values(session, &releases));
    assert!(
        registry
            .preload_pending_values(session)
            .unwrap()
            .fixture_values
            .is_empty()
    );
    assert_eq!(registry.preload_values_generation(session), Some(2));
}

#[test]
fn continuous_preload_family_gesture_has_one_undo_and_intervening_action_ends_it() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session);
    registry.arm_preload(session, true);
    let before = registry.undo_depth(session).unwrap();
    let edit = |pan| PreloadProgrammerValueMutation::SetFixture {
        fixture_id: fixture,
        attribute: AttributeKey("position".into()),
        value: AttributeValue::Position(Arc::new(light_core::programming::PositionIntent::angles(
            pan, 42.0,
        ))),
        timing: Default::default(),
    };
    for pan in [360.0, 450.0, 540.0] {
        assert!(registry.apply_preload_values_grouped(session, &[edit(pan)], Some("encoder-1")));
    }
    assert_eq!(registry.undo_depth(session), Some(before + 1));
    assert!(registry.undo(session));
    assert!(
        registry
            .preload_pending_values(session)
            .unwrap()
            .fixture_values
            .is_empty()
    );
    assert!(registry.redo(session));
    let result = registry.preload_pending_values(session).unwrap();
    assert_eq!(
        result.fixture_values[0].value,
        AttributeValue::Position(Arc::new(light_core::programming::PositionIntent::angles(
            540.0, 42.0
        )))
    );
    registry.apply_preload_values(
        session,
        &[fixture_set(fixture, "focus", 0.5, Default::default())],
    );
    let after_other = registry.undo_depth(session).unwrap();
    registry.apply_preload_values_grouped(session, &[edit(630.0)], Some("encoder-1"));
    assert_eq!(registry.undo_depth(session), Some(after_other + 1));
    registry.undo(session);
    assert_eq!(
        registry
            .preload_pending_values(session)
            .unwrap()
            .fixture_values
            .len(),
        2
    );
}

#[test]
fn show_recording_boundary_ends_preload_encoder_undo_group_even_without_redo() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session);
    registry.arm_preload(session, true);
    registry.apply_preload_values_grouped(
        session,
        &[fixture_set(fixture, "focus", 0.25, Default::default())],
        Some("touch"),
    );
    let recorded_depth = registry.undo_depth(session).unwrap();
    // This is the shared Record/Update/import history boundary used by remember_show_mutation.
    assert!(!registry.clear_redo(session));
    registry.apply_preload_values_grouped(
        session,
        &[fixture_set(fixture, "focus", 0.75, Default::default())],
        Some("touch"),
    );
    assert_eq!(registry.undo_depth(session), Some(recorded_depth + 1));
    assert!(registry.undo(session));
    assert_eq!(
        registry
            .preload_pending_values(session)
            .unwrap()
            .fixture_values[0]
            .value,
        AttributeValue::Normalized(0.25)
    );
    assert_eq!(registry.undo_depth(session), Some(recorded_depth));
}
