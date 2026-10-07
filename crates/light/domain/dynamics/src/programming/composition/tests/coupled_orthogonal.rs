use super::*;
use crate::{
    CompiledComponentExpression, CompiledCoupledExpression, CompiledProgrammingFamilyExpression,
    DynamicSampleExpression as E, DynamicTransitionReason, FamilyCompositionSample,
    FamilyExpressionOperation, RetainedFamilyCompositionScratch,
    WholeFamilyExpressionFrameResolver, compose_retained_dynamic_family,
};
use std::cell::Cell;

#[derive(Default)]
struct UnavailableFrame(Cell<usize>);
impl WholeFamilyExpressionFrameResolver for UnavailableFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.0.set(self.0.get() + 1);
        Err(TransitionError::Requires(requirement))
    }
}

fn uv_family(amount: f32) -> AttributeValue {
    semantic(ColorIntent {
        uv: UvIntent { amount },
        ..Default::default()
    })
}

fn whole_leaf(value: AttributeValue) -> Arc<E> {
    Arc::new(E::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Color, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn uv_leaf(amount: f32) -> Arc<E> {
    let value = color(ColorComponent::Uv, amount, 1);
    Arc::new(E::Programming {
        address: Arc::new(value.address().address().clone()),
        value: DynamicValue::Scalar(amount),
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn resume(from: Option<Arc<E>>, to: Option<Arc<E>>, progress: f32, occurrence: u128) -> Arc<E> {
    Arc::new(E::Transition {
        from,
        to,
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(occurrence),
        },
    })
}

fn coupled(
    expression: Arc<E>,
    rank: FamilySampleRank,
    activation_mix: f32,
) -> FamilyCompositionSample {
    FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(CompiledCoupledExpression::new(expression, None).unwrap()),
        rank,
        activation_mix,
    }
}

fn mixed(activation_mix: f32) -> FamilyCompositionSample {
    coupled(
        resume(
            Some(whole_leaf(uv_family(0.8))),
            Some(uv_leaf(0.2)),
            0.5,
            500,
        ),
        color(ColorComponent::Uv, 0.0, 10).rank,
        activation_mix,
    )
}

fn whole(amount: f32, order: u128, activation_mix: f32) -> FamilyCompositionSample {
    let mut value = sample(
        DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        None,
        DynamicValue::Family(uv_family(amount)),
        order,
    );
    value.activation_mix = activation_mix;
    value.into()
}

fn run(base: &AttributeValue, samples: &[FamilyCompositionSample]) -> AttributeValue {
    let frame = UnavailableFrame::default();
    let value = compose_retained_dynamic_family(
        ProgrammingOwner::Color,
        base,
        samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            ..Default::default()
        },
        &frame,
        &mut RetainedFamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(frame.0.get(), 0);
    value
}

fn assert_uv_only(result: &AttributeValue, expected: f32) {
    let mut actual = intent(result).clone();
    assert!(
        (actual.uv.amount - expected).abs() < 0.000001,
        "expected independent UV {expected}, got {}",
        actual.uv.amount
    );
    actual.uv.amount = 0.0;
    assert_eq!(
        actual,
        ColorIntent::default(),
        "UV must not change visible Color or white intent"
    );
}

#[test]
fn mixed_whole_to_explicit_uv_preserves_branch_defaults_and_applies_activation_once() {
    for (older_uv, activation_mix, expected) in [
        (None, 1.0, 0.5),
        (Some(0.6), 1.0, 0.4),
        (None, 0.5, 0.25),
        (Some(0.6), 0.5, 0.5),
    ] {
        let mut samples = Vec::new();
        if let Some(amount) = older_uv {
            samples.push(color(ColorComponent::Uv, amount, 1).into());
        }
        samples.push(mixed(activation_mix));
        assert_uv_only(&run(&uv_family(0.0), &samples), expected);
    }
}

#[test]
fn higher_opaque_whole_changes_implicit_uv_without_overwriting_the_explicit_branch() {
    for (older_uv, expected) in [(None, 0.55), (Some(0.6), 0.4)] {
        let mut samples = Vec::new();
        if let Some(amount) = older_uv {
            samples.push(color(ColorComponent::Uv, amount, 1).into());
        }
        samples.push(mixed(1.0));
        samples.push(whole(0.9, 20, 1.0));
        assert_uv_only(&run(&uv_family(0.0), &samples), expected);
    }
}

#[test]
fn higher_partial_whole_is_applied_to_each_branch_implicit_uv_before_explicit_composition() {
    let samples = [mixed(1.0), whole(0.0, 20, 0.5)];
    // Old branch: whole 0.8 fades halfway to 0 = 0.4. New explicit branch stays 0.2.
    assert_uv_only(&run(&uv_family(0.0), &samples), 0.3);
}

#[test]
fn higher_opaque_explicit_uv_overrides_both_mixed_branches() {
    for older_uv in [None, Some(0.6)] {
        for activation_mix in [1.0, 0.5] {
            let mut samples = Vec::new();
            if let Some(amount) = older_uv {
                samples.push(color(ColorComponent::Uv, amount, 1).into());
            }
            samples.push(mixed(activation_mix));
            samples.push(whole(0.0, 20, 0.5));
            samples.push(color(ColorComponent::Uv, 0.9, 30).into());
            assert_uv_only(&run(&uv_family(0.0), &samples), 0.9);
        }
    }
}

#[test]
fn exact_whole_and_uv_endpoints_restore_their_distinct_base_and_component_masks() {
    let base = uv_family(0.0);
    let mut red = ColorIntent {
        uv: UvIntent { amount: 0.8 },
        ..Default::default()
    };
    VirtualColorAuthoringV1
        .set_base_component(&mut red, ColorComponent::Green, 0.0)
        .unwrap();
    VirtualColorAuthoringV1
        .set_base_component(&mut red, ColorComponent::Blue, 0.0)
        .unwrap();
    let prefix = [
        color(ColorComponent::Green, 0.7, 1),
        color(ColorComponent::Red, 0.2, 2),
        color(ColorComponent::Uv, 0.6, 3),
    ];
    for progress in [0.0, 1.0] {
        let mut samples = prefix
            .iter()
            .cloned()
            .map(FamilyCompositionSample::from)
            .collect::<Vec<_>>();
        samples.push(coupled(
            resume(
                Some(whole_leaf(semantic(red.clone()))),
                Some(uv_leaf(0.2)),
                progress,
                500,
            ),
            color(ColorComponent::Uv, 0.0, 10).rank,
            1.0,
        ));
        let result = run(&base, &samples);
        if progress == 0.0 {
            let mut expected = red.clone();
            expected.uv.amount = 0.6;
            assert_eq!(result, semantic(expected));
        } else {
            let mut expected_samples = prefix.to_vec();
            expected_samples.push(color(ColorComponent::Uv, 0.2, 10));
            assert_eq!(
                result,
                compose(ProgrammingOwner::Color, &base, &expected_samples).unwrap()
            );
            assert_eq!(intent(&result).recipe.rgb, [0.2, 0.7, 1.0]);
        }
    }
}

#[test]
fn sibling_resume_lanes_share_branch_decisions_only_with_the_same_source_scope_and_occurrence() {
    for different_scope in 0..4 {
        let outgoing_rank = color(ColorComponent::Uv, 0.0, 10).rank;
        let mut incoming_rank = outgoing_rank
            .with_dynamic_lane(Uuid::from_u128(9999))
            .unwrap();
        let mut incoming_occurrence = 500;
        let FamilySampleIdentity::Dynamic {
            instance_id,
            controller_id,
            ..
        } = &mut incoming_rank.identity
        else {
            unreachable!()
        };
        match different_scope {
            1 => *instance_id = Uuid::from_u128(9001),
            2 => *controller_id = Uuid::from_u128(9002),
            3 => incoming_occurrence = 501,
            _ => {}
        }
        let samples = [
            coupled(
                resume(Some(whole_leaf(uv_family(0.8))), None, 0.5, 500),
                outgoing_rank,
                1.0,
            ),
            coupled(
                resume(None, Some(uv_leaf(0.2)), 0.5, incoming_occurrence),
                incoming_rank,
                1.0,
            ),
        ];
        // Independent effects first release the whole to 0.4, then blend that underlay to 0.2.
        // Correlated siblings represent one transition from 0.8 to 0.2, with no double fade.
        let expected = if different_scope == 0 { 0.5 } else { 0.3 };
        assert_uv_only(&run(&uv_family(0.0), &samples), expected);
    }
}

#[test]
fn coupled_orthogonal_only_color_does_not_force_a_direct_base_into_semantic_mode() {
    let model = native_model();
    let base = direct(&model);
    let samples = [coupled(
        resume(Some(uv_leaf(0.2)), Some(uv_leaf(0.8)), 0.5, 500),
        color(ColorComponent::Uv, 0.0, 10).rank,
        1.0,
    )];
    assert_eq!(run(&base, &samples), base);
    assert_eq!(model.predictions.load(Ordering::Relaxed), 0);
}

#[test]
fn whole_and_retained_component_siblings_correlate_only_with_matching_scope_and_occurrence() {
    for different_scope in 0..4 {
        let outgoing_rank = color(ColorComponent::Uv, 0.0, 10).rank;
        let mut incoming_rank = outgoing_rank
            .with_dynamic_lane(Uuid::from_u128(9999))
            .unwrap();
        let mut incoming_occurrence = 500;
        let FamilySampleIdentity::Dynamic {
            instance_id,
            controller_id,
            ..
        } = &mut incoming_rank.identity
        else {
            unreachable!()
        };
        match different_scope {
            1 => *instance_id = Uuid::from_u128(9001),
            2 => *controller_id = Uuid::from_u128(9002),
            3 => incoming_occurrence = 501,
            _ => {}
        }
        let outgoing = FamilyCompositionSample::WholeExpression {
            expression: Arc::new(
                CompiledProgrammingFamilyExpression::new(
                    resume(Some(whole_leaf(uv_family(0.8))), None, 0.5, 500),
                    ProgrammingOwner::Color,
                    None,
                    None,
                )
                .unwrap(),
            ),
            rank: outgoing_rank,
            activation_mix: 1.0,
        };
        let template = color(ColorComponent::Uv, 0.0, 10);
        let incoming = FamilySample::retained_component(
            Arc::new(
                CompiledComponentExpression::new(
                    resume(None, Some(uv_leaf(0.2)), 0.5, incoming_occurrence),
                    Arc::new(template.address().clone()),
                )
                .unwrap(),
            ),
            incoming_rank,
            1.0,
        )
        .unwrap();
        let samples = [outgoing, incoming.into()];
        let expected = if different_scope == 0 { 0.5 } else { 0.3 };
        assert_uv_only(&run(&uv_family(0.0), &samples), expected);
    }
}

#[test]
fn partial_whole_fix_at_blends_after_the_complete_lower_mixed_orthogonal_result() {
    for (older_uv, source_activation, expected) in [
        (None, 1.0, 0.25),
        (Some(0.6), 1.0, 0.2),
        (None, 0.5, 0.125),
        (Some(0.6), 0.5, 0.25),
    ] {
        let mut samples = Vec::new();
        if let Some(amount) = older_uv {
            samples.push(color(ColorComponent::Uv, amount, 1).into());
        }
        samples.push(mixed(source_activation));
        let mut mask = sample(
            DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Whole,
            },
            None,
            DynamicValue::Family(uv_family(0.0)),
            20,
        )
        .into_fix_at();
        mask.activation_mix = 0.5;
        samples.push(mask.into());
        // The mask halves the already composed lower UV: respectively 0.5, 0.4, 0.25, 0.5.
        assert_uv_only(&run(&uv_family(0.0), &samples), expected);
    }
}

#[test]
fn opaque_whole_fix_at_prunes_lower_mixed_branches_and_their_appearance_dependency() {
    let model = native_model();
    let base = direct(&model);
    let frame = UnavailableFrame::default();
    let context = FamilyCompositionContext::default();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    assert_eq!(
        compose_retained_dynamic_family(
            ProgrammingOwner::Color,
            &base,
            &[mixed(1.0)],
            &context,
            &frame,
            &mut scratch,
        )
        .unwrap_err(),
        TransitionError::Requires(TransitionRequirement::ColorAppearance)
    );
    assert!(
        frame.0.get() > 0,
        "the uncovered mixed source needs the Direct underlay's appearance"
    );
    let mask = sample(
        DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        None,
        DynamicValue::Family(uv_family(0.9)),
        20,
    )
    .into_fix_at();
    let samples = [
        color(ColorComponent::Uv, 0.6, 1).into(),
        mixed(1.0),
        mask.into(),
    ];
    frame.0.set(0);
    let result = compose_retained_dynamic_family(
        ProgrammingOwner::Color,
        &base,
        &samples,
        &context,
        &frame,
        &mut scratch,
    )
    .unwrap();
    assert_uv_only(&result, 0.9);
    assert_eq!(
        frame.0.get(),
        0,
        "the full whole mask must cover all lower branch dependencies"
    );
    assert_eq!(model.predictions.load(Ordering::Relaxed), 0);
}

#[test]
fn mixed_whole_to_white_components_preserves_units_branch_defaults_and_unrelated_color() {
    let mut common = ColorIntent {
        white_blend: 0.6,
        uv: UvIntent { amount: 0.35 },
        ..Default::default()
    };
    for (component, value) in [
        (ColorComponent::Red, 0.2),
        (ColorComponent::Green, 0.7),
        (ColorComponent::Blue, 0.3),
    ] {
        VirtualColorAuthoringV1
            .set_base_component(&mut common, component, value)
            .unwrap();
    }
    for (component, outgoing_value, incoming_value) in [
        (
            ColorComponent::Temperature,
            common.white_target.kelvin,
            2000.0,
        ),
        (ColorComponent::Temperature, 2000.0, 10000.0),
        (ColorComponent::Duv, common.white_target.duv, 0.03),
        (ColorComponent::Duv, -0.03, 0.01),
    ] {
        let mut outgoing = common.clone();
        match component {
            ColorComponent::Temperature => outgoing.white_target.kelvin = outgoing_value,
            ColorComponent::Duv => outgoing.white_target.duv = outgoing_value,
            _ => unreachable!(),
        }
        let incoming = color(component, incoming_value, 10);
        let incoming_leaf = Arc::new(E::Programming {
            address: Arc::new(incoming.address().address().clone()),
            value: DynamicValue::Scalar(incoming_value),
            occurrence: None,
            dependency_occurrence: None,
        });
        for progress in [0.0, 0.5, 1.0] {
            let result = run(
                &semantic(common.clone()),
                &[coupled(
                    resume(
                        Some(whole_leaf(semantic(outgoing.clone()))),
                        Some(incoming_leaf.clone()),
                        progress,
                        500,
                    ),
                    incoming.rank,
                    1.0,
                )],
            );
            let mut actual = intent(&result).clone();
            actual.validate().unwrap();
            let p = f64::from(progress);
            match component {
                ColorComponent::Temperature => {
                    let expected = (1.0
                        / ((1.0 - p) / f64::from(outgoing_value) + p / f64::from(incoming_value)))
                        as f32;
                    assert!(
                        (actual.white_target.kelvin - expected).abs() < 0.001,
                        "Kelvin must use reciprocal interpolation at {progress}: expected {expected}, got {}",
                        actual.white_target.kelvin
                    );
                    actual.white_target.kelvin = common.white_target.kelvin;
                }
                ColorComponent::Duv => {
                    let expected = (f64::from(outgoing_value) * (1.0 - p)
                        + f64::from(incoming_value) * p) as f32;
                    assert!(
                        (actual.white_target.duv - expected).abs() < 0.000001,
                        "Duv must use linear interpolation at {progress}: expected {expected}, got {}",
                        actual.white_target.duv
                    );
                    assert!((-0.03..=0.03).contains(&actual.white_target.duv));
                    actual.white_target.duv = common.white_target.duv;
                }
                _ => unreachable!(),
            }
            assert_eq!(
                actual, common,
                "white component {component:?} must preserve UV, white blend and the visible color recipe"
            );
        }
    }
}

#[test]
fn conditioning_a_covered_whole_does_not_require_the_unselected_direct_underlay_model() {
    let model = native_model();
    let base = direct(&model);
    let older_rank = color(ColorComponent::Uv, 0.0, 1).rank;
    let nested = resume(
        Some(whole_leaf(uv_family(0.8))),
        Some(whole_leaf(uv_family(0.6))),
        0.5,
        500,
    );
    let older = Arc::new(E::Transition {
        from: Some(nested),
        to: None,
        progress: 0.5,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::ColorAppearance,
        },
    });
    let older = FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            crate::CompiledProgrammingFamilyExpression::new(
                older,
                ProgrammingOwner::Color,
                None,
                None,
            )
            .unwrap(),
        ),
        rank: older_rank,
        activation_mix: 1.0,
    };
    let mut uv = color(ColorComponent::Uv, 0.0, 3);
    let scope = older_rank.dynamic_identity().unwrap();
    uv.rank.identity = FamilySampleIdentity::Dynamic {
        instance_id: scope.instance_id,
        controller_id: scope.controller_id,
        lane_id: uv.rank.dynamic_identity().unwrap().lane_id,
    };
    let incoming = FamilySample::retained_component(
        Arc::new(
            CompiledComponentExpression::new(
                resume(None, Some(uv_leaf(0.2)), 0.5, 500),
                uv.address.clone(),
            )
            .unwrap(),
        ),
        uv.rank,
        1.0,
    )
    .unwrap();
    let samples = [
        older,
        color(ColorComponent::Red, 0.25, 2).into(),
        incoming.into(),
    ];
    let adoption = ColorIntent {
        uv: UvIntent { amount: 0.4 },
        ..Default::default()
    };
    let frame = UnavailableFrame::default();
    let result = compose_retained_dynamic_family(
        ProgrammingOwner::Color,
        &base,
        &samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                semantic_color_adoption: Some(&adoption),
                ..Default::default()
            },
            ..Default::default()
        },
        &frame,
        &mut RetainedFamilyCompositionScratch::default(),
    )
    .unwrap();
    assert!((intent(&result).uv.amount - 0.3).abs() < 0.000001);
    assert_eq!(intent(&result).recipe.rgb, [0.25, 1.0, 1.0]);
    assert_eq!(
        frame.0.get(),
        0,
        "covered whole dependency must never resolve"
    );
    assert_eq!(model.predictions.load(Ordering::Relaxed), 0);
}
