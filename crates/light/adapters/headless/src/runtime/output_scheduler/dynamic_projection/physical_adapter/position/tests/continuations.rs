//! Actual captured Live bridge integration, including interrupted retained history and copies.
//! Synthetic profiles prove commanded/native invariants; these tests do not certify real motors
//! or physical motor performance; complete shared mechanics remain a publication requirement.
use super::programs::{
    commanded_angles, destination_dynamic_rig, position_definition, program,
    start_position_dynamic, verify_live_native,
};
use super::*;
use light_dynamics::{
    ActivationPolicy, CoupledRetainedSources, DynamicDefinition, DynamicLaneBody,
    DynamicSampleExpression, DynamicTransitionReason, DynamicValue, DynamicValueSource,
    FamilyCompositionSample, ProgrammingLaneConfiguration, RetainedExpressionNode,
    RetainedExpressionTape,
};

fn frame(
    rig: &Rig,
    lane: &PhysicalAdapterLane<PositionAdapter>,
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    scratch: &mut HybridFrameScratch,
) -> (PreparedOutputFrame, PublishedPhysicalFrame<PositionAdapter>) {
    let capture = rig.capture();
    let output = prepare_live(rig, &capture, &capture, lane, runtime, origins, scratch).unwrap();
    assert!(
        output.requirements.is_empty(),
        "interrupted single-owner program remains materializable"
    );
    assert_eq!(output.results.len(), 1);
    verify_live_native(rig, &capture, lane, &output);
    (capture, output)
}
fn edit(
    rig: &Rig,
    runtime: &mut DynamicRuntime,
    definition: &mut DynamicDefinition,
    value: AttributeValue,
) {
    definition.revision += 1;
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        panic!("Position lane")
    };
    body.address = DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap();
    let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
        panic!("keyframes")
    };
    for point in &mut configuration.points {
        point.source = DynamicValueSource::Value {
            value: DynamicValue::Family(value.clone()),
        };
    }
    let snapshot = rig.engine.snapshot();
    rig.engine
        .replace_snapshot(EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    runtime.install_definitions([definition.clone()]).unwrap();
}
fn pause(rig: &Rig, paused: bool) {
    rig.engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(
            paused,
        ))
        .unwrap();
}
fn resume_occurrences(expression: &DynamicSampleExpression) -> Vec<(Uuid, f32)> {
    let tape = RetainedExpressionTape::from_roots(&[Arc::new(expression.clone())]).unwrap();
    let mut pending = tape.roots.clone();
    let mut seen = std::collections::HashSet::new();
    let mut resumes = Vec::new();
    while let Some(node) = pending.pop() {
        if !seen.insert(node) {
            continue;
        }
        let source = &tape.nodes[node.0 as usize];
        if let RetainedExpressionNode::Transition {
            progress,
            reason: DynamicTransitionReason::Resume { occurrence_id },
            ..
        } = source
        {
            resumes.push((*occurrence_id, *progress));
        }
        pending.extend(source.children());
    }
    resumes
}

#[test]
fn live_per_copy_bridge_keeps_interrupted_target_angle_resume_history_and_original_program() {
    let (rig, copy) = destination_dynamic_rig();
    let base = angles(60., -20.);
    let first_target = target(TargetReference::Origin, [4., 8., 1.]);
    let mut definition = position_definition(
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, &first_target).unwrap(),
        [
            DynamicValue::Family(first_target.clone()),
            DynamicValue::Family(first_target),
        ],
    );
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = light_dynamics::DynamicSpeed::SpeedGroup {
        group: light_dynamics::SpeedGroup::A,
        beats_per_cycle: light_dynamics::Rational {
            numerator: 2,
            denominator: 1,
        },
    };
    let lane_id = definition.lanes[0].id;
    let mut runtime = start_position_dynamic(&rig, &base, &definition, Some(1000));
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    rig.clock.advance_millis(1100);
    frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    pause(&rig, true);
    frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    edit(&rig, &mut runtime, &mut definition, angles(80., -30.));
    frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    pause(&rig, false);
    frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    rig.clock.advance_millis(225);
    let (_, first_resume) = frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    let first_sample = first_resume
        .sampled
        .samples
        .iter()
        .find(|sample| sample.lane_id == lane_id)
        .unwrap();
    let first_occurrences = resume_occurrences(&first_sample.expression);
    assert_eq!(
        first_occurrences.len(),
        1,
        "the real synchronized transport created one interior Resume"
    );
    assert!((0.1..0.9).contains(&first_occurrences[0].1));
    let identity = (
        first_sample.instance_id,
        first_sample.controller_id,
        first_sample.activated_at_millis,
    );
    let mut original_sources = Vec::new();
    first_sample
        .expression
        .visit_source_occurrences(&mut |id| original_sources.push(id))
        .unwrap();
    assert!(!original_sources.is_empty());
    let old_records = original_sources
        .iter()
        .map(|id| (*id, Arc::clone(origins.get(*id).unwrap())))
        .collect::<Vec<_>>();
    pause(&rig, true);
    frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    edit(
        &rig,
        &mut runtime,
        &mut definition,
        target(TargetReference::Origin, [-4., 6., 0.]),
    );
    frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    pause(&rig, false);
    frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    rig.clock.advance_millis(475);
    let (capture, output) = frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    let sample = output
        .sampled
        .samples
        .iter()
        .find(|sample| sample.lane_id == lane_id)
        .unwrap();
    assert_eq!(
        (
            sample.instance_id,
            sample.controller_id,
            sample.activated_at_millis
        ),
        identity,
        "hot edits and per-copy evaluations retain one logical controller"
    );
    let resumes = resume_occurrences(&sample.expression);
    assert_eq!(
        resumes.len(),
        2,
        "the second interruption retains the first Resume inside the new one"
    );
    assert!(
        resumes
            .iter()
            .all(|(_, progress)| (0.1..0.9).contains(progress))
    );
    assert_eq!(
        resumes
            .iter()
            .map(|(id, _)| *id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        2
    );
    assert!(
        resumes.contains(&first_occurrences[0]),
        "old occurrence and frozen progress survive the second resume"
    );
    for (id, record) in old_records {
        assert!(
            Arc::ptr_eq(&record, origins.get(id).unwrap()),
            "a pool hot edit cannot replace an original retained source record"
        );
    }
    let row = &output.results[0];
    let requested = program(row);
    assert_eq!(requested.base, base);
    assert_eq!(requested.samples.len(), 1);
    let FamilyCompositionSample::CoupledExpression { expression, .. } = &requested.samples[0]
    else {
        panic!("mixed retained Position remains the original coupled program")
    };
    match expression.retained_sources() {
        CoupledRetainedSources::PositionForest(samples) => {
            let retained = samples
                .iter()
                .find(|retained| retained.lane_id == lane_id)
                .unwrap();
            assert_eq!(retained.expression, sample.expression);
            assert_eq!(
                (retained.instance_id, retained.controller_id),
                (sample.instance_id, sample.controller_id)
            );
        }
        CoupledRetainedSources::Expression(expression) => {
            assert_eq!(expression.as_ref(), &sample.expression)
        }
    }
    assert_eq!(row.achieved.destinations.len(), 2);
    let destination = |id| {
        row.achieved
            .destinations
            .iter()
            .find(|destination| destination.destination == id)
            .unwrap()
    };
    let root_angles = commanded_angles(&destination(rig.root).value);
    let copy_angles = commanded_angles(&destination(copy).value);
    assert!(
        root_angles
            .iter()
            .zip(copy_angles)
            .any(|(a, b)| (a - b).abs() > 0.2),
        "nested retained conversions still use each copy's mount and calibration"
    );
    for destination in &row.achieved.destinations {
        let controls = destination
            .provenance
            .controls
            .as_ref()
            .expect("complete authored control ranks");
        assert!(!controls.is_empty());
        assert!(
            controls.iter().all(
                |control| control
                    .rank
                    .dynamic_identity()
                    .is_some_and(|rank| rank.instance_id == sample.instance_id
                        && rank.controller_id == sample.controller_id
                        && rank.lane_id == lane_id)
            ),
            "copy fitting cannot replace original control ownership with a fitted source"
        );
        assert_eq!(
            destination.provenance.sources.unknown(),
            row.provenance.sources.unknown(),
            "unavailable physical field-transfer evidence remains passive for each destination"
        );
    }
    assert_eq!(row.value, destination(rig.root).value);
    assert!(
        lane.continuity(rig.root, ProgrammingOwner::Position)
            .is_some()
    );
    assert_eq!(
        rig.engine.snapshot().dynamics[0],
        definition,
        "continuation evaluation does not author fitted channels"
    );
    assert_eq!(
        output.token,
        capture.frame_token(),
        "publication remains tied to the actual captured Live token"
    );
}

#[test]
fn live_bridge_independent_shared_target_envelopes_preserve_unchanged_pose_and_accept_full_cohort()
{
    use super::current_cohort::{
        authored_target_definition, run_frame, shared_rig, start, verify_shared_native_acceptance,
        verify_success,
    };
    let shared = shared_rig();
    let bases = shared.angles.map(|pair| angles(pair[0], pair[1]));
    let mut runtime = start(
        &shared,
        &bases,
        &[
            (0, authored_target_definition(&shared, 0, 1), Some(1000)),
            (1, authored_target_definition(&shared, 1, 2), Some(1000)),
        ],
    );
    let authored =
        serde_json::to_value(shared.rig.programmers.get(shared.rig.session).unwrap()).unwrap();
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    shared.rig.clock.advance_millis(250);
    let (capture, output) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    verify_success(&shared, &capture, &output, 2);
    verify_shared_native_acceptance(&shared, &lane, &output, &[(shared.rig.root, 0)]);
    let runtime_state = runtime.snapshot();
    assert_eq!(runtime_state.instances.len(), 2);
    assert_eq!(output.sampled.samples.len(), 2);
    let mut identities = std::collections::HashSet::new();
    for head in shared.heads {
        let sample = output
            .sampled
            .samples
            .iter()
            .find(|sample| sample.target == head)
            .unwrap();
        let instance = runtime_state
            .instances
            .iter()
            .find(|instance| instance.targets == vec![head])
            .unwrap();
        assert_eq!(instance.controllers.len(), 1);
        assert_eq!(
            (sample.instance_id, sample.controller_id),
            (instance.id, instance.controllers[0].id)
        );
        assert!(identities.insert((sample.instance_id, sample.controller_id)));
        assert!(sample.activation_mix > 0. && sample.activation_mix < 1.);
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        let physical = output
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == shared.rig.root.0)
            .unwrap();
        let index = shared.heads.iter().position(|id| *id == head).unwrap();
        let pair = row.achieved.outcomes[0].result.achieved.unwrap();
        assert_eq!(
            pair,
            [
                physical.axes[0].absolute_degrees().unwrap(),
                physical.axes[index + 1].absolute_degrees().unwrap()
            ]
        );
        for axis in 0..2 {
            assert!(
                (pair[axis] - f64::from(shared.angles[index][axis])).abs() < 0.04,
                "unchanged physical endpoint is retained throughout activation"
            );
        }
    }
    assert_eq!(identities.len(), 2);
    assert_eq!(output.token, capture.frame_token());
    assert_eq!(
        serde_json::to_value(shared.rig.programmers.get(shared.rig.session).unwrap()).unwrap(),
        authored
    );
}
