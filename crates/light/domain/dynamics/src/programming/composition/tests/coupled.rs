use super::*;
use crate::{
    CompiledCoupledExpression, DynamicSampleExpression as E, DynamicTransitionReason,
    FamilyCompositionSample, FamilyExpressionOperation, RetainedFamilyCompositionScratch,
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

fn leaf(sample: &FamilySample) -> Arc<E> {
    Arc::new(E::Programming {
        address: Arc::new(sample.address().address().clone()),
        value: sample.materialized_value().unwrap().clone(),
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn whole_leaf(owner: ProgrammingOwner, value: AttributeValue) -> Arc<E> {
    Arc::new(E::Programming {
        address: Arc::new(DynamicValueAddress::whole_family(owner, &value).unwrap()),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn resume(from: Option<Arc<E>>, to: Option<Arc<E>>, progress: f32) -> Arc<E> {
    Arc::new(E::Transition {
        from,
        to,
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(500),
        },
    })
}

fn coupled(expression: Arc<E>, order: u128, activation_mix: f32) -> FamilyCompositionSample {
    FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(CompiledCoupledExpression::new(expression, None).unwrap()),
        rank: color(ColorComponent::Red, 0.0, order).rank,
        activation_mix,
    }
}

fn run(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: &[FamilyCompositionSample],
    frame: &dyn WholeFamilyExpressionFrameResolver,
) -> Result<AttributeValue, TransitionError> {
    compose_retained_dynamic_family(
        owner,
        base,
        samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            ..Default::default()
        },
        frame,
        &mut RetainedFamilyCompositionScratch::default(),
    )
}

fn target(reference: TargetReference, offsets: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(reference, offsets)))
}

fn offset(
    reference: TargetReference,
    component: ProgrammingComponent,
    value: f32,
    order: u128,
) -> FamilySample {
    sample(
        DynamicFamilyRepresentation::Target {
            reference: Some(reference),
        },
        Some(component),
        DynamicValue::Scalar(value),
        order,
    )
}

fn live_whole_points() -> Arc<E> {
    resume(
        Some(whole_leaf(
            ProgrammingOwner::Position,
            target(TargetReference::Origin, [1.0, 2.0, 3.0]),
        )),
        Some(whole_leaf(
            ProgrammingOwner::Position,
            target(
                TargetReference::Point {
                    point_id: Uuid::from_u128(77),
                },
                [4.0, 5.0, 6.0],
            ),
        )),
        0.5,
    )
}

#[test]
fn exact_recipe_endpoint_keeps_older_compatible_green_despite_a_newer_hue_prefix() {
    let base = semantic(ColorIntent::default());
    let prefix = [
        color(ColorComponent::Green, 0.7, 1),
        color(ColorComponent::Hue, 120.0, 2),
        color(ColorComponent::Saturation, 0.75, 3),
    ];
    let red = color(ColorComponent::Red, 0.2, 10);
    let hue = color(ColorComponent::Hue, 300.0, 10);
    for (progress, endpoint) in [(0.0, red.clone()), (1.0, hue.clone())] {
        let mut endpoint_samples = prefix.to_vec();
        endpoint_samples.push(endpoint);
        let expected = compose(ProgrammingOwner::Color, &base, &endpoint_samples).unwrap();
        let mut samples = prefix
            .iter()
            .cloned()
            .map(FamilyCompositionSample::from)
            .collect::<Vec<_>>();
        samples.push(coupled(
            resume(Some(leaf(&red)), Some(leaf(&hue)), progress),
            10,
            1.0,
        ));
        let frame = UnavailableFrame::default();
        let result = run(ProgrammingOwner::Color, &base, &samples, &frame).unwrap();
        assert_eq!(result, expected);
        if progress == 0.0 {
            assert_eq!(intent(&result).recipe.rgb, [0.2, 0.7, 1.0]);
        }
        assert_eq!(frame.0.get(), 0);
    }
}

#[test]
fn recipe_to_hue_interpolates_independent_endpoint_cohorts_then_activates_once() {
    let base = semantic(ColorIntent::default());
    let prefix = [
        color(ColorComponent::Green, 0.7, 1),
        color(ColorComponent::Hue, 120.0, 2),
        color(ColorComponent::Saturation, 0.75, 3),
    ];
    let red = color(ColorComponent::Red, 0.2, 10);
    let hue = color(ColorComponent::Hue, 300.0, 10);
    let endpoint = |sample: FamilySample| {
        let mut samples = prefix.to_vec();
        samples.push(sample);
        compose(ProgrammingOwner::Color, &base, &samples).unwrap()
    };
    let from = endpoint(red.clone());
    let to = endpoint(hue.clone());
    assert_eq!(intent(&from).recipe.rgb, [0.2, 0.7, 1.0]);
    let transitioned = CompiledProgrammingTransition::new(from, to, None)
        .unwrap()
        .sample(0.5)
        .unwrap();
    let lower_base = compose(ProgrammingOwner::Color, &base, &prefix).unwrap();
    for activation_mix in [1.0, 0.5] {
        let expected =
            CompiledProgrammingTransition::new(lower_base.clone(), transitioned.clone(), None)
                .unwrap()
                .sample(activation_mix)
                .unwrap();
        let mut samples = prefix
            .iter()
            .cloned()
            .map(FamilyCompositionSample::from)
            .collect::<Vec<_>>();
        samples.push(coupled(
            resume(Some(leaf(&red)), Some(leaf(&hue)), 0.5),
            10,
            activation_mix,
        ));
        let frame = UnavailableFrame::default();
        assert_eq!(
            run(ProgrammingOwner::Color, &base, &samples, &frame).unwrap(),
            expected
        );
        assert_eq!(frame.0.get(), 0);
    }
}

#[test]
fn completed_whole_to_red_transition_restores_narrow_mask_and_lower_green() {
    let base = semantic(ColorIntent::default());
    let mut blue = ColorIntent::default();
    VirtualColorAuthoringV1
        .set_base_component(&mut blue, ColorComponent::Red, 0.0)
        .unwrap();
    VirtualColorAuthoringV1
        .set_base_component(&mut blue, ColorComponent::Green, 0.0)
        .unwrap();
    let prefix = [
        color(ColorComponent::Green, 0.7, 1),
        color(ColorComponent::Hue, 180.0, 2),
    ];
    let red = color(ColorComponent::Red, 0.2, 10);
    let expression = resume(
        Some(whole_leaf(ProgrammingOwner::Color, semantic(blue))),
        Some(leaf(&red)),
        1.0,
    );
    let mut samples = prefix
        .into_iter()
        .map(FamilyCompositionSample::from)
        .collect::<Vec<_>>();
    samples.push(coupled(expression, 10, 1.0));
    let frame = UnavailableFrame::default();
    let result = run(ProgrammingOwner::Color, &base, &samples, &frame).unwrap();
    assert_eq!(intent(&result).recipe.rgb, [0.2, 0.7, 1.0]);
    assert_eq!(frame.0.get(), 0);
}

#[test]
fn exact_absent_coupled_endpoint_has_no_mask_and_skips_live_geometry() {
    let base = angles(30.0, 15.0);
    for expression in [
        resume(Some(live_whole_points()), None, 1.0),
        resume(None, Some(live_whole_points()), 0.0),
    ] {
        let frame = UnavailableFrame::default();
        let samples = [coupled(expression, 10, 1.0)];
        assert_eq!(
            run(ProgrammingOwner::Position, &base, &samples, &frame).unwrap(),
            base
        );
        assert_eq!(frame.0.get(), 0);
    }
}

#[test]
fn target_components_use_their_own_reference_cohorts_before_live_transition_and_pan_mask() {
    let point = TargetReference::Point {
        point_id: Uuid::from_u128(77),
    };
    let base = target(TargetReference::Origin, [1.0, 2.0, 3.0]);
    let from = offset(
        TargetReference::Origin,
        ProgrammingComponent::TargetX,
        10.0,
        10,
    );
    let to = offset(point, ProgrammingComponent::TargetY, 20.0, 10);
    let samples = [
        offset(
            TargetReference::Origin,
            ProgrammingComponent::TargetZ,
            9.0,
            1,
        )
        .into(),
        offset(
            TargetReference::Origin,
            ProgrammingComponent::TargetY,
            4.0,
            2,
        )
        .into(),
        offset(point, ProgrammingComponent::TargetX, 42.0, 3).into(),
        coupled(resume(Some(leaf(&from)), Some(leaf(&to)), 0.5), 10, 1.0),
        angle(ProgrammingComponent::Pan, 900.0, 20)
            .into_fix_at()
            .into(),
    ];
    struct EndpointFrame {
        point: TargetReference,
        calls: Cell<usize>,
    }
    impl WholeFamilyExpressionFrameResolver for EndpointFrame {
        fn resolve(
            &self,
            requirement: TransitionRequirement,
            from: &AttributeValue,
            to: &AttributeValue,
            operation: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            assert_eq!(requirement, TransitionRequirement::LiveTargetPoints);
            assert_eq!(from, &target(TargetReference::Origin, [10.0, 4.0, 9.0]));
            assert_eq!(to, &target(self.point, [42.0, 20.0, 7.0]));
            assert_eq!(
                operation,
                FamilyExpressionOperation::Transition { progress: 0.5 }
            );
            self.calls.set(self.calls.get() + 1);
            Ok(angles(720.0, 40.0))
        }
    }
    let frame = EndpointFrame {
        point,
        calls: Cell::new(0),
    };
    let adoption_calls = Cell::new(0);
    let adopt = |actual: &AttributeValue, address: &DynamicValueAddress| {
        assert_eq!(actual, &base);
        assert_eq!(
            address.representation,
            DynamicFamilyRepresentation::Target {
                reference: Some(point)
            }
        );
        adoption_calls.set(adoption_calls.get() + 1);
        Ok(target(point, [5.0, 6.0, 7.0]))
    };
    let result = compose_retained_dynamic_family(
        ProgrammingOwner::Position,
        &base,
        &samples,
        &FamilyCompositionContext {
            resolve_adoption: Some(&adopt),
            ..Default::default()
        },
        &frame,
        &mut RetainedFamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(result, angles(900.0, 40.0));
    assert_eq!(frame.calls.get(), 1);
    assert_eq!(adoption_calls.get(), 1);
}

#[test]
fn opaque_whole_and_full_fix_at_cover_coupled_geometry_without_resolving_it() {
    let base = angles(0.0, 0.0);
    let frame = UnavailableFrame::default();
    assert_eq!(
        run(
            ProgrammingOwner::Position,
            &base,
            &[coupled(live_whole_points(), 1, 1.0)],
            &frame
        )
        .unwrap_err(),
        TransitionError::Requires(TransitionRequirement::LiveTargetPoints)
    );
    assert_eq!(frame.0.get(), 1);
    let expected = angles(720.0, 20.0);
    for fixed in [false, true] {
        let cover = sample(
            DynamicFamilyRepresentation::Angles,
            None,
            DynamicValue::Family(expected.clone()),
            20,
        );
        let cover = if fixed { cover.into_fix_at() } else { cover };
        let samples = [coupled(live_whole_points(), 1, 1.0), cover.into()];
        frame.0.set(0);
        assert_eq!(
            run(ProgrammingOwner::Position, &base, &samples, &frame).unwrap(),
            expected
        );
        assert_eq!(frame.0.get(), 0);
    }
}
