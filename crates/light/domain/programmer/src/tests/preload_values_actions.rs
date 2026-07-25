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
fn pending_batch_has_one_checkpoint_timestamp_generation_and_operator_order() {
    let entered_at = Utc::now();
    let clock = Arc::new(ManualClock::new(entered_at));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    let user = UserId::new();
    let fixtures = [FixtureId::new(), FixtureId::new()];
    registry.start(session, user);
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
    let user = UserId::new();
    let fixture = FixtureId::new();
    registry.start(session, user);
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
    let user = UserId::new();
    registry.start(session, user);
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
