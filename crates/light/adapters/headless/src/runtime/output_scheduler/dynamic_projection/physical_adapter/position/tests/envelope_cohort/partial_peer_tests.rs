//! Independent partial Mask and Playback Current operations can complete the same mechanical
//! cohort. Actual captured source arithmetic and static fitted endpoints supply separate oracles;
//! synthetic native/model coherence does not certify measured lamp motion.
use super::super::super::cut_coordinator::operation::inspect_attempt;
use super::*;
use light_dynamics::{
    CompiledProgrammingFamilyExpression, FamilyExpressionOperation,
    WholeFamilyExpressionFrameResolver,
};

struct PureAngles;
impl WholeFamilyExpressionFrameResolver for PureAngles {
    fn resolve(
        &self,
        _: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("this oracle only evaluates the original retained Angle arithmetic")
    }
}
fn static_pairs(
    desk: &EnvelopeDesk,
    values: &[AttributeValue; 2],
) -> [HashMap<FixtureId, [f64; 2]>; 2] {
    let resolved = desk.shared.rig.resolve(&[
        (desk.shared.heads[0], values[0].clone()),
        (desk.shared.heads[1], values[1].clone()),
    ]);
    assert_eq!(resolved.results.len(), 2);
    let snapshot = desk.shared.rig.engine.snapshot();
    let profile = snapshot.fixtures[0]
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap();
    let mut pairs: [HashMap<FixtureId, [f64; 2]>; 2] = Default::default();
    for (index, row) in resolved.results.iter().enumerate() {
        let AttributeValue::Position(expected) = &values[index] else {
            panic!("Position oracle")
        };
        assert!(
            row.requested == *expected.as_ref(),
            "the full cohort preserves ordered original intents"
        );
        assert_eq!(row.achieved.outcomes.len(), 2);
        for outcome in &row.achieved.outcomes {
            assert_eq!(
                outcome.result.emitter_id,
                profile.geometry.emitters[index].id
            );
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(!outcome.input_requirement && !outcome.missing_mount);
            assert!(
                pairs[index]
                    .insert(outcome.destination, outcome.result.achieved.unwrap())
                    .is_none()
            );
        }
        assert!(pairs[index].contains_key(&desk.shared.rig.root));
        assert!(pairs[index].contains_key(&desk.copy));
    }
    pairs
}

#[test]
fn actual_partial_mask_peer_and_current_envelope_fit_the_complete_native_cohort_per_copy() {
    let mut desk = EnvelopeDesk::new_configured(true, 1., true, true);
    let mask = angles(desk.shared.angles[1][0], desk.shared.angles[1][1] + 15.);
    desk.mask(mask.clone(), Some(1000));
    desk.tick();
    desk.shared.rig.clock.advance_millis(100);
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(
        evidence.completed, 1,
        "actual heterogeneous consumer: {evidence:?}"
    );
    assert!(
        evidence.endpoint_cohorts >= 3,
        "both original operations need complete endpoint cohorts: {evidence:?}"
    );
    assert!(output.requirements.is_empty());
    assert_eq!(output.results.len(), 2);
    let rows = desk.shared.heads.map(|head| {
        output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap()
    });
    let bases = rows.map(|row| program(row).base.clone());
    assert_eq!(
        bases, desk.shared.targets,
        "the earlier full fixed mask is not the new mask's underlay"
    );
    assert_ne!(bases[1], desk.fixed);
    for row in rows {
        assert!(!row.quality.held);
        assert_eq!(program(row).samples.len(), 1);
        assert_eq!(row.achieved.destinations.len(), 2);
        assert_eq!(row.achieved.outcomes.len(), 2);
    }
    assert!(matches!(
        &program(rows[0]).samples[0],
        FamilyCompositionSample::WholeExpression { .. }
            | FamilyCompositionSample::CoupledExpression { .. }
    ));
    let FamilyCompositionSample::Known(known) = &program(rows[1]).samples[0] else {
        panic!("actual original FixAt sample")
    };
    assert!(known.is_fix_at());
    assert!(known.activation_mix > 0. && known.activation_mix < 1.);
    let Some(DynamicValue::Family(mask_endpoint)) = known.materialized_value() else {
        panic!("complete materialized mask")
    };
    assert_eq!(mask_endpoint, &mask);
    let sample = output
        .sampled
        .samples
        .iter()
        .find(|sample| sample.target == desk.shared.heads[0])
        .unwrap();
    let runtime = desk.runtime.snapshot();
    let instance = &runtime.instances[0];
    assert_eq!(runtime.instances.len(), 1);
    assert_eq!(sample.instance_id, instance.id);
    assert_eq!(sample.controller_id, instance.controllers[0].id);
    assert_eq!(instance.targets, vec![desk.shared.heads[0]]);
    let resume = instance
        .synchronized_resume_transition
        .expect("genuine retained Resume");
    let occurrences = resume_occurrences(&sample.expression);
    assert_eq!(occurrences.len(), 1);
    assert_eq!(occurrences[0].0, resume.occurrence_id);
    assert!(occurrences[0].1 > 0. && occurrences[0].1 < 1.);
    assert_eq!(sample.activation_mix, 1.);
    let playback = capture
        .dynamic_playbacks()
        .iter()
        .find(|playback| playback.playback_number == 1)
        .unwrap();
    assert_eq!(
        playback.master, 0.25,
        "captured fader pickup establishes actual Current mix"
    );
    let mixes = [f64::from(playback.master), f64::from(known.activation_mix)];
    assert_ne!(
        mixes[0], mixes[1],
        "independent operations retain different actual parameters"
    );
    let raw_endpoint = CompiledProgrammingFamilyExpression::new(
        Arc::new(sample.expression.clone()),
        ProgrammingOwner::Position,
        None,
        None,
    )
    .unwrap()
    .evaluate(&bases[0], &PureAngles)
    .unwrap();
    assert!(
        matches!(&raw_endpoint, AttributeValue::Position(value) if matches!(value.as_ref(), PositionIntent::Angles { .. }))
    );
    let baseline = static_pairs(&desk, &bases);
    let endpoints = static_pairs(&desk, &[raw_endpoint, mask_endpoint.clone()]);
    assert!(
        (baseline[0][&desk.copy][1] - baseline[0][&desk.shared.rig.root][1]).abs() > 1.,
        "the displaced copy has its own Target inverse"
    );
    let mut claims = HashMap::new();
    for destination in [desk.shared.rig.root, desk.copy] {
        let common_pan = baseline[0][&destination][0];
        for index in 0..2 {
            assert!((baseline[index][&destination][0] - common_pan).abs() < 0.03);
            assert!(
                (endpoints[index][&destination][0] - common_pan).abs() < 0.03,
                "independent Tilt interpolation must keep compatible shared Pan"
            );
            let expected: [f64; 2] = std::array::from_fn(|axis| {
                let base = baseline[index][&destination][axis];
                base + mixes[index] * (endpoints[index][&destination][axis] - base)
            });
            let row = rows[index];
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
                    "owner {index}, copy {destination:?}: {pair:?} != {expected:?}"
                );
            }
            let outcome = row
                .achieved
                .outcomes
                .iter()
                .find(|outcome| outcome.destination == destination)
                .unwrap();
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(!outcome.input_requirement && !outcome.missing_mount);
            let achieved = outcome.result.achieved.unwrap();
            for axis in 0..2 {
                assert!((achieved[axis] - pair[axis]).abs() < 0.03);
            }
            let physical = output
                .rendered
                .physical
                .instances
                .iter()
                .find(|instance| instance.instance_id == destination.0)
                .unwrap();
            assert!(physical.complete);
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
                    assert_eq!(previous, write.raw, "shared Pan writes agree");
                }
            }
        }
    }
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
