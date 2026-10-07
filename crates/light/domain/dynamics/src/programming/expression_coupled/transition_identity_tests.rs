//! Retained operation identity remains separate from values and authored provenance.
use super::*;
use crate::{
    CompiledProgrammingFamilyExpression, DynamicRuntimeSample, FamilyExpressionObserver,
    FamilyExpressionStep, bundle_position_component_forest,
};
use light_core::FixtureId;
use uuid::Uuid;

type E = DynamicSampleExpression;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}

fn whole(pan: f32, tilt: f32) -> Arc<E> {
    Arc::new(E::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: None,
        }),
        value: DynamicValue::Family(angles(pan, tilt)),
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn axis(component: ProgrammingComponent, value: f32) -> Arc<E> {
    Arc::new(E::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(component),
        }),
        value: DynamicValue::Scalar(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn resume(from: Arc<E>, to: Arc<E>, occurrence: u128) -> Arc<E> {
    Arc::new(E::Transition {
        from: Some(from),
        to: Some(to),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(occurrence),
        },
    })
}

fn nested_whole() -> Arc<E> {
    resume(
        resume(whole(0.0, 2.0), whole(10.0, 6.0), 101),
        whole(20.0, 10.0),
        202,
    )
}

fn resume_reason(id: u128) -> DynamicTransitionReason {
    DynamicTransitionReason::Resume {
        occurrence_id: Uuid::from_u128(id),
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
        panic!("complete Angle endpoints should not need a frame solve: {requirement:?}")
    }
}

struct NoUnderlay;
impl CoupledExpressionContext for NoUnderlay {
    fn materialize_base(
        &self,
        _: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("complete Angle endpoints should not request an underlay")
    }
    fn orthogonal_underlay(
        &self,
        _: ColorComponent,
        _: &AttributeValue,
    ) -> Result<f32, TransitionError> {
        panic!("Position should not request a Color orthogonal")
    }
}

struct CoupledReasons<'a> {
    expression: &'a CompiledCoupledExpression,
    reasons: Vec<DynamicTransitionReason>,
}
impl CoupledEvaluationObserver for CoupledReasons<'_> {
    fn evaluated(
        &mut self,
        node: usize,
        step: CoupledEvaluationStep<'_>,
        _: &AttributeValue,
    ) -> Result<(), TransitionError> {
        if let CoupledEvaluationStep::Transition { progress, .. } = step {
            assert_eq!(progress, 0.5);
            self.reasons
                .push(self.expression.transition_reason(node).unwrap());
        } else {
            assert_eq!(self.expression.transition_reason(node), None);
        }
        Ok(())
    }
}

struct WholeReasons<'a> {
    expression: &'a CompiledProgrammingFamilyExpression,
    reasons: Vec<DynamicTransitionReason>,
}
impl FamilyExpressionObserver for WholeReasons<'_> {
    fn evaluated(
        &mut self,
        node: usize,
        step: FamilyExpressionStep<'_>,
        _: &AttributeValue,
    ) -> Result<(), TransitionError> {
        if let FamilyExpressionStep::Transition { progress, .. } = step {
            assert_eq!(progress, 0.5);
            self.reasons
                .push(self.expression.transition_reason(node).unwrap());
        } else {
            assert_eq!(self.expression.transition_reason(node), None);
        }
        Ok(())
    }
}

fn assert_coupled_reasons(
    expression: &CompiledCoupledExpression,
    expected: &[DynamicTransitionReason],
) {
    let mut observer = CoupledReasons {
        expression,
        reasons: Vec::new(),
    };
    assert_eq!(
        expression
            .evaluate_base_observed(&NoUnderlay, &NoFrame, Some(&mut observer))
            .unwrap(),
        Some(angles(12.5, 7.0)),
    );
    assert_eq!(observer.reasons, expected);
    assert_eq!(expression.transition_reason(usize::MAX), None);
    assert_eq!(
        expression.transition_reason(expression.trace_root_node()),
        expected.last().copied(),
    );
}

#[test]
fn nested_equal_progress_resume_occurrences_survive_both_compiled_graphs() {
    let expression = nested_whole();
    let tape = RetainedExpressionTape::from_roots(std::slice::from_ref(&expression)).unwrap();
    let original = tape
        .nodes
        .iter()
        .filter_map(|node| match node {
            RetainedExpressionNode::Transition { reason, .. } => Some(*reason),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(original, [resume_reason(101), resume_reason(202)]);

    let coupled = CompiledCoupledExpression::new(expression.clone(), None).unwrap();
    assert!(Arc::ptr_eq(coupled.expression().unwrap(), &expression));
    assert_coupled_reasons(&coupled, &original);

    let whole = CompiledProgrammingFamilyExpression::new(
        expression.clone(),
        ProgrammingOwner::Position,
        None,
        None,
    )
    .unwrap();
    assert_eq!(whole.expression(), expression.as_ref());
    let mut observer = WholeReasons {
        expression: &whole,
        reasons: Vec::new(),
    };
    assert_eq!(
        whole
            .evaluate_optional_observed(None, &NoFrame, Some(&mut observer))
            .unwrap(),
        Some(angles(12.5, 7.0)),
    );
    assert_eq!(observer.reasons, original);
    assert_eq!(whole.transition_reason(usize::MAX), None);
    assert_eq!(
        whole.transition_reason(whole.trace_root_node()),
        Some(resume_reason(202)),
    );
}

fn sample(lane: u128, component: ProgrammingComponent, values: [f32; 3]) -> DynamicRuntimeSample {
    DynamicRuntimeSample {
        instance_id: Uuid::from_u128(1),
        controller_id: Uuid::from_u128(2),
        target: FixtureId(Uuid::from_u128(3)),
        lane_id: Uuid::from_u128(lane),
        expression: resume(
            resume(axis(component, values[0]), axis(component, values[1]), 101),
            axis(component, values[2]),
            202,
        )
        .as_ref()
        .clone(),
        priority: 3,
        activated_at_millis: 100,
        activation_mix: 0.4,
        address: None,
    }
}

#[test]
fn position_forest_join_keeps_nested_equal_progress_resume_occurrences() {
    let samples = [
        sample(10, ProgrammingComponent::Pan, [0.0, 10.0, 20.0]),
        sample(11, ProgrammingComponent::Tilt, [2.0, 6.0, 10.0]),
    ];
    let bundle = bundle_position_component_forest(
        &samples,
        &crate::programming::source::UnavailableProgrammingSources,
    )
    .unwrap();
    assert!(bundle.remainder.is_empty());
    let crate::FamilyCompositionSample::CoupledExpression {
        expression,
        rank,
        activation_mix,
    } = bundle.position.unwrap()
    else {
        panic!("Position branches should retain a coupled forest")
    };
    assert_eq!(
        rank.dynamic_identity().unwrap().instance_id,
        samples[0].instance_id
    );
    assert_eq!(activation_mix, 0.4);
    let CoupledRetainedSources::PositionForest(retained) = expression.retained_sources() else {
        panic!("original Position lane samples should remain retained")
    };
    for (actual, original) in retained.iter().zip(&samples) {
        assert_eq!(actual.expression, original.expression);
        assert_eq!(actual.lane_id, original.lane_id);
    }
    assert_coupled_reasons(&expression, &[resume_reason(101), resume_reason(202)]);
}

#[test]
fn position_forest_whole_import_retains_required_reason_and_resume_identity() {
    let required = DynamicTransitionReason::Required {
        requirement: TransitionRequirement::LiveJointAngles,
    };
    let imported = Arc::new(E::Transition {
        from: Some(whole(0.0, 2.0)),
        to: Some(whole(10.0, 6.0)),
        progress: 0.5,
        reason: required,
    });
    let forest = [
        PositionForestNode::Whole {
            expression: imported.clone(),
            lane_id: Uuid::from_u128(10),
            sources: Arc::from([]),
        },
        PositionForestNode::Whole {
            expression: whole(20.0, 10.0),
            lane_id: Uuid::from_u128(11),
            sources: Arc::from([]),
        },
        PositionForestNode::Transition {
            from: Some(0),
            to: Some(1),
            progress: 0.5,
            reason: resume_reason(202),
        },
    ];
    let compiled =
        CompiledCoupledExpression::from_position_forest(Arc::from([]), &forest, 2).unwrap();
    assert_coupled_reasons(&compiled, &[required, resume_reason(202)]);

    let whole =
        CompiledProgrammingFamilyExpression::new(imported, ProgrammingOwner::Position, None, None)
            .unwrap();
    assert_eq!(
        whole.transition_reason(whole.trace_root_node()),
        Some(required)
    );
}
