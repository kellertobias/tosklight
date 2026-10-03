//! TL-556: one genuine controller Size over an Angle owner and a Target owner. The Angle
//! owner's Size finishes inline by angle algebra; the Target owner's Size suspends. The
//! completed changing peer is re-issued at its exact original site and fitted with the
//! complete mechanical cohort and every calibrated copy. Synthetic rigs, not lamp calibration.
use super::super::super::cut_coordinator::operation::{
    with_duplicated_reach, with_evaluation_limit,
};
use super::graph_replay_tests::{assert_native_claims, endpoint_pairs};
use super::*;

type Pairs = HashMap<(usize, FixtureId), [f64; 2]>;

fn mixed(shared_emission: bool) -> OperationDesk {
    let desk = OperationDesk::new_with(Kind::Size, shared_emission, 0., |_| {}, true);
    // Displaced, inverted and calibrated second copy of the same mechanical fixture.
    let snapshot = desk.shared.rig.engine.snapshot();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    let copy = fixtures[0]
        .multipatch
        .iter_mut()
        .find(|copy| copy.id == desk.copy.0)
        .unwrap();
    copy.location.z = 1000;
    copy.invert_pan = true;
    copy.position_calibration = Some(InstalledPositionCalibration {
        tilt_zero_degrees: -7.,
        ..Default::default()
    });
    desk.shared
        .rig
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    desk
}
fn endpoint(desk: &OperationDesk) -> AttributeValue {
    angles(desk.end[0], desk.end[1])
}
/// Static complete-cohort geometry oracle: original baselines and the authored endpoint.
fn oracle(desk: &OperationDesk) -> (Pairs, Pairs) {
    let from = endpoint_pairs(desk, &desk.bases);
    let to = endpoint_pairs(desk, &[endpoint(desk), endpoint(desk)]);
    (from, to)
}
/// One complete materialized frame publishes accepted continuity for both owners.
fn accept_frame(
    desk: &mut OperationDesk,
    oracle: &(Pairs, Pairs),
) -> [Option<PositionContinuity>; 2] {
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(evidence.completed, 1);
    verify_mixed(desk, oracle, None, &capture, &output);
    desk.shared.rig.clock.advance_millis(40);
    let accepted = continuity(desk);
    assert!(accepted.iter().all(Option::is_some));
    accepted
}
fn continuity(desk: &OperationDesk) -> [Option<PositionContinuity>; 2] {
    desk.shared
        .heads
        .map(|head| desk.lane.continuity(head, ProgrammingOwner::Position))
}

/// Per-owner, per-copy native and DMX oracle. `masks` are each owner's partial fixed-mask
/// value and actual mix, applied once after the fitted Size.
fn verify_mixed(
    desk: &OperationDesk,
    (from, to): &(Pairs, Pairs),
    masks: Option<[(AttributeValue, f64); 2]>,
    capture: &PreparedOutputFrame,
    output: &PublishedPhysicalFrame<PositionAdapter>,
) {
    let (handles, parameters) = desk.evidence(output);
    assert!(matches!(
        handles[0].correspondence(&handles[1]),
        DynamicOperationCorrespondence::Shared { historical: false }
    ));
    assert_eq!(parameters, [0.5, 0.5], "actual runtime Size");
    assert!(output.requirements.is_empty());
    assert_eq!(output.results.len(), 2);
    assert_eq!(desk.runtime.snapshot().instances.len(), 1);
    let mut claims = HashMap::new();
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        assert!(!row.quality.held);
        assert_eq!(program(row).base, desk.bases[index]);
        assert_eq!(
            program(row).samples.len(),
            if masks.is_some() { 2 } else { 1 }
        );
        assert_eq!(row.achieved.destinations.len(), 2);
        for destination in [desk.shared.rig.root, desk.copy] {
            let expected: [f64; 2] = std::array::from_fn(|axis| {
                let original = from[&(index, destination)][axis];
                let sized = original
                    + f64::from(parameters[index]) * (to[&(index, destination)][axis] - original);
                match &masks {
                    Some(masks) => {
                        let mask = commanded_angles(&masks[index].0)[axis];
                        sized + masks[index].1 * (mask - sized)
                    }
                    None => sized,
                }
            });
            let pair = commanded_angles(
                &row.achieved
                    .destinations
                    .iter()
                    .find(|value| value.destination == destination)
                    .unwrap()
                    .value,
            );
            for axis in 0..2 {
                assert!(
                    (pair[axis] - expected[axis]).abs() < 0.07,
                    "owner {index}, {destination:?}: {pair:?} != {expected:?}"
                );
            }
            assert!(
                (pair[0] - from[&(index, destination)][0]).abs() > 0.5,
                "genuinely changing Pan for owner {index}"
            );
            let outcome = row
                .achieved
                .outcomes
                .iter()
                .find(|outcome| outcome.destination == destination)
                .unwrap();
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(!outcome.input_requirement && !outcome.missing_mount);
            let physical = output
                .rendered
                .physical
                .instances
                .iter()
                .find(|instance| instance.instance_id == destination.0)
                .unwrap();
            assert!(physical.complete);
            assert!((physical.axes()[0].absolute_degrees().unwrap() - pair[0]).abs() < 0.03);
            assert!(
                (physical.axes()[index + 1].absolute_degrees().unwrap() - pair[1]).abs() < 0.03
            );
            for write in row
                .writes
                .iter()
                .filter(|write| write.slot.destination == destination)
            {
                assert!(!write.parked);
                if let Some(previous) =
                    claims.insert((destination, write.slot.channel_index), write.raw)
                {
                    assert_eq!(previous, write.raw, "both owners agree on the shared Pan");
                }
            }
        }
    }
    assert_native_claims(desk, output, &claims);
    assert_eq!(output.token, capture.frame_token());
}

/// Refusal publishes no speculative owner row, keeps accepted continuity, and writes only
/// the complete captured baseline.
fn verify_refused(
    desk: &OperationDesk,
    accepted: &[Option<PositionContinuity>; 2],
    capture: &PreparedOutputFrame,
    output: &PublishedPhysicalFrame<PositionAdapter>,
) {
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        for row in output.results.iter().filter(|row| row.target == head) {
            assert!(row.quality.held, "no speculative row escapes refusal");
            assert!(row.writes.iter().all(|write| write.parked));
        }
        assert_eq!(
            desk.lane.continuity(head, ProgrammingOwner::Position),
            accepted[index],
            "held diagnostics never replace accepted continuity"
        );
    }
    let baseline = desk
        .shared
        .rig
        .engine
        .preview_static_family_frame(
            capture,
            desk.shared
                .rig
                .engine
                .prepare_static_family_frame(capture, &[]),
        )
        .unwrap();
    for destination in [desk.shared.rig.root, desk.copy] {
        let expected = baseline
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == destination.0)
            .unwrap();
        let actual = output
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == destination.0)
            .unwrap();
        assert_eq!(actual.native_raw, expected.native_raw, "no partial write");
    }
}

#[test]
fn synchronous_angle_peer_materializes_its_shared_size_with_the_suspended_target_owner() {
    let mut desk = mixed(true);
    let oracle = oracle(&desk);
    assert!(
        (oracle.0[&(1, desk.copy)][1] - oracle.0[&(1, desk.shared.rig.root)][1]).abs() > 1.,
        "displaced, calibrated copy needs its own Target-owner baseline"
    );
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(
        evidence.parents, 4,
        "both owners' Size parents in both copies: materialized Angle owner + suspended Target owner"
    );
    assert_eq!(evidence.endpoint_cohorts, 2);
    assert_eq!(evidence.completed, 1);
    assert_eq!(
        (evidence.synchronous, evidence.unmatched),
        (2, 0),
        "the Angle owner's root and calibrated copy each re-issue their original Size"
    );
    verify_mixed(&desk, &oracle, None, &capture, &output);
}

#[test]
fn equal_looking_independent_size_emissions_never_materialize_a_synchronous_peer() {
    let mut desk = mixed(false);
    let accepted = continuity(&desk);
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    let (handles, factors) = desk.evidence(&output);
    assert_eq!(
        factors[0], factors[1],
        "equal factors cannot correlate emissions"
    );
    assert!(matches!(
        handles[0].correspondence(&handles[1]),
        DynamicOperationCorrespondence::Uncorrelated(_)
    ));
    assert_eq!(desk.runtime.snapshot().instances.len(), 2);
    assert_eq!(evidence.parents, 0, "no cut without a Shared candidate");
    assert_eq!((evidence.synchronous, evidence.unmatched), (0, 1));
    assert_eq!(evidence.completed, 0);
    verify_refused(&desk, &accepted, &capture, &output);
}

#[test]
fn two_corresponding_reached_sites_on_one_peer_refuse_and_keep_accepted_continuity() {
    let mut desk = mixed(true);
    let oracle = oracle(&desk);
    let accepted = accept_frame(&mut desk, &oracle);
    let ((capture, output), evidence) = with_duplicated_reach(|| inspect_attempt(|| desk.tick()));
    assert_eq!(
        evidence.parents, 0,
        "an ambiguous peer site is never chosen"
    );
    assert_eq!((evidence.synchronous, evidence.unmatched), (0, 1));
    assert_eq!(evidence.completed, 0);
    verify_refused(&desk, &accepted, &capture, &output);
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(evidence.completed, 1, "same runtime and scratch recover");
    verify_mixed(&desk, &oracle, None, &capture, &output);
}

#[test]
fn changing_peer_whose_shared_size_is_never_reached_is_refused_not_frozen() {
    let mut desk = mixed(true);
    // A fully active fixed mask above the Angle owner's Dynamic: its program still retains
    // the Shared Size (two sources, not a constant peer), but no route ever reaches it.
    let mask = angles(desk.end[0] + 8., desk.end[1] + 12.);
    assert!(
        desk.shared.rig.programmers.apply_dynamic_values(
            desk.shared.rig.session,
            &[DynamicProgrammerValueMutation::Set {
                fixture_id: desk.shared.heads[0],
                attribute: ProgrammingOwner::Position.key(),
                value: DynamicSemanticValue::ProgrammingFixAt {
                    mask: ProgrammingFamilyFixAt::from_family(
                        ProgrammingOwner::Position,
                        None,
                        mask.clone(),
                    )
                    .unwrap(),
                    timing: DynamicValueTiming {
                        fade_millis: Some(1000),
                        delay_millis: None,
                    },
                },
            }],
            None
        )
    );
    desk.tick();
    desk.shared.rig.clock.advance_millis(1100);
    let accepted = continuity(&desk);
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    let (handles, _) = desk.evidence(&output);
    assert!(
        matches!(
            handles[0].correspondence(&handles[1]),
            DynamicOperationCorrespondence::Shared { historical: false }
        ),
        "a genuinely corresponding Size is retained, only never reached"
    );
    assert_eq!(
        evidence.parents, 0,
        "no cut without a reached corresponding site"
    );
    assert_eq!(
        (evidence.synchronous, evidence.unmatched),
        (0, 1),
        "the new consumer ran and found no reached corresponding site"
    );
    assert_eq!(
        evidence.completed, 0,
        "the inline value is never frozen into a cut"
    );
    verify_refused(&desk, &accepted, &capture, &output);
}

#[test]
fn materialized_peer_applies_its_partial_mask_suffix_exactly_once() {
    let mut desk = mixed(true);
    let oracle = oracle(&desk);
    let masks = [0, 1].map(|index| angles(desk.end[0] + 8., desk.end[1] + 12. + index as f32 * 5.));
    let mutations = desk
        .shared
        .heads
        .iter()
        .copied()
        .zip(&masks)
        .map(|(head, mask)| DynamicProgrammerValueMutation::Set {
            fixture_id: head,
            attribute: ProgrammingOwner::Position.key(),
            value: DynamicSemanticValue::ProgrammingFixAt {
                mask: ProgrammingFamilyFixAt::from_family(
                    ProgrammingOwner::Position,
                    None,
                    mask.clone(),
                )
                .unwrap(),
                timing: DynamicValueTiming {
                    fade_millis: Some(1000),
                    delay_millis: None,
                },
            },
        })
        .collect::<Vec<_>>();
    assert!(desk.shared.rig.programmers.apply_dynamic_values(
        desk.shared.rig.session,
        &mutations,
        None
    ));
    desk.tick(); // Establish the actual fixed-mask activation clock.
    desk.shared.rig.clock.advance_millis(250);
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(
        evidence.parents, 4,
        "both Size parents below their partial masks"
    );
    assert_eq!(evidence.endpoint_cohorts, 2);
    assert_eq!(evidence.completed, 1);
    assert_eq!(
        (evidence.synchronous, evidence.unmatched),
        (2, 0),
        "the Angle owner finished its Size and mask inline; it is re-issued, not frozen"
    );
    let mixes = desk.shared.heads.map(|head| {
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        let fixed = program(row)
            .samples
            .iter()
            .filter_map(|sample| match sample {
                light_dynamics::FamilyCompositionSample::Known(sample) if sample.is_fix_at() => {
                    Some(sample)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(fixed.len(), 1);
        let mix = f64::from(fixed[0].activation_mix);
        assert!(mix > 0. && mix < 1., "the mask's real fade is active");
        mix
    });
    let [first, second] = masks;
    verify_mixed(
        &desk,
        &oracle,
        Some([(first, mixes[0]), (second, mixes[1])]),
        &capture,
        &output,
    );
}

#[test]
fn restart_budget_exhaustion_refuses_cleanly_and_recovers_next_frame() {
    let mut desk = mixed(true);
    let oracle = oracle(&desk);
    let accepted = accept_frame(&mut desk, &oracle);
    // Four initial goals fit; the second synchronous restart loan does not.
    let ((capture, held), evidence) = with_evaluation_limit(5, || inspect_attempt(|| desk.tick()));
    assert_eq!(
        (evidence.synchronous, evidence.unmatched, evidence.parents),
        (1, 0, 0),
        "one restart borrowed, the next exceeds the loan budget before any cut"
    );
    assert_eq!(evidence.completed, 0);
    verify_refused(&desk, &accepted, &capture, &held);
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(evidence.completed, 1, "same runtime and scratch recover");
    assert_eq!(evidence.synchronous, 2);
    verify_mixed(&desk, &oracle, None, &capture, &output);
}

#[test]
fn rejected_finalizer_preserves_continuity_and_tracking_then_retries() {
    let mut desk = mixed(true);
    let oracle = oracle(&desk);
    let accepted = accept_frame(&mut desk, &oracle);
    let capture = desk.shared.rig.capture();
    let wrong = desk.shared.rig.capture();
    let runtime = desk.runtime.snapshot();
    let tracking = desk.lane.adapter().tracking.borrow().snapshot();
    assert!(
        prepare_live(
            &desk.shared.rig,
            &capture,
            &wrong,
            &desk.lane,
            &mut desk.runtime,
            &mut desk.origins,
            &mut desk.scratch
        )
        .is_err()
    );
    assert_eq!(desk.runtime.snapshot(), runtime);
    assert_eq!(continuity(&desk), accepted);
    let after = desk.lane.adapter().tracking.borrow().snapshot();
    match (&tracking, &after) {
        (Some(before), Some(after)) => assert!(Arc::ptr_eq(before, after)),
        (None, None) => {}
        _ => panic!("rejected finalization must preserve accepted tracking"),
    }
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(evidence.completed, 1);
    assert_eq!(evidence.synchronous, 2);
    verify_mixed(&desk, &oracle, None, &capture, &output);
    assert_ne!(
        continuity(&desk),
        accepted,
        "only the complete root cohort publishes new continuity"
    );
}
