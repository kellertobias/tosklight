use super::*;

fn base(fixture_id: FixtureId, value: f32, wraps: bool) -> ProgrammerAlignmentBase {
    ProgrammerAlignmentBase {
        fixture_id,
        value,
        wraps,
    }
}

fn values(plan: &ProgrammerAlignmentPlan) -> Vec<f32> {
    plan.values.iter().map(|value| value.value).collect()
}

#[test]
fn weights_cover_single_pair_odd_and_even_selection_shapes() {
    use ProgrammerAlignmentMode::{In, Left, Out, Right};

    for mode in [Left, Right, Out, In] {
        assert_eq!(programmer_alignment_weight(mode, 0, 1), Some(1.0));
    }
    assert_eq!(weights_for(Left, 2), vec![0.0, 1.0]);
    assert_eq!(weights_for(Right, 2), vec![1.0, 0.0]);
    assert_eq!(weights_for(Out, 2), vec![1.0, 1.0]);
    assert_eq!(weights_for(In, 2), vec![0.0, 0.0]);
    assert_eq!(weights_for(Out, 3), vec![1.0, 0.0, 1.0]);
    assert_eq!(weights_for(In, 3), vec![0.0, 1.0, 0.0]);
    assert_eq!(weights_for(Out, 4), vec![1.0, 0.0, 0.0, 1.0]);
    assert_eq!(weights_for(In, 4), vec![0.0, 1.0, 1.0, 0.0]);
    assert_eq!(weights_for(Out, 5), vec![1.0, 0.5, 0.0, 0.5, 1.0]);
    assert_eq!(weights_for(Out, 6), vec![1.0, 0.5, 0.0, 0.0, 0.5, 1.0]);
    assert_eq!(programmer_alignment_weight(Left, 0, 0), None);
    assert_eq!(programmer_alignment_weight(Left, 2, 2), None);
}

fn weights_for(mode: ProgrammerAlignmentMode, count: usize) -> Vec<f32> {
    (0..count)
        .map(|index| programmer_alignment_weight(mode, index, count).unwrap())
        .collect()
}

#[test]
fn activation_freezes_selection_and_cumulative_delta_is_value_neutral_until_applied() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    registry.start(session);
    registry.select(session, fixtures);
    let undo_depth = registry.undo_depth(session);
    let generation = registry.normal_values_generation(session);

    let active = registry
        .activate_alignment(session, ProgrammerAlignmentMode::Left)
        .unwrap();
    assert_eq!(active.fixtures, fixtures);
    assert_eq!(registry.undo_depth(session), undo_depth);
    assert_eq!(registry.normal_values_generation(session), generation);
    assert!(registry.get(session).unwrap().values.is_empty());

    registry.select(session, [FixtureId::new()]);
    assert_eq!(registry.alignment(session).unwrap().fixtures, fixtures);

    let bases = fixtures.map(|fixture_id| base(fixture_id, 0.55, false));
    let first = registry
        .plan_alignment_delta(session, AttributeKey("pan".into()), 0.1, &bases)
        .unwrap();
    assert_eq!(values(&first), vec![0.55, 0.6, 0.65]);
    registry.commit_alignment_plan(session, first).unwrap();

    let second = registry
        .plan_alignment_delta(session, AttributeKey("pan".into()), -0.2, &[])
        .unwrap();
    assert_eq!(values(&second), vec![0.55, 0.5, 0.45]);
    assert_eq!(second.next_state.input_position, -0.1);
}

#[test]
fn binding_skips_unsupported_fixtures_and_weights_the_supported_ordered_subset() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let supported = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let unsupported = FixtureId::new();
    registry.start(session);
    registry.select(
        session,
        [supported[0], unsupported, supported[1], supported[2]],
    );
    registry
        .activate_alignment(session, ProgrammerAlignmentMode::Left)
        .unwrap();

    let bases = supported.map(|fixture_id| base(fixture_id, 0.25, false));
    let plan = registry
        .plan_alignment_delta(session, AttributeKey("pan".into()), 0.2, &bases)
        .unwrap();

    assert_eq!(
        plan.values
            .iter()
            .map(|value| value.fixture_id)
            .collect::<Vec<_>>(),
        supported
    );
    assert_eq!(values(&plan), vec![0.25, 0.35, 0.45]);

    let duplicate = [
        base(supported[0], 0.25, false),
        base(supported[0], 0.25, false),
    ];
    assert_eq!(
        registry.plan_alignment_delta(session, AttributeKey("pan".into()), 0.2, &duplicate,),
        Err(ProgrammerAlignmentError::BaseFixtureNotInFrozenOrder {
            fixture_id: supported[0],
        })
    );
}

#[test]
fn mode_switch_reanchors_from_current_values_without_rollback() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    registry.start(session);
    registry.select(session, fixtures);
    registry
        .activate_alignment(session, ProgrammerAlignmentMode::Left)
        .unwrap();
    let initial = fixtures.map(|fixture_id| base(fixture_id, 0.4, false));
    let first = registry
        .plan_alignment_delta(session, AttributeKey("pan".into()), 0.2, &initial)
        .unwrap();
    let current = first
        .values
        .iter()
        .map(|value| base(value.fixture_id, value.value, false))
        .collect::<Vec<_>>();
    registry.commit_alignment_plan(session, first).unwrap();

    let switched = registry
        .reanchor_alignment(session, ProgrammerAlignmentMode::Out, &current)
        .unwrap();
    assert_eq!(switched.input_position, 0.2);
    assert_eq!(
        switched.binding.unwrap().anchor_input_position,
        switched.input_position
    );

    let next = registry
        .plan_alignment_delta(session, AttributeKey("pan".into()), 0.1, &[])
        .unwrap();
    assert_eq!(values(&next), vec![0.5, 0.5, 0.7]);
}

#[test]
fn clamp_wrap_different_attribute_and_off_are_explicit() {
    assert_eq!(apply_programmer_alignment_delta(0.95, 0.1, false), 1.0);
    assert_eq!(apply_programmer_alignment_delta(0.95, 0.1, true), 0.05);
    assert_eq!(apply_programmer_alignment_delta(0.05, -0.1, true), 0.95);

    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let pan = AttributeKey("pan".into());
    let tilt = AttributeKey("tilt".into());
    registry.start(session);
    registry.select(session, [fixture]);
    registry
        .activate_alignment(session, ProgrammerAlignmentMode::Left)
        .unwrap();
    assert!(!registry.deactivate_alignment_if_different(session, &tilt));
    let plan = registry
        .plan_alignment_delta(session, pan.clone(), 0.1, &[base(fixture, 0.5, false)])
        .unwrap();
    registry.commit_alignment_plan(session, plan).unwrap();
    assert!(!registry.deactivate_alignment_if_different(session, &pan));
    assert!(registry.deactivate_alignment_if_different(session, &tilt));
    assert!(registry.alignment(session).is_none());
    assert!(!registry.deactivate_alignment(session));

    registry
        .activate_alignment(session, ProgrammerAlignmentMode::Right)
        .unwrap();
    assert!(registry.deactivate_alignment(session));
}

#[test]
fn alignment_belongs_to_the_desk_is_transactional_and_stays_runtime_only() {
    let registry = ProgrammerRegistry::default();
    let first = SessionId::new();
    let second = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(first);
    registry.start(second);
    registry.select(first, [fixture]);
    registry
        .activate_alignment(first, ProgrammerAlignmentMode::Left)
        .unwrap();
    assert!(
        registry.alignment(second).is_some(),
        "Align is the desk's state, so a second surface shows it too"
    );

    let result = registry.with_transaction(first, || {
        assert!(registry.deactivate_alignment(first));
        Err::<(), _>("reject")
    });
    assert_eq!(result, Err("reject"));
    assert!(registry.alignment(first).is_some());

    assert!(registry.attach_command_context(second, first));
    assert_eq!(registry.alignment(second), registry.alignment(first));

    let persisted = registry.get(first).unwrap();
    let restored_session = persisted.session_id;
    let restored = ProgrammerRegistry::default();
    restored.restore(persisted);
    assert!(restored.alignment(restored_session).is_none());
}
