use super::*;
use crate::{DynamicSampleExpression, DynamicTransitionReason};
use std::cell::Cell;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target(reference: u128, xyz: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        if reference == 0 {
            TargetReference::Origin
        } else {
            TargetReference::Point {
                point_id: Uuid::from_u128(reference),
            }
        },
        xyz,
    )))
}
fn scalar(value: &ScalarIntent) -> f32 {
    match value {
        ScalarIntent::Value(value) => *value,
        _ => panic!("materialized test coordinate"),
    }
}
fn angle_value(value: &AttributeValue) -> AttributeValue {
    match value {
        AttributeValue::Position(value) => match value.as_ref() {
            PositionIntent::Angles { .. } => AttributeValue::Position(value.clone()),
            PositionIntent::Target { offset_metres, .. } => {
                angles(scalar(&offset_metres[0]), scalar(&offset_metres[1]))
            }
        },
        _ => panic!("Position test value"),
    }
}
fn rank(order: u128) -> FamilySampleRank {
    FamilySampleRank {
        priority: 10,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: order,
        identity: crate::FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(1000 + order),
            controller_id: Uuid::from_u128(2000 + order),
            lane_id: Uuid::from_u128(3000 + order),
        },
    }
}
fn leaf(value: AttributeValue) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn history(
    from: AttributeValue,
    to: AttributeValue,
    order: u128,
    activation_mix: f32,
) -> FamilyCompositionSample {
    FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::new(
                Arc::new(DynamicSampleExpression::Transition {
                    from: Some(leaf(from)),
                    to: Some(leaf(to)),
                    progress: 0.5,
                    reason: DynamicTransitionReason::Resume {
                        occurrence_id: Uuid::from_u128(5000 + order),
                    },
                }),
                None,
            )
            .unwrap(),
        ),
        rank: rank(order),
        activation_mix,
    }
}
fn whole_mask(value: AttributeValue, order: u128, mix: f32) -> FamilyCompositionSample {
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Family(value),
        rank(order),
        mix,
    )
    .unwrap()
    .into_fix_at()
    .into()
}

struct Frame {
    available: bool,
    calls: Cell<usize>,
}
impl Frame {
    fn blocked() -> Self {
        Self {
            available: false,
            calls: Cell::new(0),
        }
    }
    fn available() -> Self {
        Self {
            available: true,
            calls: Cell::new(0),
        }
    }
    fn materialize(
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> AttributeValue {
        let (from, to) = if requirement == TransitionRequirement::LiveTargetPoints {
            let xyz = |value: &AttributeValue| match value {
                AttributeValue::Position(value) => match value.as_ref() {
                    PositionIntent::Target { offset_metres, .. } => {
                        offset_metres.each_ref().map(scalar)
                    }
                    _ => panic!("Target endpoints"),
                },
                _ => panic!("Position endpoints"),
            };
            (target(0, xyz(from)), target(0, xyz(to)))
        } else {
            (angle_value(from), angle_value(to))
        };
        let compiled = CompiledProgrammingTransition::new(from, to, None).unwrap();
        match operation {
            FamilyExpressionOperation::Transition { progress } => {
                compiled.sample(progress).unwrap()
            }
            FamilyExpressionOperation::Scale { factor } => compiled.scale(factor).unwrap(),
        }
    }
}
impl WholeFamilyExpressionFrameResolver for Frame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        if self.available {
            Ok(Self::materialize(requirement, from, to, operation))
        } else {
            Err(TransitionError::Requires(requirement))
        }
    }
}
fn needed(progress: PositionCompositionProgress) -> PositionCompositionRequest {
    match progress {
        PositionCompositionProgress::NeedsMaterialization(request) => request,
        PositionCompositionProgress::Complete(_) => panic!("expected materialization"),
    }
}
fn complete(progress: PositionCompositionProgress) -> AttributeValue {
    match progress {
        PositionCompositionProgress::Complete(value) => value,
        PositionCompositionProgress::NeedsMaterialization(_) => {
            panic!("expected completed composition")
        }
    }
}
fn query(
    trace: &FamilyTraceArena,
    fields: &ProgrammingFieldScope,
) -> Option<crate::FamilyTraceQuery> {
    trace.query_fields_with_base(trace.root().unwrap(), fields)
}

#[test]
fn mixed_graph_then_partial_whole_mask_then_second_graph_preserves_trace_and_request_isolation() {
    let capture = Uuid::from_u128(700);
    let base = angles(0., 0.);
    let samples = [
        history(target(41, [8., 4., 2.]), target(42, [16., 8., 6.]), 1, 1.),
        whole_mask(angles(80., 40.), 2, 0.5),
        history(target(43, [6., 2., 4.]), angles(20., 30.), 3, 0.5),
    ];
    let unavailable_adoption = |_: &AttributeValue, _: &DynamicValueAddress| {
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&unavailable_adoption),
        ..Default::default()
    };
    let frame = Frame::blocked();
    let mut evaluation = begin_retained_position_composition(
        capture,
        &base,
        &samples,
        &context,
        RetainedFamilyCompositionScratch::default(),
        true,
    )
    .unwrap();
    let first = needed(evaluation.advance(capture, &context, &frame).unwrap());
    assert_eq!(first.requirement, TransitionRequirement::LiveTargetPoints);
    assert!(
        matches!(first.operation, PositionCompositionOperation::Base { rank: actual_rank, .. } if actual_rank == rank(1))
    );
    let calls = frame.calls.get();
    assert!(
        evaluation
            .advance(Uuid::from_u128(701), &context, &frame)
            .is_err()
    );
    assert!(
        evaluation
            .resume_materialization(
                Uuid::from_u128(701),
                first.request_id,
                target(0, [12., 6., 4.]),
                None
            )
            .is_err()
    );
    assert!(
        evaluation
            .resume_materialization(
                capture,
                Uuid::from_u128(999),
                target(0, [12., 6., 4.]),
                None
            )
            .is_err()
    );
    let repeated = needed(evaluation.advance(capture, &context, &frame).unwrap());
    assert_eq!(repeated.request_id, first.request_id);
    assert_eq!(
        frame.calls.get(),
        calls,
        "waiting must not replay graph callbacks"
    );
    evaluation
        .resume_materialization(capture, first.request_id, target(0, [12., 6., 4.]), None)
        .unwrap();
    let mask = needed(evaluation.advance(capture, &context, &frame).unwrap());
    assert_ne!(mask.request_id, first.request_id);
    assert!(
        matches!(&mask.operation, PositionCompositionOperation::MaskAdoption { rank: actual_rank, from, .. } if *actual_rank == rank(2) && from == &target(0, [12., 6., 4.]))
    );
    // This is valid Position, but it is the wrong representation for this exact Angle mask.
    assert!(
        evaluation
            .resume_materialization(capture, mask.request_id, target(0, [12., 6., 4.]), None)
            .is_err()
    );
    assert!(
        evaluation
            .resume_materialization(capture, first.request_id, angles(12., 6.), None)
            .is_err()
    );
    let repeated_mask = needed(evaluation.advance(capture, &context, &frame).unwrap());
    assert_eq!(repeated_mask.request_id, mask.request_id);
    assert_eq!(
        frame.calls.get(),
        calls,
        "mask response rejection must not revisit the graph"
    );
    evaluation
        .resume_materialization(capture, mask.request_id, angles(12., 6.), None)
        .unwrap();
    let second = needed(evaluation.advance(capture, &context, &frame).unwrap());
    assert!(
        matches!(second.operation, PositionCompositionOperation::Base { rank: actual_rank, .. } if actual_rank == rank(3))
    );
    assert_ne!(second.request_id, mask.request_id);
    assert!(
        evaluation
            .resume_materialization(
                capture,
                second.request_id,
                AttributeValue::Normalized(0.4),
                None
            )
            .is_err()
    );
    let second_calls = frame.calls.get();
    assert_eq!(
        needed(evaluation.advance(capture, &context, &frame).unwrap()).request_id,
        second.request_id
    );
    assert_eq!(frame.calls.get(), second_calls);
    evaluation
        .resume_materialization(capture, second.request_id, angles(13., 16.), None)
        .unwrap();
    let actual = complete(evaluation.advance(capture, &context, &frame).unwrap());
    assert_eq!(actual, angles(29.5, 19.5));
    assert!(evaluation.pending_materialization().is_none());
    assert_eq!(
        complete(evaluation.advance(capture, &context, &frame).unwrap()),
        actual
    );
    assert!(
        evaluation
            .resume_materialization(capture, second.request_id, angles(99., 99.), None)
            .is_err()
    );

    let adoption = |value: &AttributeValue, _: &DynamicValueAddress| Ok(angle_value(value));
    let available_context = FamilyCompositionContext {
        resolve_adoption: Some(&adoption),
        ..Default::default()
    };
    let mut reference_scratch = RetainedFamilyCompositionScratch::default();
    let expected = compose_retained_dynamic_family_traced(
        ProgrammingOwner::Position,
        &base,
        &samples,
        &available_context,
        &Frame::available(),
        &mut reference_scratch,
    )
    .unwrap();
    assert_eq!(actual, expected);
    let fields = ProgrammingFieldScope::for_value(ProgrammingOwner::Position, &actual).unwrap();
    assert_eq!(
        query(evaluation.family_trace(), &fields),
        query(reference_scratch.family_trace(), &fields)
    );
    assert_eq!(
        evaluation
            .family_trace()
            .control_sources_for_fields(evaluation.family_trace().root().unwrap(), &fields),
        reference_scratch
            .family_trace()
            .control_sources_for_fields(reference_scratch.family_trace().root().unwrap(), &fields)
    );
}

#[test]
fn last_full_whole_mask_skips_invisible_lower_graphs_and_partial_masks() {
    let capture = Uuid::from_u128(710);
    let base = target(44, [1., 2., 3.]);
    let samples = [
        history(target(45, [2., 4., 6.]), angles(60., 20.), 1, 1.),
        whole_mask(angles(70., 30.), 2, 0.5),
        history(target(46, [3., 5., 7.]), angles(80., 40.), 3, 1.),
        whole_mask(angles(90., 10.), 4, 1.),
        history(angles(10., 20.), angles(30., 40.), 5, 0.5),
    ];
    let frame = Frame::blocked();
    let adoption_calls = Cell::new(0);
    let adoption = |_: &AttributeValue, _: &DynamicValueAddress| {
        adoption_calls.set(adoption_calls.get() + 1);
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&adoption),
        ..Default::default()
    };
    let mut evaluation = begin_retained_position_composition(
        capture,
        &base,
        &samples,
        &context,
        RetainedFamilyCompositionScratch::default(),
        true,
    )
    .unwrap();
    let result = complete(evaluation.advance(capture, &context, &frame).unwrap());
    assert_eq!(result, angles(55., 20.));
    assert_eq!(
        frame.calls.get(),
        0,
        "hidden lower graphs must not require geometry"
    );
    assert_eq!(
        adoption_calls.get(),
        0,
        "hidden masks must not adopt their underlay"
    );
    let fields = ProgrammingFieldScope::for_value(ProgrammingOwner::Position, &result).unwrap();
    let contributors = query(evaluation.family_trace(), &fields).unwrap();
    assert!(
        contributors
            .sources
            .iter()
            .all(|source| source.source.rank.stable_order >= 4)
    );
    let mut scratch = RetainedFamilyCompositionScratch::default();
    assert_eq!(
        compose_retained_dynamic_family_traced(
            ProgrammingOwner::Position,
            &base,
            &samples,
            &context,
            &frame,
            &mut scratch
        )
        .unwrap(),
        result
    );
    assert_eq!(
        query(evaluation.family_trace(), &fields),
        query(scratch.family_trace(), &fields)
    );
}

#[test]
fn partial_target_mask_rejects_another_target_reference_without_consuming_pending_request() {
    let capture = Uuid::from_u128(720);
    let base = angles(5., 7.);
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Target {
            reference: Some(TargetReference::Point {
                point_id: Uuid::from_u128(47),
            }),
        },
        component: None,
    };
    let samples = [FamilySample::new(
        Arc::new(CompiledDynamicValueAddress::new(address, None).unwrap()),
        DynamicValue::Family(target(47, [4., 6., 8.])),
        rank(1),
        0.5,
    )
    .unwrap()
    .into_fix_at()
    .into()];
    let context = FamilyCompositionContext::default();
    let frame = Frame::blocked();
    let mut evaluation = begin_retained_position_composition(
        capture,
        &base,
        &samples,
        &context,
        RetainedFamilyCompositionScratch::default(),
        true,
    )
    .unwrap();
    let pending = needed(evaluation.advance(capture, &context, &frame).unwrap());
    assert!(
        matches!(&pending.operation, PositionCompositionOperation::MaskAdoption { address, .. }
        if matches!(address.representation, DynamicFamilyRepresentation::Target { reference: Some(TargetReference::Point { point_id }) } if point_id == Uuid::from_u128(47)))
    );
    assert!(
        evaluation
            .resume_materialization(capture, pending.request_id, angles(5., 7.), None)
            .is_err()
    );
    assert!(
        evaluation
            .resume_materialization(capture, pending.request_id, target(48, [2., 4., 6.]), None)
            .is_err()
    );
    assert_eq!(
        evaluation.pending_materialization().unwrap().request_id,
        pending.request_id
    );
    assert_eq!(
        needed(evaluation.advance(capture, &context, &frame).unwrap()).request_id,
        pending.request_id
    );
    assert_eq!(frame.calls.get(), 0);
    evaluation
        .resume_materialization(capture, pending.request_id, target(47, [2., 4., 6.]), None)
        .unwrap();
    assert_eq!(
        complete(evaluation.advance(capture, &context, &frame).unwrap()),
        target(47, [3., 5., 7.])
    );
}

#[path = "completion_tests.rs"]
mod completion_tests;

#[test]
fn mask_adoption_preserves_exact_transfer_and_unknown_conversion_stays_passive() {
    let capture = Uuid::from_u128(730);
    let base = angles(0., 0.);
    let samples = [
        whole_mask(target(0, [10., 20., 3.]), 1, 1.),
        whole_mask(angles(90., 40.), 2, 0.5),
    ];
    let context = FamilyCompositionContext::default();
    let fields = ProgrammingFieldScope::new([ProgrammingTraceField::Pan]);
    let make = || {
        begin_retained_position_composition(
            capture,
            &base,
            &samples,
            &context,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap()
    };
    let mut exact = make();
    let pending = needed(exact.advance(capture, &context, &Frame::blocked()).unwrap());
    assert!(matches!(
        pending.operation,
        PositionCompositionOperation::MaskAdoption { .. }
    ));
    let foreign = ProgrammingTransitionTrace {
        from: ProgrammingFieldTransfer {
            identity: ProgrammingFieldScope::new([ProgrammingTraceField::Uv]),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        exact
            .resume_materialization(capture, pending.request_id, angles(10., 20.), Some(foreign))
            .is_err()
    );
    assert_eq!(
        exact.pending_materialization().unwrap().request_id,
        pending.request_id
    );
    let transfer = ProgrammingTransitionTrace {
        from: ProgrammingFieldTransfer {
            remap: vec![
                (ProgrammingTraceField::TargetX, ProgrammingTraceField::Pan),
                (ProgrammingTraceField::TargetY, ProgrammingTraceField::Tilt),
            ]
            .into(),
            ..Default::default()
        },
        ..Default::default()
    };
    exact
        .resume_materialization(
            capture,
            pending.request_id,
            angles(10., 20.),
            Some(transfer),
        )
        .unwrap();
    assert_eq!(
        complete(exact.advance(capture, &context, &Frame::blocked()).unwrap()),
        angles(50., 30.)
    );
    let contributors =
        query(exact.family_trace(), &fields).expect("explicit conversion certifies attribution");
    assert!(
        contributors
            .sources
            .iter()
            .any(|source| source.source.rank.stable_order == 1)
    );
    assert!(
        contributors
            .sources
            .iter()
            .any(|source| source.source.rank.stable_order == 2)
    );
    let mut unknown = make();
    let pending = needed(
        unknown
            .advance(capture, &context, &Frame::blocked())
            .unwrap(),
    );
    unknown
        .resume_materialization(capture, pending.request_id, angles(10., 20.), None)
        .unwrap();
    complete(
        unknown
            .advance(capture, &context, &Frame::blocked())
            .unwrap(),
    );
    assert!(
        query(unknown.family_trace(), &fields).is_none(),
        "value-only adoption cannot certify prior fields"
    );
}
