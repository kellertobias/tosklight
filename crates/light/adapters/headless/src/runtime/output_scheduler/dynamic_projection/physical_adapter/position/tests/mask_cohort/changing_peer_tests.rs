//! Independent captured partial masks over the original runtime/static prefixes. Complete
//! static cohort fits are the per-copy oracle; synthetic fixture math is not lamp calibration.
use super::super::super::cut_coordinator::operation::inspect_attempt;
use super::*;
use light_dynamics::{
    CompiledProgrammingFamilyExpression, FamilyExpressionOperation,
    WholeFamilyExpressionFrameResolver,
};

struct PureOriginalPrefix;
impl WholeFamilyExpressionFrameResolver for PureOriginalPrefix {
    fn resolve(
        &self,
        _: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("the original constant Target prefix needs no physical operand replay")
    }
}

fn static_pairs(
    desk: &MaskDesk,
    values: &[AttributeValue; 2],
) -> [HashMap<FixtureId, [f64; 2]>; 2] {
    let resolved = desk.shared.rig.resolve(&[
        (desk.shared.heads[0], values[0].clone()),
        (desk.shared.heads[1], values[1].clone()),
    ]);
    assert_eq!(resolved.results.len(), 2);
    let mut pairs: [HashMap<FixtureId, [f64; 2]>; 2] = Default::default();
    for (index, row) in resolved.results.iter().enumerate() {
        let AttributeValue::Position(expected) = &values[index] else {
            panic!("Position intent")
        };
        assert!(row.requested == *expected.as_ref());
        assert_eq!(row.achieved.outcomes.len(), 2);
        for outcome in &row.achieved.outcomes {
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
fn independent_changing_masks_replay_original_prefixes_and_fit_every_calibrated_copy() {
    let mut desk = MaskDesk::with_distinct_copy(true);
    desk.shared.rig.clock.advance_millis(1);
    let peer_mask = angles(desk.shared.angles[1][0], desk.shared.angles[1][1] + 15.);
    desk.hold(1, peer_mask.clone(), Some(1000));
    desk.tick();
    desk.shared.rig.clock.advance_millis(100);
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(
        evidence.completed, 1,
        "genuine local-stage consumer: {evidence:?}"
    );
    assert!(
        evidence.endpoint_cohorts >= 3,
        "both cuts need complete endpoint cohorts: {evidence:?}"
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
    let endpoints = [desk.mask.clone(), peer_mask];
    let mixes: [f64; 2] = std::array::from_fn(|index| {
        let requested = program(rows[index]);
        assert_eq!(requested.base, desk.shared.targets[index]);
        assert_eq!(requested.samples.len(), if index == 0 { 2 } else { 1 });
        let masks = requested
            .samples
            .iter()
            .filter_map(|sample| match sample {
                FamilyCompositionSample::Known(known) if known.is_fix_at() => Some(known),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(masks.len(), 1);
        assert_eq!(
            masks[0].materialized_value(),
            Some(&DynamicValue::Family(endpoints[index].clone()))
        );
        assert!(masks[0].activation_mix > 0. && masks[0].activation_mix < 1.);
        f64::from(masks[0].activation_mix)
    });
    assert_ne!(
        mixes[0], mixes[1],
        "independent masks keep their actual different activation clocks"
    );
    assert_eq!(
        output.sampled.samples.len(),
        1,
        "original runtime emits once before operand replay"
    );
    let sampled = &output.sampled.samples[0];
    assert_eq!(sampled.target, desk.shared.heads[0]);
    assert_eq!(sampled.activation_mix, 1.);
    let original_prefix = CompiledProgrammingFamilyExpression::new(
        Arc::new(sampled.expression.clone()),
        ProgrammingOwner::Position,
        None,
        None,
    )
    .unwrap()
    .evaluate(&desk.shared.targets[0], &PureOriginalPrefix)
    .unwrap();
    assert_eq!(
        original_prefix, desk.prefix,
        "the original sampled source is consumed before the mask exactly once"
    );
    let runtime = desk.runtime.snapshot();
    assert_eq!(runtime.instances.len(), 1);
    assert_eq!(sampled.instance_id, runtime.instances[0].id);
    assert_eq!(
        sampled.controller_id,
        runtime.instances[0].controllers[0].id
    );
    assert_eq!(runtime.instances[0].targets, vec![desk.shared.heads[0]]);
    // Replacing the former full peer FixAt exposes the original static Target, not that
    // old mask's +8-degree command. Both original prefixes are fitted as one full cohort.
    let baseline_values = [desk.prefix.clone(), desk.shared.targets[1].clone()];
    assert_ne!(baseline_values[1], desk.peer);
    let baseline = static_pairs(&desk, &baseline_values);
    let fitted_endpoints = static_pairs(&desk, &endpoints);
    assert!(
        (baseline[1][&desk.shared.rig.root][1] - desk.peer_pairs[&desk.shared.rig.root][1]).abs()
            > 7.
    );
    assert!((baseline[1][&desk.copy][1] - baseline[1][&desk.shared.rig.root][1]).abs() > 1.);
    let mut claims = HashMap::new();
    for destination in [desk.shared.rig.root, desk.copy] {
        let physical = output
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == destination.0)
            .unwrap();
        let pan = baseline[0][&destination][0];
        for index in 0..2 {
            let row = rows[index];
            assert!(!row.quality.held);
            assert_eq!(row.achieved.outcomes.len(), 2);
            assert!((baseline[index][&destination][0] - pan).abs() < 0.03);
            assert!((fitted_endpoints[index][&destination][0] - pan).abs() < 0.03);
            let expected: [f64; 2] = std::array::from_fn(|axis| {
                let base = baseline[index][&destination][axis];
                base + mixes[index] * (fitted_endpoints[index][&destination][axis] - base)
            });
            let commanded = row
                .achieved
                .destinations
                .iter()
                .find(|value| value.destination == destination)
                .unwrap();
            let pair = commanded_angles(&commanded.value);
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
                assert!(
                    (pair[axis] - expected[axis]).abs() < 0.07,
                    "owner {index}, copy {destination:?}: {pair:?} != {expected:?}"
                );
                assert!((achieved[axis] - expected[axis]).abs() < 0.07);
            }
            assert!((physical.axes()[0].absolute_degrees().unwrap() - achieved[0]).abs() < 0.04);
            assert!(
                (physical.axes()[index + 1].absolute_degrees().unwrap() - achieved[1]).abs() < 0.04
            );
            let continuity = desk
                .lane
                .continuity(row.target, ProgrammingOwner::Position)
                .unwrap();
            let accepted = continuity
                .instances
                .iter()
                .find(|instance| instance.destination == destination)
                .unwrap();
            for write in row
                .writes
                .iter()
                .filter(|write| write.slot.destination == destination)
            {
                assert!(!write.parked);
                assert_eq!(
                    physical.native_raw[write.slot.channel_index as usize],
                    write.raw
                );
                assert!(
                    accepted
                        .controls
                        .iter()
                        .any(|&(channel, id, _, raw)| channel == write.slot.channel_index
                            && id == write.channel_id
                            && raw == write.raw)
                );
                if let Some(previous) =
                    claims.insert((destination, write.slot.channel_index), write.raw)
                {
                    assert_eq!(previous, write.raw);
                }
            }
        }
    }
    assert_eq!(claims.len(), 6);
    let universe = &output.rendered.universes[&1];
    for ((destination, channel), raw) in claims {
        let slot = if destination == desk.shared.rig.root {
            0
        } else {
            19
        } + 2 * channel as usize;
        assert_eq!(
            u32::from(u16::from_be_bytes([universe[slot], universe[slot + 1]])),
            raw
        );
    }
    assert_eq!(output.token, capture.frame_token());
    assert_eq!(desk.lane.last_accepted(), Some(output.token.clone()));
}
