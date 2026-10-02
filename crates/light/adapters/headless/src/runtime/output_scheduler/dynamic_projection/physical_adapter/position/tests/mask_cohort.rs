//! Actual retained runtime + partial whole FixAT, with a complete shared-motor cohort.
//! Synthetic calibrated geometry is a native ownership/math oracle, not lamp accuracy evidence.
use super::current_cohort::{SharedRig, shared_rig, start};
use super::programs::{commanded_angles, position_definition, program};
use super::*;
use light_dynamics::{DynamicValue, DynamicValueTiming, FamilyCompositionSample};
use std::collections::HashMap;

struct MaskDesk {
    shared: SharedRig,
    copy: FixtureId,
    prefix: AttributeValue,
    peer: AttributeValue,
    mask: AttributeValue,
    prefix_pairs: HashMap<FixtureId, [f64; 2]>,
    peer_pairs: HashMap<FixtureId, [f64; 2]>,
    runtime: DynamicRuntime,
    lane: PhysicalAdapterLane<PositionAdapter>,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
}
fn aimed(shared: &SharedRig, index: usize, delta: f64) -> AttributeValue {
    let snapshot = shared.rig.engine.snapshot();
    let profile = snapshot.fixtures[0]
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap();
    let forward = CompiledPositionForward::compile(
        profile,
        profile.modes[0].id,
        PositionInstallation::default(),
    )
    .unwrap()
    .unwrap();
    let mut axes = [
        Some(f64::from(shared.angles[0][0])),
        Some(f64::from(shared.angles[0][1])),
        Some(f64::from(shared.angles[1][1])),
    ];
    axes[index + 1] = axes[index + 1].map(|value| value + delta);
    let mut poses = forward.create_output();
    forward
        .evaluate_pose(
            &axes,
            RigidTransform::IDENTITY,
            &mut forward.create_workspace(),
            &mut poses,
        )
        .unwrap();
    let world = RigidTransform::DESK_TO_PROFILE
        .inverse()
        .point(poses[index].world.unwrap().point([0., -10., 0.]));
    target(TargetReference::Origin, world.map(|value| value as f32))
}
impl MaskDesk {
    fn tick(&mut self) -> (PreparedOutputFrame, PublishedPhysicalFrame<PositionAdapter>) {
        let capture = self.shared.rig.capture();
        let output = prepare_live(
            &self.shared.rig,
            &capture,
            &capture,
            &self.lane,
            &mut self.runtime,
            &mut self.origins,
            &mut self.scratch,
        )
        .unwrap();
        (capture, output)
    }
    fn hold(&self, index: usize, value: AttributeValue, fade: Option<u64>) {
        assert!(
            self.shared.rig.programmers.apply_dynamic_values(
                self.shared.rig.session,
                &[DynamicProgrammerValueMutation::Set {
                    fixture_id: self.shared.heads[index],
                    attribute: ProgrammingOwner::Position.key(),
                    value: DynamicSemanticValue::ProgrammingFixAt {
                        mask: ProgrammingFamilyFixAt::from_family(
                            ProgrammingOwner::Position,
                            None,
                            value
                        )
                        .unwrap(),
                        timing: DynamicValueTiming {
                            fade_millis: fade,
                            delay_millis: None
                        },
                    },
                }],
                None
            )
        );
    }
    fn new() -> Self {
        Self::with_distinct_copy(false)
    }
    fn with_distinct_copy(distinct: bool) -> Self {
        let shared = shared_rig();
        let copy = FixtureId::new();
        let snapshot = shared.rig.engine.snapshot();
        let mut fixtures = snapshot.fixtures.as_ref().clone();
        fixtures[0].multipatch.push(MultiPatchInstance {
            id: copy.0,
            universe: Some(1),
            address: Some(20),
            location: FixtureLocation {
                x: 0,
                y: 0,
                z: if distinct { 1000 } else { 0 },
            },
            position_calibration: distinct.then_some(InstalledPositionCalibration {
                tilt_zero_degrees: -7.,
                ..Default::default()
            }),
            ..Default::default()
        });
        shared
            .rig
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures: fixtures.into(),
                revision: snapshot.revision + 1,
                ..snapshot.as_ref().clone()
            })
            .unwrap();
        let prefix = aimed(&shared, 0, 4.);
        let peer = aimed(&shared, 1, 8.);
        assert_ne!(prefix, shared.targets[0]);
        assert_ne!(peer, shared.targets[1]);
        let definition = position_definition(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &prefix).unwrap(),
            [
                DynamicValue::Family(prefix.clone()),
                DynamicValue::Family(prefix.clone()),
            ],
        );
        let runtime = start(&shared, &shared.targets, &[(0, definition, None)]);
        let mask = angles(shared.angles[0][0], shared.angles[0][1] + 20.);
        let mut desk = Self {
            shared,
            copy,
            prefix,
            peer,
            mask,
            runtime,
            prefix_pairs: HashMap::new(),
            peer_pairs: HashMap::new(),
            lane: PhysicalAdapterLane::live(PositionAdapter::default()),
            origins: Default::default(),
            scratch: Default::default(),
        };
        desk.hold(1, desk.peer.clone(), None);
        let (_, initial) = desk.tick();
        assert!(initial.requirements.is_empty());
        // These are accepted commands from the mask-free runtime program, before any
        // mask operand replay. Keep the full per-copy oracle instead of broadcasting root.
        for (index, head) in desk.shared.heads.into_iter().enumerate() {
            let row = initial
                .results
                .iter()
                .find(|row| row.target == head)
                .unwrap();
            assert_eq!(
                row.value,
                if index == 0 {
                    desk.prefix.clone()
                } else {
                    desk.peer.clone()
                }
            );
            for outcome in &row.achieved.outcomes {
                assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
                let pairs = if index == 0 {
                    &mut desk.prefix_pairs
                } else {
                    &mut desk.peer_pairs
                };
                pairs.insert(outcome.destination, outcome.result.achieved.unwrap());
            }
        }
        assert!(
            (desk.prefix_pairs[&desk.shared.rig.root][1]
                - (f64::from(desk.shared.angles[0][1]) + 4.))
                .abs()
                < 0.05
        );
        assert!(
            (desk.peer_pairs[&desk.shared.rig.root][1]
                - (f64::from(desk.shared.angles[1][1]) + 8.))
                .abs()
                < 0.05
        );
        if distinct {
            for pairs in [&desk.prefix_pairs, &desk.peer_pairs] {
                assert!(
                    (pairs[&desk.copy][1] - pairs[&desk.shared.rig.root][1]).abs() > 1.,
                    "the displaced copy must actually need different Tilt commands"
                );
            }
        }
        desk.shared.rig.clock.advance_millis(1);
        desk.hold(0, desk.mask.clone(), Some(1000));
        desk.tick(); // Capture the real mask activation clock, without manufacturing a mix.
        desk.shared.rig.clock.advance_millis(250);
        desk
    }
    fn verify(
        &self,
        capture: &PreparedOutputFrame,
        output: &PublishedPhysicalFrame<PositionAdapter>,
    ) {
        assert!(
            output.requirements.is_empty(),
            "actual partial mask must fit the complete captured mechanical cohort"
        );
        assert_eq!(output.results.len(), 2);
        let row = output
            .results
            .iter()
            .find(|row| row.target == self.shared.heads[0])
            .unwrap();
        let requested = program(row);
        assert_eq!(requested.base, self.shared.targets[0]);
        assert_eq!(
            requested.samples.len(),
            2,
            "preserve the original runtime source and independent partial mask"
        );
        let masks = requested
            .samples
            .iter()
            .filter_map(|sample| match sample {
                FamilyCompositionSample::Known(sample) if sample.is_fix_at() => Some(sample),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(masks.len(), 1);
        assert_eq!(
            masks[0].materialized_value(),
            Some(&DynamicValue::Family(self.mask.clone()))
        );
        let mix = masks[0].activation_mix;
        assert!((0.0..1.0).contains(&mix) && mix > 0.);
        let dynamic = output
            .sampled
            .samples
            .iter()
            .find(|sample| sample.target == self.shared.heads[0])
            .unwrap();
        assert_eq!(dynamic.activation_mix, 1.);
        let runtime = self.runtime.snapshot();
        assert_eq!(runtime.instances.len(), 1);
        assert_eq!(dynamic.instance_id, runtime.instances[0].id);
        assert_eq!(runtime.instances[0].targets, vec![self.shared.heads[0]]);
        let mask_pair = commanded_angles(&self.mask);
        let mut claims = HashMap::new();
        for (index, head) in self.shared.heads.into_iter().enumerate() {
            let row = output
                .results
                .iter()
                .find(|row| row.target == head)
                .unwrap();
            assert!(!row.quality.held);
            if index == 0 {
                for destination in &row.achieved.destinations {
                    let pair = commanded_angles(&destination.value);
                    let prefix = self.prefix_pairs[&destination.destination];
                    let expected: [f64; 2] = std::array::from_fn(|axis| {
                        prefix[axis] + (mask_pair[axis] - prefix[axis]) * f64::from(mix)
                    });
                    for axis in 0..2 {
                        assert!(
                            (pair[axis] - expected[axis]).abs() < 0.05,
                            "{pair:?} != {expected:?}"
                        );
                    }
                }
            } else {
                assert_eq!(row.value, self.peer);
            }
            assert_eq!(row.achieved.outcomes.len(), 2);
            for outcome in &row.achieved.outcomes {
                assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
                let physical = output
                    .rendered
                    .physical
                    .instances
                    .iter()
                    .find(|instance| instance.instance_id == outcome.destination.0)
                    .unwrap();
                let pair = outcome.result.achieved.unwrap();
                assert!((physical.axes[0].absolute_degrees().unwrap() - pair[0]).abs() < 0.04);
                assert!(
                    (physical.axes[index + 1].absolute_degrees().unwrap() - pair[1]).abs() < 0.04
                );
                if index == 1 {
                    assert!((pair[1] - self.peer_pairs[&outcome.destination][1]).abs() < 0.05);
                }
                for write in row
                    .writes
                    .iter()
                    .filter(|write| write.slot.destination == outcome.destination)
                {
                    assert!(!write.parked);
                    assert_eq!(
                        physical.native_raw[write.slot.channel_index as usize],
                        write.raw
                    );
                    if let Some(previous) = claims.insert(
                        (write.slot.destination, write.slot.channel_index),
                        write.raw,
                    ) {
                        assert_eq!(previous, write.raw);
                    }
                }
            }
        }
        assert_eq!(claims.len(), 6);
        assert!(
            claims
                .keys()
                .any(|(destination, _)| *destination == self.copy)
        );
        assert_eq!(output.token, capture.frame_token());
    }
}
#[test]
fn actual_partial_angle_mask_adopts_the_runtime_target_prefix_with_constant_peer_and_copies() {
    let mut desk = MaskDesk::new();
    let (capture, output) = desk.tick();
    desk.verify(&capture, &output);
    assert!(desk.shared.rig.programmers.apply_dynamic_values(
        desk.shared.rig.session,
        &[DynamicProgrammerValueMutation::Set {
            fixture_id: desk.shared.heads[0],
            attribute: ProgrammingOwner::Position.key(),
            value: DynamicSemanticValue::ProgrammingRelease { component: None }
        }],
        None
    ));
    desk.shared.rig.clock.advance_millis(1);
    let (_, released) = desk.tick();
    assert!(released.requirements.is_empty());
    let row = released
        .results
        .iter()
        .find(|row| row.target == desk.shared.heads[0])
        .unwrap();
    assert_eq!(row.value, desk.prefix);
    assert_eq!(
        program(row).samples.len(),
        1,
        "release reveals the original continuing runtime source"
    );
    assert!(
        row.achieved
            .outcomes
            .iter()
            .all(|outcome| outcome.result.status == PositionFitStatus::Fitted)
    );
}
#[test]
fn partial_mask_wrong_finalizer_leaves_all_peer_continuity_unaccepted_and_retries_fresh() {
    let mut desk = MaskDesk::new();
    let capture = desk.shared.rig.capture();
    let wrong = desk.shared.rig.capture();
    let before = desk
        .shared
        .heads
        .map(|head| desk.lane.continuity(head, ProgrammingOwner::Position));
    let runtime = desk.runtime.snapshot();
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
    assert_eq!(
        desk.shared
            .heads
            .map(|head| desk.lane.continuity(head, ProgrammingOwner::Position)),
        before
    );
    let (capture, output) = desk.tick();
    desk.verify(&capture, &output);
}
#[test]
fn missing_or_conflicting_mask_peers_remain_passive_for_the_whole_root() {
    for case in 0..2 {
        let mut desk = MaskDesk::new();
        desk.shared.rig.clock.advance_millis(1);
        let value = match case {
            0 => target(
                TargetReference::Point {
                    point_id: Uuid::new_v4(),
                },
                [0., 0., 0.],
            ),
            1 => angles(desk.shared.angles[1][0] + 90., desk.shared.angles[1][1]),
            _ => unreachable!(),
        };
        desk.hold(1, value, None);
        desk.shared.rig.clock.advance_millis(100);
        let (_, output) = desk.tick();
        assert!(
            !output.requirements.is_empty() || output.results.iter().any(|row| row.quality.held)
        );
        assert!(
            output
                .results
                .iter()
                .flat_map(|row| &row.writes)
                .all(|write| write.parked)
        );
    }
}

#[test]
fn displaced_calibrated_copy_uses_its_own_adopted_mask_prefix_and_native_words() {
    let mut desk = MaskDesk::with_distinct_copy(true);
    let (capture, output) = desk.tick();
    desk.verify(&capture, &output);
    let row = output
        .results
        .iter()
        .find(|row| row.target == desk.shared.heads[0])
        .unwrap();
    let root = row
        .achieved
        .destinations
        .iter()
        .find(|value| value.destination == desk.shared.rig.root)
        .unwrap();
    let copy = row
        .achieved
        .destinations
        .iter()
        .find(|value| value.destination == desk.copy)
        .unwrap();
    assert!(
        (commanded_angles(&root.value)[1] - commanded_angles(&copy.value)[1]).abs() > 1.,
        "broadcasting the root's adopted Angle pair must fail this test"
    );
    let root_tilt = row
        .writes
        .iter()
        .find(|write| {
            write.slot.destination == desk.shared.rig.root && write.slot.channel_index == 1
        })
        .unwrap();
    let copy_tilt = row
        .writes
        .iter()
        .find(|write| write.slot.destination == desk.copy && write.slot.channel_index == 1)
        .unwrap();
    assert_ne!(
        root_tilt.raw, copy_tilt.raw,
        "native words must include the copy's mount and calibration"
    );
}

mod changing_peer_tests;
