use super::*;
use crate::DynamicRuntimeSample;
use crate::programming::*;
use light_core::FixtureId;
use std::{cell::Cell, sync::Arc};
use uuid::Uuid;

fn target(reference: TargetReference, offsets: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(reference, offsets)))
}

fn whole(value: AttributeValue) -> DynamicSampleExpression {
    DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    }
}

fn component(
    representation: DynamicFamilyRepresentation,
    part: ProgrammingComponent,
    value: DynamicValue,
) -> DynamicSampleExpression {
    DynamicSampleExpression::Programming {
        address: Arc::new(DynamicValueAddress {
            representation,
            component: Some(part),
        }),
        value,
        occurrence: None,
        dependency_occurrence: None,
    }
}

fn sample(
    lane: u128,
    from: DynamicSampleExpression,
    to: DynamicSampleExpression,
) -> DynamicRuntimeSample {
    DynamicRuntimeSample {
        instance_id: Uuid::from_u128(1),
        controller_id: Uuid::from_u128(2),
        target: FixtureId(Uuid::from_u128(3)),
        lane_id: Uuid::from_u128(lane),
        expression: DynamicSampleExpression::Transition {
            from: Some(Arc::new(from)),
            to: Some(Arc::new(to)),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(50),
            },
        },
        priority: 10,
        activated_at_millis: 100,
        activation_mix: 1.0,
        address: None,
    }
}

struct Sources;
impl DynamicValueSourceResolver for Sources {
    fn current(&self, _: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        (address.component == Some(ProgrammingComponent::Tilt))
            .then_some(DynamicValue::Scalar(30.0))
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

struct Frame {
    fail_once: Cell<bool>,
}
impl WholeFamilyExpressionFrameResolver for Frame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        to: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        if self.fail_once.replace(false) {
            return Err(TransitionError::Requires(requirement));
        }
        Ok(to.clone())
    }
}

#[test]
fn source_cohort_reuses_nested_buffers_after_failure_and_repeated_traced_frames() {
    let origin = TargetReference::Origin;
    let point = TargetReference::Point {
        point_id: Uuid::from_u128(77),
    };
    let deferred_whole = DynamicSampleExpression::Transition {
        from: Some(Arc::new(whole(target(point, [1.0, 2.0, 3.0])))),
        to: Some(Arc::new(whole(target(origin, [3.0, 4.0, 5.0])))),
        progress: 0.5,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::MaterializedEndpoints,
        },
    };
    let samples = [
        sample(
            10,
            deferred_whole,
            component(
                DynamicFamilyRepresentation::Angles,
                ProgrammingComponent::Pan,
                DynamicValue::Scalar(90.0),
            ),
        ),
        sample(
            11,
            component(
                DynamicFamilyRepresentation::Target {
                    reference: Some(origin),
                },
                ProgrammingComponent::TargetX,
                DynamicValue::Scalar(4.0),
            ),
            DynamicSampleExpression::AngleCurrent {
                address: Arc::new(DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Angles,
                    component: Some(ProgrammingComponent::Tilt),
                }),
            },
        ),
    ];
    let mut preparation = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&samples, &Sources, None, &mut preparation).unwrap();
    let family = &prepared.families[0];
    assert!(family.samples.iter().any(|sample| matches!(sample,
        FamilyCompositionSample::CoupledExpression { expression, .. }
            if expression.base_endpoints().iter().any(|endpoint| endpoint.cohort_sources().is_some())
    )));
    let base = target(origin, [1.0, 2.0, 3.0]);
    let frame = Frame {
        fail_once: Cell::new(true),
    };
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let compose = |scratch: &mut RetainedFamilyCompositionScratch| {
        compose_retained_dynamic_family_traced(
            ProgrammingOwner::Position,
            &base,
            &family.samples,
            &FamilyCompositionContext::default(),
            &frame,
            scratch,
        )
    };
    assert!(compose(&mut scratch).is_err());
    let first = scratch
        .source_cohort_scratch
        .as_ref()
        .expect("nested scratch restored after error");
    let nested_address = &**first as *const RetainedFamilyCompositionScratch;
    let first_value = compose(&mut scratch).unwrap();
    assert!(scratch.family_trace().root().is_some());
    let nested = scratch.source_cohort_scratch.as_ref().unwrap();
    assert_eq!(
        &**nested as *const RetainedFamilyCompositionScratch,
        nested_address
    );
    let nested_source_capacity = nested.sources.capacity();
    let candidate_capacity = scratch.source_cohort_candidates.capacity();
    for _ in 0..8 {
        assert_eq!(compose(&mut scratch).unwrap(), first_value);
        assert_eq!(
            &**scratch.source_cohort_scratch.as_ref().unwrap()
                as *const RetainedFamilyCompositionScratch,
            nested_address
        );
        assert_eq!(
            scratch
                .source_cohort_scratch
                .as_ref()
                .unwrap()
                .sources
                .capacity(),
            nested_source_capacity
        );
        assert_eq!(
            scratch.source_cohort_candidates.capacity(),
            candidate_capacity
        );
        assert!(scratch.family_trace().root().is_some());
    }
}
