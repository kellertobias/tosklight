//! A captured Programmer Pan hold edits the complete destination-bound numeric Angle pair.
//! Its release reveals the continuing source, including each copy's fresh Target Current tilt.
use super::programs::{
    commanded_angles, destination_dynamic_rig, position_definition, program,
    start_position_dynamic, verify_live_native,
};
use super::*;
use light_dynamics::{
    AngleNumericNode, CoupledRetainedSources, DynamicFamilyRepresentation, DynamicLaneBody,
    DynamicSampleExpression, DynamicValue, DynamicValueSource, DynamicValueTiming,
    FamilyCompositionSample, ProgrammingLaneConfiguration,
};

#[test]
fn actual_live_partial_pan_fixat_releases_to_continuing_numeric_current_pair_per_copy() {
    let (rig, copy) = destination_dynamic_rig();
    let mut base = target(TargetReference::Origin, [4., 8., 1.]);
    let mut definition = position_definition(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        },
        [DynamicValue::Scalar(80.), DynamicValue::Scalar(80.)],
    );
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        unreachable!()
    };
    let ProgrammingLaneConfiguration::Keyframes(config) = &mut body.configuration else {
        unreachable!()
    };
    config.points[0].source = DynamicValueSource::Current;
    assert_eq!(
        definition.lanes.len(),
        2,
        "Pan retains its explicit Tilt Current partner"
    );
    let pan_lane = definition.lanes[0].id;
    let mut runtime = start_position_dynamic(&rig, &base, &definition, None);
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    let first_capture = rig.capture();
    let first = prepare_live(
        &rig,
        &first_capture,
        &first_capture,
        &lane,
        &mut runtime,
        &mut origins,
        &mut scratch,
    )
    .unwrap();
    assert!(first.requirements.is_empty());
    verify_live_native(&rig, &first_capture, &lane, &first);
    let original = first
        .sampled
        .samples
        .iter()
        .find(|sample| sample.lane_id == pan_lane)
        .unwrap();
    let identity = (
        original.instance_id,
        original.controller_id,
        original.activated_at_millis,
    );
    let DynamicSampleExpression::AngleNumeric {
        program: original_numeric,
    } = &original.expression
    else {
        panic!("Current keyframes retain the original numeric arithmetic")
    };
    let occurrence = original_numeric.occurrence;
    assert!(occurrence.is_some());
    let started = runtime.snapshot().instances[0].started_at_millis;
    let instance_link = first_capture
        .dynamic_programmer_values()
        .iter()
        .find_map(|(_, _, row)| {
            if let DynamicSemanticValue::DynamicOn { instance_link, .. } = row.value {
                Some(instance_link)
            } else {
                None
            }
        })
        .unwrap();
    const HELD_PAN: f32 = -45.;
    // The complete capture deliberately contains an unrelated Tilt. A Pan mask must never
    // replace the live Current partner with this stored sibling component.
    let mask_family = angles(HELD_PAN, 123.);
    let mut previous_phase = 0.;
    let mut previous_pan: Option<f64> = None;
    let mut before_target_change_tilts: Option<Vec<(FixtureId, f64)>> = None;
    for stage in 0..4 {
        rig.clock.advance_millis(if stage == 1 { 25 } else { 1 });
        match stage {
            0 => assert!(
                rig.programmers.apply_dynamic_values(
                    rig.session,
                    &[DynamicProgrammerValueMutation::Set {
                        fixture_id: rig.root,
                        attribute: ProgrammingOwner::Position.key(),
                        value: DynamicSemanticValue::ProgrammingFixAt {
                            mask: ProgrammingFamilyFixAt::from_family(
                                ProgrammingOwner::Position,
                                Some(ProgrammingComponent::Pan),
                                mask_family.clone(),
                            )
                            .unwrap(),
                            timing: DynamicValueTiming {
                                fade_millis: Some(200),
                                delay_millis: None
                            },
                        },
                    }],
                    None,
                )
            ),
            2 => {
                assert!(rig.programmers.apply_dynamic_values(
                    rig.session,
                    &[DynamicProgrammerValueMutation::Set {
                        fixture_id: rig.root,
                        attribute: ProgrammingOwner::Position.key(),
                        value: DynamicSemanticValue::ProgrammingRelease {
                            component: Some(ProgrammingComponent::Pan)
                        },
                    }],
                    None,
                ));
                rig.clock.advance_millis(1);
                base = target(TargetReference::Origin, [6., 8., 2.]);
                rig.programmers.set(
                    rig.session,
                    rig.root,
                    ProgrammingOwner::Position.key(),
                    base.clone(),
                );
            }
            3 => assert!(rig.programmers.apply_dynamic_values(
                rig.session,
                &[DynamicProgrammerValueMutation::Set {
                    fixture_id: rig.root,
                    attribute: ProgrammingOwner::Position.key(),
                    value: DynamicSemanticValue::DynamicOff {
                        instance_link,
                        timing: DynamicValueTiming {
                            fade_millis: Some(400),
                            delay_millis: None
                        },
                    },
                }],
                None,
            )),
            _ => {}
        }
        if stage == 3 {
            // Off timing begins when the output transaction first reconciles the captured
            // command. Publish that onset frame before testing a later partial release.
            let onset_capture = rig.capture();
            let onset = prepare_live(
                &rig,
                &onset_capture,
                &onset_capture,
                &lane,
                &mut runtime,
                &mut origins,
                &mut scratch,
            )
            .unwrap();
            assert!(onset.requirements.is_empty());
            verify_live_native(&rig, &onset_capture, &lane, &onset);
            let onset_pan = onset
                .sampled
                .samples
                .iter()
                .find(|sample| sample.lane_id == pan_lane)
                .unwrap();
            assert_eq!(
                onset_pan.activation_mix, 1.,
                "Off starts at the current complete influence"
            );
            assert_eq!(
                (
                    onset_pan.instance_id,
                    onset_pan.controller_id,
                    onset_pan.activated_at_millis
                ),
                identity
            );
        }
        let accepted = lane
            .continuity(rig.root, ProgrammingOwner::Position)
            .unwrap();
        let endpoint = rig.resolve_with(&[(rig.root, base.clone())], Some(&accepted), &[]);
        rig.verify(&endpoint);
        let current = endpoint.results[0]
            .achieved
            .outcomes
            .iter()
            .map(|outcome| {
                assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
                (outcome.destination, outcome.result.achieved.unwrap())
            })
            .collect::<Vec<_>>();
        let capture = rig.capture();
        let output = prepare_live(
            &rig,
            &capture,
            &capture,
            &lane,
            &mut runtime,
            &mut origins,
            &mut scratch,
        )
        .unwrap();
        assert!(
            output.requirements.is_empty(),
            "stage {stage}: {:?}",
            output
                .requirements
                .iter()
                .map(|requirement| (
                    requirement.target,
                    requirement.owner,
                    super::numeric::requirement_debug(&requirement.reason),
                ))
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            output.results.len(),
            1,
            "one Position owner covers root and copy"
        );
        verify_live_native(&rig, &capture, &lane, &output);
        let row = &output.results[0];
        let logical = program(row);
        assert_eq!(
            logical.base, base,
            "the original Target remains recording input"
        );
        assert_eq!(logical.samples.len(), if stage < 2 { 2 } else { 1 });
        assert_eq!(row.achieved.destinations.len(), 2);
        let source = output
            .sampled
            .samples
            .iter()
            .find(|sample| sample.lane_id == pan_lane)
            .unwrap();
        assert_eq!(
            (
                source.instance_id,
                source.controller_id,
                source.activated_at_millis
            ),
            identity
        );
        let DynamicSampleExpression::AngleNumeric { program: numeric } = &source.expression else {
            panic!("a Pan hold cannot bake the underlying Current")
        };
        assert_eq!(numeric.occurrence, occurrence);
        assert!(
            numeric
                .nodes
                .iter()
                .any(|node| matches!(node, AngleNumericNode::Current))
        );
        assert_eq!(runtime.snapshot().instances[0].started_at_millis, started);
        assert_eq!(
            runtime.snapshot().instances.len(),
            1,
            "no oscillator is created per physical copy"
        );
        let phase = (capture.sampled_at().timestamp_millis() as u64 - started) as f64 / 500.;
        assert!(
            phase > previous_phase && phase < 1.,
            "the underlying first keyframe segment keeps advancing: {phase}"
        );
        previous_phase = phase;
        let activation = f64::from(source.activation_mix);
        assert!(if stage == 3 {
            activation > 0. && activation < 1.
        } else {
            activation == 1.
        });
        let hold = logical
            .samples
            .iter()
            .filter_map(|sample| match sample {
                FamilyCompositionSample::Known(sample) if sample.is_fix_at() => Some(sample),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(hold.len(), usize::from(stage < 2));
        let hold_mix = hold.first().map_or(0., |hold| {
            assert_eq!(
                hold.address().address().component,
                Some(ProgrammingComponent::Pan)
            );
            assert_eq!(
                hold.materialized_value(),
                Some(&DynamicValue::Scalar(HELD_PAN))
            );
            assert!(hold.activation_mix > 0. && hold.activation_mix < 1.);
            f64::from(hold.activation_mix)
        });
        let forest = logical
            .samples
            .iter()
            .find_map(|sample| match sample {
                FamilyCompositionSample::CoupledExpression { expression, .. } => {
                    match expression.retained_sources() {
                        CoupledRetainedSources::PositionForest(samples) => Some(samples),
                        _ => None,
                    }
                }
                _ => None,
            })
            .expect("one complete original Pan/Tilt forest");
        assert_eq!(forest.len(), 2);
        for retained in forest.iter() {
            let sampled = output
                .sampled
                .samples
                .iter()
                .find(|sample| sample.lane_id == retained.lane_id)
                .unwrap();
            assert_eq!(
                retained.expression, sampled.expression,
                "requested sources remain symbolic"
            );
        }
        let mut actual_tilts = Vec::new();
        for destination in &row.achieved.destinations {
            let axes = current
                .iter()
                .find(|(id, _)| *id == destination.destination)
                .unwrap()
                .1;
            let dynamic_pan = axes[0] + phase * (80. - axes[0]);
            let active_pan = axes[0] + activation * (dynamic_pan - axes[0]);
            let expected_pan = active_pan + hold_mix * (f64::from(HELD_PAN) - active_pan);
            let actual = commanded_angles(&destination.value);
            assert!(
                (actual[0] - expected_pan).abs() < 0.06,
                "stage {stage} {:?}: Pan {} != {expected_pan}",
                destination.destination,
                actual[0]
            );
            assert!(
                (actual[1] - axes[1]).abs() < 0.06,
                "stage {stage}: Tilt Current remains live and is not the held 123 degrees"
            );
            let outcome = row
                .achieved
                .outcomes
                .iter()
                .find(|outcome| outcome.destination == destination.destination)
                .unwrap();
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(
                outcome
                    .result
                    .achieved
                    .unwrap()
                    .iter()
                    .zip(actual)
                    .all(|(a, b)| (a - b).abs() < 0.023)
            );
            actual_tilts.push((destination.destination, actual[1]));
            if destination.destination == rig.root {
                if stage == 2 {
                    assert!(
                        (previous_pan.unwrap() - actual[0]).abs() > 1.,
                        "release reveals the continuing unheld Pan"
                    );
                }
                previous_pan = Some(actual[0]);
            }
        }
        assert!(actual_tilts.iter().any(|(id, _)| *id == rig.root));
        assert!(actual_tilts.iter().any(|(id, _)| *id == copy));
        assert!(
            (actual_tilts[0].1 - actual_tilts[1].1).abs() > 1.,
            "copies retain independent Current partners"
        );
        if stage == 1 {
            before_target_change_tilts = Some(actual_tilts);
        } else if stage == 2 {
            for (id, old) in before_target_change_tilts.as_ref().unwrap() {
                let new = actual_tilts
                    .iter()
                    .find(|(destination, _)| destination == id)
                    .unwrap()
                    .1;
                assert!(
                    (new - old).abs() > 1.,
                    "changing the authored Target updates the released Current partner for {id:?}"
                );
            }
        }
        assert_eq!(
            rig.engine.snapshot().dynamics[0],
            definition,
            "fitting and masking never rewrite the Dynamic"
        );
        if stage < 2 {
            let stored = capture
                .dynamic_programmer_values()
                .iter()
                .find_map(|(_, _, row)| match &row.value {
                    DynamicSemanticValue::ProgrammingFixAt { mask, .. } => Some(mask),
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                stored.family, mask_family,
                "mask capture retains its complete original family"
            );
        }
    }
}
