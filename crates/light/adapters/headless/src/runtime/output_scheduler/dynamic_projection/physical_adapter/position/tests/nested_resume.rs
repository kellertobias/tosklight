//! Actual twice-interrupted shared transport: Target Current -> Pan + Tilt Current -> Target.
//! Co-located copies deliberately keep every speculative mechanical endpoint compatible.
//! Missing input coverage checks quiet complete-cohort refusal; no source/scope IDs are invented.
use super::current_cohort::{SharedRig, run_frame, shared_rig};
use super::programs::{commanded_angles, position_definition, program};
use super::shared_resume::{resume_occurrences, start_shared};
use super::*;
use light_dynamics::{
    ActivationPolicy, DynamicDefinition, DynamicDefinitionSnapshot, DynamicFamilyRepresentation,
    DynamicInstanceOverrides, DynamicLaneBody, DynamicReference, DynamicSpeed,
    DynamicTargetBinding, DynamicValue, DynamicValueSource, DynamicValueTiming,
    ProgrammingLaneConfiguration, Rational, SpeedGroup,
};
use std::collections::{HashMap, HashSet};

fn hot_install(
    shared: &SharedRig,
    runtime: &mut DynamicRuntime,
    definitions: &[DynamicDefinition],
) {
    let snapshot = shared.rig.engine.snapshot();
    shared
        .rig
        .engine
        .replace_snapshot(EngineSnapshot {
            dynamics: definitions.to_vec().into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    runtime
        .install_definitions(definitions.iter().cloned())
        .unwrap();
}
fn pause(
    shared: &SharedRig,
    runtime: &mut DynamicRuntime,
    capture: &PreparedOutputFrame,
    paused: bool,
) {
    shared
        .rig
        .engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(
            paused,
        ))
        .unwrap();
    runtime.set_global_paused(paused, capture.sampled_at().timestamp_millis() as u64);
}
fn source_records(
    output: &PublishedPhysicalFrame<PositionAdapter>,
    origins: &DynamicSourceOrigins,
) -> Vec<(
    light_dynamics::DynamicSourceOccurrenceId,
    Arc<crate::runtime::dynamic_source_origins::DynamicSourceRecord>,
)> {
    let mut ids = HashSet::new();
    for sample in &output.sampled.samples {
        sample
            .expression
            .visit_source_occurrences(&mut |id| {
                ids.insert(id);
            })
            .unwrap();
    }
    assert!(!ids.is_empty());
    ids.into_iter()
        .map(|id| (id, Arc::clone(origins.get(id).unwrap())))
        .collect()
}

#[test]
fn two_actual_shared_resumes_interpolate_nested_history_and_publish_complete_copies() {
    let shared = shared_rig();
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
    let primary = definition.lanes[0].id;
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
    let first_instance = runtime.snapshot().instances.remove(0);
    assert_eq!(first_instance.targets, shared.heads);
    assert_eq!(first_instance.controllers.len(), 1);
    let identity = (first_instance.id, first_instance.controllers[0].id);
    pause(&shared, &mut runtime, &capture, true);
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
    assert!(definition.lanes[1].is_angle_current_passthrough());
    hot_install(&shared, &mut runtime, &[definition.clone()]);
    let (capture, _) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    pause(&shared, &mut runtime, &capture, false);
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    shared.rig.clock.advance_millis(250);
    let (capture, first) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    assert!(first.requirements.is_empty());
    let first_scope = runtime.snapshot().instances[0]
        .synchronized_resume_transition
        .unwrap()
        .occurrence_id;
    let old_records = source_records(&first, &origins);
    let first_progress = resume_occurrences(
        &first
            .sampled
            .samples
            .iter()
            .find(|s| s.target == shared.heads[0] && s.lane_id == primary)
            .unwrap()
            .expression,
    );
    assert_eq!(first_progress.len(), 1);
    assert_eq!(first_progress[0].0, first_scope);
    assert!((0.1..0.9).contains(&first_progress[0].1));
    // Interrupt while the first Resume is still interior. Keep its exact retained graph;
    // the next whole Target Current endpoint restores each distinct captured static Target.
    pause(&shared, &mut runtime, &capture, true);
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    definition.revision += 1;
    definition.lanes.truncate(1);
    assert_eq!(definition.lanes[0].id, primary);
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        unreachable!()
    };
    body.address =
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, &shared.targets[0]).unwrap();
    let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
        unreachable!()
    };
    for point in &mut configuration.points {
        point.source = DynamicValueSource::Current;
    }
    definition.normalize_angle_pair();
    hot_install(&shared, &mut runtime, &[definition.clone()]);
    let (capture, _) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    pause(&shared, &mut runtime, &capture, false);
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    shared.rig.clock.advance_millis(250);
    let (capture, output) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    let runtime_view = runtime.snapshot();
    assert_eq!(runtime_view.instances.len(), 1);
    let instance = &runtime_view.instances[0];
    assert_eq!((instance.id, instance.controllers[0].id), identity);
    assert_eq!(instance.targets, shared.heads);
    let second_scope = instance
        .synchronized_resume_transition
        .unwrap()
        .occurrence_id;
    assert_ne!(second_scope, first_scope);
    let mut expected_progress = None;
    for head in shared.heads {
        let sample = output
            .sampled
            .samples
            .iter()
            .find(|s| s.target == head && s.lane_id == primary)
            .unwrap();
        let resumes = resume_occurrences(&sample.expression);
        assert_eq!(
            resumes.len(),
            2,
            "the original first Resume survives inside the second"
        );
        let first = resumes.iter().find(|(id, _)| *id == first_scope).unwrap().1;
        let second = resumes
            .iter()
            .find(|(id, _)| *id == second_scope)
            .unwrap()
            .1;
        assert!((0.1..0.9).contains(&first) && (0.1..0.9).contains(&second));
        if let Some(progress) = expected_progress {
            assert_eq!((first, second), progress);
        } else {
            expected_progress = Some((first, second));
        }
    }
    for (id, record) in old_records {
        assert!(
            Arc::ptr_eq(&record, origins.get(id).unwrap()),
            "hot-edit keeps original retained authored source records"
        );
    }
    assert!(
        output.requirements.is_empty(),
        "nested compatible endpoint environments must finish: {:?}",
        output
            .requirements
            .iter()
            .map(|r| (r.target, super::numeric::requirement_debug(&r.reason)))
            .collect::<Vec<_>>()
    );
    assert_eq!(output.results.len(), 2);
    let (frozen_first, second) = expected_progress.unwrap();
    let expected_delta = 5. * f64::from(frozen_first) * (1. - f64::from(second));
    assert!(expected_delta > 0.5);
    let mut claims = HashMap::new();
    for (index, head) in shared.heads.into_iter().enumerate() {
        let row = output.results.iter().find(|r| r.target == head).unwrap();
        assert!(!row.quality.held);
        assert_eq!(program(row).base, shared.targets[index]);
        assert_eq!(
            program(row).samples.len(),
            1,
            "original nested source forest stays one logical source slot"
        );
        assert_eq!(row.achieved.destinations.len(), 2);
        for destination in [shared.rig.root, copy] {
            let achieved = row
                .achieved
                .outcomes
                .iter()
                .find(|o| o.destination == destination)
                .unwrap();
            assert_eq!(achieved.result.status, PositionFitStatus::Fitted);
            let calculated = commanded_angles(
                &row.achieved
                    .destinations
                    .iter()
                    .find(|d| d.destination == destination)
                    .unwrap()
                    .value,
            );
            let expected = [
                f64::from(shared.angles[index][0]) + expected_delta,
                f64::from(shared.angles[index][1]),
            ];
            for axis in 0..2 {
                assert!(
                    (calculated[axis] - expected[axis]).abs() < 0.04,
                    "nested original operands must interpolate once: {calculated:?} != {expected:?}"
                );
            }
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
            if let Some(old) = claims.insert(
                (write.slot.destination, write.slot.channel_index),
                write.raw,
            ) {
                assert_eq!(old, write.raw);
            }
        }
    }
    assert_eq!(claims.len(), 6);
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
        assert_eq!(actual.native_raw.as_ref(), expected.as_slice());
    }
    // Make one peer unavailable through an actual missing Preset, rather than omitting its
    // emitter. All formerly accepted continuity must remain unchanged and native output must
    // receive no speculative shared-motor fragment from the nested active peer.
    let continuity = shared
        .heads
        .map(|head| lane.continuity(head, ProgrammingOwner::Position));
    let mut missing = position_definition(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        },
        [DynamicValue::Scalar(0.), DynamicValue::Scalar(0.)],
    );
    missing.pool_number = 2;
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
    hot_install(&shared, &mut runtime, &[definition, missing.clone()]);
    assert!(shared.rig.programmers.apply_dynamic_values(
        shared.rig.session,
        &[DynamicProgrammerValueMutation::Set {
            fixture_id: shared.heads[1],
            attribute: ProgrammingOwner::Position.key(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: Uuid::new_v4(),
                lane_id: missing.lanes[0].id,
                dynamic: DynamicReference {
                    dynamic_id: Some(missing.id),
                    last_known_pool_number: 2,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(missing)
                    }
                },
                overrides: DynamicInstanceOverrides {
                    size: 1.,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.
                },
                timing: DynamicValueTiming::default(),
            },
        }],
        None
    ));
    let (capture, blocked) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    assert!(blocked.requirements.iter().any(|r| r.target == shared.heads[1] && matches!(&r.reason, crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::HybridFamilyRequirementReason::Input(_))));
    for (index, head) in shared.heads.into_iter().enumerate() {
        assert!(blocked.results.iter().all(|r| r.target != head));
        assert_eq!(
            lane.continuity(head, ProgrammingOwner::Position),
            continuity[index]
        );
    }
    let baseline = shared
        .rig
        .engine
        .preview_static_family_frame(
            &capture,
            shared.rig.engine.prepare_static_family_frame(&capture, &[]),
        )
        .unwrap();
    assert_eq!(blocked.rendered.universes, baseline.universes);
    for original in &baseline.physical.instances {
        let actual = blocked
            .rendered
            .physical
            .instances
            .iter()
            .find(|i| i.instance_id == original.instance_id)
            .unwrap();
        assert_eq!(actual.native_raw, original.native_raw);
        assert_eq!(actual.complete, original.complete);
    }
}

#[test]
fn nested_angle_resume_applies_outer_pan_suffix_exactly_once() {
    let shared = shared_rig();
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
    let primary = definition.lanes[0].id;
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
    let first_instance = runtime.snapshot().instances.remove(0);
    assert_eq!(first_instance.targets, shared.heads);
    assert_eq!(first_instance.controllers.len(), 1);
    let identity = (first_instance.id, first_instance.controllers[0].id);
    pause(&shared, &mut runtime, &capture, true);
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
    assert!(definition.lanes[1].is_angle_current_passthrough());
    hot_install(&shared, &mut runtime, &[definition.clone()]);
    let (capture, _) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    pause(&shared, &mut runtime, &capture, false);
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    shared.rig.clock.advance_millis(250);
    let (capture, first) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    assert!(first.requirements.is_empty());
    let first_scope = runtime.snapshot().instances[0]
        .synchronized_resume_transition
        .unwrap()
        .occurrence_id;
    let old_records = source_records(&first, &origins);
    let first_progress = resume_occurrences(
        &first
            .sampled
            .samples
            .iter()
            .find(|s| s.target == shared.heads[0] && s.lane_id == primary)
            .unwrap()
            .expression,
    );
    assert_eq!(first_progress.len(), 1);
    assert_eq!(first_progress[0].0, first_scope);
    assert!((0.1..0.9).contains(&first_progress[0].1));
    // The second endpoint remains Angles and changes Pan by ten degrees. A full-program
    // child evaluation would apply this outer suffix while fitting the inner Resume, then
    // apply it again on return. The exact original-prefix expectation below forbids that.
    pause(&shared, &mut runtime, &capture, true);
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    definition.revision += 1;
    assert_eq!(definition.lanes[0].id, primary);
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
            value: DynamicValue::Scalar(shared.angles[0][0] + 10.),
        };
    }
    definition.normalize_angle_pair();
    assert_eq!(definition.lanes.len(), 2);
    assert!(definition.lanes[1].is_angle_current_passthrough());
    hot_install(&shared, &mut runtime, &[definition.clone()]);
    let (capture, _) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    pause(&shared, &mut runtime, &capture, false);
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    shared.rig.clock.advance_millis(250);
    let (capture, output) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    let runtime_view = runtime.snapshot();
    assert_eq!(runtime_view.instances.len(), 1);
    let instance = &runtime_view.instances[0];
    assert_eq!((instance.id, instance.controllers[0].id), identity);
    assert_eq!(instance.targets, shared.heads);
    let second_scope = instance
        .synchronized_resume_transition
        .unwrap()
        .occurrence_id;
    assert_ne!(second_scope, first_scope);
    let mut expected_progress = None;
    for head in shared.heads {
        let sample = output
            .sampled
            .samples
            .iter()
            .find(|s| s.target == head && s.lane_id == primary)
            .unwrap();
        let resumes = resume_occurrences(&sample.expression);
        assert_eq!(
            resumes.len(),
            2,
            "the original first Resume survives inside the second"
        );
        let first = resumes.iter().find(|(id, _)| *id == first_scope).unwrap().1;
        let second = resumes
            .iter()
            .find(|(id, _)| *id == second_scope)
            .unwrap()
            .1;
        assert!((0.1..0.9).contains(&first) && (0.1..0.9).contains(&second));
        if let Some(progress) = expected_progress {
            assert_eq!((first, second), progress);
        } else {
            expected_progress = Some((first, second));
        }
    }
    for (id, record) in old_records {
        assert!(
            Arc::ptr_eq(&record, origins.get(id).unwrap()),
            "hot-edit keeps original retained authored source records"
        );
    }
    assert!(
        output.requirements.is_empty(),
        "nested compatible endpoint environments must finish: {:?}",
        output
            .requirements
            .iter()
            .map(|r| (r.target, super::numeric::requirement_debug(&r.reason)))
            .collect::<Vec<_>>()
    );
    assert_eq!(output.results.len(), 2);
    let (frozen_first, second) = expected_progress.unwrap();
    let expected_delta =
        5. * f64::from(frozen_first) * (1. - f64::from(second)) + 10. * f64::from(second);
    assert!(
        expected_delta > 5. * f64::from(frozen_first) + 0.5,
        "the new Pan endpoint must move beyond the frozen first Resume pose"
    );
    let mut claims = HashMap::new();
    for (index, head) in shared.heads.into_iter().enumerate() {
        let row = output.results.iter().find(|r| r.target == head).unwrap();
        assert!(!row.quality.held);
        assert_eq!(program(row).base, shared.targets[index]);
        assert_eq!(
            program(row).samples.len(),
            1,
            "original nested source forest stays one logical source slot"
        );
        assert_eq!(row.achieved.destinations.len(), 2);
        for destination in [shared.rig.root, copy] {
            let achieved = row
                .achieved
                .outcomes
                .iter()
                .find(|o| o.destination == destination)
                .unwrap();
            assert_eq!(achieved.result.status, PositionFitStatus::Fitted);
            let calculated = commanded_angles(
                &row.achieved
                    .destinations
                    .iter()
                    .find(|d| d.destination == destination)
                    .unwrap()
                    .value,
            );
            let expected = [
                f64::from(shared.angles[index][0]) + expected_delta,
                f64::from(shared.angles[index][1]),
            ];
            for axis in 0..2 {
                assert!(
                    (calculated[axis] - expected[axis]).abs() < 0.04,
                    "nested original operands must interpolate once: {calculated:?} != {expected:?}"
                );
            }
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
            if let Some(old) = claims.insert(
                (write.slot.destination, write.slot.channel_index),
                write.raw,
            ) {
                assert_eq!(old, write.raw);
            }
        }
    }
    assert_eq!(claims.len(), 6);
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
        assert_eq!(actual.native_raw.as_ref(), expected.as_slice());
    }
}
