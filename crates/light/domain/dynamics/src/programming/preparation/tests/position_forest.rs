use super::*;
use std::cell::RefCell;

fn target(reference: TargetReference, offsets: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(reference, offsets)))
}
fn offset(reference: TargetReference, component: ProgrammingComponent, value: f32) -> E {
    E::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target {
                reference: Some(reference),
            },
            component: Some(component),
        }),
        value: DynamicValue::Scalar(value),
        occurrence: None,
        dependency_occurrence: None,
    }
}
fn values(value: &AttributeValue) -> [f32; 2] {
    let AttributeValue::Position(value) = value else {
        panic!("Position")
    };
    match value.as_ref() {
        PositionIntent::Angles {
            pan_degrees: ScalarIntent::Value(pan),
            tilt_degrees: ScalarIntent::Value(tilt),
        } => [*pan, *tilt],
        PositionIntent::Target {
            offset_metres:
                [
                    ScalarIntent::Value(x),
                    ScalarIntent::Value(y),
                    ScalarIntent::Value(z),
                ],
            ..
        } => [x * 10.0, y * 10.0 + z],
        _ => panic!("materialized coordinates"),
    }
}
#[derive(Default)]
struct ResolvedFrame {
    endpoints: RefCell<Vec<(AttributeValue, AttributeValue)>>,
}
impl WholeFamilyExpressionFrameResolver for ResolvedFrame {
    fn resolve(
        &self,
        _: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.endpoints.borrow_mut().push((from.clone(), to.clone()));
        let (a, b) = (values(from), values(to));
        let FamilyExpressionOperation::Transition { progress } = operation else {
            panic!("transition")
        };
        Ok(angles(
            a[0] + (b[0] - a[0]) * progress,
            a[1] + (b[1] - a[1]) * progress,
        ))
    }
}
fn evaluate(
    group: &DynamicFamilySampleGroup,
    base: &AttributeValue,
    frame: &ResolvedFrame,
) -> AttributeValue {
    compose_retained_dynamic_family(
        group.owner,
        base,
        &group.samples,
        &FamilyCompositionContext::default(),
        frame,
        &mut RetainedFamilyCompositionScratch::default(),
    )
    .unwrap()
}
fn crossing(progress: f32, reverse: bool) -> Vec<DynamicRuntimeSample> {
    let reference = TargetReference::Origin;
    let branches = [
        (
            Some(angle(ProgrammingComponent::Pan, 90.0)),
            Some(offset(reference, ProgrammingComponent::TargetX, 4.0)),
        ),
        (
            Some(current(ProgrammingComponent::Tilt)),
            Some(offset(reference, ProgrammingComponent::TargetY, 5.0)),
        ),
    ];
    branches
        .into_iter()
        .enumerate()
        .map(|(index, (from, to))| {
            let (from, to) = if reverse { (to, from) } else { (from, to) };
            sample(10 + index as u128, resume(from, to, progress))
        })
        .collect()
}

#[test]
fn both_directions_resolve_one_target_cohort_and_keep_untouched_z() {
    let base = target(TargetReference::Origin, [1.0, 2.0, 3.0]);
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for reverse in [false, true] {
        let prepared = prepare_dynamic_family_samples(
            &crossing(0.5, reverse),
            &Sources {
                tilt: Some(30.0),
                ..Default::default()
            },
            None,
            &mut scratch,
        )
        .unwrap();
        let frame = ResolvedFrame::default();
        assert_eq!(
            evaluate(&prepared.families[0], &base, &frame),
            angles(65.0, 41.5)
        );
        let expected = target(TargetReference::Origin, [4.0, 5.0, 3.0]);
        assert!(
            frame
                .endpoints
                .borrow()
                .iter()
                .any(|(a, b)| a == &expected || b == &expected)
        );
        let FamilyCompositionSample::CoupledExpression { expression, .. } =
            &prepared.families[0].samples[0]
        else {
            panic!()
        };
        assert!(expression.expression().is_none());
        let CoupledRetainedSources::PositionForest(sources) = expression.retained_sources() else {
            panic!()
        };
        assert_eq!(sources.len(), 2);
        let cohort = expression
            .base_endpoints()
            .iter()
            .find_map(CoupledBaseEndpoint::cohort)
            .unwrap();
        assert_eq!(
            cohort.iter().map(|v| v.lane_id).collect::<Vec<_>>(),
            vec![Uuid::from_u128(10), Uuid::from_u128(11)]
        );
    }
}

#[test]
fn target_endpoint_selects_matching_lower_reference_cohort_before_whole_transition() {
    let mut inputs = crossing(0.5, false);
    let mut z = sample(
        20,
        offset(TargetReference::Origin, ProgrammingComponent::TargetZ, 9.0),
    );
    z.controller_id = Uuid::from_u128(30);
    z.priority = 1;
    let rival_reference = TargetReference::Point {
        point_id: Uuid::from_u128(5000),
    };
    let mut rival = sample(
        21,
        offset(rival_reference, ProgrammingComponent::TargetX, 99.0),
    );
    rival.controller_id = Uuid::from_u128(31);
    rival.priority = 2;
    inputs.extend([rival, z]);
    let base = target(TargetReference::Origin, [1.0, 2.0, 3.0]);
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let prepared = prepare_dynamic_family_samples(
        &inputs,
        &Sources {
            tilt: Some(30.0),
            ..Default::default()
        },
        None,
        &mut scratch,
    )
    .unwrap();
    let frame = ResolvedFrame::default();
    assert_eq!(
        evaluate(&prepared.families[0], &base, &frame),
        angles(65.0, 44.5)
    );
    assert!(
        frame
            .endpoints
            .borrow()
            .iter()
            .any(|(_, b)| b == &target(TargetReference::Origin, [4.0, 5.0, 9.0]))
    );
}

#[test]
fn activation_is_applied_once_and_exact_endpoint_restores_component_masks() {
    let base = target(TargetReference::Origin, [1.0, 2.0, 3.0]);
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let mut inputs = crossing(0.5, false);
    for sample in &mut inputs {
        sample.activation_mix = 0.5;
    }
    let prepared = prepare_dynamic_family_samples(
        &inputs,
        &Sources {
            tilt: Some(30.0),
            ..Default::default()
        },
        None,
        &mut scratch,
    )
    .unwrap();
    assert_eq!(
        evaluate(&prepared.families[0], &base, &ResolvedFrame::default()),
        angles(37.5, 32.25)
    );
    let prepared = prepare_dynamic_family_samples(
        &crossing(1.0, false),
        &Sources::default(),
        None,
        &mut scratch,
    )
    .unwrap();
    assert_eq!(prepared.families[0].samples.len(), 2);
    for sample in &prepared.families[0].samples {
        let FamilyCompositionSample::Known(sample) = sample else {
            panic!("exact component")
        };
        assert!(sample.address().address().component.is_some());
    }
    assert_eq!(
        evaluate(&prepared.families[0], &base, &ResolvedFrame::default()),
        target(TargetReference::Origin, [4.0, 5.0, 3.0])
    );
}

#[test]
fn interrupted_forest_keeps_live_current_partners_and_correlated_membership() {
    let mut inputs = crossing(0.25, false);
    for (index, sample) in inputs.iter_mut().enumerate() {
        let incoming = if index == 0 {
            angle(ProgrammingComponent::Tilt, 60.0)
        } else {
            current(ProgrammingComponent::Pan)
        };
        sample.expression = E::Transition {
            from: Some(Arc::new(sample.expression.clone())),
            to: Some(Arc::new(incoming)),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(600),
            },
        };
    }
    let base = target(TargetReference::Origin, [1.0, 2.0, 3.0]);
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for (pan, tilt) in [(10.0, 30.0), (20.0, 40.0)] {
        let sources = Sources {
            pan: Some(pan),
            tilt: Some(tilt),
            ..Default::default()
        };
        let prepared =
            prepare_dynamic_family_samples(&inputs, &sources, None, &mut scratch).unwrap();
        let first = [90.0 + (40.0 - 90.0) * 0.25, tilt + (53.0 - tilt) * 0.25];
        assert_eq!(
            evaluate(&prepared.families[0], &base, &ResolvedFrame::default()),
            angles((first[0] + pan) * 0.5, (first[1] + 60.0) * 0.5)
        );
        assert_eq!(sources.reads.get(), 2);
        // A paused/restored instance retains authored branches and Current tokens, not the
        // solved Angle values from the preceding frame. Resume progress deliberately stays put.
        for sample in &mut inputs {
            let tape =
                RetainedExpressionTape::from_roots(&[Arc::new(sample.expression.clone())]).unwrap();
            let restored: RetainedExpressionTape =
                serde_json::from_str(&serde_json::to_string(&tape).unwrap()).unwrap();
            let root = restored.roots[0];
            sample.expression = E::Retained {
                tape: Arc::new(restored),
                root,
            };
        }
    }
}

struct EndpointContext {
    base: AttributeValue,
}
impl CoupledExpressionContext for EndpointContext {
    fn materialize_base(
        &self,
        endpoint: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
    ) -> Result<AttributeValue, TransitionError> {
        assert!(endpoint.is_none());
        Ok(self.base.clone())
    }
    fn materialize_cohort(
        &self,
        components: &[CoupledComponentEndpoint],
    ) -> Result<AttributeValue, TransitionError> {
        let edits = components
            .iter()
            .map(|source| {
                let DynamicValue::Scalar(value) = source.value else {
                    panic!()
                };
                ComponentEdit::Scalar {
                    component: source.address.address().component.unwrap(),
                    operation: ScalarEdit::Set(ScalarIntent::Value(value)),
                }
            })
            .collect::<Vec<_>>();
        Ok(edit_family(
            &self.base,
            &edits,
            &FamilyEditContext::default(),
        )?)
    }
    fn orthogonal_underlay(
        &self,
        _: ColorComponent,
        _: &AttributeValue,
    ) -> Result<f32, TransitionError> {
        panic!("Position")
    }
}
#[derive(Default)]
struct Observer {
    sources: Vec<(Uuid, ProgrammingComponent, CoupledLeafRole)>,
    transitions: usize,
}
impl CoupledEvaluationObserver for Observer {
    fn evaluated(
        &mut self,
        _: usize,
        step: CoupledEvaluationStep<'_>,
        _: &AttributeValue,
    ) -> Result<(), TransitionError> {
        let sources = match step {
            CoupledEvaluationStep::Leaf { sources, .. } => sources,
            CoupledEvaluationStep::Cohort { components } => components,
            CoupledEvaluationStep::Transition { .. } => {
                self.transitions += 1;
                return Ok(());
            }
            _ => return Ok(()),
        };
        self.sources.extend(sources.iter().map(|source| {
            (
                source.lane_id,
                source.address.address().component.unwrap(),
                source.role,
            )
        }));
        Ok(())
    }
}

#[test]
fn observed_evaluation_distinguishes_authored_axes_current_dependency_and_target_offsets() {
    let bundle = bundle_position_component_forest(
        &crossing(0.5, false),
        &Sources {
            tilt: Some(30.0),
            ..Default::default()
        },
    )
    .unwrap();
    let Some(FamilyCompositionSample::CoupledExpression { expression, .. }) = bundle.position
    else {
        panic!()
    };
    let mut observer = Observer::default();
    let value = expression
        .evaluate_base_observed(
            &EndpointContext {
                base: target(TargetReference::Origin, [1.0, 2.0, 3.0]),
            },
            &ResolvedFrame::default(),
            Some(&mut observer),
        )
        .unwrap()
        .unwrap();
    assert_eq!(value, angles(65.0, 41.5));
    assert_eq!(observer.transitions, 1);
    for expected in [
        (10, ProgrammingComponent::Pan, CoupledLeafRole::Authored),
        (11, ProgrammingComponent::Tilt, CoupledLeafRole::Current),
        (10, ProgrammingComponent::TargetX, CoupledLeafRole::Authored),
        (11, ProgrammingComponent::TargetY, CoupledLeafRole::Authored),
    ] {
        assert!(
            observer
                .sources
                .contains(&(Uuid::from_u128(expected.0), expected.1, expected.2))
        );
    }
}

#[test]
fn exact_forest_target_endpoint_expands_original_component_lanes() {
    let sources = Sources::default();
    let bundle = bundle_position_component_forest(&crossing(1.0, false), &sources).unwrap();
    let position = bundle.position.unwrap();
    let FamilyCompositionSample::CoupledExpression { expression, .. } = &position else {
        panic!()
    };
    assert!(
        matches!(expression.footprint(CoupledExpressionRole::Base), CoupledExpressionFootprint::ExactCohort { components } if components.len() == 2)
    );
    let group = DynamicFamilySampleGroup {
        target: FixtureId(Uuid::from_u128(3)),
        owner: ProgrammingOwner::Position,
        samples: vec![position],
    };
    let base = target(TargetReference::Origin, [1.0, 2.0, 9.0]);
    assert_eq!(
        evaluate(&group, &base, &ResolvedFrame::default()),
        target(TargetReference::Origin, [4.0, 5.0, 9.0])
    );
    assert_eq!(sources.reads.get(), 0);
}

#[test]
fn ordinary_and_completed_angle_output_keeps_authored_pan_separate_from_current_tilt() {
    let ordinary = vec![
        sample(10, angle(ProgrammingComponent::Pan, 90.0)),
        sample(11, current(ProgrammingComponent::Tilt)),
    ];
    let completed = crossing(1.0, true);
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for inputs in [ordinary, completed] {
        let prepared = prepare_dynamic_family_samples(
            &inputs,
            &Sources {
                tilt: Some(30.0),
                ..Default::default()
            },
            None,
            &mut scratch,
        )
        .unwrap();
        let group = &prepared.families[0];
        let mut composition = RetainedFamilyCompositionScratch::default();
        assert_eq!(
            compose_retained_dynamic_family_traced(
                group.owner,
                &angles(10.0, 30.0),
                &group.samples,
                &FamilyCompositionContext::default(),
                &ResolvedFrame::default(),
                &mut composition
            )
            .unwrap(),
            angles(90.0, 30.0)
        );
        let trace = composition.family_trace();
        let root = trace.root().unwrap();
        let pan = trace
            .sources_for_component(root, ProgrammingComponent::Pan)
            .unwrap();
        let tilt = trace
            .sources_for_component(root, ProgrammingComponent::Tilt)
            .unwrap();
        assert_eq!(pan.len(), 1);
        assert_eq!(
            pan[0].rank.dynamic_identity().unwrap().lane_id,
            Uuid::from_u128(10)
        );
        assert_eq!(pan[0].role, FamilyTraceRole::Authored);
        assert_eq!(tilt.len(), 1);
        assert_eq!(
            tilt[0].rank.dynamic_identity().unwrap().lane_id,
            Uuid::from_u128(11)
        );
        assert_eq!(tilt[0].role, FamilyTraceRole::CalculationDependency);
        assert!(
            trace
                .sources_for_component(root, ProgrammingComponent::TargetX)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn a_newer_pan_effect_uses_static_tilt_even_when_another_controller_animates_tilt() {
    let mut older = vec![
        sample(20, angle(ProgrammingComponent::Tilt, 60.0)),
        sample(21, current(ProgrammingComponent::Pan)),
    ];
    for sample in &mut older {
        sample.controller_id = Uuid::from_u128(40);
        sample.activated_at_millis = 50;
    }
    older.extend([
        sample(10, angle(ProgrammingComponent::Pan, 90.0)),
        sample(11, current(ProgrammingComponent::Tilt)),
    ]);
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for static_tilt in [30.0, 35.0] {
        older.reverse();
        let prepared = prepare_dynamic_family_samples(
            &older,
            &Sources {
                pan: Some(10.0),
                tilt: Some(static_tilt),
                ..Default::default()
            },
            None,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(
            evaluate(
                &prepared.families[0],
                &angles(10.0, static_tilt),
                &ResolvedFrame::default()
            ),
            angles(90.0, static_tilt)
        );
    }
}

fn fixed_whole(reference: TargetReference, offsets: [f32; 3]) -> E {
    E::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target {
                reference: Some(reference),
            },
            component: None,
        }),
        value: DynamicValue::Family(target(reference, offsets)),
        occurrence: None,
        dependency_occurrence: None,
    }
}

#[test]
fn duplicate_target_offsets_keep_normal_lane_order_through_angle_resume() {
    let base = target(TargetReference::Origin, [1.0, 2.0, 3.0]);
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for reverse in [false, true] {
        let mut inputs = crossing(0.5, reverse);
        let extra = offset(TargetReference::Origin, ProgrammingComponent::TargetX, 7.0);
        let (from, to) = if reverse {
            (Some(extra), None)
        } else {
            (None, Some(extra))
        };
        inputs.push(sample(12, resume(from, to, 0.5)));
        inputs.reverse();
        let prepared = prepare_dynamic_family_samples(
            &inputs,
            &Sources {
                tilt: Some(30.0),
                ..Default::default()
            },
            None,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(
            evaluate(&prepared.families[0], &base, &ResolvedFrame::default()),
            angles(80.0, 41.5)
        );
        let FamilyCompositionSample::CoupledExpression { expression, .. } =
            &prepared.families[0].samples[0]
        else {
            panic!("forest")
        };
        let cohort = expression
            .base_endpoints()
            .iter()
            .find_map(CoupledBaseEndpoint::cohort)
            .unwrap();
        assert_eq!(cohort.len(), 3);
        assert_eq!(
            cohort
                .iter()
                .filter(|source| source.address.address().component
                    == Some(ProgrammingComponent::TargetX))
                .count(),
            2
        );
        validate_dynamic_value_addresses(cohort.iter().map(|source| source.address.address()))
            .unwrap();
    }
}

#[test]
fn whole_target_and_same_reference_offset_follow_their_original_lane_order() {
    let base = target(TargetReference::Origin, [10.0, 20.0, 30.0]);
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for whole_first in [false, true] {
        let complete = fixed_whole(TargetReference::Origin, [1.0, 2.0, 3.0]);
        let x = offset(TargetReference::Origin, ProgrammingComponent::TargetX, 4.0);
        let [first, second] = if whole_first {
            [complete, x]
        } else {
            [x, complete]
        };
        let inputs = [
            sample(
                10,
                resume(
                    Some(first),
                    Some(angle(ProgrammingComponent::Pan, 90.0)),
                    0.5,
                ),
            ),
            sample(
                11,
                resume(Some(second), Some(current(ProgrammingComponent::Tilt)), 0.5),
            ),
        ];
        let prepared = prepare_dynamic_family_samples(
            &inputs,
            &Sources {
                tilt: Some(30.0),
                ..Default::default()
            },
            None,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(
            evaluate(&prepared.families[0], &base, &ResolvedFrame::default()),
            angles(if whole_first { 65.0 } else { 50.0 }, 26.5)
        );
        let FamilyCompositionSample::CoupledExpression { expression, .. } =
            &prepared.families[0].samples[0]
        else {
            panic!("forest")
        };
        let cohort = expression
            .base_endpoints()
            .iter()
            .find_map(CoupledBaseEndpoint::cohort)
            .unwrap();
        validate_dynamic_value_addresses(cohort.iter().map(|source| source.address.address()))
            .unwrap();
    }
}

#[test]
fn deferred_whole_target_and_offset_use_the_same_cohort_without_baking_its_value() {
    let reference = TargetReference::Origin;
    let baseline = target(reference, [1.0, 2.0, 3.0]);
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let definitions = [
        (
            E::Transition {
                from: Some(Arc::new(fixed_whole(reference, [1.0, 2.0, 3.0]))),
                to: Some(Arc::new(fixed_whole(reference, [3.0, 4.0, 5.0]))),
                progress: 0.5,
                reason: DynamicTransitionReason::Required {
                    requirement: TransitionRequirement::MaterializedEndpoints,
                },
            },
            32.0,
        ),
        (
            E::Scale {
                address: Arc::new(DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Target {
                        reference: Some(reference),
                    },
                    component: None,
                }),
                base: DynamicValue::Family(baseline.clone()),
                value: Arc::new(fixed_whole(reference, [3.0, 4.0, 5.0])),
                factor: 2.0,
                baseline_occurrence: None,
            },
            48.5,
        ),
    ];
    for (whole, expected_tilt) in definitions {
        let inputs = [
            sample(
                10,
                resume(
                    Some(whole),
                    Some(angle(ProgrammingComponent::Pan, 90.0)),
                    0.5,
                ),
            ),
            sample(
                11,
                resume(
                    Some(offset(reference, ProgrammingComponent::TargetX, 4.0)),
                    Some(current(ProgrammingComponent::Tilt)),
                    0.5,
                ),
            ),
        ];
        let prepared = prepare_dynamic_family_samples(
            &inputs,
            &Sources {
                tilt: Some(30.0),
                ..Default::default()
            },
            None,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(
            evaluate(&prepared.families[0], &baseline, &ResolvedFrame::default()),
            angles(65.0, expected_tilt)
        );
        let FamilyCompositionSample::CoupledExpression { expression, .. } =
            &prepared.families[0].samples[0]
        else {
            panic!("forest")
        };
        let cohort = expression
            .base_endpoints()
            .iter()
            .find_map(CoupledBaseEndpoint::cohort_sources)
            .unwrap();
        assert!(
            matches!(&cohort[0], CoupledCohortEndpoint::WholeExpression { lane_id, .. } if *lane_id == Uuid::from_u128(10))
        );
    }
}

#[test]
fn deferred_whole_null_endpoint_sees_earlier_cohort_sibling_before_later_offset() {
    let reference = TargetReference::Origin;
    let whole = E::Transition {
        from: None,
        to: Some(Arc::new(fixed_whole(reference, [3.0, 4.0, 5.0]))),
        progress: 0.5,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::MaterializedEndpoints,
        },
    };
    let inputs = [
        sample(
            9,
            resume(
                Some(offset(reference, ProgrammingComponent::TargetY, 8.0)),
                None,
                0.5,
            ),
        ),
        sample(
            10,
            resume(
                Some(whole),
                Some(angle(ProgrammingComponent::Pan, 90.0)),
                0.5,
            ),
        ),
        sample(
            11,
            resume(
                Some(offset(reference, ProgrammingComponent::TargetX, 4.0)),
                Some(current(ProgrammingComponent::Tilt)),
                0.5,
            ),
        ),
    ];
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let prepared = prepare_dynamic_family_samples(
        &inputs,
        &Sources {
            tilt: Some(30.0),
            ..Default::default()
        },
        None,
        &mut scratch,
    )
    .unwrap();
    assert_eq!(
        evaluate(
            &prepared.families[0],
            &target(reference, [1.0, 2.0, 3.0]),
            &ResolvedFrame::default()
        ),
        angles(65.0, 47.0)
    );
}
