//! Genuine staged Required/Size emissions through the captured observer and native finalizer.
//! Shared Pan + independent Tilts and a co-located copy are synthetic mathematical fixtures;
//! these tests establish physical computation for synthetic rigs, not lamp calibration.
use super::super::cut_coordinator::operation::inspect_attempt;
use super::current_cohort::{SharedRig, shared_rig};
use super::programs::{commanded_angles, position_definition, program};
use super::*;
use light_dynamics::{
    DynamicControllerSizeRole, DynamicDefinition, DynamicDefinitionSnapshot,
    DynamicInstanceOverrides, DynamicLaneBody, DynamicOperationCorrespondence,
    DynamicOperationHandle, DynamicOperationSite, DynamicReference, DynamicTargetBinding,
    DynamicTransitionReason, DynamicValue, DynamicValueSource, DynamicValueTiming,
    ProgrammingLaneConfiguration, Rational, RetainedExpressionNode, RetainedExpressionTape,
};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
enum Kind {
    Required,
    Size,
}
struct OperationDesk {
    shared: SharedRig,
    copy: FixtureId,
    kind: Kind,
    bases: [AttributeValue; 2],
    end: [f32; 2],
    runtime: DynamicRuntime,
    lane: PhysicalAdapterLane<PositionAdapter>,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
}
fn ray(shared: &SharedRig, pan: f64, tilt: f64) -> [f32; 3] {
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
    let axes = [Some(pan), Some(tilt), Some(tilt)];
    let mut poses = forward.create_output();
    forward
        .evaluate_pose(
            &axes,
            RigidTransform::IDENTITY,
            &mut forward.create_workspace(),
            &mut poses,
        )
        .unwrap();
    RigidTransform::DESK_TO_PROFILE
        .inverse()
        .point(poses[0].world.unwrap().point([0., -10., 0.]))
        .map(|value| value as f32)
}
impl OperationDesk {
    fn new(kind: Kind, shared_emission: bool, spread: f32) -> Self {
        Self::new_configured(kind, shared_emission, spread, |_| {})
    }
    fn new_configured(
        kind: Kind,
        shared_emission: bool,
        spread: f32,
        configure: impl FnOnce(&mut DynamicDefinition),
    ) -> Self {
        Self::new_with(kind, shared_emission, spread, configure, false)
    }
    fn authored(
        shared: &SharedRig,
        kind: Kind,
        point_id: FixtureId,
        end: [f32; 2],
        mixed: bool,
    ) -> (AttributeValue, [AttributeValue; 2]) {
        if mixed {
            let bases = [
                angles(shared.angles[0][0], shared.angles[0][1]),
                shared.targets[1].clone(),
            ];
            return (angles(end[0], end[1]), bases);
        }
        let endpoint = target(
            TargetReference::Point {
                point_id: point_id.0,
            },
            ray(shared, f64::from(end[0]), f64::from(end[1])),
        );
        let bases = match kind {
            Kind::Required => shared.targets.clone(),
            Kind::Size => shared.angles.map(|pair| angles(pair[0], pair[1])),
        };
        (endpoint, bases)
    }
    /// `mixed`: an Angles endpoint over an Angle-owner and a Target-owner baseline. The Angle
    /// owner's Size completes by angle algebra; the Target owner's Size genuinely suspends.
    fn new_with(
        kind: Kind,
        shared_emission: bool,
        spread: f32,
        configure: impl FnOnce(&mut DynamicDefinition),
        mixed: bool,
    ) -> Self {
        let shared = shared_rig();
        let copy = FixtureId::new();
        let point_id = FixtureId::new();
        let end = [shared.angles[0][0] + 6., shared.angles[0][1] + 10.];
        let (endpoint, bases) = Self::authored(&shared, kind, point_id, end, mixed);
        let mut definition = position_definition(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &endpoint).unwrap(),
            [
                DynamicValue::Family(endpoint.clone()),
                DynamicValue::Family(endpoint.clone()),
            ],
        );
        definition.target_binding = if shared_emission {
            DynamicTargetBinding::LiveGroup {
                group_id: "operation-cohort".into(),
            }
        } else {
            DynamicTargetBinding::Targetless
        };
        definition.phase.span_degrees = spread;
        if matches!(kind, Kind::Required) {
            let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
                unreachable!()
            };
            let ProgrammingLaneConfiguration::Keyframes(config) = &mut body.configuration else {
                unreachable!()
            };
            config.points[0].source = DynamicValueSource::Current;
        }
        configure(&mut definition);
        let snapshot = shared.rig.engine.snapshot();
        let mut fixtures = snapshot.fixtures.as_ref().clone();
        fixtures[0].multipatch.push(MultiPatchInstance {
            id: copy.0,
            universe: Some(1),
            address: Some(20),
            ..Default::default()
        });
        fixtures.push(point(point_id, FixtureLocation::default()));
        let mut groups = snapshot.groups.as_ref().clone();
        if shared_emission {
            groups.push(light_programmer::GroupDefinition {
                id: "operation-cohort".into(),
                fixtures: shared.heads.to_vec(),
                ..Default::default()
            });
        }
        shared
            .rig
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures: fixtures.into(),
                groups: groups.into(),
                dynamics: vec![definition.clone()].into(),
                revision: snapshot.revision + 1,
                ..snapshot.as_ref().clone()
            })
            .unwrap();
        // The Point's neutral control values make its reference frame exactly the Origin;
        // Required still preserves distinct authored reference identities.
        for attribute in [
            "point.position.x",
            "point.position.y",
            "point.position.z",
            "point.rotation.x",
            "point.rotation.y",
            "point.rotation.z",
        ] {
            shared.rig.set(point_id, attribute, 0.5);
        }
        let link = Uuid::new_v4();
        let mutations = shared
            .heads
            .iter()
            .zip(&bases)
            .map(|(&head, base)| {
                shared.rig.programmers.set(
                    shared.rig.session,
                    head,
                    ProgrammingOwner::Position.key(),
                    base.clone(),
                );
                DynamicProgrammerValueMutation::Set {
                    fixture_id: head,
                    attribute: ProgrammingOwner::Position.key(),
                    value: DynamicSemanticValue::DynamicOn {
                        instance_link: if shared_emission {
                            link
                        } else {
                            Uuid::new_v4()
                        },
                        lane_id: definition.lanes[0].id,
                        dynamic: DynamicReference {
                            dynamic_id: Some(definition.id),
                            last_known_pool_number: definition.pool_number,
                            embedded_fallback: DynamicDefinitionSnapshot {
                                definition: Arc::new(definition.clone()),
                            },
                        },
                        overrides: DynamicInstanceOverrides {
                            size: if matches!(kind, Kind::Size) { 0.5 } else { 1. },
                            speed_multiplier: Rational::ONE,
                            phase_offset_degrees: 0.,
                        },
                        timing: DynamicValueTiming {
                            fade_millis: Some(1000),
                            delay_millis: None,
                        },
                    },
                }
            })
            .collect::<Vec<_>>();
        assert!(
            shared
                .rig
                .programmers
                .apply_dynamic_values(shared.rig.session, &mutations, None)
        );
        let mut runtime =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        runtime.install_definitions([definition]).unwrap();
        let mut desk = Self {
            shared,
            copy,
            kind,
            bases,
            end,
            runtime,
            lane: PhysicalAdapterLane::live(PositionAdapter::default()),
            origins: Default::default(),
            scratch: Default::default(),
        };
        desk.tick();
        desk.shared.rig.clock.advance_millis(1100);
        desk
    }
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
    fn evidence(
        &self,
        output: &PublishedPhysicalFrame<PositionAdapter>,
    ) -> (Vec<DynamicOperationHandle>, [f32; 2]) {
        let mut handles = Vec::new();
        let mut parameters = [0.; 2];
        for (index, head) in self.shared.heads.into_iter().enumerate() {
            let sample = output
                .sampled
                .samples
                .iter()
                .find(|sample| sample.target == head)
                .unwrap();
            let provenance = sample.expression.operation_provenance().unwrap();
            assert!(provenance.is_complete());
            let site = match self.kind {
                Kind::Required => DynamicOperationSite::KeyframeTransition { segment_index: 0 },
                Kind::Size => DynamicOperationSite::ControllerSize {
                    role: DynamicControllerSizeRole::FamilyScale,
                },
            };
            let handle = provenance
                .handles()
                .iter()
                .find(|handle| handle.site() == site)
                .unwrap()
                .clone();
            assert_eq!(handle.target(), head);
            assert_eq!(handle.lane_id(), sample.lane_id);
            assert_eq!(handle.emission().instance_id(), sample.instance_id);
            assert_eq!(handle.emission().controller().id, sample.controller_id);
            assert!(!handle.is_historical());
            let tape =
                RetainedExpressionTape::from_roots(&[Arc::new(sample.expression.clone())]).unwrap();
            parameters[index] = tape
                .nodes
                .iter()
                .find_map(|node| match (self.kind, node) {
                    (
                        Kind::Required,
                        RetainedExpressionNode::Transition {
                            progress,
                            reason: DynamicTransitionReason::Required { .. },
                            ..
                        },
                    ) => Some(*progress),
                    (Kind::Size, RetainedExpressionNode::Scale { factor, .. }) => Some(*factor),
                    _ => None,
                })
                .unwrap();
            handles.push(handle);
        }
        (handles, parameters)
    }
    fn verify(
        &self,
        capture: &PreparedOutputFrame,
        output: &PublishedPhysicalFrame<PositionAdapter>,
    ) {
        let (handles, parameters) = self.evidence(output);
        assert!(matches!(
            handles[0].correspondence(&handles[1]),
            DynamicOperationCorrespondence::Shared { historical: false }
        ));
        assert_eq!(handles[0].emission().targets(), self.shared.heads);
        assert!(
            output.requirements.is_empty(),
            "{:?}",
            output
                .requirements
                .iter()
                .map(|r| super::numeric::requirement_debug(&r.reason))
                .collect::<Vec<_>>()
        );
        assert_eq!(output.results.len(), 2);
        assert_eq!(self.runtime.snapshot().instances.len(), 1);
        let mut claims = HashMap::new();
        let mut authored = Vec::new();
        for (index, head) in self.shared.heads.into_iter().enumerate() {
            let row = output
                .results
                .iter()
                .find(|row| row.target == head)
                .unwrap();
            assert!(!row.quality.held);
            assert_eq!(program(row).base, self.bases[index]);
            assert_eq!(
                program(row).samples.len(),
                1,
                "original program remains retained"
            );
            assert_eq!(row.achieved.destinations.len(), 2);
            let sample = output
                .sampled
                .samples
                .iter()
                .find(|sample| sample.target == head)
                .unwrap();
            let mut source_ids = HashSet::new();
            sample
                .expression
                .visit_source_occurrences(&mut |id| {
                    source_ids.insert(id);
                })
                .unwrap();
            assert!(!source_ids.is_empty());
            authored.push(source_ids);
            for destination in [self.shared.rig.root, self.copy] {
                let outcome = row
                    .achieved
                    .outcomes
                    .iter()
                    .find(|outcome| outcome.destination == destination)
                    .unwrap();
                assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
                assert!(!outcome.input_requirement && !outcome.missing_mount);
                let value = commanded_angles(
                    &row.achieved
                        .destinations
                        .iter()
                        .find(|value| value.destination == destination)
                        .unwrap()
                        .value,
                );
                let original = self.shared.angles[index];
                let expected = [0, 1].map(|axis| {
                    f64::from(original[axis])
                        + f64::from(parameters[index]) * f64::from(self.end[axis] - original[axis])
                });
                for axis in 0..2 {
                    assert!(
                        (value[axis] - expected[axis]).abs() < 0.06,
                        "owner {index} copy {destination:?}: {value:?} != {expected:?}"
                    );
                }
                assert!(
                    (value[0] - f64::from(original[0])).abs() > 0.5,
                    "genuine changing Pan"
                );
                let physical = output
                    .rendered
                    .physical
                    .instances
                    .iter()
                    .find(|instance| instance.instance_id == destination.0)
                    .unwrap();
                assert!(physical.complete);
                assert!((physical.axes[0].absolute_degrees().unwrap() - expected[0]).abs() < 0.06);
                assert!(
                    (physical.axes[index + 1].absolute_degrees().unwrap() - expected[1]).abs()
                        < 0.06
                );
                for write in row
                    .writes
                    .iter()
                    .filter(|write| write.slot.destination == destination)
                {
                    assert!(!write.parked);
                    assert_eq!(
                        physical.native_raw[write.slot.channel_index as usize],
                        write.raw
                    );
                    if let Some(old) =
                        claims.insert((destination, write.slot.channel_index), write.raw)
                    {
                        assert_eq!(old, write.raw, "shared Pan words agree");
                    }
                }
            }
        }
        assert_ne!(
            authored[0], authored[1],
            "shared operation retains per-target source evidence"
        );
        assert_eq!(
            claims.len(),
            6,
            "complete three-motor cohort in each physical copy"
        );
        assert_eq!(output.token, capture.frame_token());
    }
    fn verify_hold(
        &self,
        capture: &PreparedOutputFrame,
        output: &PublishedPhysicalFrame<PositionAdapter>,
    ) {
        assert!(
            !output.requirements.is_empty() || output.results.iter().any(|row| row.quality.held),
            "unsupported/conflicting operation remains passive"
        );
        for row in &output.results {
            assert!(row.quality.held);
            assert!(row.writes.iter().all(|write| write.parked));
        }
        let baseline = self
            .shared
            .rig
            .engine
            .preview_static_family_frame(
                capture,
                self.shared
                    .rig
                    .engine
                    .prepare_static_family_frame(capture, &[]),
            )
            .unwrap();
        for destination in [self.shared.rig.root, self.copy] {
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
            assert_eq!(
                actual.native_raw, expected.native_raw,
                "no partial native write"
            );
        }
    }
}

#[test]
fn actual_shared_size_uses_each_angle_current_baseline_and_fits_target_endpoint_cohort() {
    let mut desk = OperationDesk::new(Kind::Size, true, 0.);
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(
        evidence.parents, 4,
        "actual pending Size parents in both copies"
    );
    assert_eq!(evidence.endpoint_cohorts, 2);
    assert_eq!(
        evidence.completed, 1,
        "the new consumer, not ordinary direct Target resolution"
    );
    desk.verify(&capture, &output);
}
#[test]
fn equal_looking_independent_size_emissions_do_not_authorize_shared_cuts() {
    let mut desk = OperationDesk::new(Kind::Size, false, 0.);
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    let (handles, progress) = desk.evidence(&output);
    assert_eq!(
        progress[0], progress[1],
        "equal factors cannot correlate emissions"
    );
    assert_eq!(evidence.completed, 0);
    assert!(matches!(
        handles[0].correspondence(&handles[1]),
        DynamicOperationCorrespondence::Uncorrelated(_)
    ));
    assert_eq!(desk.runtime.snapshot().instances.len(), 2);
    desk.verify_hold(&capture, &output);
}
#[test]
fn ordinary_required_target_resolution_with_phase_spread_still_holds_shared_pan_conflict() {
    // LiveTargetPoints resolves through the ordinary destination frame. This is a producer
    // membership/final-cohort guard, explicitly not a positive Required-consumer proof.
    let mut desk = OperationDesk::new(Kind::Required, true, 30.);
    let (capture, output) = desk.tick();
    let (handles, progress) = desk.evidence(&output);
    assert!(matches!(
        handles[0].correspondence(&handles[1]),
        DynamicOperationCorrespondence::Shared { historical: false }
    ));
    assert_ne!(
        progress[0], progress[1],
        "phase spread must exercise per-target progress"
    );
    desk.verify_hold(&capture, &output);
}
#[test]
fn rejected_operation_finalizer_preserves_runtime_continuity_and_tracking_then_retries() {
    for kind in [Kind::Size] {
        let mut desk = OperationDesk::new(kind, true, 0.);
        let capture = desk.shared.rig.capture();
        let wrong = desk.shared.rig.capture();
        let continuity = desk
            .shared
            .heads
            .map(|head| desk.lane.continuity(head, ProgrammingOwner::Position));
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
        assert_eq!(
            desk.shared
                .heads
                .map(|head| desk.lane.continuity(head, ProgrammingOwner::Position)),
            continuity
        );
        let after = desk.lane.adapter().tracking.borrow().snapshot();
        match (&tracking, &after) {
            (Some(before), Some(after)) => assert!(Arc::ptr_eq(before, after)),
            (None, None) => {}
            _ => panic!("rejected operation must preserve accepted tracking"),
        }
        let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
        assert_eq!(evidence.completed, 1);
        desk.verify(&capture, &output);
    }
}

mod graph_replay_tests;

mod nested_graph_tests;

mod resume_operand_tests;

mod synchronous_peer_tests;
