//! Actual synchronized Resume scope across two emitters sharing one Pan motor.
//! The second physical copy is deliberately co-located: both endpoint cohorts are physically
//! compatible. This does not certify arbitrary mixed scopes, changed shared controls or lamps.
use super::current_cohort::{SharedRig, run_frame, shared_rig};
use super::programs::{commanded_angles, position_definition, program};
use super::*;
use light_dynamics::{
    ActivationPolicy, DynamicDefinition, DynamicDefinitionSnapshot, DynamicFamilyRepresentation,
    DynamicInstanceOverrides, DynamicLaneBody, DynamicReference, DynamicSpeed,
    DynamicTargetBinding, DynamicTransitionReason, DynamicValue, DynamicValueSource,
    DynamicValueTiming, ProgrammingLaneConfiguration, Rational, RetainedExpressionNode,
    RetainedExpressionTape, SpeedGroup,
};
use std::collections::{HashMap, HashSet};

pub(super) fn start_shared(
    shared: &SharedRig,
    copy: FixtureId,
    definition: &DynamicDefinition,
) -> DynamicRuntime {
    let rig = &shared.rig;
    let snapshot = rig.engine.snapshot();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    fixtures[0].multipatch = vec![MultiPatchInstance {
        id: copy.0,
        universe: Some(1),
        address: Some(20),
        ..Default::default()
    }];
    rig.engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            groups: vec![light_programmer::GroupDefinition {
                id: "shared-resume".into(),
                fixtures: shared.heads.to_vec(),
                ..Default::default()
            }]
            .into(),
            dynamics: vec![definition.clone()].into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    let instance_link = Uuid::new_v4();
    let mutations = shared
        .heads
        .iter()
        .zip(&shared.targets)
        .map(|(&head, base)| {
            rig.programmers.set(
                rig.session,
                head,
                ProgrammingOwner::Position.key(),
                base.clone(),
            );
            DynamicProgrammerValueMutation::Set {
                fixture_id: head,
                attribute: ProgrammingOwner::Position.key(),
                value: DynamicSemanticValue::DynamicOn {
                    instance_link,
                    lane_id: definition.lanes[0].id,
                    dynamic: DynamicReference {
                        dynamic_id: Some(definition.id),
                        last_known_pool_number: definition.pool_number,
                        embedded_fallback: DynamicDefinitionSnapshot {
                            definition: Arc::new(definition.clone()),
                        },
                    },
                    overrides: DynamicInstanceOverrides {
                        size: 1.,
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
        rig.programmers
            .apply_dynamic_values(rig.session, &mutations, None)
    );
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
}

pub(super) fn resume_occurrences(
    expression: &light_dynamics::DynamicSampleExpression,
) -> Vec<(Uuid, f32)> {
    let tape = RetainedExpressionTape::from_roots(&[Arc::new(expression.clone())]).unwrap();
    tape.nodes
        .iter()
        .filter_map(|node| match node {
            RetainedExpressionNode::Transition {
                progress,
                reason: DynamicTransitionReason::Resume { occurrence_id },
                ..
            } => Some((*occurrence_id, *progress)),
            _ => None,
        })
        .collect()
}

#[test]
fn actual_shared_resume_retains_distinct_targets_and_fits_one_full_native_cohort_per_copy() {
    let shared = shared_rig();
    assert_ne!(shared.targets[0], shared.targets[1]);
    let copy = FixtureId::new();
    let mut definition = position_definition(
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, &shared.targets[0]).unwrap(),
        [
            DynamicValue::Family(shared.targets[0].clone()),
            DynamicValue::Family(shared.targets[0].clone()),
        ],
    );
    definition.target_binding = DynamicTargetBinding::LiveGroup {
        group_id: "shared-resume".into(),
    };
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational {
            numerator: 2,
            denominator: 1,
        },
    };
    let pan_lane = definition.lanes[0].id;
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        unreachable!()
    };
    let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
        unreachable!()
    };
    for point in &mut configuration.points {
        point.source = DynamicValueSource::Current;
    }
    let mut runtime = start_shared(&shared, copy, &definition);
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    shared.rig.clock.advance_millis(1100);
    let (capture, _) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    let original = runtime.snapshot();
    assert_eq!(
        original.instances.len(),
        1,
        "both authored rows must address one actual runtime instance"
    );
    let instance = &original.instances[0];
    assert_eq!(instance.targets, shared.heads);
    assert_eq!(instance.controllers.len(), 1);
    let identity = (instance.id, instance.controllers[0].id);
    shared
        .rig
        .engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
        .unwrap();
    runtime.set_global_paused(true, capture.sampled_at().timestamp_millis() as u64);
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    definition.revision += 1;
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
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
            value: DynamicValue::Scalar(shared.angles[0][0] + 5.),
        };
    }
    definition.normalize_angle_pair();
    assert_eq!(definition.lanes[0].id, pan_lane);
    assert_eq!(definition.lanes.len(), 2);
    assert!(definition.lanes[1].is_angle_current_passthrough());
    let snapshot = shared.rig.engine.snapshot();
    shared
        .rig
        .engine
        .replace_snapshot(EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    runtime.install_definitions([definition]).unwrap();
    let (capture, _) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    shared
        .rig
        .engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(
            false,
        ))
        .unwrap();
    runtime.set_global_paused(false, capture.sampled_at().timestamp_millis() as u64);
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    shared.rig.clock.advance_millis(250);
    let (capture, output) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.instances.len(), 1);
    let instance = &snapshot.instances[0];
    assert_eq!((instance.id, instance.controllers[0].id), identity);
    assert_eq!(instance.targets, shared.heads);
    let resume = instance
        .synchronized_resume_transition
        .expect("the runtime creates the shared synchronized Resume occurrence");
    let mut authored = Vec::new();
    let mut resume_progress = None;
    for head in shared.heads {
        let sample = output
            .sampled
            .samples
            .iter()
            .find(|sample| sample.target == head && sample.lane_id == pan_lane)
            .unwrap();
        assert_eq!((sample.instance_id, sample.controller_id), identity);
        let occurrences = resume_occurrences(&sample.expression);
        assert_eq!(occurrences.len(), 1);
        assert_eq!(occurrences[0].0, resume.occurrence_id);
        assert!((0.1..0.9).contains(&occurrences[0].1));
        if let Some(progress) = resume_progress {
            assert_eq!(
                occurrences[0].1, progress,
                "the actual shared Resume has one coherent progress across both targets"
            );
        } else {
            resume_progress = Some(occurrences[0].1);
        }
        let mut ids = HashSet::new();
        sample
            .expression
            .visit_source_occurrences(&mut |id| {
                ids.insert(id);
            })
            .unwrap();
        assert!(!ids.is_empty());
        authored.push(ids);
    }
    let resume_progress = f64::from(resume_progress.unwrap());
    assert_ne!(
        authored[0], authored[1],
        "authored source bindings remain target-specific despite the shared Resume scope"
    );
    assert!(
        output.requirements.is_empty(),
        "compatible full shared cohort must complete: {:?}",
        output
            .requirements
            .iter()
            .map(|r| (r.target, super::numeric::requirement_debug(&r.reason)))
            .collect::<Vec<_>>()
    );
    assert_eq!(output.results.len(), 2);
    let mut claims = HashMap::new();
    for (head_index, head) in shared.heads.into_iter().enumerate() {
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        assert!(!row.quality.held);
        assert_eq!(program(row).base, shared.targets[head_index]);
        assert!(!program(row).samples.is_empty());
        assert_eq!(row.achieved.destinations.len(), 2);
        assert_eq!(row.achieved.outcomes.len(), 2);
        for destination in [shared.rig.root, copy] {
            let outcome = row
                .achieved
                .outcomes
                .iter()
                .find(|outcome| outcome.destination == destination)
                .unwrap();
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(!outcome.input_requirement && !outcome.missing_mount);
            let calculated = commanded_angles(
                &row.achieved
                    .destinations
                    .iter()
                    .find(|d| d.destination == destination)
                    .unwrap()
                    .value,
            );
            let expected = [
                f64::from(shared.angles[head_index][0]) + 5. * resume_progress,
                f64::from(shared.angles[head_index][1]),
            ];
            for axis in 0..2 {
                assert!(
                    (calculated[axis] - expected[axis]).abs() < 0.04,
                    "head {head_index} copy {destination:?}: shared Resume must interpolate changed Pan and retain Tilt Current: {calculated:?} != {expected:?}"
                );
            }
            assert!(
                (calculated[0] - f64::from(shared.angles[head_index][0])).abs() > 0.5,
                "this is a genuinely moving shared motor, not an unchanged endpoint traversal"
            );
        }
        assert_eq!(
            lane.continuity(head, ProgrammingOwner::Position)
                .unwrap()
                .instances
                .len(),
            2
        );
        for write in &row.writes {
            assert!(!write.parked);
            let key = (write.slot.destination, write.slot.channel_index);
            if let Some(old) = claims.insert(key, write.raw) {
                assert_eq!(
                    old, write.raw,
                    "the two heads must agree on their shared motor for every copy"
                );
            }
        }
    }
    assert_eq!(claims.len(), 6, "three native motors in each physical copy");
    let baseline = shared
        .rig
        .engine
        .preview_static_family_frame(
            &capture,
            shared.rig.engine.prepare_static_family_frame(&capture, &[]),
        )
        .unwrap();
    for destination in [shared.rig.root, copy] {
        let original = baseline
            .physical
            .instances
            .iter()
            .find(|i| i.instance_id == destination.0)
            .unwrap();
        let actual = output
            .rendered
            .physical
            .instances
            .iter()
            .find(|i| i.instance_id == destination.0)
            .unwrap();
        let mut expected = original.native_raw.to_vec();
        for ((owner, index), raw) in &claims {
            if *owner == destination {
                expected[*index as usize] = *raw;
            }
        }
        assert!(actual.complete);
        assert_eq!(
            actual.native_raw.as_ref(),
            expected.as_slice(),
            "published native output must contain the accepted full shared cohort"
        );
    }
}
