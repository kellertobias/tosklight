use super::*;
use crate::{
    CompiledCoupledExpression, DynamicPresetSourceBinding, DynamicSampleExpression as E,
    DynamicSourceDependency, DynamicSourceOccurrenceId, DynamicTransitionReason,
    DynamicValueSourceResolver, FamilyExpressionOperation, WholeFamilyExpressionFrameResolver,
};
use light_core::FixtureId;
use std::cell::Cell;

struct Current {
    value: AttributeValue,
    calls: Cell<usize>,
    error: Option<TransitionError>,
}
impl Current {
    fn new(value: AttributeValue) -> Self {
        Self {
            value,
            calls: Cell::new(0),
            error: None,
        }
    }
}
fn occurrence(value: u128) -> DynamicSourceOccurrenceId {
    DynamicSourceOccurrenceId::new(Uuid::from_u128(value)).unwrap()
}
impl DynamicValueSourceResolver for Current {
    fn current(&self, _: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        extract_compatible_dynamic_value(
            &self.value,
            address,
            &FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn try_current(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<DynamicValue>, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(self.current(target, address))
    }
    fn try_current_family_base(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(Some(self.value.clone()))
    }
    fn current_family_occurrence(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Option<DynamicSourceOccurrenceId> {
        Some(occurrence(700))
    }
    fn current_dependency(
        &self,
        _: FixtureId,
        address: &DynamicValueAddress,
    ) -> DynamicSourceDependency {
        DynamicSourceDependency::compatible(Some(occurrence(700)), address)
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}
struct NoFrame;
impl WholeFamilyExpressionFrameResolver for NoFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        Err(TransitionError::Requires(requirement))
    }
}
fn focus(value: f32, order: u128, activation: f32) -> FamilySample {
    let mut result = sample(
        DynamicFamilyRepresentation::Focus,
        None,
        DynamicValue::Family(AttributeValue::Normalized(value)),
        order,
    );
    result.activation_mix = activation;
    result
}
fn run(
    owner: ProgrammingOwner,
    current: &Current,
    samples: &[FamilyCompositionSample],
    control: &dyn Fn(FamilySampleRank) -> FamilyEndpointOutputControl,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<AttributeValue, TransitionError> {
    compose_retained_dynamic_family_traced(
        owner,
        &current.value,
        samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            endpoint_output: Some(FamilyEndpointOutputContext {
                control,
                target: FixtureId::new(),
                current,
                native_models: None,
            }),
            ..Default::default()
        },
        &NoFrame,
        scratch,
    )
}
fn at(order: u128, mix: f32) -> impl Fn(FamilySampleRank) -> FamilyEndpointOutputControl {
    move |rank| {
        if rank.stable_order == order {
            FamilyEndpointOutputControl::CrossfadeCurrent { mix }
        } else {
            FamilyEndpointOutputControl::Unchanged
        }
    }
}
fn near(value: f32, expected: f32) {
    assert!((value - expected).abs() < 1e-6, "{value} != {expected}");
}

#[test]
fn held_endpoint_master_reads_static_current_before_activation_over_lower_dynamic() {
    let current = Current::new(AttributeValue::Normalized(0.2));
    let samples = [focus(1.0, 1, 1.0).into(), focus(0.8, 2, 0.5).into()];
    let mut scratch = RetainedFamilyCompositionScratch::default();
    for (mix, expected) in [(0.5, 0.75), (0.0, 0.6), (1.0, 0.9), (0.5, 0.75)] {
        let AttributeValue::Normalized(value) = run(
            ProgrammingOwner::Focus,
            &current,
            &samples,
            &at(2, mix),
            &mut scratch,
        )
        .unwrap() else {
            panic!()
        };
        near(value, expected);
    }
    assert_eq!(
        current.calls.get(),
        3,
        "full endpoint must not read Current"
    );
    // Reusing paused/captured samples leaves their endpoint and activation unchanged.
    let FamilyCompositionSample::Known(sample) = &samples[1] else {
        panic!()
    };
    assert_eq!(
        sample.materialized_value(),
        Some(&DynamicValue::Family(AttributeValue::Normalized(0.8)))
    );
    assert_eq!(sample.activation_mix, 0.5);
}

#[test]
fn zero_master_appearance_is_current_dependency_but_controller_keeps_coverage() {
    let current = Current::new(AttributeValue::Normalized(0.2));
    let endpoint = focus(0.8, 2, 1.0).with_trace_sources(
        vec![FamilyTraceSource {
            rank: focus(0.8, 2, 1.0).rank,
            footprint: FamilyTraceFootprint::Whole,
            role: FamilyTraceRole::Authored,
            occurrence: Some(occurrence(701)),
        }]
        .into(),
    );
    let rank = endpoint.rank;
    let samples = [focus(1.0, 1, 1.0).into(), endpoint.into()];
    let mut scratch = RetainedFamilyCompositionScratch::default();
    for mix in [0.0, 0.5, 1.0] {
        run(
            ProgrammingOwner::Focus,
            &current,
            &samples,
            &at(2, mix),
            &mut scratch,
        )
        .unwrap();
        let trace = scratch.family_trace();
        let root = trace.root().unwrap();
        let fields = ProgrammingFieldScope::new([ProgrammingTraceField::Focus]);
        let sources = trace.query_fields(root, &fields).unwrap();
        assert_eq!(
            sources
                .iter()
                .any(|s| s.source.occurrence == Some(occurrence(700))),
            mix < 1.0
        );
        assert_eq!(
            sources
                .iter()
                .any(|s| s.source.occurrence == Some(occurrence(701))),
            mix > 0.0
        );
        assert!(
            sources
                .iter()
                .filter(|s| s.source.occurrence == Some(occurrence(700)))
                .all(|s| s.source.role == FamilyTraceRole::CalculationDependency)
        );
        assert!(
            trace
                .control_sources_for_fields(root, &fields)
                .unwrap()
                .iter()
                .any(|entry| entry.rank == rank)
        );
    }
}

#[test]
fn suppression_and_fixed_bypass_validate_original_samples_and_control() {
    let current = Current::new(AttributeValue::Normalized(0.2));
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let samples = [focus(1.0, 1, 1.0).into(), focus(0.8, 2, 1.0).into()];
    assert_eq!(
        run(
            ProgrammingOwner::Focus,
            &current,
            &samples,
            &|rank| if rank.stable_order == 2 {
                FamilyEndpointOutputControl::Suppressed
            } else {
                FamilyEndpointOutputControl::Unchanged
            },
            &mut scratch
        )
        .unwrap(),
        AttributeValue::Normalized(1.0)
    );
    assert_eq!(current.calls.get(), 0);
    let mut fixed = focus(0.4, 3, 1.0).into_fix_at();
    fixed.rank.identity = FamilySampleIdentity::Fixed {
        source: FamilyFixedSampleSource::Cue,
        row_index: 0,
    };
    assert_eq!(
        run(
            ProgrammingOwner::Focus,
            &current,
            &[fixed.into()],
            &|_| FamilyEndpointOutputControl::CrossfadeCurrent { mix: f32::NAN },
            &mut scratch
        )
        .unwrap(),
        AttributeValue::Normalized(0.4)
    );
    assert!(matches!(
        run(
            ProgrammingOwner::Focus,
            &current,
            &samples,
            &at(2, f32::NAN),
            &mut scratch
        ),
        Err(TransitionError::Invalid(_))
    ));
    let mut bad = focus(0.8, 2, 1.0);
    bad.body =
        FamilySampleBody::Materialized(DynamicValue::Family(AttributeValue::Normalized(f32::NAN)));
    assert!(matches!(
        run(
            ProgrammingOwner::Focus,
            &current,
            &[bad.into()],
            &|_| FamilyEndpointOutputControl::Suppressed,
            &mut scratch
        ),
        Err(TransitionError::Invalid(_))
    ));
}

#[test]
fn expected_current_requirement_is_preserved_and_full_endpoint_never_reads_it() {
    let mut current = Current::new(AttributeValue::Normalized(0.2));
    current.error = Some(TransitionError::Requires(
        TransitionRequirement::ZoomConvention,
    ));
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let samples = [focus(0.8, 2, 1.0).into()];
    assert!(matches!(
        run(
            ProgrammingOwner::Focus,
            &current,
            &samples,
            &at(2, 0.5),
            &mut scratch
        ),
        Err(TransitionError::Requires(
            TransitionRequirement::ZoomConvention
        ))
    ));
    assert_eq!(
        run(
            ProgrammingOwner::Focus,
            &current,
            &samples,
            &at(2, 1.0),
            &mut scratch
        )
        .unwrap(),
        AttributeValue::Normalized(0.8)
    );
    assert_eq!(current.calls.get(), 1);
}

#[test]
fn angle_pair_is_completed_before_one_whole_endpoint_envelope() {
    let current = Current::new(angles(10.0, 20.0));
    let pan = angle(ProgrammingComponent::Pan, 100.0, 2);
    let mut tilt = angle(ProgrammingComponent::Tilt, 80.0, 2);
    tilt.rank = pan.rank.with_dynamic_lane(Uuid::from_u128(999)).unwrap();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let result = run(
        ProgrammingOwner::Position,
        &current,
        &[pan.into(), tilt.into()],
        &at(2, 0.5),
        &mut scratch,
    )
    .unwrap();
    assert_eq!(result, angles(55.0, 50.0));
    assert_eq!(current.calls.get(), 1);
}

fn uv(value: f32) -> AttributeValue {
    semantic(ColorIntent {
        uv: UvIntent { amount: value },
        ..Default::default()
    })
}
fn leaf(sample: &FamilySample) -> Arc<E> {
    Arc::new(E::Programming {
        address: Arc::new(sample.address.address().clone()),
        value: sample.materialized_value().unwrap().clone(),
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn resume(from: Arc<E>, to: Option<Arc<E>>) -> Arc<E> {
    Arc::new(E::Transition {
        from: Some(from),
        to,
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(777),
        },
    })
}

#[test]
fn retained_orthogonal_endpoint_blends_after_release_evaluation_and_keeps_component_mask() {
    let current = Current::new(uv(0.1));
    let endpoint = color(ColorComponent::Uv, 0.8, 3);
    let expression = Arc::new(
        CompiledComponentExpression::new(resume(leaf(&endpoint), None), endpoint.address.clone())
            .unwrap(),
    );
    let endpoint = FamilySample::retained_component(expression, endpoint.rank, 0.5).unwrap();
    let samples = [
        color(ColorComponent::Uv, 0.6, 1).into(),
        color(ColorComponent::WhiteBlend, 0.7, 2).into(),
        endpoint.into(),
    ];
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let result = run(
        ProgrammingOwner::Color,
        &current,
        &samples,
        &at(3, 0.5),
        &mut scratch,
    )
    .unwrap();
    // Raw release .8 -> .6 = .7; output .1 -> .7 = .4; activation .6 -> .4 = .5.
    near(intent(&result).uv.amount, 0.5);
    near(intent(&result).white_blend, 0.7);
    let controls = scratch
        .family_trace()
        .control_sources_for_fields(
            scratch.family_trace().root().unwrap(),
            &ProgrammingFieldScope::new([ProgrammingTraceField::Uv]),
        )
        .unwrap();
    assert!(
        controls
            .iter()
            .filter(|entry| entry.rank.stable_order == 3)
            .all(|entry| entry.footprint
                == FamilyTraceFootprint::Component(ProgrammingComponent::Color(
                    ColorComponent::Uv
                )))
    );
}

#[test]
fn coupled_whole_to_uv_evaluates_branch_defaults_before_root_envelope() {
    let current = Current::new(uv(0.1));
    let whole = sample(
        DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        None,
        DynamicValue::Family(uv(0.8)),
        3,
    );
    let explicit = color(ColorComponent::Uv, 0.2, 3);
    let expression = Arc::new(
        CompiledCoupledExpression::new(resume(leaf(&whole), Some(leaf(&explicit))), None).unwrap(),
    );
    let samples = [FamilyCompositionSample::CoupledExpression {
        expression,
        rank: whole.rank,
        activation_mix: 1.0,
    }];
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let result = run(
        ProgrammingOwner::Color,
        &current,
        &samples,
        &at(3, 0.5),
        &mut scratch,
    )
    .unwrap();
    // Raw whole .8 -> explicit .2 = .5, then captured .1 -> .5 = .3.
    near(intent(&result).uv.amount, 0.3);
    assert_eq!(
        intent(&result).white_blend,
        intent(&current.value).white_blend
    );
}

#[test]
fn restored_held_whole_tree_responds_to_master_without_changing_history_on_requirement() {
    let source = resume(leaf(&focus(0.4, 2, 1.0)), Some(leaf(&focus(0.8, 2, 1.0))));
    let tape = crate::RetainedExpressionTape::from_roots(&[source]).unwrap();
    let saved = serde_json::to_value(&tape).unwrap();
    let tape: crate::RetainedExpressionTape = serde_json::from_value(saved.clone()).unwrap();
    let tape = Arc::new(tape);
    let held = Arc::new(E::Retained {
        root: tape.roots()[0],
        tape: tape.clone(),
    });
    let samples = [
        focus(1.0, 1, 1.0).into(),
        FamilyCompositionSample::WholeExpression {
            expression: Arc::new(
                crate::CompiledProgrammingFamilyExpression::new(
                    held,
                    ProgrammingOwner::Focus,
                    None,
                    None,
                )
                .unwrap(),
            ),
            rank: focus(0.8, 2, 1.0).rank,
            activation_mix: 0.5,
        },
    ];
    let mut current = Current::new(AttributeValue::Normalized(0.2));
    let mut scratch = RetainedFamilyCompositionScratch::default();
    for (mix, expected) in [(0.5, 0.7), (0.0, 0.6), (1.0, 0.8)] {
        let AttributeValue::Normalized(value) = run(
            ProgrammingOwner::Focus,
            &current,
            &samples,
            &at(2, mix),
            &mut scratch,
        )
        .unwrap() else {
            panic!()
        };
        near(value, expected);
    }
    current.error = Some(TransitionError::Requires(
        TransitionRequirement::MaterializedEndpoints,
    ));
    assert!(matches!(
        run(
            ProgrammingOwner::Focus,
            &current,
            &samples,
            &at(2, 0.5),
            &mut scratch
        ),
        Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints
        ))
    ));
    assert_eq!(serde_json::to_value(tape.as_ref()).unwrap(), saved);
    current.error = None;
    let AttributeValue::Normalized(value) = run(
        ProgrammingOwner::Focus,
        &current,
        &samples,
        &at(2, 0.5),
        &mut scratch,
    )
    .unwrap() else {
        panic!()
    };
    near(value, 0.7);
}

#[test]
fn recipe_component_master_preserves_independent_recipe_component_vote() {
    let mut base = ColorIntent::default();
    VirtualColorAuthoringV1
        .set_base_component(&mut base, ColorComponent::Red, 0.2)
        .unwrap();
    let current = Current::new(semantic(base));
    let mut red = color(ColorComponent::Red, 0.8, 3);
    red.activation_mix = 0.5;
    let samples = [
        color(ColorComponent::Red, 1.0, 1).into(),
        color(ColorComponent::Green, 0.7, 2).into(),
        red.into(),
    ];
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let result = run(
        ProgrammingOwner::Color,
        &current,
        &samples,
        &at(3, 0.5),
        &mut scratch,
    )
    .unwrap();
    for (component, expected) in [(ColorComponent::Red, 0.75), (ColorComponent::Green, 0.7)] {
        let address = color(component, 0.0, 9);
        let DynamicValue::Scalar(value) = extract_compatible_dynamic_value(
            &result,
            address.address.address(),
            &FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap() else {
            panic!()
        };
        near(value, expected);
    }
}

#[test]
fn unchanged_complete_angle_owns_generated_current_axis_independently_from_appearance() {
    let current = Current::new(angles(10.0, 20.0));
    let pan = angle(ProgrammingComponent::Pan, 100.0, 2);
    let mut tilt = angle(ProgrammingComponent::Tilt, 20.0, 2);
    tilt.rank = pan.rank.with_dynamic_lane(Uuid::from_u128(999)).unwrap();
    let tilt_rank = tilt.rank;
    tilt = tilt.with_trace_sources(
        vec![FamilyTraceSource {
            rank: tilt_rank,
            footprint: FamilyTraceFootprint::Component(ProgrammingComponent::Tilt),
            role: FamilyTraceRole::CalculationDependency,
            occurrence: Some(occurrence(700)),
        }]
        .into(),
    );
    let samples = [pan.into(), tilt.into()];
    for no_context in [false, true] {
        let mut scratch = RetainedFamilyCompositionScratch::default();
        if no_context {
            compose_retained_dynamic_family_traced(
                ProgrammingOwner::Position,
                &current.value,
                &samples,
                &FamilyCompositionContext::default(),
                &NoFrame,
                &mut scratch,
            )
            .unwrap();
        } else {
            run(
                ProgrammingOwner::Position,
                &current,
                &samples,
                &|_| FamilyEndpointOutputControl::Unchanged,
                &mut scratch,
            )
            .unwrap();
        }
        let trace = scratch.family_trace();
        let root = trace.root().unwrap();
        let fields = ProgrammingFieldScope::new([ProgrammingTraceField::Tilt]);
        let appearance = trace.query_fields(root, &fields).unwrap();
        assert_eq!(appearance.len(), 1);
        assert_eq!(
            appearance[0].source.role,
            FamilyTraceRole::CalculationDependency
        );
        assert_eq!(appearance[0].source.occurrence, Some(occurrence(700)));
        let controls = trace.control_sources_for_fields(root, &fields).unwrap();
        assert_eq!(controls.len(), 1);
        assert_eq!(controls[0].footprint, FamilyTraceFootprint::Whole);
        assert_eq!(controls[0].rank.stable_order, 2);
    }
    assert_eq!(current.calls.get(), 0);
}

#[test]
fn logical_fix_at_with_legacy_dynamic_rank_bypasses_all_endpoint_controls() {
    let current = Current::new(angles(0.0, 10.0));
    let dynamic = sample(
        DynamicFamilyRepresentation::Angles,
        None,
        DynamicValue::Family(angles(90.0, 45.0)),
        1,
    );
    for component in [None, Some(ProgrammingComponent::Pan)] {
        let mask = crate::ProgrammingFamilyFixAt::from_family(
            ProgrammingOwner::Position,
            component,
            angles(-30.0, -70.0),
        )
        .unwrap();
        let fixed = mask
            .compile(
                None,
                &FamilyEditContext::default(),
                angle(ProgrammingComponent::Pan, 0.0, 2).rank,
                1.0,
            )
            .unwrap();
        assert!(fixed.rank.dynamic_identity().is_some());
        for control in [
            FamilyEndpointOutputControl::Suppressed,
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.5 },
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: f32::NAN },
        ] {
            let fixed_lookups = Cell::new(0);
            let lookup = |rank: FamilySampleRank| {
                if rank.stable_order == 2 {
                    fixed_lookups.set(fixed_lookups.get() + 1);
                    control
                } else {
                    FamilyEndpointOutputControl::Unchanged
                }
            };
            let result = run(
                ProgrammingOwner::Position,
                &current,
                &[dynamic.clone().into(), fixed.clone().into()],
                &lookup,
                &mut RetainedFamilyCompositionScratch::default(),
            )
            .unwrap();
            assert_eq!(
                result,
                if component.is_some() {
                    angles(-30.0, 45.0)
                } else {
                    angles(-30.0, -70.0)
                }
            );
            assert_eq!(
                fixed_lookups.get(),
                0,
                "logical FixAT never requests a Dynamic control"
            );
        }
    }
    assert_eq!(current.calls.get(), 0);
}

mod native;
