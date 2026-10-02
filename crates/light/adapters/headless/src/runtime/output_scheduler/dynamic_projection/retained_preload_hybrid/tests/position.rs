//! TL-556: real Position fitting in the retained paired evaluator. The authored profile is
//! synthetic; these checks prove frame/lane/cohort behavior, not physical lamp measurements.
use super::super::position_episode::{PositionPendingEpisode, PositionPendingPair};
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::pending_publication::{
    PendingEpisodeIdentity, PendingPublicationGate,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::color::profiles::{
    patched, rgb,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::position::{
    PositionAdapter, PositionContinuity, PositionPreloadObserver, PositionRequest,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::*;
use crate::runtime::preload::retained_history::PendingHistoryGap;
use crate::runtime::preload::retained_history::paired::PendingPairWindowOutcome;
use light_core::spatial::RigidTransform;
use light_core::{AttributeKey, ShowId};
use light_fixture::*;

#[path = "../state/tests.rs"]
mod evaluator_state;

type Sidecar = PhysicalHeadResult<PositionAdapter>;
type Pair = PairedPendingHistory<PendingHybridResult<Sidecar>>;
const BRANCHES: [PreloadBranch; 2] = [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease];

fn channel(head: Uuid, attribute: &str, slot: u16) -> FixtureChannel {
    let mut channel = rgb().modes[0].channels[0].clone();
    channel.id = Uuid::new_v4();
    channel.head_id = head;
    channel.attribute = AttributeKey(attribute.into());
    channel.fixture_attribute = channel.attribute.clone();
    channel.resolution = ChannelResolution::U16;
    channel.secondary_slots = vec![slot + 1];
    channel.default_raw = 32768;
    channel.highlight_raw = u16::MAX.into();
    channel.physical_min = None;
    channel.physical_max = None;
    channel.functions = vec![ChannelFunction::continuous(
        attribute,
        channel.attribute.clone(),
        u16::MAX.into(),
    )];
    channel
}

fn moving_head() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "TL-556 authored U16 two-axis".into();
    let head = profile.modes[0].heads[0].id;
    profile.geometry = GeometryGraph::template(GeometryTemplate::MovingHead, &[head]);
    profile.geometry.physical_contract = Some(GeometryPhysicalContract {
        version: 1,
        provenance: OpticalProvenance::default(),
        bracket: GeometryBracket::Fixed,
    });
    let mut bindings = Vec::new();
    for (index, role) in [PositionAxisRole::Pan, PositionAxisRole::Tilt]
        .into_iter()
        .enumerate()
    {
        let mut channel = channel(
            head,
            if index == 0 { "pan" } else { "tilt" },
            1 + 2 * index as u16,
        );
        channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
            physical_min: 720.,
            physical_max: -720.,
            unit: Some("deg".into()),
        };
        channel.functions[0].angular_motion = Some(AngularMotion {
            kind: AngularMotionKind::AbsolutePosition,
            max_speed_degrees_per_second: None,
            acceleration_degrees_per_second_squared: None,
            deceleration_degrees_per_second_squared: None,
        });
        bindings.push(MotionFunctionBinding {
            node_id: profile.geometry.nodes[index + 1].id,
            channel_id: channel.id,
            function_id: channel.functions[0].id,
            role,
        });
        profile.modes[0].channels.push(channel);
    }
    for (slot, attribute, raw) in [(5, "beam.focus", 77), (6, "intensity", 204)] {
        let mut control = channel(head, attribute, slot);
        control.resolution = ChannelResolution::U8;
        control.secondary_slots.clear();
        control.default_raw = raw;
        control.highlight_raw = 255;
        control.functions = vec![ChannelFunction::continuous(
            attribute,
            control.attribute.clone(),
            255,
        )];
        profile.modes[0].channels.push(control);
    }
    profile.modes[0].splits[0].footprint = 6;
    profile.modes[0].position_physical = Some(PositionPhysicalModel {
        version: 1,
        revision: 0,
        bindings,
    });
    let emitter = GeometryEmitter {
        id: Uuid::new_v4(),
        name: "Lens".into(),
        node_id: profile.geometry.nodes[2].id,
        head_id: None,
        origin: light_fixture::Vector3 {
            x: 0.,
            y: -600.,
            z: 100.,
        },
        orientation_degrees: light_fixture::Vector3::default(),
        beam_angle_degrees: 10.,
        field_angle_degrees: 20.,
        feather: 0.,
        focus: 1.,
        directional: true,
        layout: EmitterLayout::Point,
    };
    profile.modes[0].emitter_heads = vec![EmitterHeadBinding {
        emitter_id: emitter.id,
        head_id: head,
    }];
    profile.geometry.emitters = vec![emitter];
    profile.validate().unwrap();
    profile
}

fn install_mover(rig: &Rig, with_copy: bool) -> Option<FixtureId> {
    let mut fixture = patched(&moving_head(), rig.target, 1);
    fixture.location.z = 3000;
    let copy = with_copy.then(FixtureId::new);
    if let Some(copy) = copy {
        fixture.multipatch.push(MultiPatchInstance {
            id: copy.0,
            universe: Some(1),
            address: Some(10),
            invert_pan: true,
            position_calibration: Some(InstalledPositionCalibration {
                pan_zero_degrees: 23.,
                tilt_zero_degrees: -11.,
                ..Default::default()
            }),
            location: FixtureLocation {
                x: 2000,
                y: 0,
                z: 4000,
            },
            ..Default::default()
        });
    }
    let snapshot = rig.engine.snapshot();
    rig.engine
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![fixture].into(),
            dynamics: snapshot.dynamics.clone(),
            revision: snapshot.revision + 1,
            ..Default::default()
        })
        .unwrap();
    rig.publication.installed(rig.engine.snapshot());
    copy
}

/// Both branch seeds of one episode, forked from the same authoritative capture.
fn seeds(rig: &Rig, activation: Uuid) -> (PendingHistorySeed, PendingHistorySeed) {
    let mut live = rig.live.borrow_mut();
    let (cold, controls) = rig
        .publication
        .begin_retained_history(&mut live, &rig.engine.snapshot(), capacity(64))
        .unwrap();
    let key = PendingEpisodeKey {
        activation,
        ..rig.key
    };
    let seed = |branch| PendingHistorySeed {
        key: PendingEpisodeKey { branch, ..key },
        runtime: live.fork_for_pending_preview(),
        origins: Default::default(),
        snapshot: rig.engine.snapshot(),
        position: PendingHistoryPosition {
            inputs: rig.publication.input_capture_cursor().unwrap(),
            cold,
            controls,
        },
        live_sample: live.committed_sample_boundary(),
    };
    (seed(BRANCHES[0]), seed(BRANCHES[1]))
}

fn pair(rig: &Rig) -> Pair {
    let (before, after) = seeds(rig, rig.key.activation);
    PairedPendingHistory::new(before, after).unwrap()
}

/// Capture one new retained input and prepare its window from each branch's own cursor.
fn window<W>(
    rig: &Rig,
    (before, after): (PendingHistoryPosition, PendingHistoryPosition),
    prepare: impl FnOnce(
        &[Arc<RetainedInputCapture>],
        &light_dynamics::DynamicControlBatch,
        &light_dynamics::DynamicControlBatch,
    ) -> Result<W, PendingHistoryGap>,
) -> W {
    let input = rig.capture();
    let live = rig.live.borrow();
    let before_controls = live.controls_since(before.controls).unwrap().unwrap();
    let after_controls = live.controls_since(after.controls).unwrap().unwrap();
    prepare(&[input], &before_controls, &after_controls).unwrap()
}

fn consume(
    rig: &Rig,
    pair: &mut Pair,
    evaluator: &mut impl PendingPairEvaluator<PendingHybridResult<Sidecar>>,
) -> PendingPairWindowOutcome {
    let window = window(rig, pair.positions(), |inputs, before, after| {
        pair.prepare_window(inputs, &[], before, &[], after, limits())
    });
    pair.consume_window(window, evaluator)
}

fn owned(branch: &PendingHybridBranch<Sidecar>) -> &Sidecar {
    branch
        .sidecars
        .iter()
        .find(|row| row.owner == ProgrammingOwner::Position)
        .expect("the branch produced a fitted Position")
}

fn verify_retained_native(accepted: &PendingHybridResult<Sidecar>) {
    let sidecar = owned(&accepted.after);
    for instance in &accepted.rendered.projection.physical.instances {
        let mut expected = vec![32768, 32768, 77, 204];
        for write in &sidecar.writes {
            if write.slot.destination.0 == instance.instance_id {
                expected[write.slot.channel_index as usize] = write.raw;
            }
        }
        assert!(instance.complete);
        assert_eq!(
            instance.native_raw.as_ref(),
            expected.as_slice(),
            "retained After final physical output receives fitted Position and preserves unrelated controls"
        );
        let outcome = sidecar
            .achieved
            .outcomes
            .iter()
            .find(|outcome| outcome.destination.0 == instance.instance_id)
            .unwrap();
        let achieved = outcome.result.achieved.unwrap();
        for (role, expected) in [
            (PositionAxisRole::Pan, achieved[0]),
            (PositionAxisRole::Tilt, achieved[1]),
        ] {
            let axis = instance
                .axes
                .iter()
                .find(|axis| axis.role == Some(role))
                .unwrap();
            assert_eq!(
                axis.absolute_degrees(),
                Some(expected),
                "retained final forward calculation decodes the actual accepted native commands"
            );
        }
    }
}

fn fix_at(rig: &Rig, value: AttributeValue) {
    // The fixture is authored after Rig::new's retained Angle Current Dynamic.
    // A frozen ManualClock would otherwise make this a rank tie with its controller UUID.
    rig.clock.advance_millis(40);
    rig.programmers.set(
        rig.session,
        rig.target,
        ProgrammingOwner::Position.key(),
        value.clone(),
    );
    assert!(
        rig.programmers.apply_dynamic_values(
            rig.session,
            &[light_programmer::DynamicProgrammerValueMutation::Set {
                fixture_id: rig.target,
                attribute: ProgrammingOwner::Position.key(),
                value: DynamicSemanticValue::ProgrammingFixAt {
                    mask: ProgrammingFamilyFixAt::from_family(
                        ProgrammingOwner::Position,
                        None,
                        value
                    )
                    .unwrap(),
                    timing: Default::default(),
                },
            }],
            None,
        )
    );
}

#[test]
fn retained_position_target_fits_each_copy_from_the_paired_capture_geometry() {
    let rig = Rig::new();
    let copy = install_mover(&rig, true).unwrap();
    let target = [2., 6., 1.];
    let value = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        target,
    )));
    fix_at(&rig, value.clone());
    // Before and After both receive non-release pending masks. Check the authoritative
    // captured rows and their age before testing the composed destination results.
    let capture = rig.engine.prepare_output_frame(Default::default());
    let input = rig.engine.prepare_preload_frame(&capture, None);
    for rows in [
        &input.sources().dynamic_values_before,
        &input.sources().dynamic_values_after,
    ] {
        let dynamic = rows
            .iter()
            .map(|(_, _, row)| row)
            .find(|row| {
                row.fixture_id == rig.target
                    && matches!(row.value, DynamicSemanticValue::DynamicOn { .. })
            })
            .expect("the original Angle Current Dynamic remains present");
        let fixed = rows
            .iter()
            .map(|(_, _, row)| row)
            .find(|row| {
                row.fixture_id == rig.target
                    && matches!(row.value, DynamicSemanticValue::ProgrammingFixAt { .. })
            })
            .expect("both captured branches contain the whole Target FixAT");
        assert!(fixed.changed_at_millis > dynamic.changed_at_millis);
        let DynamicSemanticValue::ProgrammingFixAt { mask, .. } = &fixed.value else {
            unreachable!()
        };
        assert_eq!(mask.family, value);
        assert_eq!(
            mask.address.component, None,
            "FixAT owns the complete Position family"
        );
    }

    let authored_before = serde_json::to_value(rig.programmers.get(rig.session).unwrap()).unwrap();
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
        let sidecar = owned(result);
        assert_eq!(sidecar.token, result.frame_token);
        assert_eq!(sidecar.token.lane().preload_branch(), Some(branch));
        let PositionRequest::Program(program) = &sidecar.requested else {
            panic!("the original Position program must survive destination calculation")
        };
        assert_eq!(
            program.base, value,
            "the authored Target base is retained in both branches"
        );
        assert_eq!(sidecar.achieved.destinations.len(), 2);
        assert!(
            !program.samples.is_empty(),
            "the retained Dynamic/FixAT samples remain attached"
        );
        assert_eq!(
            sidecar.value, value,
            "the newer whole Target FixAT wins in both branches"
        );
        assert_eq!(
            sidecar.writes.len(),
            4,
            "Pan/Tilt for the root and its physical copy"
        );
        assert_eq!(sidecar.achieved.outcomes.len(), 2);
        let outcomes = &sidecar.achieved.outcomes;
        for destination in [rig.target, copy] {
            let outcome = outcomes
                .iter()
                .find(|o| o.destination == destination)
                .unwrap();
            assert!(!outcome.missing_mount && !outcome.input_requirement);
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            let calculated = &sidecar
                .achieved
                .destinations
                .iter()
                .find(|d| d.destination == destination)
                .unwrap()
                .value;
            assert_eq!(
                calculated, &value,
                "every copy retains the whole Target request"
            );
            assert_eq!(
                outcome.result.requested,
                Some(PositionFitRequest::Target {
                    world: Some(RigidTransform::DESK_TO_PROFILE.point(target.map(f64::from))),
                })
            );
            assert!(outcome.result.angular_error_degrees.unwrap() < 0.04);
            // Verify the final forward ray independently of the fitter's reported error.
            let pose = outcome.result.pose.unwrap();
            let origin = pose.point([0.; 3]);
            let world = RigidTransform::DESK_TO_PROFILE.point(target.map(f64::from));
            let toward: [f64; 3] = std::array::from_fn(|i| world[i] - origin[i]);
            let beam = pose.direction([0., -1., 0.]);
            let length = |v: [f64; 3]| v[0].hypot(v[1]).hypot(v[2]);
            let cosine =
                (0..3).map(|i| toward[i] * beam[i]).sum::<f64>() / (length(toward) * length(beam));
            assert!(
                cosine.clamp(-1., 1.).acos().to_degrees() < 0.06,
                "{branch:?} {destination:?}: final forward ray must hit the original Target"
            );
            assert_eq!(
                sidecar
                    .writes
                    .iter()
                    .filter(|w| w.slot.destination == destination)
                    .count(),
                2
            );
        }
        assert_ne!(
            outcomes[0].result.achieved, outcomes[1].result.achieved,
            "one world target resolves separately from different physical mount locations"
        );
        let lane = lanes.lane(branch);
        assert_eq!(lane.last_accepted(), Some(result.frame_token.clone()));
        assert_eq!(
            lane.continuity(rig.target, ProgrammingOwner::Position)
                .unwrap()
                .instances
                .len(),
            2
        );
        assert!(
            lane.adapter().counters().fits >= 2,
            "both copies fit; retained lower sources may additionally solve Current endpoints"
        );
    }
    assert_eq!(
        evaluator.observe.compose_checks, 2,
        "each branch retains its original samples"
    );
    assert_eq!(
        serde_json::to_value(rig.programmers.get(rig.session).unwrap()).unwrap(),
        authored_before,
        "destination calculation must not rewrite stored Target or Dynamic programming"
    );
}

/// Delegate every solve to the real observer. Only simulate an unavailable Before owner, and
/// inspect committed state inside finish to prove that preparation does not publish continuity.
struct InspectObserver<'a> {
    inner: PositionPreloadObserver<'a>,
    lanes: &'a PhysicalPreloadLanes<PositionAdapter>,
    target: FixtureId,
    hold_before: bool,
    expected: [Option<PositionContinuity>; 2],
    expected_tokens: [Option<CapturedFrameToken>; 2],
    compose_checks: usize,
}
impl InspectObserver<'_> {
    fn assert_uncommitted(&self) {
        for (index, branch) in BRANCHES.into_iter().enumerate() {
            assert_eq!(
                self.lanes.lane(branch).last_accepted(),
                self.expected_tokens[index]
            );
            assert_eq!(
                self.lanes
                    .lane(branch)
                    .continuity(self.target, ProgrammingOwner::Position),
                self.expected[index]
            );
        }
    }
    fn snapshot_accepted(&mut self) {
        self.expected = BRANCHES.map(|branch| {
            self.lanes
                .lane(branch)
                .continuity(self.target, ProgrammingOwner::Position)
        });
        self.expected_tokens = BRANCHES.map(|branch| self.lanes.lane(branch).last_accepted());
    }
}
impl RetainedHybridFrameObserver<Sidecar> for InspectObserver<'_> {
    fn static_program_targets(
        &mut self,
        branch: PreloadBranch,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
    ) -> Result<Vec<(FixtureId, ProgrammingOwner)>, TransitionError> {
        self.inner.static_program_targets(branch, frame, baseline)
    }
    fn prepare_programs(
        &mut self,
        branch: PreloadBranch,
        frame: HybridFrameContext<'_>,
        programs: &[HybridFamilyProgram<'_>],
    ) -> Result<(), TransitionError> {
        self.inner.prepare_programs(branch, frame, programs)
    }
    fn project_native(
        &mut self,
        branch: PreloadBranch,
        capture: &light_engine::PreparedOutputFrame,
        frame_token: &CapturedFrameToken,
        token: &mut light_engine::PreparedStaticFamilyFrame,
        sidecars: &[Sidecar],
    ) -> Result<(), TransitionError> {
        self.assert_uncommitted();
        self.inner
            .project_native(branch, capture, frame_token, token, sidecars)?;
        self.assert_uncommitted();
        Ok(())
    }
    fn begin_frame(
        &mut self,
        branch: PreloadBranch,
        token: &CapturedFrameToken,
    ) -> Result<(), TransitionError> {
        self.inner.begin_frame(branch, token)
    }
    fn prepare_current(
        &mut self,
        branch: PreloadBranch,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
        protected: &[FixtureId],
    ) -> Result<(), TransitionError> {
        self.inner
            .prepare_current(branch, frame, baseline, protected)
    }
    fn compose_program(
        &mut self,
        branch: PreloadBranch,
        program: HybridFamilyProgram<'_>,
        composer: &mut dyn HybridProgramComposer<Sidecar>,
    ) -> Result<Option<OwnedHybridProjection<Sidecar>>, TransitionError> {
        if self.hold_before && branch == BRANCHES[0] && program.owner == ProgrammingOwner::Position
        {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        }
        let base = program.base.clone();
        let samples = program.samples.to_vec();
        let result = self.inner.compose_program(branch, program, composer)?;
        if let Some(row) = &result {
            if samples.is_empty() {
                let PositionRequest::Intent(retained) = &row.sidecar.requested else {
                    panic!("static destination must retain original intent")
                };
                let AttributeValue::Position(base) = &base else {
                    panic!("static Position base")
                };
                assert_eq!(retained, base.as_ref());
                self.compose_checks += 1;
                return Ok(result);
            }
            let PositionRequest::Program(retained) = &row.sidecar.requested else {
                panic!("destination observer must retain the original program")
            };
            assert_eq!(retained.base, base);
            assert_eq!(retained.samples.len(), samples.len());
            for (original, retained) in samples.iter().zip(retained.samples.iter()) {
                match (original, retained) {
                    (FamilyCompositionSample::Known(a), FamilyCompositionSample::Known(b)) => {
                        assert!(std::ptr::eq(a.address(), b.address()));
                        assert_eq!(a.materialized_value(), b.materialized_value());
                        assert_eq!(a.rank, b.rank);
                        assert_eq!(a.activation_mix, b.activation_mix);
                    }
                    (
                        FamilyCompositionSample::WholeExpression {
                            expression: a,
                            rank: ar,
                            activation_mix: am,
                        },
                        FamilyCompositionSample::WholeExpression {
                            expression: b,
                            rank: br,
                            activation_mix: bm,
                        },
                    ) => {
                        assert!(Arc::ptr_eq(a, b));
                        assert_eq!((ar, am), (br, bm));
                    }
                    (
                        FamilyCompositionSample::CoupledExpression {
                            expression: a,
                            rank: ar,
                            activation_mix: am,
                        },
                        FamilyCompositionSample::CoupledExpression {
                            expression: b,
                            rank: br,
                            activation_mix: bm,
                        },
                    ) => {
                        assert!(Arc::ptr_eq(a, b));
                        assert_eq!((ar, am), (br, bm));
                    }
                    _ => {
                        panic!("destination calculation changed an original sample representation")
                    }
                }
            }
            self.compose_checks += 1;
        }
        Ok(result)
    }
    fn observe(
        &mut self,
        branch: PreloadBranch,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, Sidecar), TransitionError> {
        if self.hold_before
            && branch == BRANCHES[0]
            && observation.owner == ProgrammingOwner::Position
        {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        }
        self.inner.observe(branch, observation)
    }
    fn finish(
        &mut self,
        branch: PreloadBranch,
        frame: HybridFrameContext<'_>,
        rows: &mut Vec<OwnedHybridProjection<Sidecar>>,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        self.assert_uncommitted();
        self.inner.finish(branch, frame, rows, requirements)?;
        self.assert_uncommitted();
        Ok(())
    }
}

#[test]
fn retained_position_hold_and_foreign_pair_rejection_preserve_independent_accepted_continuity() {
    let rig = Rig::new();
    install_mover(&rig, false);
    let mut pair = pair(&rig);
    let live_lane = PhysicalAdapterLane::live(PositionAdapter::default());
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
    let initial = BRANCHES.map(|branch| {
        lanes
            .lane(branch)
            .continuity(rig.target, ProgrammingOwner::Position)
            .unwrap()
    });
    evaluator.observe.snapshot_accepted();
    evaluator.observe.hold_before = true;
    rig.position(90., 10.);
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
            .any(|r| r.owner == ProgrammingOwner::Position)
    );
    assert_eq!(owned(&held.after).value, position(90., 10.));
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
    evaluator.observe.snapshot_accepted();
    let accepted_success = pair.last_success().unwrap().value.after.frame_token.clone();
    let accepted_native = pair
        .last_success()
        .unwrap()
        .value
        .rendered
        .projection
        .physical
        .instances
        .iter()
        .map(|instance| (instance.instance_id, instance.native_raw.to_vec()))
        .collect::<Vec<_>>();

    // Both real branches prepare new fitted writes, but the actual engine finalizer rejects
    // their exchanged branch tokens. Neither lane may commit the prepared joint continuity.
    evaluator.observe.hold_before = false;
    evaluator.swap_finalization_tokens = true;
    rig.position(120., -20.);
    let failed = consume(&rig, &mut pair, &mut evaluator);
    assert_eq!(failed.successful_attempts, 0);
    assert_eq!(failed.failed_attempts.len(), 1);
    evaluator.observe.assert_uncommitted();
    assert_eq!(
        pair.last_success().unwrap().value.after.frame_token,
        accepted_success
    );

    assert_eq!(
        pair.last_success()
            .unwrap()
            .value
            .rendered
            .projection
            .physical
            .instances
            .iter()
            .map(|instance| (instance.instance_id, instance.native_raw.to_vec()))
            .collect::<Vec<_>>(),
        accepted_native,
        "failed pair cannot replace the last accepted physical output"
    );

    // Retrying the evaluator discards failed pending observations and publishes only once the
    // paired engine finalizer succeeds; Before and After keep their separate lane tokens.
    evaluator.swap_finalization_tokens = false;
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let retry = &pair.last_success().unwrap().value;
    verify_retained_native(retry);
    for (branch, result) in [(BRANCHES[0], &retry.before), (BRANCHES[1], &retry.after)] {
        let sidecar = owned(result);
        assert_eq!(sidecar.value, position(120., -20.));
        assert_eq!(
            sidecar.achieved.outcomes[0].result.status,
            PositionFitStatus::Fitted
        );
        assert_eq!(
            lanes.lane(branch).last_accepted(),
            Some(result.frame_token.clone())
        );
        assert_ne!(
            lanes
                .lane(branch)
                .continuity(rig.target, ProgrammingOwner::Position),
            evaluator.observe.expected[usize::from(branch == BRANCHES[1])]
        );
    }
    assert!(live_lane.last_accepted().is_none());
    assert_eq!(live_lane.adapter().counters().fits, 0);
}

/// TL-610/TL-556 test seam: this module's real Position fixture rig driving one production
/// `PositionPendingEpisode`, so the Pending publication helper can prepare readouts from
/// actually accepted pairs. Each attempt lends the episode's state and lanes to a fresh
/// evaluator inside the episode; the rig composes nothing itself.
pub(in crate::runtime) struct RealPositionPendingRig {
    rig: Rig,
    copy: Option<FixtureId>,
    /// Holds the episode's lease as the current one; the rig itself publishes nothing.
    _gate: PendingPublicationGate,
    episode: PositionPendingEpisode,
}

impl RealPositionPendingRig {
    pub(in crate::runtime) fn new(with_copy: bool) -> Self {
        let rig = Rig::new();
        let copy = install_mover(&rig, with_copy);
        let mut gate = PendingPublicationGate::default();
        let identity = episode_identity(&rig, ShowId::new(), rig.key.activation);
        let (before, after) = seeds(&rig, identity.activation);
        let episode = PositionPendingEpisode::begin(&mut gate, identity, before, after).unwrap();
        Self {
            rig,
            copy,
            _gate: gate,
            episode,
        }
    }

    pub(in crate::runtime) fn programmer(&self) -> ProgrammerId {
        self.rig.key.programmer
    }

    pub(in crate::runtime) fn target(&self) -> FixtureId {
        self.rig.target
    }

    pub(in crate::runtime) fn copy(&self) -> Option<FixtureId> {
        self.copy
    }

    /// Author a newer Pending Position after earlier captures were retained.
    pub(in crate::runtime) fn move_position(&self, pan: f32, tilt: f32) {
        self.rig.position(pan, tilt);
    }

    /// Author a static Pending value on the target (for example intensity).
    pub(in crate::runtime) fn set_pending(&self, attribute: AttributeKey, value: AttributeValue) {
        self.rig.clock.advance_millis(40);
        self.rig
            .programmers
            .set(self.rig.session, self.rig.target, attribute, value);
    }

    pub(in crate::runtime) fn attempt(&mut self) -> PendingPairWindowOutcome {
        self.attempt_with(false)
    }

    /// A real failed pair: the engine finalizer rejects exchanged branch tokens.
    pub(in crate::runtime) fn attempt_with_swapped_finalization(
        &mut self,
    ) -> PendingPairWindowOutcome {
        self.attempt_with(true)
    }

    fn attempt_with(&mut self, swap: bool) -> PendingPairWindowOutcome {
        let episode = &mut self.episode;
        let window = window(&self.rig, episode.positions(), |inputs, before, after| {
            episode.prepare_window(inputs, &[], before, &[], after, limits())
        });
        if swap {
            episode.consume_window_with_swapped_finalization(&self.rig.engine, window)
        } else {
            episode.consume_window(&self.rig.engine, window)
        }
    }

    pub(in crate::runtime) fn accepted(&self) -> Option<&PositionPendingPair> {
        self.episode.accepted()
    }

    /// The episode identity the rig began; its Programmer owns every retained capture.
    pub(in crate::runtime) fn identity(&self) -> PendingEpisodeIdentity {
        self.episode.identity()
    }

    /// The engine that evaluated every pair; readouts validate against its generation.
    pub(in crate::runtime) fn engine(&self) -> &Engine {
        &self.rig.engine
    }

    /// Author a pending whole-family Target FixAT, newer than the rig's Angle Dynamic.
    pub(in crate::runtime) fn fix_target(&self, offset: [f32; 3]) {
        fix_at(
            &self.rig,
            AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Origin,
                offset,
            ))),
        );
    }

    /// Author a pending Position Release: Before keeps the pending family, After releases it.
    pub(in crate::runtime) fn release_position(&self) {
        self.rig.clock.advance_millis(40);
        assert!(self.rig.programmers.apply_dynamic_values(
            self.rig.session,
            &[light_programmer::DynamicProgrammerValueMutation::Set {
                fixture_id: self.rig.target,
                attribute: ProgrammingOwner::Position.key(),
                value: DynamicSemanticValue::Release,
            }],
            None,
        ));
    }

    /// Install a new runtime generation (same fixtures) so accepted pairs become stale.
    pub(in crate::runtime) fn bump_generation(&self) {
        let mut snapshot = self.rig.engine.snapshot().as_ref().clone();
        snapshot.revision += 1;
        self.rig.engine.replace_snapshot(snapshot).unwrap();
        self.rig.publication.installed(self.rig.engine.snapshot());
    }
}

/// A new Pending episode of the rig's Programmer and actual show under `activation`.
fn episode_identity(rig: &Rig, show_id: ShowId, activation: Uuid) -> PendingEpisodeIdentity {
    PendingEpisodeIdentity {
        show_id,
        activation,
        programmer: rig.key.programmer,
        episode: Uuid::new_v4(),
    }
}

#[path = "position/episode_recreation.rs"]
mod episode_recreation;
#[path = "position/static_programs.rs"]
mod static_programs;
