//! Two authentic controller Size emissions retained below a real synchronized Resume.
//! No resolver failure, forest, operation witness, or sample identity is manufactured.
use super::*;
use light_dynamics::{ActivationPolicy, DynamicSpeed, SpeedGroup};

struct Oracle {
    base: HashMap<(usize, FixtureId), [f64; 2]>,
    outgoing: HashMap<(usize, FixtureId), [f64; 2]>,
    incoming: HashMap<(usize, FixtureId), [f64; 2]>,
    scope: Uuid,
    instance: Uuid,
    controller: Uuid,
}
fn endpoint(desk: &OperationDesk) -> AttributeValue {
    let snapshot = desk.shared.rig.engine.snapshot();
    let DynamicLaneBody::Programming(body) = &snapshot.dynamics[0].lanes[0].body else {
        panic!("typed lane")
    };
    let ProgrammingLaneConfiguration::Keyframes(config) = &body.configuration else {
        panic!("keyframes")
    };
    let DynamicValueSource::Value {
        value: DynamicValue::Family(value),
    } = &config.points[0].source
    else {
        panic!("authored Target")
    };
    value.clone()
}
fn pairs(
    desk: &OperationDesk,
    values: &[AttributeValue; 2],
) -> HashMap<(usize, FixtureId), [f64; 2]> {
    let output = desk.shared.rig.resolve(
        &desk
            .shared
            .heads
            .iter()
            .copied()
            .zip(values.iter().cloned())
            .collect::<Vec<_>>(),
    );
    let mut pairs = HashMap::new();
    for (index, row) in output.results.iter().enumerate() {
        for outcome in &row.achieved.outcomes {
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(!outcome.input_requirement && !outcome.missing_mount);
            pairs.insert(
                (index, outcome.destination),
                outcome.result.achieved.unwrap(),
            );
        }
    }
    assert_eq!(pairs.len(), 4);
    pairs
}
fn pause(desk: &mut OperationDesk, capture: &PreparedOutputFrame, paused: bool) {
    desk.shared
        .rig
        .engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(
            paused,
        ))
        .unwrap();
    desk.runtime
        .set_global_paused(paused, capture.sampled_at().timestamp_millis() as u64);
}
fn nested() -> (OperationDesk, Oracle) {
    let mut desk = OperationDesk::new_configured(Kind::Size, true, 0., |definition| {
        definition.default_activation = ActivationPolicy::JoinSyncNow;
        definition.speed = DynamicSpeed::SpeedGroup {
            group: SpeedGroup::A,
            beats_per_cycle: Rational {
                numerator: 2,
                denominator: 1,
            },
        };
    });
    let snapshot = desk.shared.rig.engine.snapshot();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    let copy = fixtures[0]
        .multipatch
        .iter_mut()
        .find(|copy| copy.id == desk.copy.0)
        .unwrap();
    copy.location.z = 1000;
    copy.invert_pan = true;
    copy.position_calibration = Some(InstalledPositionCalibration {
        tilt_zero_degrees: -7.,
        ..Default::default()
    });
    desk.shared
        .rig
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    let before = endpoint(&desk);
    let after = target(
        TargetReference::Origin,
        ray(
            &desk.shared,
            f64::from(desk.end[0]),
            f64::from(desk.end[1] + 8.),
        ),
    );
    let base = pairs(&desk, &desk.bases);
    let outgoing = pairs(&desk, &[before.clone(), before]);
    let incoming = pairs(&desk, &[after.clone(), after.clone()]);
    for index in 0..2 {
        assert!(
            (outgoing[&(index, desk.copy)][1] - outgoing[&(index, desk.shared.rig.root)][1]).abs()
                > 1.
        );
        assert!(
            (incoming[&(index, desk.copy)][1] - incoming[&(index, desk.shared.rig.root)][1]).abs()
                > 1.
        );
    }
    let (capture, initial) = desk.tick();
    assert!(initial.requirements.is_empty());
    let snapshot = desk.runtime.snapshot();
    assert_eq!(snapshot.instances.len(), 1);
    assert_eq!(snapshot.instances[0].targets, desk.shared.heads);
    assert_eq!(snapshot.instances[0].controllers.len(), 1);
    let instance = snapshot.instances[0].id;
    let controller = snapshot.instances[0].controllers[0].id;
    pause(&mut desk, &capture, true);
    desk.tick();
    let snapshot = desk.shared.rig.engine.snapshot();
    let mut definitions = snapshot.dynamics.as_ref().clone();
    definitions[0].revision += 1;
    let DynamicLaneBody::Programming(body) = &mut definitions[0].lanes[0].body else {
        panic!("typed lane")
    };
    let ProgrammingLaneConfiguration::Keyframes(config) = &mut body.configuration else {
        panic!("keyframes")
    };
    for keyframe in &mut config.points {
        keyframe.source = DynamicValueSource::Value {
            value: DynamicValue::Family(after.clone()),
        };
    }
    desk.shared
        .rig
        .engine
        .replace_snapshot(EngineSnapshot {
            dynamics: definitions.clone().into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    desk.runtime.install_definitions(definitions).unwrap();
    let (capture, _) = desk.tick();
    pause(&mut desk, &capture, false);
    desk.tick();
    desk.shared.rig.clock.advance_millis(250);
    let snapshot = desk.runtime.snapshot();
    assert_eq!(snapshot.instances[0].id, instance);
    assert_eq!(snapshot.instances[0].controllers[0].id, controller);
    let scope = snapshot.instances[0]
        .synchronized_resume_transition
        .unwrap()
        .occurrence_id;
    (
        desk,
        Oracle {
            base,
            outgoing,
            incoming,
            scope,
            instance,
            controller,
        },
    )
}
fn verify(
    desk: &OperationDesk,
    oracle: &Oracle,
    capture: &PreparedOutputFrame,
    output: &PublishedPhysicalFrame<PositionAdapter>,
) {
    assert!(
        output.requirements.is_empty(),
        "genuine nested Size histories must complete"
    );
    assert_eq!(output.results.len(), 2);
    let mut claims = HashMap::new();
    let mut histories = Vec::new();
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        let sample = output
            .sampled
            .samples
            .iter()
            .find(|sample| sample.target == head)
            .unwrap();
        assert_eq!(
            (sample.instance_id, sample.controller_id),
            (oracle.instance, oracle.controller)
        );
        let provenance = sample.expression.operation_provenance().unwrap();
        assert!(provenance.is_complete());
        let handles = provenance
            .handles()
            .iter()
            .filter(|handle| {
                matches!(
                    handle.site(),
                    DynamicOperationSite::ControllerSize {
                        role: DynamicControllerSizeRole::FamilyScale
                    }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            handles.len(),
            2,
            "actual historical and fresh producer Size emissions"
        );
        let old = handles
            .iter()
            .find(|handle| handle.emission().definition().revision == 1)
            .unwrap();
        let new = handles
            .iter()
            .find(|handle| handle.emission().definition().revision == 2)
            .unwrap();
        assert!(
            !std::ptr::eq(old.emission(), new.emission()),
            "distinct real sampling emission objects"
        );
        // Historical in the provenance API means restored/deserialized, not an older
        // still-live retained emission. Both witnesses here came from this real runtime.
        assert!(!old.is_historical() && !new.is_historical());
        assert_eq!(old.target(), head);
        assert_eq!(new.target(), head);
        histories.push(((**old).clone(), (**new).clone()));
        let tape =
            RetainedExpressionTape::from_roots(&[Arc::new(sample.expression.clone())]).unwrap();
        let factors = tape
            .nodes
            .iter()
            .filter_map(|node| {
                if let RetainedExpressionNode::Scale { factor, .. } = node {
                    Some(*factor)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(factors, vec![0.5, 0.5]);
        let progress = tape
            .nodes
            .iter()
            .filter_map(|node| match node {
                RetainedExpressionNode::Transition {
                    progress,
                    reason: DynamicTransitionReason::Resume { occurrence_id },
                    ..
                } if *occurrence_id == oracle.scope => Some(*progress),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(progress.len(), 1);
        assert!((0.1..0.9).contains(&progress[0]));
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        assert!(!row.quality.held);
        assert_eq!(program(row).base, desk.bases[index]);
        assert_eq!(
            program(row).samples.len(),
            1,
            "original whole retained history remains intact"
        );
        assert_eq!(row.achieved.destinations.len(), 2);
        for destination in [desk.shared.rig.root, desk.copy] {
            let pair = commanded_angles(
                &row.achieved
                    .destinations
                    .iter()
                    .find(|value| value.destination == destination)
                    .unwrap()
                    .value,
            );
            let expected: [f64; 2] = std::array::from_fn(|axis| {
                let base = oracle.base[&(index, destination)][axis];
                let old = base + 0.5 * (oracle.outgoing[&(index, destination)][axis] - base);
                let new = base + 0.5 * (oracle.incoming[&(index, destination)][axis] - base);
                old + f64::from(progress[0]) * (new - old)
            });
            for axis in 0..2 {
                assert!(
                    (pair[axis] - expected[axis]).abs() < 0.07,
                    "both Size operands then Resume once: {pair:?} != {expected:?}"
                );
            }
            let outcome = row
                .achieved
                .outcomes
                .iter()
                .find(|outcome| outcome.destination == destination)
                .unwrap();
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            let physical = output
                .rendered
                .physical
                .instances
                .iter()
                .find(|instance| instance.instance_id == destination.0)
                .unwrap();
            assert!(physical.complete);
            assert!((physical.axes[0].absolute_degrees().unwrap() - pair[0]).abs() < 0.03);
            assert!((physical.axes[index + 1].absolute_degrees().unwrap() - pair[1]).abs() < 0.03);
            for write in row
                .writes
                .iter()
                .filter(|write| write.slot.destination == destination)
            {
                assert!(!write.parked);
                if let Some(previous) =
                    claims.insert((destination, write.slot.channel_index), write.raw)
                {
                    assert_eq!(previous, write.raw, "shared Pan agrees");
                }
            }
        }
    }
    assert!(matches!(
        histories[0].0.correspondence(&histories[1].0),
        DynamicOperationCorrespondence::Shared { historical: false }
    ));
    assert!(matches!(
        histories[0].1.correspondence(&histories[1].1),
        DynamicOperationCorrespondence::Shared { historical: false }
    ));
    assert_eq!(claims.len(), 6);
    for (destination, start) in [(desk.shared.rig.root, 0), (desk.copy, 19)] {
        let physical = output
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == destination.0)
            .unwrap();
        for channel in 0..3u32 {
            let slot = start + channel as usize * 2;
            let bytes = &output.rendered.universes[&1];
            let wire = u32::from(u16::from_be_bytes([bytes[slot], bytes[slot + 1]]));
            assert_eq!(wire, claims[&(destination, channel)]);
            assert_eq!(physical.native_raw[channel as usize], wire);
        }
    }
    assert_eq!(output.token, capture.frame_token());
}
#[test]
fn actual_resume_of_historical_and_fresh_size_resolves_second_parent_cut_per_copy() {
    let (mut desk, oracle) = nested();
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert!(
        evidence.parents >= 8,
        "two distinct Size cuts in all four owner/copy parents: {evidence:?}"
    );
    assert!(
        evidence.endpoint_cohorts >= 4,
        "both independent endpoint cohorts for both real Size cuts: {evidence:?}"
    );
    assert_eq!(evidence.completed, 1);
    verify(&desk, &oracle, &capture, &output);
}
#[test]
fn exhausted_nested_graph_budget_keeps_all_accepted_continuity_and_recovers_next_frame() {
    let (mut desk, oracle) = nested();
    let continuity = desk
        .shared
        .heads
        .map(|head| desk.lane.continuity(head, ProgrammingOwner::Position));
    assert!(continuity.iter().all(Option::is_some));
    let (capture, held) =
        super::super::super::cut_coordinator::operation::with_environment_limit(1, || desk.tick());
    assert!(!held.requirements.is_empty());
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        assert!(
            held.results.iter().all(|row| row.target != head),
            "no speculative owner/copy may escape budget refusal"
        );
        assert_eq!(
            desk.lane.continuity(head, ProgrammingOwner::Position),
            continuity[index]
        );
    }
    let baseline = desk
        .shared
        .rig
        .engine
        .preview_static_family_frame(
            &capture,
            desk.shared
                .rig
                .engine
                .prepare_static_family_frame(&capture, &[]),
        )
        .unwrap();
    assert_eq!(held.rendered.universes, baseline.universes);
    for expected in &baseline.physical.instances {
        let actual = held
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == expected.instance_id)
            .unwrap();
        assert_eq!(actual.native_raw, expected.native_raw);
        assert_eq!(actual.complete, expected.complete);
    }
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(
        evidence.completed, 1,
        "same runtime and scratch recover after bounded refusal"
    );
    verify(&desk, &oracle, &capture, &output);
}
