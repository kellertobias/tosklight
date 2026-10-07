//! Independent real controllers must keep their own Resume scopes even at equal progress.
//! The two independent Tilt motors move; every Cartesian endpoint retains the common Pan.
use super::current_cohort::{SharedRig, run_frame, shared_rig, start};
use super::programs::{commanded_angles, position_definition, program};
use super::shared_resume::resume_occurrences;
use super::*;
use light_dynamics::{
    ActivationPolicy, DynamicDefinition, DynamicFamilyRepresentation, DynamicLaneBody,
    DynamicSpeed, DynamicValue, DynamicValueSource, ProgrammingLaneConfiguration, Rational,
    SpeedGroup,
};
use std::collections::{HashMap, HashSet};

const DELTAS: [f32; 2] = [5., 9.];

struct Independent {
    shared: SharedRig,
    copy: FixtureId,
    runtime: DynamicRuntime,
    lane: PhysicalAdapterLane<PositionAdapter>,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
    primary: [Uuid; 2],
    identities: [(Uuid, Uuid); 2],
}

fn current_definition(base: &AttributeValue, pool: u16) -> DynamicDefinition {
    let mut definition = position_definition(
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, base).unwrap(),
        [
            DynamicValue::Family(base.clone()),
            DynamicValue::Family(base.clone()),
        ],
    );
    definition.pool_number = pool;
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
    definition
}

fn independent() -> Independent {
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
    let mut definitions = [
        current_definition(&shared.targets[0], 1),
        current_definition(&shared.targets[1], 2),
    ];
    let primary = definitions
        .each_ref()
        .map(|definition| definition.lanes[0].id);
    // This helper authors two distinct DynamicOn instance links through ProgrammerRegistry;
    // runtime reconciliation creates the instances/controllers and source occurrences.
    let mut runtime = start(
        &shared,
        &shared.targets,
        &[
            (0, definitions[0].clone(), Some(1000)),
            (1, definitions[1].clone(), Some(1000)),
        ],
    );
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    shared.rig.clock.advance_millis(1100);
    let (capture, initial) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    assert!(initial.requirements.is_empty());
    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.instances.len(), 2);
    let identities = std::array::from_fn(|index| {
        let instance = snapshot
            .instances
            .iter()
            .find(|instance| instance.targets == vec![shared.heads[index]])
            .unwrap();
        assert_eq!(instance.controllers.len(), 1);
        (instance.id, instance.controllers[0].id)
    });
    assert_ne!(identities[0].0, identities[1].0);
    assert_ne!(identities[0].1, identities[1].1);
    shared
        .rig
        .engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
        .unwrap();
    runtime.set_global_paused(true, capture.sampled_at().timestamp_millis() as u64);
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    for (index, definition) in definitions.iter_mut().enumerate() {
        definition.revision += 1;
        let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
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
                value: DynamicValue::Scalar(shared.angles[index][1] + DELTAS[index]),
            };
        }
        definition.normalize_angle_pair();
        assert_eq!(definition.lanes[0].id, primary[index]);
        assert!(definition.lanes[1].is_angle_current_passthrough());
    }
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
    runtime.install_definitions(definitions).unwrap();
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
    Independent {
        shared,
        copy,
        runtime,
        lane,
        origins,
        scratch,
        primary,
        identities,
    }
}

fn check_scopes(rig: &Independent, output: &PublishedPhysicalFrame<PositionAdapter>) -> [f32; 2] {
    let snapshot = rig.runtime.snapshot();
    assert_eq!(snapshot.instances.len(), 2);
    let mut scopes = HashSet::new();
    let progress = std::array::from_fn(|index| {
        let sample = output
            .sampled
            .samples
            .iter()
            .find(|sample| {
                sample.target == rig.shared.heads[index] && sample.lane_id == rig.primary[index]
            })
            .unwrap();
        assert_eq!(
            (sample.instance_id, sample.controller_id),
            rig.identities[index]
        );
        let instance = snapshot
            .instances
            .iter()
            .find(|instance| instance.id == sample.instance_id)
            .unwrap();
        assert_eq!(instance.targets, vec![rig.shared.heads[index]]);
        let occurrence = instance
            .synchronized_resume_transition
            .unwrap()
            .occurrence_id;
        let resumes = resume_occurrences(&sample.expression);
        assert_eq!(resumes.len(), 1);
        assert_eq!(resumes[0].0, occurrence);
        assert!(
            scopes.insert(occurrence),
            "separate real controllers never become one Resume scope"
        );
        assert!((0.1..0.9).contains(&resumes[0].1));
        resumes[0].1
    });
    assert_eq!(
        progress[0], progress[1],
        "equal progress is deliberately not scope authority"
    );
    progress
}

#[test]
fn independent_actual_resume_scopes_resolve_all_cartesian_peers_without_conflating_equal_progress()
{
    let mut rig = independent();
    let (capture, output) = run_frame(
        &rig.shared,
        &mut rig.runtime,
        &rig.lane,
        &mut rig.origins,
        &mut rig.scratch,
    );
    let progress = check_scopes(&rig, &output);
    assert!(
        output.requirements.is_empty(),
        "compatible independent Tilt endpoints must complete: {:?}",
        output
            .requirements
            .iter()
            .map(|row| (row.target, super::numeric::requirement_debug(&row.reason)))
            .collect::<Vec<_>>()
    );
    assert_eq!(output.results.len(), 2);
    let mut claims = HashMap::new();
    for (index, head) in rig.shared.heads.into_iter().enumerate() {
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        assert!(!row.quality.held);
        assert_eq!(program(row).base, rig.shared.targets[index]);
        assert!(!program(row).samples.is_empty());
        for source in program(row).samples.iter() {
            let rank = match source {
                light_dynamics::FamilyCompositionSample::Known(sample) => sample.rank,
                light_dynamics::FamilyCompositionSample::WholeExpression { rank, .. }
                | light_dynamics::FamilyCompositionSample::CoupledExpression { rank, .. } => *rank,
            };
            let identity = rank
                .dynamic_identity()
                .expect("the original independent Dynamic source remains authored");
            assert_eq!(
                (identity.instance_id, identity.controller_id),
                rig.identities[index]
            );
        }
        assert_eq!(row.achieved.destinations.len(), 2);
        assert_eq!(row.achieved.outcomes.len(), 2);
        for destination in [rig.shared.rig.root, rig.copy] {
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
                    .find(|value| value.destination == destination)
                    .unwrap()
                    .value,
            );
            let expected = [
                f64::from(rig.shared.angles[index][0]),
                f64::from(rig.shared.angles[index][1]) + f64::from(DELTAS[index] * progress[index]),
            ];
            for axis in 0..2 {
                assert!(
                    (calculated[axis] - expected[axis]).abs() < 0.04,
                    "independent scope {index}, copy {destination:?}: {calculated:?} != {expected:?}"
                );
            }
            assert!((calculated[1] - f64::from(rig.shared.angles[index][1])).abs() > 0.5);
        }
        assert_eq!(
            rig.lane
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
                assert_eq!(previous, write.raw);
            }
        }
    }
    assert_eq!(claims.len(), 6);
    let baseline = rig
        .shared
        .rig
        .engine
        .preview_static_family_frame(
            &capture,
            rig.shared
                .rig
                .engine
                .prepare_static_family_frame(&capture, &[]),
        )
        .unwrap();
    for destination in [rig.shared.rig.root, rig.copy] {
        let original = baseline
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

#[test]
fn exhausted_resume_environment_budget_holds_complete_cohort_and_next_attempt_recovers() {
    let mut rig = independent();
    let continuity = rig
        .shared
        .heads
        .map(|head| rig.lane.continuity(head, ProgrammingOwner::Position));
    assert!(continuity.iter().all(Option::is_some));
    // The real setup succeeds with normal limits. Restrict just this captured attempt;
    // one environment cannot allocate the two endpoint children for either controller.
    let (capture, blocked) = super::super::cut_coordinator::with_environment_limit(1, || {
        run_frame(
            &rig.shared,
            &mut rig.runtime,
            &rig.lane,
            &mut rig.origins,
            &mut rig.scratch,
        )
    });
    check_scopes(&rig, &blocked);
    assert!(
        !blocked.requirements.is_empty(),
        "budget exhaustion is passive physical unavailability"
    );
    for (index, head) in rig.shared.heads.into_iter().enumerate() {
        assert!(
            blocked.results.iter().all(|row| row.target != head),
            "no partial mechanical owner may escape an exhausted attempt"
        );
        assert_eq!(
            rig.lane.continuity(head, ProgrammingOwner::Position),
            continuity[index]
        );
    }
    let baseline = rig
        .shared
        .rig
        .engine
        .preview_static_family_frame(
            &capture,
            rig.shared
                .rig
                .engine
                .prepare_static_family_frame(&capture, &[]),
        )
        .unwrap();
    assert_eq!(blocked.rendered.universes, baseline.universes);
    for original in &baseline.physical.instances {
        let actual = blocked
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == original.instance_id)
            .unwrap();
        assert_eq!(actual.native_raw, original.native_raw);
        assert_eq!(actual.complete, original.complete);
    }
    // Reuse the same lane, runtime and workspace after the guard resets. Failure cannot
    // leave a suspended speculative request or install child continuity into the next frame.
    let (_, recovered) = run_frame(
        &rig.shared,
        &mut rig.runtime,
        &rig.lane,
        &mut rig.origins,
        &mut rig.scratch,
    );
    check_scopes(&rig, &recovered);
    assert!(
        recovered.requirements.is_empty(),
        "a bounded refusal must not poison the next attempt"
    );
    assert_eq!(recovered.results.len(), 2);
    for head in rig.shared.heads {
        let row = recovered
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        assert!(!row.quality.held);
        assert!(
            row.achieved
                .outcomes
                .iter()
                .all(|outcome| outcome.result.status == PositionFitStatus::Fitted)
        );
    }
}
