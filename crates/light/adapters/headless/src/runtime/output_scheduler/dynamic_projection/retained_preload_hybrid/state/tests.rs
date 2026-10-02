//! Persistent evaluator loans over the actual paired journal and calibrated physical lanes.
//! The synthetic profile proves state/lane lifetime and acceptance, not physical measurements.
use super::*;
use light_engine::CapturedFrameLane;

#[path = "tests/worker.rs"]
mod worker;

fn state_identity(token: &CapturedFrameToken) -> (Arc<()>, u64) {
    let CapturedFrameLane::Preload {
        state, revision, ..
    } = token.lane()
    else {
        panic!("the retained result must carry its own Pending lane identity")
    };
    (Arc::clone(state), *revision)
}

#[test]
fn recreated_borrowing_evaluator_keeps_both_branch_state_and_physical_history() {
    let rig = Rig::new();
    install_mover(&rig, true).unwrap();
    fix_at(
        &rig,
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [2., 6., 1.],
        ))),
    );
    let mut pair = pair(&rig);
    let lanes = PhysicalPreloadLanes::new(PositionAdapter::default(), PositionAdapter::default());
    let mut state = RetainedPreloadHybridState::default();
    // These addresses certify that both compiler workspaces and the engine cache owner are
    // retained, rather than replaced when the short-lived resolver/observer loans end.
    let addresses = (
        &state.state as *const PreloadFrameState,
        &state.before_scratch as *const HybridFrameScratch,
        &state.after_scratch as *const HybridFrameScratch,
    );
    {
        let mut evaluator = RetainedPreloadHybridEvaluator::with_state_and_observer(
            &rig.engine,
            rig.key.programmer,
            &mut state,
            &lanes,
            PositionPreloadObserver::new(&lanes),
        );
        assert_eq!(
            consume(&rig, &mut pair, &mut evaluator).successful_attempts,
            1
        );
    }
    let first = &pair.last_success().unwrap().value;
    verify_retained_native(first);
    let first_identity = state_identity(&first.before.frame_token);
    let first_after_identity = state_identity(&first.after.frame_token);
    assert!(Arc::ptr_eq(&first_identity.0, &first_after_identity.0));
    let positions = pair.positions();
    let initial = BRANCHES.map(|branch| {
        lanes
            .lane(branch)
            .continuity(rig.target, ProgrammingOwner::Position)
            .unwrap()
    });
    let initial_tokens = BRANCHES.map(|branch| lanes.lane(branch).last_accepted());
    let compiles = BRANCHES.map(|branch| lanes.lane(branch).adapter().counters().compiles);
    let observer = InspectObserver {
        inner: PositionPreloadObserver::new(&lanes),
        lanes: &lanes,
        target: rig.target,
        hold_before: true,
        expected: initial.clone().map(Some),
        expected_tokens: initial_tokens,
        compose_checks: 0,
    };
    fix_at(
        &rig,
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [-2., 6., 2.],
        ))),
    );
    {
        let mut evaluator = RetainedPreloadHybridEvaluator::with_state_and_observer(
            &rig.engine,
            rig.key.programmer,
            &mut state,
            &lanes,
            observer,
        );
        assert_eq!(
            consume(&rig, &mut pair, &mut evaluator).successful_attempts,
            1
        );
    }
    assert_ne!(positions.0.inputs, pair.positions().0.inputs);
    assert_ne!(positions.1.inputs, pair.positions().1.inputs);
    assert_eq!(
        addresses,
        (
            &state.state as *const PreloadFrameState,
            &state.before_scratch as *const HybridFrameScratch,
            &state.after_scratch as *const HybridFrameScratch,
        )
    );
    let second = &pair.last_success().unwrap().value;
    verify_retained_native(second);
    assert!(second.before.sidecars.is_empty());
    assert!(!second.before.requirements.is_empty());
    assert_eq!(
        lanes
            .lane(BRANCHES[0])
            .continuity(rig.target, ProgrammingOwner::Position),
        Some(initial[0].clone())
    );
    assert_ne!(
        lanes
            .lane(BRANCHES[1])
            .continuity(rig.target, ProgrammingOwner::Position),
        Some(initial[1].clone())
    );
    for (branch, result) in [(BRANCHES[0], &second.before), (BRANCHES[1], &second.after)] {
        let identity = state_identity(&result.frame_token);
        assert!(
            Arc::ptr_eq(&first_identity.0, &identity.0),
            "both evaluator loans retain the same engine state lineage"
        );
        assert_eq!(identity.1, first_identity.1 + 1);
        assert_eq!(
            lanes.lane(branch).last_accepted(),
            Some(result.frame_token.clone())
        );
    }
    assert_eq!(
        compiles,
        BRANCHES.map(|branch| lanes.lane(branch).adapter().counters().compiles),
        "the unchanged captured generation retains compiled physical descriptors"
    );
}

#[test]
fn closure_constructor_borrows_the_same_episode_state_across_attempts() {
    let rig = Rig::numeric_pan();
    let mut pair = rig.pair();
    let mut state = RetainedPreloadHybridState::default();
    let mut first_identity = None;
    for _ in 0..2 {
        rig.clock.advance_millis(40);
        let capture = rig.capture();
        let (before, after) = pair.positions();
        let live = rig.live.borrow();
        let before_controls = live.controls_since(before.controls).unwrap().unwrap();
        let after_controls = live.controls_since(after.controls).unwrap().unwrap();
        let window = pair
            .prepare_window(
                &[capture],
                &[],
                &before_controls,
                &[],
                &after_controls,
                limits(),
            )
            .unwrap();
        drop(live);
        {
            let mut evaluator = RetainedPreloadHybridEvaluator::with_state(
                &rig.engine,
                rig.key.programmer,
                &mut state,
                Compatible,
                observe,
            );
            assert_eq!(
                pair.consume_window(window, &mut evaluator)
                    .successful_attempts,
                1
            );
        }
        let accepted = &pair.last_success().unwrap().value;
        assert!(!accepted.before.sidecars.is_empty());
        assert!(!accepted.after.sidecars.is_empty());
        let current = state_identity(&accepted.after.frame_token);
        if let Some((identity, revision)) = &first_identity {
            assert!(Arc::ptr_eq(identity, &current.0));
            assert_eq!(current.1, revision + 1);
        } else {
            first_identity = Some(current);
        }
    }
}
