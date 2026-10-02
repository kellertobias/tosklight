use super::*;
use std::cell::Cell;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target(reference: TargetReference) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(reference, [4., 2., 3.])))
}
fn sample(
    component: Option<ProgrammingComponent>,
    value: DynamicValue,
    order: u128,
    mix: f32,
) -> FamilySample {
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Angles,
                    component,
                },
                None,
            )
            .unwrap(),
        ),
        value,
        FamilySampleRank {
            priority: 10,
            changed_at_millis: 100,
            changed_at_submillis_nanos: 0,
            stable_order: order,
            identity: FamilySampleIdentity::Fixed {
                source: FamilyFixedSampleSource::Programmer,
                row_index: order as usize,
            },
        },
        mix,
    )
    .unwrap()
    .into_fix_at()
}
#[derive(Default)]
struct Frame {
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for Frame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        Err(TransitionError::Requires(requirement))
    }
}
fn needed(progress: PositionSegmentProgress) -> PositionSegmentRequest {
    let PositionSegmentProgress::NeedsMaterialization(request) = progress else {
        panic!("expected request")
    };
    request
}
fn complete(progress: PositionSegmentProgress) -> retained_family::TracedValue {
    let PositionSegmentProgress::Complete(value) = progress else {
        panic!("expected completed segment")
    };
    value
}

#[test]
fn angle_components_wait_once_with_exact_target_then_preserve_pair_trace_and_mix() {
    let base = target(TargetReference::Origin);
    let samples = [
        sample(
            Some(ProgrammingComponent::Pan),
            DynamicValue::Scalar(90.),
            1,
            0.5,
        ),
        sample(
            Some(ProgrammingComponent::Tilt),
            DynamicValue::Scalar(40.),
            2,
            1.,
        ),
    ];
    let calls = Cell::new(0);
    let adoption = |from: &AttributeValue, address: &DynamicValueAddress| {
        calls.set(calls.get() + 1);
        assert_eq!(from, &base);
        assert_eq!(address.component, Some(ProgrammingComponent::Pan));
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&adoption),
        ..Default::default()
    };
    let mut scratch = ComponentCompositionScratch::default();
    let mut trace = FamilyTraceArena::default();
    // This is an already-completed lower prefix, which must survive waiting untouched.
    let prefix = trace.base();
    let mut evaluation =
        PositionSegmentEvaluation::begin(&base, Some(prefix), &samples, &[0, 1], &base, true);
    let frame = Frame::default();
    let request = needed(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .unwrap(),
    );
    assert_eq!(request.requirement, TransitionRequirement::LiveJointAngles);
    let PositionSegmentOperation::Adoption { from, address } = &request.operation else {
        panic!()
    };
    assert_eq!(from, &base);
    assert_eq!(address.component, Some(ProgrammingComponent::Pan));
    assert_eq!(calls.get(), 1);
    assert_eq!(evaluation.trace, Some(prefix));
    for _ in 0..3 {
        let waiting = needed(
            evaluation
                .advance(&context, &frame, &mut scratch, &mut trace)
                .unwrap(),
        );
        assert_eq!(waiting.node, request.node);
    }
    assert!(
        evaluation
            .resume(request.node + 1, angles(10., 20.), None)
            .is_err()
    );
    assert!(
        evaluation
            .resume(request.node, AttributeValue::Normalized(0.5), None)
            .is_err()
    );
    assert!(evaluation.resume(request.node, base.clone(), None).is_err());
    assert_eq!(
        needed(
            evaluation
                .advance(&context, &frame, &mut scratch, &mut trace)
                .unwrap()
        )
        .node,
        request.node
    );
    evaluation
        .resume(request.node, angles(10., 20.), None)
        .unwrap();
    let result = complete(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .unwrap(),
    );
    assert_eq!(result.value, angles(50., 40.));
    assert!(scratch.components.is_empty());
    assert_eq!(calls.get(), 1);
    assert_eq!(frame.calls.get(), 0);
    trace.set_root(result.trace.unwrap());
    assert!(
        trace
            .sources_for_component(result.trace.unwrap(), ProgrammingComponent::Pan)
            .is_none()
    );
    assert_eq!(
        trace
            .sources_for_component(result.trace.unwrap(), ProgrammingComponent::Tilt)
            .unwrap()
            .len(),
        1
    );
    let again = complete(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .unwrap(),
    );
    assert_eq!(again.trace, result.trace);
    assert_eq!(again.value, result.value);
    let expected = compose_segment(
        &base,
        None,
        &samples,
        &[0, 1],
        &FamilyCompositionContext {
            adopted_base: Some(&angles(10., 20.)),
            ..Default::default()
        },
        &base,
        &mut ComponentCompositionScratch::default(),
        None,
    )
    .unwrap();
    assert_eq!(result.value, expected.value);
}

#[test]
fn complete_component_batches_and_whole_activation_match_existing_arithmetic() {
    let base = angles(10., 20.);
    let samples = [
        sample(
            Some(ProgrammingComponent::Pan),
            DynamicValue::Scalar(90.),
            1,
            1.,
        ),
        sample(None, DynamicValue::Family(angles(30., 40.)), 2, 0.5),
        sample(
            Some(ProgrammingComponent::Tilt),
            DynamicValue::Scalar(80.),
            3,
            0.25,
        ),
    ];
    let mut trace = FamilyTraceArena::default();
    let prefix = trace.base();
    let context = FamilyCompositionContext::default();
    let expected = compose_segment(
        &base,
        None,
        &samples,
        &[0, 1, 2],
        &context,
        &base,
        &mut ComponentCompositionScratch::default(),
        None,
    )
    .unwrap();
    let mut evaluation =
        PositionSegmentEvaluation::begin(&base, Some(prefix), &samples, &[0, 1, 2], &base, true);
    let result = complete(
        evaluation
            .advance(
                &context,
                &Frame::default(),
                &mut ComponentCompositionScratch::default(),
                &mut trace,
            )
            .unwrap(),
    );
    assert_eq!(result.value, angles(60., 42.5));
    assert_eq!(result.value, expected.value);
    trace.set_root(result.trace.unwrap());
    assert_eq!(
        trace
            .sources_for_component(result.trace.unwrap(), ProgrammingComponent::Pan)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        trace
            .sources_for_component(result.trace.unwrap(), ProgrammingComponent::Tilt)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn partial_whole_target_transition_owns_exact_endpoints_and_is_not_replayed() {
    let base = target(TargetReference::Origin);
    let incoming = target(TargetReference::Point {
        point_id: Uuid::from_u128(77),
    });
    let sample = FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Target { reference: None },
                    component: None,
                },
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Family(incoming.clone()),
        sample(None, DynamicValue::Family(angles(0., 0.)), 1, 1.).rank,
        0.25,
    )
    .unwrap();
    let mut evaluation =
        PositionSegmentEvaluation::begin(&base, None, &[sample], &[0], &base, false);
    let mut scratch = ComponentCompositionScratch::default();
    let mut trace = FamilyTraceArena::default();
    let frame = Frame::default();
    let context = FamilyCompositionContext::default();
    let request = needed(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .unwrap(),
    );
    assert_eq!(request.requirement, TransitionRequirement::LiveTargetPoints);
    let PositionSegmentOperation::Transition { from, to, progress } = &request.operation else {
        panic!()
    };
    assert_eq!(from, &base);
    assert_eq!(to, &incoming);
    assert_eq!(*progress, 0.25);
    assert_eq!(
        needed(
            evaluation
                .advance(&context, &frame, &mut scratch, &mut trace)
                .unwrap()
        )
        .node,
        request.node
    );
    assert_eq!(frame.calls.get(), 1);
    assert!(
        evaluation
            .resume(request.node, AttributeValue::Normalized(0.1), None)
            .is_err()
    );
    evaluation
        .resume(request.node, angles(35., 25.), None)
        .unwrap();
    assert_eq!(
        complete(
            evaluation
                .advance(&context, &frame, &mut scratch, &mut trace)
                .unwrap()
        )
        .value,
        angles(35., 25.)
    );
    assert_eq!(frame.calls.get(), 1);
}

#[test]
fn partial_whole_adoption_retains_original_underlay_and_applies_activation_after_resume() {
    let base = target(TargetReference::Origin);
    let samples = [sample(None, DynamicValue::Family(angles(90., 40.)), 1, 0.5)];
    let mut evaluation =
        PositionSegmentEvaluation::begin(&base, None, &samples, &[0], &base, false);
    let mut scratch = ComponentCompositionScratch::default();
    let mut trace = FamilyTraceArena::default();
    let context = FamilyCompositionContext::default();
    let frame = Frame::default();
    let request = needed(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .unwrap(),
    );
    let PositionSegmentOperation::Adoption { from, address } = &request.operation else {
        panic!()
    };
    assert_eq!(from, &base);
    assert_eq!(address.component, None);
    evaluation
        .resume(request.node, angles(10., 20.), None)
        .unwrap();
    assert_eq!(
        complete(
            evaluation
                .advance(&context, &frame, &mut scratch, &mut trace)
                .unwrap()
        )
        .value,
        angles(50., 30.)
    );
}

#[test]
fn callback_unwind_is_terminal_and_cannot_repeat_side_effects() {
    let base = target(TargetReference::Origin);
    let samples = [sample(
        Some(ProgrammingComponent::Pan),
        DynamicValue::Scalar(20.),
        1,
        1.,
    )];
    let calls = Cell::new(0);
    let adoption =
        |_: &AttributeValue, _: &DynamicValueAddress| -> Result<AttributeValue, TransitionError> {
            calls.set(calls.get() + 1);
            panic!("captured callback failure")
        };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&adoption),
        ..Default::default()
    };
    let mut evaluation =
        PositionSegmentEvaluation::begin(&base, None, &samples, &[0], &base, false);
    let mut scratch = ComponentCompositionScratch::default();
    let mut trace = FamilyTraceArena::default();
    let frame = Frame::default();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = evaluation.advance(&context, &frame, &mut scratch, &mut trace);
        }))
        .is_err()
    );
    assert!(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .is_err()
    );
    assert!(evaluation.resume(0, angles(0., 0.), None).is_err());
    assert_eq!(calls.get(), 1);
}

#[test]
fn covered_component_and_whole_replacement_do_not_request_discarded_adoption() {
    let base = target(TargetReference::Origin);
    let samples = [
        sample(
            Some(ProgrammingComponent::Pan),
            DynamicValue::Scalar(90.),
            1,
            1.,
        ),
        sample(None, DynamicValue::Family(angles(30., 40.)), 2, 1.),
        sample(
            Some(ProgrammingComponent::Pan),
            DynamicValue::Scalar(120.),
            3,
            1.,
        ),
        sample(
            Some(ProgrammingComponent::Pan),
            DynamicValue::Scalar(180.),
            4,
            1.,
        ),
    ];
    let frame = Frame::default();
    let mut evaluation =
        PositionSegmentEvaluation::begin(&base, None, &samples, &[0, 1, 2, 3], &base, false);
    let result = complete(
        evaluation
            .advance(
                &FamilyCompositionContext::default(),
                &frame,
                &mut ComponentCompositionScratch::default(),
                &mut FamilyTraceArena::default(),
            )
            .unwrap(),
    );
    assert_eq!(result.value, angles(180., 40.));
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn completed_whole_value_and_trace_survive_a_later_component_adoption_wait() {
    let base = target(TargetReference::Origin);
    let point = TargetReference::Point {
        point_id: Uuid::from_u128(77),
    };
    let incoming = target(point);
    let rank = sample(None, DynamicValue::Family(angles(0., 0.)), 1, 1.).rank;
    let whole = FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Target { reference: None },
                    component: None,
                },
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Family(incoming),
        rank,
        0.25,
    )
    .unwrap();
    let component = FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Target {
                        reference: Some(point),
                    },
                    component: Some(ProgrammingComponent::TargetX),
                },
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Scalar(9.),
        sample(None, DynamicValue::Family(angles(0., 0.)), 2, 1.).rank,
        1.,
    )
    .unwrap();
    let mut trace = FamilyTraceArena::default();
    let prefix = trace.base();
    let mut evaluation = PositionSegmentEvaluation::begin(
        &base,
        Some(prefix),
        &[whole, component],
        &[0, 1],
        &base,
        true,
    );
    let mut scratch = ComponentCompositionScratch::default();
    let frame = Frame::default();
    let adoption_calls = Cell::new(0);
    let adoption = |from: &AttributeValue, _: &DynamicValueAddress| {
        adoption_calls.set(adoption_calls.get() + 1);
        assert_eq!(from, &angles(35., 25.));
        Err(TransitionError::Requires(
            TransitionRequirement::LiveTargetPoints,
        ))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&adoption),
        ..Default::default()
    };
    let first = needed(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .unwrap(),
    );
    // Initial whole Target interpolation must not call the component adoption callback.
    assert!(matches!(
        first.operation,
        PositionSegmentOperation::Transition { .. }
    ));
    evaluation
        .resume(first.node, angles(35., 25.), None)
        .unwrap();
    let second = needed(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .unwrap(),
    );
    assert_ne!(first.node, second.node);
    let saved_trace = evaluation.trace;
    assert_ne!(saved_trace, Some(prefix));
    let PositionSegmentOperation::Adoption { from, address } = &second.operation else {
        panic!()
    };
    assert_eq!(from, &angles(35., 25.));
    assert_eq!(address.component, Some(ProgrammingComponent::TargetX));
    assert!(
        evaluation
            .resume(second.node, angles(10., 20.), None)
            .is_err()
    );
    for _ in 0..3 {
        assert_eq!(
            needed(
                evaluation
                    .advance(&context, &frame, &mut scratch, &mut trace)
                    .unwrap()
            )
            .node,
            second.node
        );
        assert_eq!(evaluation.trace, saved_trace);
        assert_eq!(evaluation.value, angles(35., 25.));
    }
    evaluation.resume(second.node, target(point), None).unwrap();
    let result = complete(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .unwrap(),
    );
    assert_eq!(
        result.value,
        AttributeValue::Position(Arc::new(PositionIntent::target(point, [9., 2., 3.])))
    );
    assert_eq!(frame.calls.get(), 1);
    assert_eq!(adoption_calls.get(), 1);
    assert_eq!(
        trace
            .sources_for_component(result.trace.unwrap(), ProgrammingComponent::TargetX)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn foreign_color_transfer_cannot_consume_the_pending_position_request() {
    let base = target(TargetReference::Origin);
    let samples = [sample(
        Some(ProgrammingComponent::Pan),
        DynamicValue::Scalar(90.),
        1,
        0.5,
    )];
    let mut trace = FamilyTraceArena::default();
    let prefix = trace.base();
    let mut evaluation =
        PositionSegmentEvaluation::begin(&base, Some(prefix), &samples, &[0], &base, true);
    let mut scratch = ComponentCompositionScratch::default();
    let frame = Frame::default();
    let context = FamilyCompositionContext::default();
    let request = needed(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .unwrap(),
    );
    let foreign_identity = ProgrammingTransitionTrace {
        from: ProgrammingFieldTransfer {
            identity: ProgrammingFieldScope::new([ProgrammingTraceField::ColorXyz]),
            ..Default::default()
        },
        ..Default::default()
    };
    let foreign_remap = ProgrammingTransitionTrace {
        to: ProgrammingFieldTransfer {
            remap: vec![(
                ProgrammingTraceField::Pan,
                ProgrammingTraceField::ColorRecipeRed,
            )]
            .into(),
            ..Default::default()
        },
        ..Default::default()
    };
    for transfer in [foreign_identity, foreign_remap] {
        assert!(
            evaluation
                .resume(request.node, angles(10., 20.), Some(transfer))
                .is_err()
        );
        assert_eq!(evaluation.value, base);
        assert_eq!(evaluation.trace, Some(prefix));
        assert_eq!(
            needed(
                evaluation
                    .advance(&context, &frame, &mut scratch, &mut trace)
                    .unwrap()
            )
            .node,
            request.node
        );
    }
    evaluation
        .resume(request.node, angles(10., 20.), None)
        .unwrap();
    assert_eq!(
        complete(
            evaluation
                .advance(&context, &frame, &mut scratch, &mut trace)
                .unwrap()
        )
        .value,
        angles(50., 20.)
    );
}

#[test]
fn immediate_frame_result_rejects_a_foreign_transfer_before_publishing_value_or_trace() {
    struct ForeignFrame(Cell<usize>);
    impl WholeFamilyExpressionFrameResolver for ForeignFrame {
        fn resolve(
            &self,
            _: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            unreachable!("traced result required")
        }
        fn resolve_with_trace(
            &self,
            _: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
            self.0.set(self.0.get() + 1);
            Ok((
                angles(35., 25.),
                Some(ProgrammingTransitionTrace {
                    from: ProgrammingFieldTransfer {
                        remap: vec![(ProgrammingTraceField::Uv, ProgrammingTraceField::Tilt)]
                            .into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            ))
        }
    }
    let base = target(TargetReference::Origin);
    let incoming = target(TargetReference::Point {
        point_id: Uuid::from_u128(77),
    });
    let whole = FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Target { reference: None },
                    component: None,
                },
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Family(incoming),
        sample(None, DynamicValue::Family(angles(0., 0.)), 1, 1.).rank,
        0.25,
    )
    .unwrap();
    let mut trace = FamilyTraceArena::default();
    let prefix = trace.base();
    let mut evaluation =
        PositionSegmentEvaluation::begin(&base, Some(prefix), &[whole], &[0], &base, true);
    let mut scratch = ComponentCompositionScratch::default();
    let frame = ForeignFrame(Cell::new(0));
    let context = FamilyCompositionContext::default();
    assert!(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .is_err()
    );
    assert_eq!(evaluation.value, base);
    assert_eq!(evaluation.trace, Some(prefix));
    assert!(
        evaluation
            .advance(&context, &frame, &mut scratch, &mut trace)
            .is_err()
    );
    assert_eq!(frame.0.get(), 1);
}
