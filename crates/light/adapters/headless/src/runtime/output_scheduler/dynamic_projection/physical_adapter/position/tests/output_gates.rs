//! Actual Playback masters around a genuinely changing shared Resume. Synthetic profile
//! geometry proves native ownership/math only, not physical lamp calibration. No operation
//! identity is inferred from a matching master, rank, or fitted value.
use super::current_cohort::{SharedRig, shared_rig};
use super::programs::{commanded_angles, position_definition, program};
use super::shared_resume::resume_occurrences;
use super::*;
use light_dynamics::{
    ActivationPolicy, DynamicDefinition, DynamicDefinitionSnapshot, DynamicFamilyRepresentation,
    DynamicLaneBody, DynamicReference, DynamicSpeed, DynamicTargetBinding, DynamicValue,
    DynamicValueSource, ProgrammingLaneConfiguration, Rational, SpeedGroup,
};
use light_engine::{EnginePlaybackCommand, PoolPlaybackAction};
use light_playback::{
    DynamicPlaybackAssignment, DynamicPlaybackFaderMode, DynamicPlaybackResumePolicy,
    PlaybackDefinition, PlaybackTarget,
};
use std::collections::HashMap;

fn playback(definition: &DynamicDefinition, number: u16, crossfade: bool) -> PlaybackDefinition {
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
        "number": number, "name": format!("Position gates {number}"),
        "target": target, "xfade_millis": 1000,
    }))
    .unwrap()
}

struct ResumeDesk {
    shared: SharedRig,
    copy: FixtureId,
    definition: DynamicDefinition,
    runtime: DynamicRuntime,
    lane: PhysicalAdapterLane<PositionAdapter>,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
    crossfade: bool,
}
impl ResumeDesk {
    fn tick(&mut self) -> (PreparedOutputFrame, PublishedPhysicalFrame<PositionAdapter>) {
        self.tick_with_gate(if self.crossfade {
            light_dynamics::FamilyEndpointOutputControl::CrossfadeCurrent { mix: 1. }
        } else {
            light_dynamics::FamilyEndpointOutputControl::Unchanged
        })
    }
    fn tick_with_gate(
        &mut self,
        expected: light_dynamics::FamilyEndpointOutputControl,
    ) -> (PreparedOutputFrame, PublishedPhysicalFrame<PositionAdapter>) {
        let capture = self.shared.rig.capture();
        let active = capture
            .dynamic_playbacks()
            .iter()
            .find(|active| active.playback_number == 1)
            .expect("engine captured the actual running Playback");
        assert!(active.enabled);
        let snapshot = capture.snapshot();
        let playback = snapshot
            .playbacks
            .iter()
            .find(|playback| playback.number == 1)
            .unwrap();
        let PlaybackTarget::Dynamic { assignment } = &playback.target else {
            unreachable!()
        };
        assert_eq!(assignment.crossfade_non_intensity, self.crossfade);
        let actual = if assignment.crossfade_non_intensity {
            light_dynamics::FamilyEndpointOutputControl::CrossfadeCurrent { mix: active.master }
        } else if active.master == 0. {
            light_dynamics::FamilyEndpointOutputControl::Suppressed
        } else {
            light_dynamics::FamilyEndpointOutputControl::Unchanged
        };
        assert_eq!(
            actual, expected,
            "assert the captured gate before any composition or fitting; a fader waiting for pickup is not a master change"
        );
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
        let controls = output
            .sampled
            .playback_controls
            .values()
            .collect::<Vec<_>>();
        assert!(
            controls.iter().any(|control| control.identity.number() == 1
                && control.master == active.master
                && control.crossfade_non_intensity == self.crossfade),
            "the same actual captured gate reaches controller reconciliation"
        );
        (capture, output)
    }
    fn command(&self, action: PoolPlaybackAction) {
        self.shared
            .rig
            .engine
            .execute_playback(EnginePlaybackCommand::Pool { number: 1, action })
            .unwrap();
    }
    fn new(crossfade: bool) -> Self {
        let shared = shared_rig();
        let copy = FixtureId::new();
        let mut definition = position_definition(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &shared.targets[0])
                .unwrap(),
            [
                DynamicValue::Family(shared.targets[0].clone()),
                DynamicValue::Family(shared.targets[0].clone()),
            ],
        );
        definition.target_binding = DynamicTargetBinding::FrozenTargets {
            targets: shared.heads.to_vec(),
        };
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
                dynamics: vec![definition.clone()].into(),
                playbacks: vec![playback(&definition, 1, crossfade)].into(),
                revision: snapshot.revision + 1,
                ..snapshot.as_ref().clone()
            })
            .unwrap();
        for (&head, base) in shared.heads.iter().zip(&shared.targets) {
            shared.rig.programmers.set(
                shared.rig.session,
                head,
                ProgrammingOwner::Position.key(),
                base.clone(),
            );
        }
        let mut runtime =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        runtime.install_definitions([definition.clone()]).unwrap();
        let mut desk = Self {
            shared,
            copy,
            definition,
            runtime,
            crossfade,
            lane: PhysicalAdapterLane::live(PositionAdapter::default()),
            origins: Default::default(),
            scratch: Default::default(),
        };
        desk.command(PoolPlaybackAction::On);
        // SetMaster is the actual physical fader path. First meet its authoritative pickup
        // target; subsequent movement to zero/half must change the master, not only touch state.
        desk.command(PoolPlaybackAction::SetMaster(1.));
        desk.tick();
        desk.shared.rig.clock.advance_millis(1100);
        let (capture, initial) = desk.tick();
        assert!(initial.requirements.is_empty());
        assert_eq!(desk.runtime.snapshot().instances.len(), 1);
        desk.shared
            .rig
            .engine
            .execute_playback(EnginePlaybackCommand::SetDynamicsPaused(true))
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
            component: Some(ProgrammingComponent::Pan),
        };
        let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
            unreachable!()
        };
        for point in &mut configuration.points {
            point.source = DynamicValueSource::Value {
                value: DynamicValue::Scalar(desk.shared.angles[0][0] + 5.),
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
            .execute_playback(EnginePlaybackCommand::SetDynamicsPaused(false))
            .unwrap();
        desk.runtime
            .set_global_paused(false, capture.sampled_at().timestamp_millis() as u64);
        desk.tick();
        desk.shared.rig.clock.advance_millis(250);
        desk
    }
    fn prove_resume(&self, output: &PublishedPhysicalFrame<PositionAdapter>) -> (Uuid, Uuid, f64) {
        let snapshot = self.runtime.snapshot();
        assert_eq!(snapshot.instances.len(), 1);
        let instance = &snapshot.instances[0];
        assert_eq!(instance.targets, self.shared.heads);
        assert_eq!(instance.controllers.len(), 1);
        let actual = instance
            .synchronized_resume_transition
            .expect("genuine shared runtime Resume");
        let mut progress = None;
        for head in self.shared.heads {
            let sample = output
                .sampled
                .samples
                .iter()
                .find(|sample| {
                    sample.target == head && sample.lane_id == self.definition.lanes[0].id
                })
                .unwrap();
            assert_eq!(
                (sample.instance_id, sample.controller_id),
                (instance.id, instance.controllers[0].id)
            );
            let occurrences = resume_occurrences(&sample.expression);
            assert_eq!(occurrences.len(), 1);
            assert_eq!(occurrences[0].0, actual.occurrence_id);
            assert!((0.1..0.9).contains(&occurrences[0].1));
            if let Some(previous) = progress {
                assert_eq!(previous, occurrences[0].1);
            }
            progress = Some(occurrences[0].1);
        }
        (
            instance.id,
            instance.controllers[0].id,
            f64::from(progress.unwrap()),
        )
    }
}

#[test]
fn actual_full_playback_master_completes_shared_resume_and_retains_original_controls_per_copy() {
    let mut desk = ResumeDesk::new(true);
    let (_, output) = desk.tick();
    let (instance, controller, progress) = desk.prove_resume(&output);
    assert!(output.requirements.is_empty());
    assert_eq!(output.results.len(), 2);
    let mut claims = HashMap::new();
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        assert!(!row.quality.held);
        assert_eq!(program(row).base, desk.shared.targets[index]);
        assert_eq!(program(row).samples.len(), 1);
        let original_rank = match &program(row).samples[0] {
            light_dynamics::FamilyCompositionSample::Known(sample) => sample.rank,
            light_dynamics::FamilyCompositionSample::WholeExpression { rank, .. }
            | light_dynamics::FamilyCompositionSample::CoupledExpression { rank, .. } => *rank,
        };
        let identity = original_rank.dynamic_identity().unwrap();
        assert_eq!(
            (identity.instance_id, identity.controller_id),
            (instance, controller)
        );
        // The complete Pan/Tilt controller keeps a rank from its captured original cohort.
        // Its representative lane can be the required Current partner, depending on UUID order.
        let original = output
            .sampled
            .samples
            .iter()
            .find(|sample| {
                sample.target == head
                    && sample.instance_id == instance
                    && sample.controller_id == controller
                    && sample.lane_id == identity.lane_id
            })
            .expect("the representative control rank belongs to the original sampled cohort");
        assert_eq!(original_rank.priority, original.priority);
        assert_eq!(
            original_rank.changed_at_millis,
            original.activated_at_millis
        );
        assert_eq!(original_rank.changed_at_submillis_nanos, 0);
        assert_eq!(original_rank.stable_order, original.controller_id.as_u128());
        assert_eq!(row.achieved.destinations.len(), 2);
        for destination in &row.achieved.destinations {
            let actual = commanded_angles(&destination.value);
            let expected = [
                f64::from(desk.shared.angles[index][0]) + 5. * progress,
                f64::from(desk.shared.angles[index][1]),
            ];
            for axis in 0..2 {
                assert!(
                    (actual[axis] - expected[axis]).abs() < 0.04,
                    "{actual:?} != {expected:?}"
                );
            }
            assert!((actual[0] - f64::from(desk.shared.angles[index][0])).abs() > 0.5);
            let controls = destination
                .provenance
                .controls
                .as_ref()
                .expect("original full-master controller evidence");
            assert!(!controls.is_empty());
            assert!(
                controls.iter().all(|control| control.rank == original_rank),
                "native copies retain the exact original complete-pair rank: {controls:?}"
            );
        }
        for outcome in &row.achieved.outcomes {
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
        }
        assert_eq!(
            desk.lane
                .continuity(head, ProgrammingOwner::Position)
                .unwrap()
                .instances
                .len(),
            2
        );
        for write in &row.writes {
            assert!(!write.parked);
            if let Some(previous) = claims.insert(
                (write.slot.destination, write.slot.channel_index),
                write.raw,
            ) {
                assert_eq!(previous, write.raw, "one shared motor per physical copy");
            }
        }
    }
    assert_eq!(claims.len(), 6, "all three motors of both native copies");
    assert!(
        claims
            .keys()
            .any(|(destination, _)| *destination == desk.copy)
    );
}

#[test]
fn actual_zero_non_crossfading_master_suppresses_resume_sources_and_preserves_static_targets() {
    let mut desk = ResumeDesk::new(false);
    desk.command(PoolPlaybackAction::SetMaster(0.));
    let (_, output) = desk.tick_with_gate(light_dynamics::FamilyEndpointOutputControl::Suppressed);
    desk.prove_resume(&output);
    assert!(output.requirements.is_empty());
    assert_eq!(output.results.len(), 2);
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        assert_eq!(row.value, desk.shared.targets[index]);
        assert_eq!(program(row).base, desk.shared.targets[index]);
        assert_eq!(
            program(row).samples.len(),
            1,
            "suppression does not edit original programming"
        );
        assert!(!row.quality.held);
        assert!(
            row.provenance.controls.as_ref().is_none_or(Vec::is_empty),
            "suppressed controller cannot claim control observation"
        );
        let evidence = row
            .provenance
            .sources
            .entries()
            .expect("exact static baseline evidence");
        assert!(!evidence.is_empty());
        assert!(
            evidence.iter().all(|entry| matches!(
                entry.record().binding,
                crate::runtime::dynamic_source_origins::DynamicSourceBinding::StaticBaseline { .. }
            )),
            "suppressed retained leaves cannot appear as observed sources"
        );
        for destination in &row.achieved.destinations {
            assert_eq!(
                destination.value, desk.shared.targets[index],
                "static Target remains an intent, not fitted channels"
            );
            let actual = row
                .achieved
                .outcomes
                .iter()
                .find(|outcome| outcome.destination == destination.destination)
                .unwrap()
                .result
                .achieved
                .expect("fitted static Target");
            for axis in 0..2 {
                assert!((actual[axis] - f64::from(desk.shared.angles[index][axis])).abs() < 0.04);
            }
        }
    }
}

fn assert_passive(
    desk: &ResumeDesk,
    capture: &PreparedOutputFrame,
    output: &PublishedPhysicalFrame<PositionAdapter>,
) {
    for head in desk.shared.heads {
        assert!(
            output
                .requirements
                .iter()
                .any(|row| row.target == head && row.owner == ProgrammingOwner::Position)
        );
        assert!(output.results.iter().all(|row| row.target != head));
        assert!(
            desk.lane
                .continuity(head, ProgrammingOwner::Position)
                .is_none()
        );
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
        "passive cohort publishes no proposed native motor writes"
    );
}

#[test]
fn actual_partial_playback_master_remains_passive_without_envelope_cut_authority() {
    let mut desk = ResumeDesk::new(true);
    desk.command(PoolPlaybackAction::SetMaster(0.5));
    desk.lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let (capture, output) = desk
        .tick_with_gate(light_dynamics::FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.5 });
    desk.prove_resume(&output);
    assert_passive(&desk, &capture, &output);
}

#[test]
fn actual_full_master_does_not_omit_requirements_only_peer_source() {
    use crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::HybridFamilyRequirementReason;
    let mut desk = ResumeDesk::new(true);
    let mut missing = position_definition(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        },
        [DynamicValue::Scalar(0.), DynamicValue::Scalar(0.)],
    );
    missing.pool_number = 2;
    missing.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![desk.shared.heads[1]],
    };
    let DynamicLaneBody::Programming(body) = &mut missing.lanes[0].body else {
        unreachable!()
    };
    let address = body.address.clone();
    let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
        unreachable!()
    };
    for point in &mut configuration.points {
        point.source = DynamicValueSource::Preset {
            preset_id: "999.999".into(),
            address: address.clone(),
            last_valid_by_target: vec![],
            retained: None,
        };
    }
    missing.normalize_angle_pair();
    let snapshot = desk.shared.rig.engine.snapshot();
    let mut definitions = snapshot.dynamics.as_ref().clone();
    definitions.push(missing.clone());
    let mut playbacks = snapshot.playbacks.as_ref().clone();
    playbacks.push(playback(&missing, 2, true));
    desk.shared
        .rig
        .engine
        .replace_snapshot(EngineSnapshot {
            dynamics: definitions.clone().into(),
            playbacks: playbacks.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    desk.runtime.install_definitions(definitions).unwrap();
    desk.shared
        .rig
        .engine
        .execute_playback(EnginePlaybackCommand::Pool {
            number: 2,
            action: PoolPlaybackAction::On,
        })
        .unwrap();
    desk.lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let (capture, output) = desk.tick();
    assert!(
        output
            .requirements
            .iter()
            .any(|row| row.target == desk.shared.heads[1]
                && matches!(&row.reason, HybridFamilyRequirementReason::Input(_))),
        "the unavailable Preset must remain a captured peer input requirement"
    );
    assert_passive(&desk, &capture, &output);
}
