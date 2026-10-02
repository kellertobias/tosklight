//! One genuine runtime Resume with an independently constant whole Fixed peer. The mask is
//! not assigned the Resume scope. Authored masks, original source records, all native copies
//! and accepted final geometry are checked; this is synthetic profile evidence, not a lamp test.
use super::current_cohort::{SharedRig, run_frame, shared_rig, start};
use super::programs::{commanded_angles, position_definition, program};
use super::shared_resume::resume_occurrences;
use super::*;
use crate::runtime::dynamic_source_origins::{DynamicSourceBinding, DynamicSourceOrigin};
use light_dynamics::{
    ActivationPolicy, DynamicDefinition, DynamicFamilyRepresentation, DynamicLaneBody,
    DynamicSpeed, DynamicValue, DynamicValueSource, DynamicValueTiming, FamilyCompositionSample,
    FamilySampleIdentity, ProgrammingLaneConfiguration, Rational, SpeedGroup,
};
use std::collections::HashMap;

struct FixedDesk {
    shared: SharedRig,
    copy: FixtureId,
    definition: DynamicDefinition,
    runtime: DynamicRuntime,
    lane: PhysicalAdapterLane<PositionAdapter>,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
    identity: (Uuid, Uuid),
}
impl FixedDesk {
    fn tick(&mut self) -> (PreparedOutputFrame, PublishedPhysicalFrame<PositionAdapter>) {
        run_frame(
            &self.shared,
            &mut self.runtime,
            &self.lane,
            &mut self.origins,
            &mut self.scratch,
        )
    }
    fn new() -> Self {
        let shared = shared_rig();
        let copy = FixtureId::new();
        let snapshot = shared.rig.engine.snapshot();
        let mut fixtures = snapshot.fixtures.as_ref().clone();
        fixtures[0].multipatch.push(MultiPatchInstance {
            id: copy.0,
            universe: Some(1),
            address: Some(20),
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
        let mut definition = position_definition(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &shared.targets[0])
                .unwrap(),
            [
                DynamicValue::Family(shared.targets[0].clone()),
                DynamicValue::Family(shared.targets[0].clone()),
            ],
        );
        definition.default_activation = ActivationPolicy::JoinSyncNow;
        definition.speed = DynamicSpeed::SpeedGroup {
            group: SpeedGroup::A,
            beats_per_cycle: Rational {
                numerator: 2,
                denominator: 1,
            },
        };
        let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
            unreachable!()
        };
        let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
            unreachable!()
        };
        for point in &mut configuration.points {
            point.source = DynamicValueSource::Current;
        }
        let runtime = start(
            &shared,
            &shared.targets,
            &[(0, definition.clone(), Some(1000))],
        );
        let mut desk = Self {
            shared,
            copy,
            definition,
            runtime,
            lane: PhysicalAdapterLane::live(PositionAdapter::default()),
            origins: Default::default(),
            scratch: Default::default(),
            identity: (Uuid::nil(), Uuid::nil()),
        };
        desk.tick();
        desk.shared.rig.clock.advance_millis(1100);
        let (capture, initial) = desk.tick();
        assert!(initial.requirements.is_empty());
        let snapshot = desk.runtime.snapshot();
        assert_eq!(snapshot.instances.len(), 1);
        assert_eq!(snapshot.instances[0].targets, vec![desk.shared.heads[0]]);
        assert_eq!(snapshot.instances[0].controllers.len(), 1);
        desk.identity = (
            snapshot.instances[0].id,
            snapshot.instances[0].controllers[0].id,
        );
        desk.shared
            .rig
            .engine
            .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
            .unwrap();
        desk.runtime
            .set_global_paused(true, capture.sampled_at().timestamp_millis() as u64);
        desk.tick();
        desk.definition.revision += 1;
        let DynamicLaneBody::Programming(body) = &mut desk.definition.lanes[0].body else {
            unreachable!()
        };
        body.address = DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Tilt),
        };
        let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
            unreachable!()
        };
        for point in &mut configuration.points {
            point.source = DynamicValueSource::Value {
                value: DynamicValue::Scalar(desk.shared.angles[0][1] + 10.),
            };
        }
        desk.definition.normalize_angle_pair();
        let snapshot = desk.shared.rig.engine.snapshot();
        desk.shared
            .rig
            .engine
            .replace_snapshot(EngineSnapshot {
                dynamics: vec![desk.definition.clone()].into(),
                revision: snapshot.revision + 1,
                ..snapshot.as_ref().clone()
            })
            .unwrap();
        desk.runtime
            .install_definitions([desk.definition.clone()])
            .unwrap();
        let (capture, _) = desk.tick();
        desk.shared
            .rig
            .engine
            .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(
                false,
            ))
            .unwrap();
        desk.runtime
            .set_global_paused(false, capture.sampled_at().timestamp_millis() as u64);
        desk.tick();
        desk.shared.rig.clock.advance_millis(250);
        desk
    }
    fn set_mask(&self, value: AttributeValue, fade: Option<u64>) {
        assert!(
            self.shared.rig.programmers.apply_dynamic_values(
                self.shared.rig.session,
                &[DynamicProgrammerValueMutation::Set {
                    fixture_id: self.shared.heads[1],
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
                    }
                }],
                None
            )
        );
    }
    fn resume_progress(&self, output: &PublishedPhysicalFrame<PositionAdapter>) -> (Uuid, f64) {
        let snapshot = self.runtime.snapshot();
        assert_eq!(
            snapshot.instances.len(),
            1,
            "a constant mask creates no Dynamic instance"
        );
        let instance = &snapshot.instances[0];
        assert_eq!((instance.id, instance.controllers[0].id), self.identity);
        let actual = instance
            .synchronized_resume_transition
            .expect("real original Resume occurrence");
        let sample = output
            .sampled
            .samples
            .iter()
            .find(|sample| {
                sample.target == self.shared.heads[0]
                    && sample.lane_id == self.definition.lanes[0].id
            })
            .unwrap();
        assert_eq!((sample.instance_id, sample.controller_id), self.identity);
        let occurrences = resume_occurrences(&sample.expression);
        assert_eq!(occurrences.len(), 1);
        assert_eq!(occurrences[0].0, actual.occurrence_id);
        assert!((0.1..0.9).contains(&occurrences[0].1));
        (actual.occurrence_id, f64::from(occurrences[0].1))
    }
    fn verify_dynamic(&self, output: &PublishedPhysicalFrame<PositionAdapter>, progress: f64) {
        let row = output
            .results
            .iter()
            .find(|row| row.target == self.shared.heads[0])
            .unwrap();
        assert!(!row.quality.held);
        assert_eq!(program(row).base, self.shared.targets[0]);
        assert_eq!(program(row).samples.len(), 1);
        assert_eq!(row.achieved.destinations.len(), 2);
        for destination in &row.achieved.destinations {
            let actual = commanded_angles(&destination.value);
            let expected = [
                f64::from(self.shared.angles[0][0]),
                f64::from(self.shared.angles[0][1]) + 10. * progress,
            ];
            for axis in 0..2 {
                assert!(
                    (actual[axis] - expected[axis]).abs() < 0.04,
                    "{actual:?} != {expected:?}"
                );
            }
            let controls = destination
                .provenance
                .controls
                .as_ref()
                .expect("original Dynamic control ownership");
            assert!(!controls.is_empty());
            assert!(
                controls
                    .iter()
                    .all(
                        |control| control.rank.dynamic_identity().is_some_and(|identity| (
                            identity.instance_id,
                            identity.controller_id
                        ) == self
                            .identity)
                    )
            );
        }
    }
    fn verify_native(
        &self,
        capture: &PreparedOutputFrame,
        output: &PublishedPhysicalFrame<PositionAdapter>,
    ) {
        let mut claims = HashMap::new();
        for row in &output.results {
            assert!(!row.quality.held);
            let accepted = self
                .lane
                .continuity(row.target, ProgrammingOwner::Position)
                .unwrap();
            for outcome in &row.achieved.outcomes {
                assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
                let physical = output
                    .rendered
                    .physical
                    .instances
                    .iter()
                    .find(|instance| instance.instance_id == outcome.destination.0)
                    .unwrap();
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
                    let controls = &accepted
                        .instances
                        .iter()
                        .find(|instance| instance.destination == outcome.destination)
                        .unwrap()
                        .controls;
                    assert!(
                        controls
                            .iter()
                            .any(|&(index, _, _, raw)| index == write.slot.channel_index
                                && raw == write.raw)
                    );
                    if let Some(old) = claims.insert(
                        (write.slot.destination, write.slot.channel_index),
                        write.raw,
                    ) {
                        assert_eq!(old, write.raw, "shared motor agrees across both owner rows");
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
fn whole_fixed_peer_keeps_original_mask_provenance_and_release_reveals_continuing_resume() {
    let mut desk = FixedDesk::new();
    let fixed = angles(desk.shared.angles[1][0], desk.shared.angles[1][1] + 8.);
    desk.set_mask(fixed.clone(), None);
    let (capture, output) = desk.tick();
    let (occurrence, progress) = desk.resume_progress(&output);
    assert!(
        output.requirements.is_empty(),
        "complete constant peer must participate in the real Resume cut"
    );
    assert_eq!(output.results.len(), 2);
    desk.verify_dynamic(&output, progress);
    let peer = output
        .results
        .iter()
        .find(|row| row.target == desk.shared.heads[1])
        .unwrap();
    assert_eq!(peer.value, fixed);
    assert_eq!(program(peer).base, desk.shared.targets[1]);
    let [FamilyCompositionSample::Known(mask)] = program(peer).samples.as_ref() else {
        panic!("exactly one original whole Fixed source")
    };
    assert!(mask.is_fix_at());
    assert_eq!(mask.activation_mix, 1.);
    assert!(mask.address().address().component.is_none());
    assert_eq!(
        mask.materialized_value(),
        Some(&DynamicValue::Family(fixed.clone()))
    );
    for destination in &peer.achieved.destinations {
        assert_eq!(destination.value, fixed);
        let controls = destination
            .provenance
            .controls
            .as_ref()
            .expect("original Fixed control trace");
        assert!(!controls.is_empty());
        assert!(
            controls
                .iter()
                .all(|control| matches!(control.rank.identity, FamilySampleIdentity::Fixed { .. }))
        );
        let evidence = destination
            .provenance
            .sources
            .entries()
            .expect("exact original mask source records");
        assert!(!evidence.is_empty());
        assert!(evidence.iter().all(|entry| matches!(&entry.record().binding, DynamicSourceBinding::Fixed {
            target, owner: ProgrammingOwner::Position, component: None, .. } if *target == desk.shared.heads[1])));
        assert!(evidence.iter().all(|entry| matches!(&entry.record().origin, DynamicSourceOrigin::Fixed {
            value: DynamicSemanticValue::ProgrammingFixAt { mask, .. }, .. } if mask.family == fixed)),
            "the constant peer is never attributed to the changing owner's Resume");
    }
    desk.verify_native(&capture, &output);
    let before_raw = output
        .results
        .iter()
        .flat_map(|row| &row.writes)
        .map(|write| {
            (
                (write.slot.destination, write.slot.channel_index),
                write.raw,
            )
        })
        .collect::<HashMap<_, _>>();
    assert!(desk.shared.rig.programmers.apply_dynamic_values(
        desk.shared.rig.session,
        &[DynamicProgrammerValueMutation::Set {
            fixture_id: desk.shared.heads[1],
            attribute: ProgrammingOwner::Position.key(),
            value: DynamicSemanticValue::ProgrammingRelease { component: None }
        }],
        None
    ));
    desk.shared.rig.clock.advance_millis(100);
    let (capture, released) = desk.tick();
    let (same, later) = desk.resume_progress(&released);
    assert_eq!(same, occurrence);
    assert!(
        later > progress,
        "release reveals the original running history, without restart"
    );
    assert!(released.requirements.is_empty());
    desk.verify_dynamic(&released, later);
    let peer = released
        .results
        .iter()
        .find(|row| row.target == desk.shared.heads[1])
        .unwrap();
    assert_eq!(peer.value, desk.shared.targets[1]);
    assert!(
        matches!(&peer.requested, PositionRequest::Intent(value) if value == intent(&desk.shared.targets[1]).unwrap())
    );
    assert!(
        peer.provenance
            .sources
            .entries()
            .unwrap()
            .iter()
            .all(|entry| matches!(
                entry.record().binding,
                DynamicSourceBinding::StaticBaseline { .. }
            ))
    );
    desk.verify_native(&capture, &released);
    for destination in [desk.shared.rig.root, desk.copy] {
        let changed = |channel| {
            released
                .results
                .iter()
                .flat_map(|row| &row.writes)
                .find(|write| {
                    write.slot.destination == destination && write.slot.channel_index == channel
                })
                .unwrap()
                .raw
                != before_raw[&(destination, channel)]
        };
        assert!(!changed(0), "shared Pan stays fixed");
        assert!(changed(1), "the original Dynamic Tilt continues");
        assert!(
            changed(2),
            "Fixed peer Tilt releases to its static underlying Target"
        );
    }
}

fn verify_passive(
    desk: &FixedDesk,
    capture: &PreparedOutputFrame,
    output: &PublishedPhysicalFrame<PositionAdapter>,
) {
    assert!(
        output.requirements.iter().any(
            |row| row.target == desk.shared.heads[0] && row.owner == ProgrammingOwner::Position
        )
    );
    assert!(
        output
            .results
            .iter()
            .all(|row| row.target != desk.shared.heads[0])
    );
    assert!(
        desk.lane
            .continuity(desk.shared.heads[0], ProgrammingOwner::Position)
            .is_none()
    );
    for row in &output.results {
        assert!(row.quality.held);
        assert!(
            row.achieved
                .outcomes
                .iter()
                .all(|outcome| outcome.result.status != PositionFitStatus::Fitted)
        );
        assert!(row.writes.iter().all(|write| write.parked));
    }
    let baseline = desk
        .shared
        .rig
        .engine
        .prepare_static_family_frame(capture, &[]);
    let baseline = desk
        .shared
        .rig
        .engine
        .preview_static_family_frame(capture, baseline)
        .unwrap();
    assert_eq!(
        output.rendered.universes, baseline.universes,
        "failed child cohort cannot publish proposed native writes"
    );
}

#[test]
fn whole_fixed_peer_cannot_override_an_incompatible_shared_pan_endpoint() {
    let mut desk = FixedDesk::new();
    desk.set_mask(
        angles(
            desk.shared.angles[1][0] + 90.,
            desk.shared.angles[1][1] + 8.,
        ),
        None,
    );
    desk.lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let (capture, output) = desk.tick();
    desk.resume_progress(&output);
    verify_passive(&desk, &capture, &output);
}

#[test]
fn partially_active_whole_mask_is_not_a_constant_peer() {
    // TL-556 Resume operand cuts: the partial mask is still never frozen as a constant peer.
    // It is a changing owner Absent from the Resume scope, so each Resume child replays its own
    // original goal, including its owner-local MaskAdoption stage, and the complete cohort is
    // fitted. The single-source runner refused this cut (formerly asserted passive here).
    let mut desk = FixedDesk::new();
    let fixed = angles(desk.shared.angles[1][0], desk.shared.angles[1][1] + 8.);
    desk.set_mask(fixed.clone(), Some(1000));
    desk.lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let ((capture, output), evidence) =
        super::super::cut_coordinator::operation::inspect_attempt(|| desk.tick());
    let (_, progress) = desk.resume_progress(&output);
    // The root Resume cut, and again in the root mask-adoption child of the peer, where the
    // changing Resume owner replays its own inherited Full goal.
    assert_eq!(evidence.resume_cuts, 2);
    assert_eq!(
        evidence.completed, 1,
        "complete cohort through the Resume operand consumer"
    );
    assert!(output.requirements.is_empty());
    assert_eq!(output.results.len(), 2);
    desk.verify_dynamic(&output, progress);
    let peer = output
        .results
        .iter()
        .find(|row| row.target == desk.shared.heads[1])
        .unwrap();
    let [FamilyCompositionSample::Known(mask)] = program(peer).samples.as_ref() else {
        panic!("exactly one original partial whole Fixed source")
    };
    assert!(mask.is_fix_at());
    let mix = f64::from(mask.activation_mix);
    assert!(
        mix > 0. && mix < 1.,
        "the mask is genuinely changing, not constant"
    );
    for destination in &peer.achieved.destinations {
        let actual = commanded_angles(&destination.value);
        let expected = [
            f64::from(desk.shared.angles[1][0]),
            f64::from(desk.shared.angles[1][1]) + 8. * mix,
        ];
        for axis in 0..2 {
            assert!(
                (actual[axis] - expected[axis]).abs() < 0.04,
                "the peer's own mask stage executes once: {actual:?} != {expected:?}"
            );
        }
    }
    desk.verify_native(&capture, &output);
}
