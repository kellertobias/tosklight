//! No Dynamic or FixAT carrier is needed to fit an actual pending static Target.
use super::*;
fn target(xyz: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        xyz,
    )))
}
fn static_rig() -> Rig {
    let rig = Rig::new();
    assert!(rig.programmers.clear_values(rig.session));
    assert!(rig.programmers.clear_preload_pending(rig.session));
    assert!(rig.programmers.arm_preload(rig.session, false));
    rig.programmers.set(
        rig.session,
        rig.target,
        ProgrammingOwner::Position.key(),
        position(10., 20.),
    );
    assert!(rig.programmers.arm_preload(rig.session, true));
    rig.clock.advance_millis(40);
    rig.programmers.set(
        rig.session,
        rig.target,
        ProgrammingOwner::Position.key(),
        target([2., 6., 1.]),
    );
    rig
}
fn assert_no_dynamic_rows(rig: &Rig) {
    let capture = rig.engine.prepare_output_frame(Default::default());
    let input = rig.engine.prepare_preload_frame(&capture, None);
    assert!(input.sources().dynamic_values_before.is_empty());
    assert!(input.sources().dynamic_values_after.is_empty());
    let state = rig.programmers.get(rig.session).unwrap();
    assert!(state.dynamic_values.is_empty());
    assert!(state.preload_dynamic_pending.is_empty());
    assert!(state.preload_dynamic_active.is_empty());
}
#[test]
fn pending_static_target_without_fixat_fits_both_branches_and_preserves_original_evidence() {
    let rig = static_rig();
    let copy = install_mover(&rig, true).unwrap();
    assert_no_dynamic_rows(&rig);
    let state = rig.programmers.get(rig.session).unwrap();
    let pending = state
        .preload_pending
        .iter()
        .find(|row| {
            row.fixture_id == rig.target && row.attribute == ProgrammingOwner::Position.key()
        })
        .unwrap()
        .clone();
    assert_eq!(pending.value, target([2., 6., 1.]));
    let authored = serde_json::to_value(&state).unwrap();
    let mut pair = pair(&rig);
    let lanes = PhysicalPreloadLanes::new(PositionAdapter::default(), PositionAdapter::default());
    let observer = InspectObserver {
        inner: PositionPreloadObserver::new(&lanes),
        lanes: &lanes,
        target: rig.target,
        hold_before: false,
        expected: [None, None],
        expected_tokens: [None, None],
        compose_checks: 0,
    };
    let mut evaluator = RetainedPreloadHybridEvaluator::new_with_observer(
        &rig.engine,
        rig.key.programmer,
        &lanes,
        observer,
    );
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let accepted = &pair.last_success().unwrap().value;
    verify_retained_native(accepted);
    assert_ne!(accepted.before.frame_token, accepted.after.frame_token);
    assert!(
        accepted
            .before
            .frame_token
            .same_capture(&accepted.after.frame_token)
    );
    for (branch, result) in [
        (BRANCHES[0], &accepted.before),
        (BRANCHES[1], &accepted.after),
    ] {
        assert!(
            result.sampled.samples.is_empty(),
            "no hidden Dynamic was manufactured for static fitting"
        );
        assert!(result.requirements.is_empty());
        let sidecar = owned(result);
        assert_eq!(sidecar.value, pending.value);
        let PositionRequest::Intent(requested) = &sidecar.requested else {
            panic!("static Target must retain Intent, not a synthetic program")
        };
        let AttributeValue::Position(intent) = &pending.value else {
            unreachable!()
        };
        assert_eq!(requested, intent.as_ref());
        assert_eq!(sidecar.token, result.frame_token);
        assert_eq!(sidecar.token.lane().preload_branch(), Some(branch));
        assert_eq!(sidecar.writes.len(), 4);
        assert_eq!(sidecar.achieved.outcomes.len(), 2);
        assert_eq!(sidecar.achieved.destinations.len(), 2);
        for destination in [rig.target, copy] {
            let outcome = sidecar
                .achieved
                .outcomes
                .iter()
                .find(|outcome| outcome.destination == destination)
                .unwrap();
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(!outcome.input_requirement && !outcome.missing_mount);
            assert!(outcome.result.angular_error_degrees.unwrap() < 0.04);
            let calculated = sidecar
                .achieved
                .destinations
                .iter()
                .find(|calculated| calculated.destination == destination)
                .unwrap();
            assert_eq!(
                calculated.value, pending.value,
                "fitting preserves the original Target request for every copy"
            );
            let entries = calculated
                .provenance
                .sources
                .entries()
                .expect("static baseline evidence remains known");
            assert!(!entries.is_empty());
            for entry in entries {
                assert_eq!(
                    result.origins.get(entry.record().occurrence_id),
                    Some(entry.record())
                );
                let source = entry
                    .static_source()
                    .expect("there is no Dynamic source in this test");
                assert!(
                    matches!(source.source,DynamicStaticSource::Programmer { programmer_id,lane: DynamicStaticProgrammerLane::Preload } if programmer_id == rig.key.programmer)
                );
                assert_eq!(source.changed_at, pending.changed_at);
                assert_eq!(source.programmer_order, pending.programmer_order);
                assert_eq!(entry.role(), FamilyTraceRole::Authored);
            }
        }
        assert_ne!(
            sidecar.achieved.outcomes[0].result.achieved,
            sidecar.achieved.outcomes[1].result.achieved
        );
        assert!(
            lanes
                .lane(branch)
                .continuity(rig.target, ProgrammingOwner::Position)
                .is_some()
        );
        assert_eq!(
            lanes.lane(branch).last_accepted(),
            Some(result.frame_token.clone())
        );
    }
    assert_eq!(evaluator.observe.compose_checks, 2);
    assert_eq!(
        serde_json::to_value(rig.programmers.get(rig.session).unwrap()).unwrap(),
        authored
    );
}
#[test]
fn static_pending_target_holds_before_independently_and_rejected_pair_commits_neither_branch() {
    let rig = static_rig();
    install_mover(&rig, false);
    let mut pair = pair(&rig);
    let lanes = PhysicalPreloadLanes::new(PositionAdapter::default(), PositionAdapter::default());
    let observer = InspectObserver {
        inner: PositionPreloadObserver::new(&lanes),
        lanes: &lanes,
        target: rig.target,
        hold_before: false,
        expected: [None, None],
        expected_tokens: [None, None],
        compose_checks: 0,
    };
    let mut evaluator = RetainedPreloadHybridEvaluator::new_with_observer(
        &rig.engine,
        rig.key.programmer,
        &lanes,
        observer,
    );
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    evaluator.observe.snapshot_accepted();
    let initial = evaluator.observe.expected.clone();
    evaluator.observe.hold_before = true;
    rig.clock.advance_millis(40);
    rig.programmers.set(
        rig.session,
        rig.target,
        ProgrammingOwner::Position.key(),
        target([-2., 6., 1.]),
    );
    assert_no_dynamic_rows(&rig);
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let held = &pair.last_success().unwrap().value;
    verify_retained_native(held);
    assert!(held.before.sidecars.is_empty());
    assert!(
        held.before
            .requirements
            .iter()
            .any(|requirement| requirement.owner == ProgrammingOwner::Position)
    );
    assert_eq!(owned(&held.after).value, target([-2., 6., 1.]));
    assert_eq!(
        lanes
            .lane(BRANCHES[0])
            .continuity(rig.target, ProgrammingOwner::Position),
        initial[0]
    );
    assert_eq!(
        lanes.lane(BRANCHES[0]).last_accepted(),
        Some(held.before.frame_token.clone()),
        "frame acceptance advances independently of held owner continuity"
    );
    assert_ne!(
        lanes
            .lane(BRANCHES[1])
            .continuity(rig.target, ProgrammingOwner::Position),
        initial[1]
    );
    evaluator.observe.snapshot_accepted();
    let accepted_token = held.after.frame_token.clone();
    evaluator.observe.hold_before = false;
    evaluator.swap_finalization_tokens = true;
    rig.clock.advance_millis(40);
    rig.programmers.set(
        rig.session,
        rig.target,
        ProgrammingOwner::Position.key(),
        target([1., 7., 1.]),
    );
    let rejected = consume(&rig, &mut pair, &mut evaluator);
    assert_eq!(rejected.successful_attempts, 0);
    assert_eq!(rejected.failed_attempts.len(), 1);
    evaluator.observe.assert_uncommitted();
    assert_eq!(
        pair.last_success().unwrap().value.after.frame_token,
        accepted_token
    );
    evaluator.swap_finalization_tokens = false;
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let retried = &pair.last_success().unwrap().value;
    verify_retained_native(retried);
    for (branch, result) in [
        (BRANCHES[0], &retried.before),
        (BRANCHES[1], &retried.after),
    ] {
        assert_eq!(owned(result).value, target([1., 7., 1.]));
        assert_eq!(
            lanes.lane(branch).last_accepted(),
            Some(result.frame_token.clone())
        );
    }
}
