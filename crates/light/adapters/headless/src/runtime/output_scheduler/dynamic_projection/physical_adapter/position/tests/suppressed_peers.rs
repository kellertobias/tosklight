//! A genuine runtime Resume fits against an entire peer stack removed by actual Playback
//! gates. Original sources remain captured, but their hidden operations are not conditioned.
//! Synthetic geometry/native output evidence does not establish physical lamp calibration.
use super::current_cohort::{SharedRig, run_frame, shared_rig, start};
use super::programs::{commanded_angles, position_definition, program};
use super::shared_resume::resume_occurrences;
use super::*;
use crate::runtime::dynamic_source_origins::DynamicSourceBinding;
use light_dynamics::{
    ActivationPolicy, DynamicDefinition, DynamicDefinitionSnapshot, DynamicFamilyRepresentation,
    DynamicLaneBody, DynamicReference, DynamicSpeed, DynamicTargetBinding, DynamicValue,
    DynamicValueSource, FamilyCompositionSample, ProgrammingLaneConfiguration, Rational,
    SpeedGroup,
};
use light_engine::{EnginePlaybackCommand, PoolPlaybackAction};
use light_playback::{
    DynamicPlaybackAssignment, DynamicPlaybackFaderMode, DynamicPlaybackResumePolicy,
    PlaybackDefinition, PlaybackTarget,
};
use std::collections::HashMap;

struct SuppressedDesk {
    shared: SharedRig,
    copy: FixtureId,
    definition: DynamicDefinition,
    runtime: DynamicRuntime,
    lane: PhysicalAdapterLane<PositionAdapter>,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
    identity: (Uuid, Uuid),
    hidden: Vec<DynamicDefinition>,
}
impl SuppressedDesk {
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
            hidden: Vec::new(),
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
    fn resume_progress(&self, output: &PublishedPhysicalFrame<PositionAdapter>) -> (Uuid, f64) {
        let snapshot = self.runtime.snapshot();
        assert_eq!(
            snapshot.instances.len(),
            3,
            "both suppressed Playback producers still exist"
        );
        let instance = snapshot
            .instances
            .iter()
            .find(|instance| instance.id == self.identity.0)
            .unwrap();
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

fn playback(definition: &DynamicDefinition, crossfade: bool) -> PlaybackDefinition {
    let target = PlaybackTarget::Dynamic {
        assignment: DynamicPlaybackAssignment {
            dynamic: DynamicReference {
                dynamic_id: Some(definition.id),
                last_known_pool_number: definition.pool_number,
                embedded_fallback: DynamicDefinitionSnapshot {
                    definition: Arc::new(definition.clone()),
                },
            },
            revision: 1,
            target_scope: None,
            fader_mode: DynamicPlaybackFaderMode::Master,
            priority: 10,
            activation_override: None,
            resume_policy: DynamicPlaybackResumePolicy::FollowDynamic,
            local_speed_multiplier: Rational::ONE,
            learned_duration_millis: None,
            crossfade_non_intensity: crossfade,
            auto_off_at_zero: false,
            auto_off_flash_release: false,
            auto_off_full_control: false,
        },
    };
    serde_json::from_value(serde_json::json!({
        "number": definition.pool_number, "name": "Hidden Position peer", "target": target,
        "xfade_millis": 1000,
    }))
    .unwrap()
}
impl SuppressedDesk {
    fn command(&self, number: u16, action: PoolPlaybackAction) {
        self.shared
            .rig
            .engine
            .execute_playback(EnginePlaybackCommand::Pool { number, action })
            .unwrap();
    }
    fn install_hidden(&mut self, crossfade: bool, unavailable: bool) {
        self.hidden = (2..=3)
            .map(|number| {
                let value = if number == 2 {
                    self.shared.targets[1].clone()
                } else {
                    angles(
                        self.shared.angles[1][0] + 60.,
                        self.shared.angles[1][1] + 15.,
                    )
                };
                let mut definition = position_definition(
                    DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
                    [
                        DynamicValue::Family(value.clone()),
                        DynamicValue::Family(value),
                    ],
                );
                definition.pool_number = number;
                definition.target_binding = DynamicTargetBinding::FrozenTargets {
                    targets: vec![self.shared.heads[1]],
                };
                if unavailable && number == 3 {
                    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
                        unreachable!()
                    };
                    let address = body.address.clone();
                    let ProgrammingLaneConfiguration::Keyframes(config) = &mut body.configuration
                    else {
                        unreachable!()
                    };
                    for point in &mut config.points {
                        point.source = DynamicValueSource::Preset {
                            preset_id: "999.999".into(),
                            address: address.clone(),
                            last_valid_by_target: vec![],
                            retained: None,
                        };
                    }
                }
                definition
            })
            .collect();
        let definitions = std::iter::once(self.definition.clone())
            .chain(self.hidden.iter().cloned())
            .collect::<Vec<_>>();
        let snapshot = self.shared.rig.engine.snapshot();
        self.shared
            .rig
            .engine
            .replace_snapshot(EngineSnapshot {
                dynamics: definitions.clone().into(),
                playbacks: self
                    .hidden
                    .iter()
                    .map(|definition| playback(definition, crossfade))
                    .collect::<Vec<_>>()
                    .into(),
                revision: snapshot.revision + 1,
                ..snapshot.as_ref().clone()
            })
            .unwrap();
        self.runtime.install_definitions(definitions).unwrap();
        for number in 2..=3 {
            self.command(number, PoolPlaybackAction::On);
            self.command(number, PoolPlaybackAction::SetMaster(1.));
            self.command(number, PoolPlaybackAction::SetMaster(0.));
        }
        // First sample reconciles both actual producers; the second has interior activation.
        self.tick();
        self.shared.rig.clock.advance_millis(100);
    }
    fn prove_hidden_gate(&self, capture: &PreparedOutputFrame, crossfade: bool, masters: [f32; 2]) {
        for (index, number) in (2..=3).enumerate() {
            let active = capture
                .dynamic_playbacks()
                .iter()
                .find(|entry| entry.playback_number == number)
                .unwrap();
            assert!(active.enabled);
            assert_eq!(
                active.master, masters[index],
                "actual physical fader must meet pickup before moving"
            );
            let snapshot = capture.snapshot();
            let definition = snapshot
                .playbacks
                .iter()
                .find(|entry| entry.number == number)
                .unwrap();
            let PlaybackTarget::Dynamic { assignment } = &definition.target else {
                unreachable!()
            };
            assert_eq!(assignment.crossfade_non_intensity, crossfade);
        }
    }
    fn verify_static_peer(
        &self,
        output: &PublishedPhysicalFrame<PositionAdapter>,
        expected_sources: usize,
    ) {
        let row = output
            .results
            .iter()
            .find(|row| row.target == self.shared.heads[1])
            .unwrap();
        assert_eq!(row.value, self.shared.targets[1]);
        assert_eq!(program(row).base, self.shared.targets[1]);
        assert_eq!(
            program(row).samples.len(),
            expected_sources,
            "the complete actually captured original stack is retained"
        );
        assert!(row.provenance.controls.as_ref().is_none_or(Vec::is_empty));
        let evidence = row
            .provenance
            .sources
            .entries()
            .expect("exact baseline evidence");
        assert!(!evidence.is_empty());
        assert!(evidence.iter().all(|entry| matches!(
            entry.record().binding,
            DynamicSourceBinding::StaticBaseline { .. }
        )));
        for destination in &row.achieved.destinations {
            assert_eq!(destination.value, self.shared.targets[1]);
            assert!(
                destination
                    .provenance
                    .controls
                    .as_ref()
                    .is_none_or(Vec::is_empty)
            );
        }
    }
    #[track_caller]
    fn verify_passive(
        &self,
        capture: &PreparedOutputFrame,
        output: &PublishedPhysicalFrame<PositionAdapter>,
    ) {
        assert!(
            output
                .requirements
                .iter()
                .any(|row| self.shared.heads.contains(&row.target))
        );
        assert!(
            output
                .results
                .iter()
                .all(|row| row.quality.held || row.writes.iter().all(|write| write.parked))
        );
        let baseline = self
            .shared
            .rig
            .engine
            .prepare_static_family_frame(capture, &[]);
        let baseline = self
            .shared
            .rig
            .engine
            .preview_static_family_frame(capture, baseline)
            .unwrap();
        assert_eq!(
            output.rendered.universes, baseline.universes,
            "unsupported peer cannot publish a partial motor proposal"
        );
        for head in self.shared.heads {
            match self.lane.continuity(head, ProgrammingOwner::Position) {
                Some(accepted) => {
                    let row = output.results.iter().find(|row| row.target == head).expect(
                        "a fresh lane can only accept an actually observed parked baseline",
                    );
                    assert!(row.quality.held);
                    for instance in &accepted.instances {
                        for &(index, _, captured_raw, accepted_raw) in &instance.controls {
                            assert_eq!(
                                accepted_raw, captured_raw,
                                "protected static peer continuity describes its captured native baseline"
                            );
                            let write = row
                                .writes
                                .iter()
                                .find(|write| {
                                    write.slot.destination == instance.destination
                                        && write.slot.channel_index == index
                                })
                                .unwrap();
                            assert!(write.parked);
                            assert_eq!(
                                write.raw, captured_raw,
                                "no speculative Dynamic endpoint became accepted"
                            );
                        }
                    }
                }
                None => assert!(
                    output.results.iter().all(|row| row.target != head
                        || (row.quality.held && row.writes.iter().all(|write| write.parked))),
                    "an unresolved fresh peer may have a parked diagnostic row without accepted continuity"
                ),
            }
        }
    }
}

#[test]
fn actual_suppressed_multisource_peer_keeps_baseline_and_all_native_copies_during_resume() {
    let mut desk = SuppressedDesk::new();
    desk.install_hidden(false, false);
    let (capture, output) = desk.tick();
    desk.prove_hidden_gate(&capture, false, [0., 0.]);
    assert!(output.requirements.is_empty());
    assert_eq!(output.results.len(), 2);
    let (occurrence, progress) = desk.resume_progress(&output);
    desk.verify_dynamic(&output, progress);
    desk.verify_static_peer(&output, 2);
    desk.verify_native(&capture, &output);
    assert!(
        program(
            output
                .results
                .iter()
                .find(|row| row.target == desk.shared.heads[1])
                .unwrap()
        )
        .samples
        .iter()
        .any(|sample| {
            match sample {
                FamilyCompositionSample::Known(sample) => sample.activation_mix < 1.,
                FamilyCompositionSample::WholeExpression { activation_mix, .. }
                | FamilyCompositionSample::CoupledExpression { activation_mix, .. } => {
                    *activation_mix < 1.
                }
            }
        }),
        "suppression remains exact even during genuine partial activation"
    );

    // Re-enabling a source restores the existing conservative multi-source eligibility.
    desk.command(3, PoolPlaybackAction::SetMaster(1.));
    desk.lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let (capture, enabled) = desk.tick();
    desk.prove_hidden_gate(&capture, false, [0., 1.]);
    desk.verify_passive(&capture, &enabled);
    desk.command(3, PoolPlaybackAction::SetMaster(0.));
    desk.shared.rig.clock.advance_millis(50);
    let (capture, restored) = desk.tick();
    desk.prove_hidden_gate(&capture, false, [0., 0.]);
    let (next_occurrence, next_progress) = desk.resume_progress(&restored);
    assert_eq!(next_occurrence, occurrence);
    assert!(next_progress > progress);
    desk.verify_dynamic(&restored, next_progress);
    desk.verify_static_peer(&restored, 2);
    desk.verify_native(&capture, &restored);
}

// Actual unavailable-input protection is covered separately by
// current_cohort::single_active_endpoint_does_not_treat_requirements_only_angle_peer_as_static
// and output_gates::actual_full_master_does_not_omit_requirements_only_peer_source.
#[test]
fn zero_crossfade_peer_joins_resume_cut_and_suppressed_missing_preset_keeps_static_peer() {
    for (crossfade, unavailable) in [(true, false), (false, true)] {
        let mut desk = SuppressedDesk::new();
        desk.install_hidden(crossfade, unavailable);
        desk.lane = PhysicalAdapterLane::live(PositionAdapter::default());
        let (capture, output) = desk.tick();
        desk.prove_hidden_gate(&capture, crossfade, [0., 0.]);
        if unavailable {
            // A suppressed missing Preset need not emit an input requirement. Preserve
            // its authored reference and prove the actually emitted original stack instead.
            assert!(output.requirements.is_empty());
            assert_eq!(output.results.len(), 2);
            let snapshot = capture.snapshot();
            let original = snapshot
                .dynamics
                .iter()
                .find(|definition| definition.id == desk.hidden[1].id)
                .unwrap();
            let DynamicLaneBody::Programming(body) = &original.lanes[0].body else {
                unreachable!()
            };
            let ProgrammingLaneConfiguration::Keyframes(config) = &body.configuration else {
                unreachable!()
            };
            assert!(config.points.iter().all(|point| matches!(&point.source,
                DynamicValueSource::Preset { preset_id, .. } if preset_id == "999.999")));
            assert!(output.sampled.samples.iter().all(|sample| {
                sample.target != desk.shared.heads[1]
                    || sample.lane_id != desk.hidden[1].lanes[0].id
            }));
            let (_, progress) = desk.resume_progress(&output);
            desk.verify_dynamic(&output, progress);
            desk.verify_static_peer(&output, 1);
            desk.verify_native(&capture, &output);
        } else {
            // TL-556 Resume operand cuts: a zero-master crossfaded peer is Absent from the
            // Resume scope and replays its own original goal, whose mix-0 activation leaves
            // exactly its static baseline. The single-source runner refused this cut (formerly
            // asserted passive here); the complete cut equals the suppressed-gate output.
            assert!(output.requirements.is_empty());
            assert_eq!(output.results.len(), 2);
            let (_, progress) = desk.resume_progress(&output);
            desk.verify_dynamic(&output, progress);
            let peer = output
                .results
                .iter()
                .find(|row| row.target == desk.shared.heads[1])
                .unwrap();
            assert!(!peer.quality.held);
            assert_eq!(peer.value, desk.shared.targets[1]);
            assert_eq!(program(peer).base, desk.shared.targets[1]);
            // Unlike a gate, a crossfade keeps both original sources in the captured program;
            // their actual Playback masters are 0 (proved above), so the replayed goal
            // contributes exactly the static baseline to every Resume child and the root.
            assert_eq!(program(peer).samples.len(), 2);
            for destination in &peer.achieved.destinations {
                assert_eq!(destination.value, desk.shared.targets[1]);
            }
            desk.verify_native(&capture, &output);
        }
        let accepted = desk
            .shared
            .heads
            .map(|head| desk.lane.continuity(head, ProgrammingOwner::Position));
        let runtime_before = desk.runtime.snapshot();
        // The two contexts have different valid outcomes. Reject the next finalizer
        // and prove that neither a settled Current basis nor a proposed cut can overwrite
        // either context's exact accepted native/continuity publication.
        let next_capture = desk.shared.rig.capture();
        assert!(
            prepare_live(
                &desk.shared.rig,
                &next_capture,
                &capture,
                &desk.lane,
                &mut desk.runtime,
                &mut desk.origins,
                &mut desk.scratch
            )
            .is_err()
        );
        assert_eq!(desk.runtime.snapshot(), runtime_before);
        for (index, head) in desk.shared.heads.into_iter().enumerate() {
            assert_eq!(
                desk.lane.continuity(head, ProgrammingOwner::Position),
                accepted[index],
                "a rejected subsequent candidate preserves exact accepted cohort continuity"
            );
        }
    }
}

#[test]
fn suppressed_peer_complete_cut_is_not_accepted_after_wrong_finalizer_then_retries_original_resume()
{
    let mut desk = SuppressedDesk::new();
    desk.install_hidden(false, false);
    let (capture, accepted_output) = desk.tick();
    assert!(accepted_output.requirements.is_empty());
    let (occurrence, progress) = desk.resume_progress(&accepted_output);
    let continuities = desk
        .shared
        .heads
        .map(|head| desk.lane.continuity(head, ProgrammingOwner::Position));
    let snapshot = desk.runtime.snapshot();
    let accepted_tracking = desk.lane.adapter().tracking.borrow().snapshot().unwrap();
    desk.shared.rig.clock.advance_millis(50);
    let next = desk.shared.rig.capture();
    assert!(
        prepare_live(
            &desk.shared.rig,
            &next,
            &capture,
            &desk.lane,
            &mut desk.runtime,
            &mut desk.origins,
            &mut desk.scratch
        )
        .is_err()
    );
    assert_eq!(
        desk.runtime.snapshot(),
        snapshot,
        "rejected publication rolls back producer sampling"
    );
    assert!(
        Arc::ptr_eq(
            &accepted_tracking,
            &desk.lane.adapter().tracking.borrow().snapshot().unwrap()
        ),
        "rejected finalizer cannot publish speculative cohort geometry"
    );
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        assert_eq!(
            desk.lane.continuity(head, ProgrammingOwner::Position),
            continuities[index]
        );
    }
    let (_, retry) = desk.tick();
    assert!(retry.requirements.is_empty());
    let (retry_occurrence, retry_progress) = desk.resume_progress(&retry);
    assert_eq!(retry_occurrence, occurrence);
    assert!(retry_progress > progress);
    desk.verify_dynamic(&retry, retry_progress);
    desk.verify_static_peer(&retry, 2);
}
