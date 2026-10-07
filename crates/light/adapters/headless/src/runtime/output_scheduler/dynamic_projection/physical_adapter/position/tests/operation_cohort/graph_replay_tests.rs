//! Genuine graph cuts across distinct installed copies and actual layered programmer sources.
//! Static complete-endpoint fitting supplies the geometry oracle; real sampler parameters
//! supply the interpolation oracle. Synthetic profiles do not certify physical lamps.
use super::*;
use light_dynamics::FamilyCompositionSample;

fn target_endpoint(desk: &OperationDesk) -> AttributeValue {
    let snapshot = desk.shared.rig.engine.snapshot();
    let DynamicLaneBody::Programming(body) = &snapshot.dynamics[0].lanes[0].body else {
        panic!("actual typed lane")
    };
    let ProgrammingLaneConfiguration::Keyframes(configuration) = &body.configuration else {
        panic!("actual keyframe producer")
    };
    let DynamicValueSource::Value {
        value: DynamicValue::Family(value),
    } = &configuration.points[0].source
    else {
        panic!("Size fixture uses the original authored Target endpoint")
    };
    assert!(
        matches!(value, AttributeValue::Position(intent) if matches!(intent.as_ref(), PositionIntent::Target { .. }))
    );
    value.clone()
}

pub(super) fn endpoint_pairs(
    desk: &OperationDesk,
    values: &[AttributeValue; 2],
) -> HashMap<(usize, FixtureId), [f64; 2]> {
    let resolved = desk.shared.rig.resolve(
        &desk
            .shared
            .heads
            .iter()
            .copied()
            .zip(values.iter().cloned())
            .collect::<Vec<_>>(),
    );
    let mut pairs = HashMap::new();
    for (index, resolution) in resolved.results.iter().enumerate() {
        for outcome in &resolution.achieved.outcomes {
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(!outcome.input_requirement && !outcome.missing_mount);
            pairs.insert(
                (index, outcome.destination),
                outcome.result.achieved.unwrap(),
            );
        }
    }
    assert_eq!(
        pairs.len(),
        4,
        "complete static endpoint cohort includes every owner and physical copy"
    );
    pairs
}

pub(super) fn assert_native_claims(
    desk: &OperationDesk,
    output: &PublishedPhysicalFrame<PositionAdapter>,
    claims: &HashMap<(FixtureId, u32), u32>,
) {
    assert_eq!(
        claims.len(),
        6,
        "all three mechanical motors in both destinations"
    );
    for (destination, start) in [(desk.shared.rig.root, 0), (desk.copy, 19)] {
        let physical = output
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == destination.0)
            .unwrap();
        assert!(physical.complete);
        let bytes = &output.rendered.universes[&1];
        for channel in 0..3u32 {
            let raw = claims[&(destination, channel)];
            let slot = start + channel as usize * 2;
            let wire = u32::from(u16::from_be_bytes([bytes[slot], bytes[slot + 1]]));
            assert_eq!(
                wire, raw,
                "complete coarse/fine word reaches DMX once for {destination:?}, channel {channel}"
            );
            assert_eq!(
                physical.native_raw[channel as usize], raw,
                "physical forward and native transport use the same command"
            );
        }
    }
}

#[test]
fn actual_size_uses_each_displaced_calibrated_copy_endpoint_before_native_finalization() {
    let mut desk = OperationDesk::new(Kind::Size, true, 0.);
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

    // Both complete endpoint cohorts use the actual installation independently of graph
    // replay: original static Angle baselines and the sampler's authored Target endpoint.
    let outgoing_pairs = endpoint_pairs(&desk, &desk.bases);
    let endpoint = target_endpoint(&desk);
    let incoming_pairs = endpoint_pairs(&desk, &[endpoint.clone(), endpoint]);
    for index in 0..2 {
        assert!(
            (incoming_pairs[&(index, desk.copy)][1]
                - incoming_pairs[&(index, desk.shared.rig.root)][1])
                .abs()
                > 1.,
            "displaced and calibrated copy must need a different incoming Tilt"
        );
    }
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(evidence.parents, 4);
    assert_eq!(evidence.endpoint_cohorts, 2);
    assert_eq!(
        evidence.completed, 1,
        "real Size bridge completed every original parent"
    );
    let (handles, parameters) = desk.evidence(&output);
    assert!(matches!(
        handles[0].correspondence(&handles[1]),
        DynamicOperationCorrespondence::Shared { historical: false }
    ));
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
        assert_eq!(program(row).base, desk.bases[index]);
        assert_eq!(program(row).samples.len(), 1);
        assert_eq!(row.achieved.destinations.len(), 2);
        let mut final_pairs = HashMap::new();
        for destination in [desk.shared.rig.root, desk.copy] {
            let value = row
                .achieved
                .destinations
                .iter()
                .find(|value| value.destination == destination)
                .unwrap();
            let pair = commanded_angles(&value.value);
            let from = outgoing_pairs[&(index, destination)];
            let to = incoming_pairs[&(index, destination)];
            let expected: [f64; 2] = std::array::from_fn(|axis| {
                from[axis] + f64::from(parameters[index]) * (to[axis] - from[axis])
            });
            for axis in 0..2 {
                assert!(
                    (pair[axis] - expected[axis]).abs() < 0.07,
                    "owner {index}, {destination:?}: {pair:?} != {expected:?}"
                );
            }
            final_pairs.insert(destination, pair);
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
            assert!((physical.axes()[0].absolute_degrees().unwrap() - pair[0]).abs() < 0.03);
            assert!(
                (physical.axes()[index + 1].absolute_degrees().unwrap() - pair[1]).abs() < 0.03
            );
            for write in row
                .writes
                .iter()
                .filter(|write| write.slot.destination == destination)
            {
                assert!(!write.parked);
                if let Some(previous) =
                    claims.insert((destination, write.slot.channel_index), write.raw)
                {
                    assert_eq!(
                        previous, write.raw,
                        "all logical peers agree on the shared Pan command"
                    );
                }
            }
        }
        assert!(
            (final_pairs[&desk.copy][1] - final_pairs[&desk.shared.rig.root][1]).abs() > 1.,
            "final parent must not broadcast the root's operand pair to its displaced copy"
        );
    }
    assert_native_claims(&desk, &output, &claims);
    assert_eq!(output.token, capture.frame_token());
}

#[test]
fn actual_size_below_two_partial_fixed_masks_replays_its_underlay_and_applies_each_suffix_once() {
    let mut desk = OperationDesk::new(Kind::Size, true, 0.);
    let outgoing_pairs = endpoint_pairs(&desk, &desk.bases);
    let endpoint = target_endpoint(&desk);
    let incoming_pairs = endpoint_pairs(&desk, &[endpoint.clone(), endpoint]);
    let masks = [0, 1].map(|index| angles(desk.end[0] + 8., desk.end[1] + 12. + index as f32 * 5.));
    let mutations = desk
        .shared
        .heads
        .iter()
        .copied()
        .zip(&masks)
        .map(|(head, mask)| DynamicProgrammerValueMutation::Set {
            fixture_id: head,
            attribute: ProgrammingOwner::Position.key(),
            value: DynamicSemanticValue::ProgrammingFixAt {
                mask: ProgrammingFamilyFixAt::from_family(
                    ProgrammingOwner::Position,
                    None,
                    mask.clone(),
                )
                .unwrap(),
                timing: DynamicValueTiming {
                    fade_millis: Some(1000),
                    delay_millis: None,
                },
            },
        })
        .collect::<Vec<_>>();
    assert!(desk.shared.rig.programmers.apply_dynamic_values(
        desk.shared.rig.session,
        &mutations,
        None
    ));
    desk.tick(); // Establish the actual fixed-mask activation clock.
    desk.shared.rig.clock.advance_millis(250);
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(
        evidence.parents, 4,
        "both genuine Size operations suspend below their independent masks"
    );
    assert_eq!(evidence.endpoint_cohorts, 2);
    assert_eq!(
        evidence.completed, 1,
        "multi-source graph consumer, not one-owner mask consumer"
    );
    let (handles, parameters) = desk.evidence(&output);
    assert!(matches!(
        handles[0].correspondence(&handles[1]),
        DynamicOperationCorrespondence::Shared { historical: false }
    ));
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
        let requested = program(row);
        assert_eq!(requested.base, desk.bases[index]);
        assert_eq!(
            requested.samples.len(),
            2,
            "retain actual runtime history and its independent fixed-mask source"
        );
        let fixed = requested
            .samples
            .iter()
            .filter_map(|sample| match sample {
                FamilyCompositionSample::Known(sample) if sample.is_fix_at() => Some(sample),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(fixed.len(), 1);
        assert_eq!(
            fixed[0].materialized_value(),
            Some(&DynamicValue::Family(masks[index].clone()))
        );
        let mix = f64::from(fixed[0].activation_mix);
        assert!(mix > 0. && mix < 1., "the mask's real fade must be active");
        let mask_pair = commanded_angles(&masks[index]);
        for destination in [desk.shared.rig.root, desk.copy] {
            let expected: [f64; 2] = std::array::from_fn(|axis| {
                let original = outgoing_pairs[&(index, destination)][axis];
                let incoming = incoming_pairs[&(index, destination)][axis];
                let dynamic = original + f64::from(parameters[index]) * (incoming - original);
                dynamic + mix * (mask_pair[axis] - dynamic)
            });
            let pair = commanded_angles(
                &row.achieved
                    .destinations
                    .iter()
                    .find(|value| value.destination == destination)
                    .unwrap()
                    .value,
            );
            for axis in 0..2 {
                assert!(
                    (pair[axis] - expected[axis]).abs() < 0.07,
                    "the original non-idempotent mask suffix must execute once: {pair:?} != {expected:?}"
                );
            }
            for write in row
                .writes
                .iter()
                .filter(|write| write.slot.destination == destination)
            {
                assert!(!write.parked);
                if let Some(previous) =
                    claims.insert((destination, write.slot.channel_index), write.raw)
                {
                    assert_eq!(
                        previous, write.raw,
                        "independent mask suffixes keep shared Pan compatible"
                    );
                }
            }
        }
    }
    assert_native_claims(&desk, &output, &claims);
    assert_eq!(output.token, capture.frame_token());
}
