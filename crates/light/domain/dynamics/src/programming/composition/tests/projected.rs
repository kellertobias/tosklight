use super::*;
use crate::{
    DynamicTransitionReason, FamilyCompositionSample, FamilyExpressionOperation,
    RetainedFamilyCompositionScratch, WholeFamilyExpressionFrameResolver,
    compose_retained_dynamic_family,
};

fn projected() -> CompiledComponentExpressionSet {
    let leaf = |component, value| {
        let sample = color(component, value, 1);
        Arc::new(DynamicSampleExpression::Programming {
            address: Arc::new(sample.address().address().clone()),
            value: sample.materialized_value().unwrap().clone(),
            occurrence: None,
            dependency_occurrence: None,
        })
    };
    CompiledComponentExpressionSet::new(
        Arc::new(DynamicSampleExpression::Transition {
            from: Some(leaf(ColorComponent::Uv, 0.8)),
            to: Some(leaf(ColorComponent::WhiteBlend, 1.0)),
            progress: 0.25,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(55),
            },
        }),
        None,
    )
    .unwrap()
}

struct NoFrame;
impl WholeFamilyExpressionFrameResolver for NoFrame {
    fn resolve(
        &self,
        _: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("independent semantic component fragments need no geometry or appearance")
    }
}

#[test]
fn one_lane_can_release_uv_while_acquiring_white_without_claiming_other_components() {
    let mut base = ColorIntent::default();
    base.uv.amount = 0.2;
    base.white_blend = 0.1;
    let base = semantic(base);
    let projection = projected();
    let rank = color(ColorComponent::Red, 0.0, 20).rank;
    let mut samples = FamilySample::retained_components(&projection, rank, 0.5).unwrap();
    let lower = color(ColorComponent::Red, 0.1, 1);
    let expected_base = compose(ProgrammingOwner::Color, &base, &[lower.clone()]).unwrap();
    samples.push(lower);
    for _ in 0..2 {
        let result = compose(ProgrammingOwner::Color, &base, &samples).unwrap();
        let actual = intent(&result);
        assert!((actual.uv.amount - 0.425).abs() < 1e-6);
        assert!((actual.white_blend - 0.2125).abs() < 1e-6);
        let mut expected = intent(&expected_base).clone();
        expected.uv.amount = actual.uv.amount;
        expected.white_blend = actual.white_blend;
        assert_eq!(result, semantic(expected));
        let inputs = samples
            .iter()
            .cloned()
            .map(FamilyCompositionSample::from)
            .collect::<Vec<_>>();
        assert_eq!(
            result,
            compose_retained_dynamic_family(
                ProgrammingOwner::Color,
                &base,
                &inputs,
                &FamilyCompositionContext {
                    edit: FamilyEditContext {
                        color_model: Some(&VirtualColorAuthoringV1),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                &NoFrame,
                &mut RetainedFamilyCompositionScratch::default(),
            )
            .unwrap()
        );
        samples.reverse();
    }
}

#[test]
fn fragment_ties_cannot_hide_duplicate_lanes_or_unrelated_authored_trees() {
    let projection = projected();
    let rank = color(ColorComponent::Red, 0.0, 20).rank;
    let mut fragments = FamilySample::retained_components(&projection, rank, 1.0).unwrap();
    let base = semantic(ColorIntent::default());
    assert!(compose(ProgrammingOwner::Color, &base, &fragments).is_ok());
    fragments.push(fragments[0].clone());
    assert!(compose(ProgrammingOwner::Color, &base, &fragments).is_err());
    fragments.pop();
    let unrelated = projected();
    let other = FamilySample::retained_components(&unrelated, rank, 1.0).unwrap();
    fragments[1] = other[1].clone();
    assert!(compose(ProgrammingOwner::Color, &base, &fragments).is_err());
    fragments = FamilySample::retained_components(&projection, rank, 1.0).unwrap();
    let mut ordinary = color(ColorComponent::RelativeOutput, 0.25, 1);
    ordinary.rank = rank;
    fragments.push(ordinary);
    assert!(compose(ProgrammingOwner::Color, &base, &fragments).is_err());
}
