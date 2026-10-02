use super::*;
use crate::{CompiledComponentExpression, DynamicSampleExpression, DynamicTransitionReason};

fn leaf(template: &FamilySample, value: DynamicValue) -> DynamicSampleExpression {
    DynamicSampleExpression::Programming {
        address: Arc::new(template.address().address().clone()),
        value,
        occurrence: None,
        dependency_occurrence: None,
    }
}

fn resume(
    from: Option<DynamicSampleExpression>,
    to: Option<DynamicSampleExpression>,
    progress: f32,
    occurrence: u128,
) -> DynamicSampleExpression {
    DynamicSampleExpression::Transition {
        from: from.map(Arc::new),
        to: to.map(Arc::new),
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(occurrence),
        },
    }
}

fn retained(
    template: FamilySample,
    expression: DynamicSampleExpression,
    activation_mix: f32,
) -> FamilySample {
    let compiled = CompiledComponentExpression::new(
        Arc::new(expression),
        Arc::new(template.address().clone()),
    )
    .unwrap();
    FamilySample::retained_component(Arc::new(compiled), template.rank, activation_mix).unwrap()
}

fn release(template: FamilySample, value: DynamicValue, mix: f32) -> FamilySample {
    let expression = resume(Some(leaf(&template, value)), None, 0.5, 100);
    retained(template, expression, mix)
}

#[test]
fn retained_uv_uses_lower_dynamic_or_component_fix_at_then_activates_once() {
    let original = ColorIntent {
        white_blend: 0.125,
        uv: UvIntent { amount: 0.125 },
        ..Default::default()
    };
    let base = semantic(original.clone());
    for fixed in [false, true] {
        let lower = color(ColorComponent::Uv, 0.25, 1);
        let lower = if fixed { lower.into_fix_at() } else { lower };
        let samples = [
            lower,
            color(ColorComponent::WhiteBlend, 0.625, 2),
            release(
                color(ColorComponent::Uv, 0.0, 3),
                DynamicValue::Scalar(0.75),
                0.5,
            ),
        ];
        let result = compose(ProgrammingOwner::Color, &base, &samples).unwrap();
        let mut expected = original.clone();
        // Resume: 0.75 -> eligible 0.25 at 50% = 0.5; activation over 0.25 = 0.375.
        expected.uv.amount = 0.375;
        expected.white_blend = 0.625;
        assert_eq!(result, semantic(expected));
    }
}

#[test]
fn retained_target_offset_uses_current_component_underlay_without_changing_other_offsets() {
    let reference = TargetReference::Point {
        point_id: Uuid::from_u128(55),
    };
    let target =
        |offsets| AttributeValue::Position(Arc::new(PositionIntent::target(reference, offsets)));
    let offset = |x, order| {
        sample(
            DynamicFamilyRepresentation::Target {
                reference: Some(reference),
            },
            Some(ProgrammingComponent::TargetX),
            DynamicValue::Scalar(x),
            order,
        )
    };
    for fixed in [false, true] {
        let lower = offset(10.0, 1);
        let lower = if fixed { lower.into_fix_at() } else { lower };
        let samples = [
            lower,
            release(offset(0.0, 2), DynamicValue::Scalar(20.0), 0.5),
        ];
        assert_eq!(
            compose(
                ProgrammingOwner::Position,
                &target([1.0, 2.0, 3.0]),
                &samples
            )
            .unwrap(),
            target([12.5, 2.0, 3.0])
        );
    }
}

#[test]
fn exact_absent_endpoint_does_not_select_recipe_over_active_hue_saturation() {
    let base = semantic(ColorIntent::default());
    let active = [
        color(ColorComponent::Hue, 180.0, 1),
        color(ColorComponent::Saturation, 1.0, 2),
        color(ColorComponent::WhiteBlend, 0.25, 3),
    ];
    let expected = compose(ProgrammingOwner::Color, &base, &active).unwrap();
    for at_release_end in [false, true] {
        let template = color(ColorComponent::Red, 0.0, 90);
        let value = leaf(&template, DynamicValue::Scalar(0.0));
        let expression = if at_release_end {
            resume(Some(value), None, 1.0, 100)
        } else {
            resume(None, Some(value), 0.0, 100)
        };
        let mut samples = active.to_vec();
        samples.push(retained(template, expression, 1.0));
        assert_eq!(
            compose(ProgrammingOwner::Color, &base, &samples).unwrap(),
            expected,
            "an absent endpoint must not compete as a Recipe base writer"
        );
    }
}

#[test]
fn whole_fix_at_covers_retained_component_before_requesting_target_geometry() {
    let base = angles(0.0, 0.0);
    let retained_target = release(
        sample(
            DynamicFamilyRepresentation::Target {
                reference: Some(TargetReference::Point {
                    point_id: Uuid::from_u128(55),
                }),
            },
            Some(ProgrammingComponent::TargetX),
            DynamicValue::Scalar(0.0),
            1,
        ),
        DynamicValue::Scalar(90.0),
        1.0,
    );
    assert_eq!(
        compose(
            ProgrammingOwner::Position,
            &base,
            &[retained_target.clone()]
        )
        .unwrap_err(),
        TransitionError::Requires(TransitionRequirement::LiveTargetPoints)
    );
    let fixed = angles(20.0, 30.0);
    let mask = sample(
        DynamicFamilyRepresentation::Angles,
        None,
        DynamicValue::Family(fixed.clone()),
        2,
    )
    .into_fix_at();
    assert_eq!(
        compose(ProgrammingOwner::Position, &base, &[retained_target, mask]).unwrap(),
        fixed
    );
}

#[test]
fn higher_retained_component_resumes_against_whole_fix_at_underlay() {
    let fixed = ColorIntent {
        uv: UvIntent { amount: 0.25 },
        white_blend: 0.75,
        ..Default::default()
    };
    let mask = sample(
        DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        None,
        DynamicValue::Family(semantic(fixed.clone())),
        2,
    )
    .into_fix_at();
    let samples = [
        color(ColorComponent::Uv, 1.0, 1),
        mask,
        release(
            color(ColorComponent::Uv, 0.0, 3),
            DynamicValue::Scalar(0.75),
            0.5,
        ),
    ];
    let mut expected = fixed;
    expected.uv.amount = 0.375;
    assert_eq!(
        compose(
            ProgrammingOwner::Color,
            &semantic(ColorIntent::default()),
            &samples,
        )
        .unwrap(),
        semantic(expected)
    );
}

#[test]
fn retained_explicit_orthogonal_keeps_its_mask_over_a_newer_implicit_whole_color() {
    let whole = ColorIntent {
        uv: UvIntent { amount: 0.25 },
        white_blend: 0.75,
        ..Default::default()
    };
    let samples = [
        release(
            color(ColorComponent::Uv, 0.0, 1),
            DynamicValue::Scalar(0.75),
            0.5,
        ),
        sample(
            DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Whole,
            },
            None,
            DynamicValue::Family(semantic(whole.clone())),
            90,
        ),
    ];
    let mut expected = whole;
    expected.uv.amount = 0.375;
    assert_eq!(
        compose(
            ProgrammingOwner::Color,
            &semantic(ColorIntent::default()),
            &samples,
        )
        .unwrap(),
        semantic(expected)
    );
}

#[test]
fn retained_direct_uses_all_32_bits_and_predicts_only_the_final_complete_recipe() {
    let model = native_model();
    let base = semantic_to_direct(&model, [u32::MAX - 9, 17]);
    let samples = [
        native_sample(&model, 0, u32::MAX - 5, 1),
        release(
            native_sample(&model, 0, 0, 2),
            DynamicValue::Native(u32::MAX - 1),
            0.5,
        ),
        native_sample(&model, 1, 37, 3),
    ];
    assert_eq!(model.predictions.load(Ordering::Relaxed), 0);
    let result = compose_dynamic_family(
        ProgrammingOwner::Color,
        &base,
        &samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                native_model: Some(model.as_ref()),
                ..Default::default()
            },
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(result, semantic_to_direct(&model, [u32::MAX - 4, 37]));
    assert_eq!(model.predictions.load(Ordering::Relaxed), 1);
}

#[test]
fn interrupted_same_component_resume_uses_one_eligible_underlay_for_every_missing_endpoint() {
    let template = color(ColorComponent::Uv, 0.0, 2);
    let inner = resume(
        Some(leaf(&template, DynamicValue::Scalar(0.75))),
        None,
        0.5,
        100,
    );
    let outer = resume(Some(inner), None, 0.5, 200);
    let samples = [
        color(ColorComponent::Uv, 0.25, 1),
        retained(template, outer, 0.5),
    ];
    let original = ColorIntent {
        uv: UvIntent { amount: 0.125 },
        white_blend: 0.75,
        ..Default::default()
    };
    let result = compose(
        ProgrammingOwner::Color,
        &semantic(original.clone()),
        &samples,
    )
    .unwrap();
    let mut expected = original;
    // Inner = 0.5, outer = 0.375, activation over the same 0.25 underlay = 0.3125.
    expected.uv.amount = 0.3125;
    assert_eq!(result, semantic(expected));
}

#[test]
fn opaque_higher_component_skips_the_covered_retained_underlay_read() {
    struct ObservedModel(std::cell::Cell<usize>);
    impl ColorAuthoringModel for ObservedModel {
        fn read_base_component(
            &self,
            intent: &ColorIntent,
            component: ColorComponent,
        ) -> Result<f32, IntentError> {
            if component == ColorComponent::Hue {
                self.0.set(self.0.get() + 1);
            }
            VirtualColorAuthoringV1.read_base_component(intent, component)
        }
        fn set_base_component(
            &self,
            intent: &mut ColorIntent,
            component: ColorComponent,
            value: f32,
        ) -> Result<(), IntentError> {
            VirtualColorAuthoringV1.set_base_component(intent, component, value)
        }
        fn set_coordinates(
            &self,
            intent: &mut ColorIntent,
            xyz: light_core::Xyz,
        ) -> Result<(), IntentError> {
            VirtualColorAuthoringV1.set_coordinates(intent, xyz)
        }
    }
    let model = ObservedModel(std::cell::Cell::new(0));
    let samples = [
        release(
            color(ColorComponent::Hue, 0.0, 1),
            DynamicValue::Scalar(120.0),
            1.0,
        ),
        color(ColorComponent::Hue, 240.0, 2),
        color(ColorComponent::Saturation, 1.0, 3),
    ];
    let result = compose_dynamic_family(
        ProgrammingOwner::Color,
        &semantic(ColorIntent::default()),
        &samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&model),
                ..Default::default()
            },
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(
        model.0.get(),
        1,
        "only the final complete edit reads Hue; the covered retained endpoint must not"
    );
    let hue = VirtualColorAuthoringV1
        .read_base_component(intent(&result), ColorComponent::Hue)
        .unwrap();
    assert!((hue - 240.0).abs() < 0.01);
}

#[test]
fn whole_mask_interpolation_prevents_pruning_an_earlier_same_component() {
    let mut red = ColorIntent::default();
    VirtualColorAuthoringV1
        .set_base_component(&mut red, ColorComponent::Green, 0.0)
        .unwrap();
    VirtualColorAuthoringV1
        .set_base_component(&mut red, ColorComponent::Blue, 0.0)
        .unwrap();
    let template = color(ColorComponent::Hue, 0.0, 1);
    let cyan = leaf(&template, DynamicValue::Scalar(180.0));
    let lower = retained(template, cyan, 1.0);
    let mut mask = sample(
        DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        None,
        DynamicValue::Family(semantic(red.clone())),
        2,
    )
    .into_fix_at();
    mask.activation_mix = 0.5;
    let samples = [lower, mask, color(ColorComponent::Hue, 0.0, 3)];
    let result = compose(ProgrammingOwner::Color, &semantic(red.clone()), &samples).unwrap();
    let saturation = VirtualColorAuthoringV1
        .read_base_component(intent(&result), ColorComponent::Saturation)
        .unwrap();
    assert!(
        saturation < 0.001,
        "the lower cyan mixed with the red mask must remain achromatic"
    );
    let without_lower = compose(ProgrammingOwner::Color, &semantic(red), &samples[1..]).unwrap();
    let saturation_without_lower = VirtualColorAuthoringV1
        .read_base_component(intent(&without_lower), ColorComponent::Saturation)
        .unwrap();
    assert!(saturation_without_lower > 0.999);
}
