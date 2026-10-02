//! A static-only mechanical peer (captured baseline, no sampled program) still belongs to the
//! complete physical cut of one genuinely changing owner. It joins as an independent constant;
//! no Dynamic is invented for it and no changing owner is frozen. All-static roots stay out.
//! Synthetic displaced/inverted/calibrated copies are math oracles, not lamp calibration.
use super::super::cut_coordinator::operation::{AttemptEvidence, inspect_attempt};
use super::current_cohort::{SharedRig, shared_rig, start_sized, verify_shared_native_acceptance};
use super::programs::{commanded_angles, position_definition, program};
use super::*;
use crate::runtime::dynamic_source_origins::DynamicSourceBinding;
use light_dynamics::{
    DynamicTransitionReason, DynamicValue, RetainedExpressionNode, RetainedExpressionTape,
};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq)]
enum PeerBase {
    Angles,
    Target,
}
struct StaticPeerDesk {
    shared: SharedRig,
    copy: FixtureId,
    bases: [AttributeValue; 2],
    // Independent per-copy complete-cohort fits: (cohort, owner, destination) -> pair.
    // Cohort 0 is the captured baseline pose, cohort 1 replaces owner 0 by its endpoint.
    pairs: HashMap<(usize, usize, FixtureId), [f64; 2]>,
    runtime: DynamicRuntime,
    lane: PhysicalAdapterLane<PositionAdapter>,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
}
// Head `index` aimed through the actual forward model with its own axes changed.
fn aimed(shared: &SharedRig, index: usize, pan: f64, tilt: f64) -> AttributeValue {
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
        Some(f64::from(shared.angles[0][0]) + pan),
        Some(f64::from(shared.angles[0][1])),
        Some(f64::from(shared.angles[1][1])),
    ];
    axes[index + 1] = axes[index + 1].map(|value| value + tilt);
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
impl StaticPeerDesk {
    /// Size 0.5 on head 0 only; head 1 keeps only its captured static programmer value.
    fn new(peer: PeerBase, distinct: bool, pan_delta: f64) -> Self {
        Self::new_at(peer, distinct, pan_delta, 1100)
    }
    fn new_at(peer: PeerBase, distinct: bool, pan_delta: f64, elapsed: i64) -> Self {
        let shared = shared_rig();
        let copy = FixtureId::new();
        let snapshot = shared.rig.engine.snapshot();
        let mut fixtures = snapshot.fixtures.as_ref().clone();
        fixtures[0].multipatch.push(MultiPatchInstance {
            id: copy.0,
            universe: Some(1),
            address: Some(20),
            location: FixtureLocation {
                z: if distinct { 1000 } else { 0 },
                ..Default::default()
            },
            invert_pan: distinct,
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
        let endpoint = aimed(&shared, 0, pan_delta, 10.);
        let bases = [
            angles(shared.angles[0][0], shared.angles[0][1]),
            match peer {
                PeerBase::Angles => angles(shared.angles[1][0], shared.angles[1][1]),
                PeerBase::Target => shared.targets[1].clone(),
            },
        ];
        let mut pairs = HashMap::new();
        if pan_delta == 0. {
            // Fit both complete mechanical cohorts before any Dynamic exists, independently
            // of the coordinator. Each copy uses its own mount, inversion and calibration.
            for (cohort, first) in [bases[0].clone(), endpoint.clone()].into_iter().enumerate() {
                let requested = [
                    (shared.heads[0], first),
                    (shared.heads[1], bases[1].clone()),
                ];
                let resolved = shared.rig.resolve(&requested);
                assert_eq!(resolved.results.len(), 2);
                for (index, row) in resolved.results.iter().enumerate() {
                    assert_eq!(row.achieved.outcomes.len(), 2);
                    for outcome in &row.achieved.outcomes {
                        assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
                        assert!(row.writes.iter().all(|write| !write.parked));
                        pairs.insert(
                            (cohort, index, outcome.destination),
                            outcome.result.achieved.unwrap(),
                        );
                    }
                }
            }
            if distinct {
                assert!(
                    (pairs[&(1, 0, copy)][1] - pairs[&(1, 0, shared.rig.root)][1]).abs() > 1.,
                    "the displaced copy must actually need different Tilt commands"
                );
            }
        }
        let definition = position_definition(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &endpoint).unwrap(),
            [
                DynamicValue::Family(endpoint.clone()),
                DynamicValue::Family(endpoint),
            ],
        );
        let runtime = start_sized(&shared, &bases, &[(0, definition, Some(1000))], 0.5);
        let mut desk = Self {
            shared,
            copy,
            bases,
            pairs,
            runtime,
            lane: PhysicalAdapterLane::live(PositionAdapter::default()),
            origins: Default::default(),
            scratch: Default::default(),
        };
        desk.tick();
        desk.shared.rig.clock.advance_millis(elapsed);
        desk
    }
    fn continuity(&self) -> [Option<PositionContinuity>; 2] {
        self.shared
            .heads
            .map(|head| self.lane.continuity(head, ProgrammingOwner::Position))
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
    fn programmer(&self) -> serde_json::Value {
        serde_json::to_value(
            self.shared
                .rig
                .programmers
                .get(self.shared.rig.session)
                .unwrap(),
        )
        .unwrap()
    }
    /// The actual retained Size factor of the single changing owner.
    fn factor(&self, output: &PublishedPhysicalFrame<PositionAdapter>) -> f64 {
        assert_eq!(output.sampled.samples.len(), 1, "no Dynamic for the peer");
        let sample = &output.sampled.samples[0];
        assert_eq!(sample.target, self.shared.heads[0]);
        let tape =
            RetainedExpressionTape::from_roots(&[Arc::new(sample.expression.clone())]).unwrap();
        let factors = tape
            .nodes
            .iter()
            .filter_map(|node| match node {
                RetainedExpressionNode::Scale { factor, .. } => Some(f64::from(*factor)),
                RetainedExpressionNode::Transition {
                    reason: DynamicTransitionReason::Required { .. },
                    ..
                } => panic!("this fixture exercises Size, not Required"),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(factors.len(), 1);
        assert!((factors[0] - 0.5).abs() < 1e-6);
        factors[0]
    }
    fn verify(
        &self,
        capture: &PreparedOutputFrame,
        output: &PublishedPhysicalFrame<PositionAdapter>,
        mix: f64,
    ) {
        assert_eq!(f64::from(output.sampled.samples[0].activation_mix), mix);
        verify_shared_native_acceptance(
            &self.shared,
            &self.lane,
            output,
            &[(self.shared.rig.root, 0), (self.copy, 19)],
        );
        assert_eq!(output.token, capture.frame_token());
        assert_eq!(self.runtime.snapshot().instances.len(), 1);
        let factor = self.factor(output);
        for (index, head) in self.shared.heads.into_iter().enumerate() {
            let row = output
                .results
                .iter()
                .find(|row| row.target == head)
                .unwrap();
            assert!(!row.quality.held);
            if index == 0 {
                assert_eq!(program(row).base, self.bases[0]);
                assert_eq!(program(row).samples.len(), 1, "original program retained");
            } else {
                assert!(
                    matches!(&row.requested, PositionRequest::Intent(requested) if requested == intent(&self.bases[1]).unwrap()),
                    "the static peer keeps its captured intent; no program is invented"
                );
                assert_eq!(row.value, self.bases[1]);
                assert!(matches!(
                    row.metadata.evidence,
                    FamilyProjectionEvidence::PreserveBaseline
                ));
                let evidence = row
                    .provenance
                    .sources
                    .entries()
                    .expect("exact static baseline evidence");
                assert!(!evidence.is_empty());
                assert!(evidence.iter().all(|entry| matches!(
                    entry.record().binding,
                    DynamicSourceBinding::StaticBaseline { .. }
                )));
            }
            assert_eq!(row.achieved.destinations.len(), 2);
            for destination in [self.shared.rig.root, self.copy] {
                let a = self.pairs[&(0, index, destination)];
                let b = self.pairs[&(1, index, destination)];
                let outcome = row
                    .achieved
                    .outcomes
                    .iter()
                    .find(|outcome| outcome.destination == destination)
                    .unwrap();
                assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
                let achieved = outcome.result.achieved.unwrap();
                let physical = output
                    .rendered
                    .physical
                    .instances
                    .iter()
                    .find(|instance| instance.instance_id == destination.0)
                    .unwrap();
                for axis in 0..2 {
                    let expected = a[axis] + mix * factor * (b[axis] - a[axis]);
                    assert!(
                        (achieved[axis] - expected).abs() < 0.07,
                        "owner {index} copy {destination:?}: {achieved:?} vs {a:?}->{b:?}"
                    );
                    let command = physical.axes[if axis == 0 { 0 } else { index + 1 }]
                        .absolute_degrees()
                        .unwrap();
                    assert!(
                        (command - expected).abs() < 0.07,
                        "rendered native commands match the independent per-copy oracle"
                    );
                }
                if index == 0 {
                    let value = commanded_angles(
                        &row.achieved
                            .destinations
                            .iter()
                            .find(|value| value.destination == destination)
                            .unwrap()
                            .value,
                    );
                    assert!((value[1] - a[1]).abs() > 0.5, "genuinely changing owner");
                }
            }
        }
    }
    fn verify_hold(
        &self,
        capture: &PreparedOutputFrame,
        output: &PublishedPhysicalFrame<PositionAdapter>,
    ) {
        assert!(
            !output.requirements.is_empty() || output.results.iter().any(|row| row.quality.held),
            "an incompatible static peer keeps the cut passive"
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

// Path evidence: a single changing owner never suspends a Required/Size parent here. Its
// Target endpoint is adopted inline as one complete mechanical endpoint cohort including the
// static peer (`adopt_single_program_endpoint`), so no coordinator operation consumer runs.
fn assert_inline_endpoint_cut(evidence: AttemptEvidence) {
    assert_eq!(
        (
            evidence.parents,
            evidence.endpoint_cohorts,
            evidence.completed
        ),
        (0, 0, 0),
        "no fabricated suspended operation for a single changing owner: {evidence:?}"
    );
}

#[test]
fn static_only_peer_joins_actual_size_cut_with_complete_copies() {
    for (peer, distinct) in [
        (PeerBase::Angles, false),
        (PeerBase::Target, false),
        (PeerBase::Angles, true),
        (PeerBase::Target, true),
    ] {
        let mut desk = StaticPeerDesk::new(peer, distinct, 0.);
        let authored = desk.programmer();
        let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
        assert_inline_endpoint_cut(evidence);
        desk.verify(&capture, &output, 1.);
        assert_eq!(
            desk.programmer(),
            authored,
            "speculative cuts never reprogram"
        );
        // The next frame continues from the accepted complete hold without drifting.
        desk.shared.rig.clock.advance_millis(40);
        let (capture, output) = desk.tick();
        desk.verify(&capture, &output, 1.);
    }
}

#[test]
fn static_only_peer_joins_activation_envelope_of_the_size_cut_per_copy() {
    for peer in [PeerBase::Angles, PeerBase::Target] {
        let mut desk = StaticPeerDesk::new_at(peer, true, 0., 500);
        let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
        assert_inline_endpoint_cut(evidence);
        let mix = f64::from(output.sampled.samples[0].activation_mix);
        assert!(
            mix > 0. && mix < 1.,
            "a genuine partial activation envelope: {mix}"
        );
        desk.verify(&capture, &output, mix);
    }
}

#[test]
fn static_only_peer_with_incompatible_shared_pan_keeps_cut_passive() {
    for distinct in [false, true] {
        // The changing owner's Target endpoint needs another shared Pan; the static peer
        // pins the current Pan. No complete endpoint cohort exists for any copy.
        let mut desk = StaticPeerDesk::new(PeerBase::Angles, distinct, 6.);
        let accepted = desk.continuity();
        let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
        assert_eq!(evidence.completed, 0);
        desk.verify_hold(&capture, &output);
        assert!(
            output
                .requirements
                .iter()
                .any(|row| row.target == desk.shared.heads[0]
                    && row.owner == ProgrammingOwner::Position),
            "the changing owner reports its unavailable complete cut"
        );
        assert!(
            output
                .results
                .iter()
                .flat_map(|row| &row.achieved.outcomes)
                .all(|outcome| outcome.result.status != PositionFitStatus::Fitted)
        );
        assert_eq!(
            desk.continuity(),
            accepted,
            "held diagnostic rows never replace the accepted complete hold"
        );
        assert!(
            accepted[0].is_none(),
            "the conflicting endpoint was already unavailable when the fade started"
        );
    }
}

#[test]
fn all_static_root_does_not_enter_coordinator() {
    let shared = shared_rig();
    let bases = [
        angles(shared.angles[0][0], shared.angles[0][1]),
        shared.targets[1].clone(),
    ];
    for (&head, base) in shared.heads.iter().zip(&bases) {
        shared.rig.programmers.set(
            shared.rig.session,
            head,
            ProgrammingOwner::Position.key(),
            base.clone(),
        );
    }
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    for _ in 0..2 {
        let capture = shared.rig.capture();
        let (output, evidence) = inspect_attempt(|| {
            prepare_live(
                &shared.rig,
                &capture,
                &capture,
                &lane,
                &mut runtime,
                &mut origins,
                &mut scratch,
            )
            .unwrap()
        });
        assert_eq!(
            (
                evidence.roots,
                evidence.parents,
                evidence.endpoint_cohorts,
                evidence.completed
            ),
            (0, 0, 0, 0),
            "an all-static root never enters the cut coordinator"
        );
        assert!(output.requirements.is_empty());
        assert!(output.sampled.samples.is_empty(), "no Dynamic is invented");
        for (index, head) in shared.heads.into_iter().enumerate() {
            if let Some(row) = output.results.iter().find(|row| row.target == head) {
                assert!(!row.quality.held);
                assert!(
                    matches!(&row.requested, PositionRequest::Intent(requested) if requested == intent(&bases[index]).unwrap())
                );
            }
        }
    }
}

#[test]
fn static_peer_cut_rejected_finalizer_preserves_continuity_then_retries() {
    let mut desk = StaticPeerDesk::new(PeerBase::Target, true, 0.);
    let capture = desk.shared.rig.capture();
    let wrong = desk.shared.rig.capture();
    let continuity = desk.continuity();
    assert!(
        continuity.iter().all(Option::is_some),
        "a complete accepted hold exists before the failed finalization"
    );
    let token = desk.lane.last_accepted();
    let runtime = desk.runtime.snapshot();
    let tracking = desk.lane.adapter().tracking.borrow().snapshot();
    let authored = desk.programmer();
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
    assert_eq!(desk.continuity(), continuity);
    assert_eq!(desk.lane.last_accepted(), token);
    let after = desk.lane.adapter().tracking.borrow().snapshot();
    match (&tracking, &after) {
        (Some(before), Some(after)) => assert!(Arc::ptr_eq(before, after)),
        (None, None) => {}
        _ => panic!("a rejected static-peer cut must preserve accepted tracking"),
    }
    assert_eq!(desk.programmer(), authored);
    let (capture, output) = desk.tick();
    desk.verify(&capture, &output, 1.);
}
