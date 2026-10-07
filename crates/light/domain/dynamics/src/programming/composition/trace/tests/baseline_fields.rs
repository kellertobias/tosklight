use super::*;
use std::cell::Cell;

type F = ProgrammingTraceField;
type S = ProgrammingFieldScope;

fn compose(samples: &[FamilyCompositionSample]) -> RetainedFamilyCompositionScratch {
    let mut scratch = RetainedFamilyCompositionScratch::default();
    compose_retained_dynamic_family_traced(
        ProgrammingOwner::Color,
        &semantic(),
        samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            ..Default::default()
        },
        &NoFrame,
        &mut scratch,
    )
    .unwrap();
    scratch
}

fn query(scratch: &RetainedFamilyCompositionScratch, fields: &[F]) -> Option<FamilyTraceQuery> {
    let trace = scratch.family_trace();
    trace.query_fields_with_base(trace.root().unwrap(), &S::new(fields.iter().copied()))
}

fn amber_semantic() -> AttributeValue {
    let mut intent = ColorIntent::default();
    VirtualColorAuthoringV1
        .set_base_component(&mut intent, ColorComponent::Amber, 0.7)
        .unwrap();
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }))
}

#[test]
fn untouched_base_has_fields_without_an_invented_dynamic_source() {
    let scratch = compose(&[]);
    let fields = [F::ColorRecipeRed, F::ColorXyz, F::ColorWheel(3)];
    assert_eq!(
        query(&scratch, &fields),
        Some(FamilyTraceQuery {
            sources: vec![],
            base_fields: S::new(fields),
            base_dependency_fields: S::empty(),
        })
    );
    assert_eq!(query(&scratch, &[]), Some(FamilyTraceQuery::default()));
    let trace = scratch.family_trace();
    assert_eq!(
        trace.query_fields(trace.root().unwrap(), &S::new(fields)),
        Some(vec![]),
        "source-only callers retain their existing contract"
    );
}

#[test]
fn component_activation_retains_base_until_complete_and_leaves_other_fields_untouched() {
    for mix in [0.0, 0.5, 1.0] {
        let sample = color_sample(Some(ColorComponent::Red), DynamicValue::Scalar(0.8), 1, mix);
        let scratch = compose(&[sample.clone().into()]);
        let red = query(&scratch, &[F::ColorRecipeRed]).unwrap();
        assert_eq!(
            red.base_fields,
            if mix < 1.0 {
                S::new([F::ColorRecipeRed])
            } else {
                S::empty()
            }
        );
        assert!(red.base_dependency_fields.is_empty());
        assert_eq!(red.sources.len(), usize::from(mix > 0.0));
        if mix > 0.0 {
            assert_eq!(red.sources[0].source.rank, sample.rank);
        }
        let green = query(&scratch, &[F::ColorRecipeGreen]).unwrap();
        assert!(green.sources.is_empty());
        assert_eq!(green.base_fields, S::new([F::ColorRecipeGreen]));
    }
}

#[test]
fn complete_whole_replacement_cuts_base_even_for_equal_values() {
    let sample = color_sample(None, DynamicValue::Family(semantic()), 1, 1.0);
    let scratch = compose(&[sample.clone().into()]);
    let result = query(&scratch, &[F::ColorRecipeRed, F::Uv, F::ColorWheel(2)]).unwrap();
    assert!(result.base_fields.is_empty());
    assert!(result.base_dependency_fields.is_empty());
    assert_eq!(result.sources.len(), 1);
    assert_eq!(result.sources[0].source.rank, sample.rank);
    assert_eq!(
        result.sources[0].source.footprint,
        FamilyTraceFootprint::Whole
    );
}

#[test]
fn whole_activation_reverse_maps_baseline_and_proves_generated_amber_empty() {
    let sample = color_sample(None, DynamicValue::Family(amber_semantic()), 1, 0.5);
    let scratch = compose(&[sample.into()]);
    let recipe = query(&scratch, &[F::ColorRecipeRed, F::ColorRecipeGreen]).unwrap();
    assert_eq!(recipe.base_fields, S::new([F::ColorXyz]));
    assert!(recipe.base_dependency_fields.is_empty());
    assert_eq!(recipe.sources.len(), 1);
    assert_eq!(recipe.sources[0].fields, S::new([F::ColorXyz]));
    assert_eq!(
        query(&scratch, &[F::ColorRecipeAmber]),
        Some(FamilyTraceQuery::default())
    );
    let wheel = query(&scratch, &[F::ColorWheel(2)]).unwrap();
    assert_eq!(wheel.base_fields, S::new([F::ColorWheel(2)]));
    assert!(
        wheel.sources.is_empty(),
        "interior held wheel uses only the underlay"
    );
}

#[test]
fn recipe_and_hsv_writes_keep_original_baseline_input_fields_and_relationships() {
    let red = color_sample(Some(ColorComponent::Red), DynamicValue::Scalar(0.7), 1, 1.0);
    let result = query(&compose(&[red.into()]), &[F::ColorXyz]).unwrap();
    assert_eq!(
        result.base_fields,
        S::new([F::ColorRecipeGreen, F::ColorRecipeBlue, F::ColorRecipeAmber])
    );
    assert!(result.base_dependency_fields.is_empty());
    assert_eq!(result.sources.len(), 1);
    assert_eq!(result.sources[0].fields, S::new([F::ColorXyz]));

    let rgb = S::new([F::ColorRecipeRed, F::ColorRecipeGreen, F::ColorRecipeBlue]);
    for component in [ColorComponent::Hue, ColorComponent::Saturation] {
        let sample = color_sample(Some(component), DynamicValue::Scalar(0.5), 1, 1.0);
        let result = query(&compose(&[sample.into()]), &[F::ColorXyz]).unwrap();
        assert_eq!(result.base_fields, S::new([F::ColorRecipeAmber]));
        assert_eq!(result.base_dependency_fields, rgb);
        assert_eq!(result.sources.len(), 1);
        assert_eq!(result.sources[0].source.role, FamilyTraceRole::Authored);
    }

    let partial = color_sample(Some(ColorComponent::Hue), DynamicValue::Scalar(0.5), 1, 0.5);
    let result = query(
        &compose(&[partial.into()]),
        &rgb.fields().collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(result.base_fields, rgb);
    assert_eq!(
        result.base_dependency_fields, rgb,
        "distinct paths retain both roles"
    );
}

#[test]
fn retained_release_reaches_original_base_through_the_same_field_transfers() {
    let owner = ProgrammingOwner::Color;
    let occurrence = DynamicSourceOccurrenceId::new(Uuid::from_u128(991)).unwrap();
    for progress in [0.0, 0.5, 1.0] {
        let expression = Arc::new(DynamicSampleExpression::Transition {
            from: Some(Arc::new(DynamicSampleExpression::Programming {
                address: Arc::new(
                    DynamicValueAddress::whole_family(owner, &amber_semantic()).unwrap(),
                ),
                value: DynamicValue::Family(amber_semantic()),
                occurrence: Some(occurrence),
                dependency_occurrence: None,
            })),
            to: None,
            progress,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(992),
            },
        });
        let sample = FamilyCompositionSample::WholeExpression {
            expression: Arc::new(
                CompiledProgrammingFamilyExpression::new(expression, owner, None, None).unwrap(),
            ),
            rank: color_sample(None, DynamicValue::Family(semantic()), 1, 1.0).rank,
            activation_mix: 1.0,
        };
        let scratch = compose(&[sample]);
        let result = query(&scratch, &[F::ColorRecipeRed]).unwrap();
        assert_eq!(
            result.base_fields,
            match progress {
                0.0 => S::empty(),
                1.0 => S::new([F::ColorRecipeRed]),
                _ => S::new([F::ColorXyz]),
            }
        );
        assert!(result.base_dependency_fields.is_empty());
        assert_eq!(result.sources.len(), usize::from(progress < 1.0));
        if progress < 1.0 {
            assert_eq!(result.sources[0].source.occurrence, Some(occurrence));
        }
    }
}

#[test]
fn adopted_component_keeps_unknown_base_distinct_from_known_complete_replacement() {
    let base = AttributeValue::Position(Arc::new(PositionIntent::angles(15.0, 30.0)));
    let calls = Cell::new(0);
    let adopt = |_: &AttributeValue, _: &DynamicValueAddress| {
        calls.set(calls.get() + 1);
        Ok(AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [10.0, 20.0, 30.0],
        ))))
    };
    let sample = FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Target {
                        reference: Some(TargetReference::Origin),
                    },
                    component: Some(ProgrammingComponent::TargetX),
                },
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Scalar(42.0),
        color_sample(None, DynamicValue::Family(semantic()), 1, 1.0).rank,
        1.0,
    )
    .unwrap();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let value = compose_retained_dynamic_family_traced(
        ProgrammingOwner::Position,
        &base,
        &[sample.into()],
        &FamilyCompositionContext {
            resolve_adoption: Some(&adopt),
            ..Default::default()
        },
        &NoFrame,
        &mut scratch,
    )
    .unwrap();
    assert_eq!(
        value,
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [42.0, 20.0, 30.0]
        )))
    );
    assert_eq!(query(&scratch, &[F::TargetY]), None);
    assert_eq!(query(&scratch, &[F::TargetX, F::TargetY]), None);
    let replacement = query(&scratch, &[F::TargetX]).unwrap();
    assert!(replacement.base_fields.is_empty());
    assert!(replacement.base_dependency_fields.is_empty());
    assert_eq!(replacement.sources.len(), 1);
    assert_eq!(query(&scratch, &[]), Some(FamilyTraceQuery::default()));
    assert_eq!(calls.get(), 1, "queries do not repeat adoption");
}
