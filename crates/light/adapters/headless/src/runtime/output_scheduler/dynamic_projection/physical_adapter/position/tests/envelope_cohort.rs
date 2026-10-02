//! Actual runtime envelopes over shared Pan/independent Tilts. A fixed peer deliberately
//! differs from its static Target; substituting that baseline is observably wrong. Synthetic
//! calibrated profile and encoded native results are not physical-lamp accuracy evidence.
use super::current_cohort::{SharedRig, shared_rig, start_sized};
use super::programs::{commanded_angles, position_definition, program};
use super::shared_resume::resume_occurrences;
use super::*;
use light_dynamics::{
    ActivationPolicy, DynamicDefinition, DynamicDefinitionSnapshot, DynamicLaneBody,
    DynamicReference, DynamicSpeed, DynamicTargetBinding, DynamicValue, DynamicValueSource,
    DynamicValueTiming, FamilyCompositionSample, ProgrammingLaneConfiguration, Rational,
    SpeedGroup,
};
use light_engine::{EnginePlaybackCommand, PoolPlaybackAction};
use light_playback::{
    DynamicPlaybackAssignment, DynamicPlaybackFaderMode, DynamicPlaybackResumePolicy,
    PlaybackDefinition, PlaybackTarget,
};
use std::collections::HashMap;

struct EnvelopeDesk {
    shared: SharedRig,
    copy: FixtureId,
    fixed: AttributeValue,
    definition: DynamicDefinition,
    runtime: DynamicRuntime,
    lane: PhysicalAdapterLane<PositionAdapter>,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
    playback: bool,
}
fn playback(definition: &DynamicDefinition) -> PlaybackDefinition {
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
            crossfade_non_intensity: true,
            auto_off_at_zero: false,
            auto_off_flash_release: false,
            auto_off_full_control: false,
        },
    };
    serde_json::from_value(serde_json::json!({
        "number":1,"name":"Envelope Current crossfade","target":target,"xfade_millis":1000,
    }))
    .unwrap()
}
fn fixed_target(shared: &SharedRig) -> AttributeValue {
    let snapshot = shared.rig.engine.snapshot();
    let fixture = &snapshot.fixtures[0];
    let profile = fixture.definition.profile_snapshot.as_ref().unwrap();
    let forward = CompiledPositionForward::compile(
        profile,
        profile.modes[0].id,
        PositionInstallation::default(),
    )
    .unwrap()
    .unwrap();
    let axes = [
        Some(f64::from(shared.angles[0][0])),
        Some(f64::from(shared.angles[0][1])),
        Some(f64::from(shared.angles[1][1]) + 8.),
    ];
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
        .point(poses[1].world.unwrap().point([0., -10., 0.]));
    target(TargetReference::Origin, world.map(|value| value as f32))
}
impl EnvelopeDesk {
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
    fn command(&self, action: PoolPlaybackAction) {
        self.shared
            .rig
            .engine
            .execute_playback(EnginePlaybackCommand::Pool { number: 1, action })
            .unwrap();
    }
    fn mask(&self, value: AttributeValue, fade: Option<u64>) {
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
                    },
                }],
                None
            )
        );
    }
    fn new(use_playback: bool) -> Self {
        Self::new_configured(use_playback, 1., true, false)
    }
    fn new_configured(
        use_playback: bool,
        controller_size: f32,
        retained_resume: bool,
        distinct_copy: bool,
    ) -> Self {
        assert!(
            !use_playback || controller_size == 1.,
            "Size override fixture uses Programmer control"
        );
        let shared = shared_rig();
        let fixed = fixed_target(&shared);
        assert_ne!(fixed, shared.targets[1]);
        let copy = FixtureId::new();
        let snapshot = shared.rig.engine.snapshot();
        let mut fixtures = snapshot.fixtures.as_ref().clone();
        fixtures[0].multipatch.push(MultiPatchInstance {
            id: copy.0,
            universe: Some(1),
            address: Some(20),
            location: FixtureLocation {
                z: if distinct_copy { 1000 } else { 0 },
                ..Default::default()
            },
            invert_pan: distinct_copy,
            position_calibration: distinct_copy.then_some(InstalledPositionCalibration {
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
        let endpoint = angles(shared.angles[0][0], shared.angles[0][1] + 10.);
        let mut definition = position_definition(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &endpoint).unwrap(),
            [
                DynamicValue::Family(endpoint.clone()),
                DynamicValue::Family(endpoint),
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
        let runtime = if use_playback {
            definition.target_binding = DynamicTargetBinding::FrozenTargets {
                targets: vec![shared.heads[0]],
            };
            let snapshot = shared.rig.engine.snapshot();
            shared
                .rig
                .engine
                .replace_snapshot(EngineSnapshot {
                    dynamics: vec![definition.clone()].into(),
                    playbacks: vec![playback(&definition)].into(),
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
            runtime
        } else {
            start_sized(
                &shared,
                &shared.targets,
                &[(0, definition.clone(), Some(1000))],
                controller_size,
            )
        };
        let mut desk = Self {
            shared,
            copy,
            fixed,
            definition,
            runtime,
            lane: PhysicalAdapterLane::live(PositionAdapter::default()),
            origins: Default::default(),
            scratch: Default::default(),
            playback: use_playback,
        };
        desk.mask(desk.fixed.clone(), None);
        if use_playback {
            desk.command(PoolPlaybackAction::On);
            desk.command(PoolPlaybackAction::SetMaster(1.));
        }
        desk.tick();
        desk.shared
            .rig
            .clock
            .advance_millis(if use_playback { 1100 } else { 250 });
        let (capture, _) = desk.tick();
        if !retained_resume {
            return desk;
        }
        // Actual pause/hot-edit/resume creates a retained Angle→Angle expression. The only
        // live materialization is its OUTER Target/Angle envelope, not invented sample IDs.
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
        let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
            unreachable!()
        };
        for point in &mut configuration.points {
            point.source = DynamicValueSource::Value {
                value: DynamicValue::Family(angles(
                    desk.shared.angles[0][0],
                    desk.shared.angles[0][1] + 20.,
                )),
            };
        }
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
        if use_playback {
            desk.command(PoolPlaybackAction::SetMaster(0.25));
        }
        desk
    }
    fn verify(
        &self,
        capture: &PreparedOutputFrame,
        output: &PublishedPhysicalFrame<PositionAdapter>,
    ) {
        assert!(
            output.requirements.is_empty(),
            "envelope must resolve against the actual constant peer"
        );
        assert_eq!(output.results.len(), 2);
        let runtime = self.runtime.snapshot();
        assert_eq!(runtime.instances.len(), 1);
        let instance = &runtime.instances[0];
        assert_eq!(instance.targets, vec![self.shared.heads[0]]);
        let transition = instance
            .synchronized_resume_transition
            .expect("actual retained runtime Resume");
        let sampled = output
            .sampled
            .samples
            .iter()
            .find(|sample| sample.target == self.shared.heads[0])
            .unwrap();
        assert_eq!(sampled.instance_id, instance.id);
        let scopes = resume_occurrences(&sampled.expression);
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0].0, transition.occurrence_id);
        let progress = f64::from(scopes[0].1);
        assert!((0.0..1.0).contains(&progress));
        let mix = if self.playback {
            let playback = capture
                .dynamic_playbacks()
                .iter()
                .find(|playback| playback.playback_number == 1)
                .unwrap();
            assert_eq!(
                playback.master, 0.25,
                "actual fader pickup must change the captured output gate"
            );
            assert_eq!(sampled.activation_mix, 1.);
            0.25
        } else {
            assert!((0.0..1.0).contains(&sampled.activation_mix));
            f64::from(sampled.activation_mix)
        };
        let expected = [
            f64::from(self.shared.angles[0][0]),
            f64::from(self.shared.angles[0][1]) + (10. + 10. * progress) * mix,
        ];
        let mut claims = HashMap::new();
        for (index, head) in self.shared.heads.into_iter().enumerate() {
            let row = output
                .results
                .iter()
                .find(|row| row.target == head)
                .unwrap();
            assert!(!row.quality.held);
            assert_eq!(program(row).base, self.shared.targets[index]);
            assert_eq!(program(row).samples.len(), 1);
            if index == 0 {
                assert!(
                    matches!(
                        &program(row).samples[0],
                        FamilyCompositionSample::WholeExpression { .. }
                            | FamilyCompositionSample::CoupledExpression { .. }
                    ),
                    "the test must exercise an actual retained original expression, not an unbound Known shortcut"
                );
                for destination in &row.achieved.destinations {
                    let pair = commanded_angles(&destination.value);
                    for axis in 0..2 {
                        assert!(
                            (pair[axis] - expected[axis]).abs() < 0.05,
                            "{pair:?} != {expected:?}"
                        );
                    }
                }
            } else {
                assert_eq!(
                    row.value, self.fixed,
                    "constant peer output must not be replaced by its static baseline"
                );
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
                    assert!((pair[1] - (f64::from(self.shared.angles[1][1]) + 8.)).abs() < 0.05);
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
fn actual_activation_envelope_uses_the_constant_peers_completed_target_for_every_copy() {
    let mut desk = EnvelopeDesk::new(false);
    let (capture, output) = desk.tick();
    desk.verify(&capture, &output);
}
#[test]
fn actual_current_playback_crossfade_uses_original_current_and_preserves_the_parent_suffix_once() {
    let mut desk = EnvelopeDesk::new(true);
    let (capture, output) = desk.tick();
    desk.verify(&capture, &output);
}
#[test]
fn envelope_wrong_finalizer_accepts_no_continuity_and_a_fresh_frame_can_retry() {
    let mut desk = EnvelopeDesk::new(true);
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
fn missing_and_conflicting_peers_remain_passive_without_partial_native_output() {
    for case in ["missing", "conflict"] {
        let mut desk = EnvelopeDesk::new(true);
        let (value, fade) = match case {
            "missing" => (
                target(
                    TargetReference::Point {
                        point_id: Uuid::new_v4(),
                    },
                    [0.; 3],
                ),
                None,
            ),
            "conflict" => (
                angles(desk.shared.angles[1][0] + 90., desk.shared.angles[1][1]),
                None,
            ),
            _ => unreachable!(),
        };
        desk.mask(value, fade);
        let (_, output) = desk.tick();
        assert!(
            !output.requirements.is_empty() || output.results.iter().any(|row| row.quality.held),
            "{case} must not silently accept an incomplete envelope cohort"
        );
        assert!(
            output
                .results
                .iter()
                .all(|row| row.writes.iter().all(|write| write.parked)),
            "{case} must protect every shared motor"
        );
    }
}

mod graph_prefix_tests;

mod partial_peer_tests;

mod held_lane_tests;
