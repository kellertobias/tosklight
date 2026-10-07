//! Child operand views stop at actual active operations without changing parent evaluation.
use super::*;
use std::cell::Cell;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [4., 2., 3.],
    )))
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
struct Frame(Cell<usize>);
impl WholeFamilyExpressionFrameResolver for Frame {
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
fn ready(progress: PositionSegmentProgress) -> AttributeValue {
    let PositionSegmentProgress::OperandReady(value) = progress else {
        panic!("exact operand stop")
    };
    value
}
fn needed(progress: PositionSegmentProgress) -> PositionSegmentRequest {
    let PositionSegmentProgress::NeedsMaterialization(request) = progress else {
        panic!("ordinary wait")
    };
    request
}
fn complete(progress: PositionSegmentProgress) -> AttributeValue {
    let PositionSegmentProgress::Complete(value) = progress else {
        panic!("ordinary completion")
    };
    value.value
}
fn prefix_samples() -> [FamilySample; 3] {
    [
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
    ]
}

#[test]
fn flushed_adoption_input_includes_lower_edits_and_excludes_current_and_higher_suffix() {
    let base = angles(10., 20.);
    let samples = prefix_samples();
    let mut trace = FamilyTraceArena::default();
    let prefix = trace.base();
    let mut scratch = ComponentCompositionScratch::default();
    let frame = Frame::default();
    let context = FamilyCompositionContext::default();
    let mut child =
        PositionSegmentEvaluation::begin(&base, Some(prefix), &samples, &[0, 1, 2], &base, true);
    child
        .stop_at(1, PositionSegmentOperand::AdoptionInput)
        .unwrap();
    let operand = ready(
        child
            .advance(&context, &frame, &mut scratch, &mut trace)
            .unwrap(),
    );
    assert_eq!(operand, angles(90., 20.));
    assert!(
        scratch.components.is_empty(),
        "lower edit batch is flushed before adoption"
    );
    let retained_trace = child.trace;
    assert_ne!(retained_trace, Some(prefix));
    assert_eq!(
        trace
            .sources_for_component(retained_trace.unwrap(), ProgrammingComponent::Pan)
            .unwrap()
            .len(),
        1
    );
    for _ in 0..3 {
        assert_eq!(
            ready(
                child
                    .advance(&context, &frame, &mut scratch, &mut trace)
                    .unwrap()
            ),
            operand
        );
        assert_eq!(child.trace, retained_trace);
    }
    assert_eq!(frame.0.get(), 0);
    assert!(child.pending.is_none());
    assert!(child.completed.is_none());
    let mut parent =
        PositionSegmentEvaluation::begin(&base, None, &samples, &[0, 1, 2], &base, false);
    let actual = complete(
        parent
            .advance(
                &context,
                &frame,
                &mut ComponentCompositionScratch::default(),
                &mut FamilyTraceArena::default(),
            )
            .unwrap(),
    );
    assert_eq!(actual, angles(60., 42.5));
    assert_eq!(
        actual,
        compose_segment(
            &base,
            None,
            &samples,
            &[0, 1, 2],
            &context,
            &base,
            &mut ComponentCompositionScratch::default(),
            None
        )
        .unwrap()
        .value
    );
}

#[test]
fn synchronous_transition_operand_stops_before_interpolation_or_higher_edits() {
    let base = angles(10., 20.);
    let samples = prefix_samples();
    let frame = Frame::default();
    let context = FamilyCompositionContext::default();
    for (role, expected) in [
        (PositionSegmentOperand::TransitionFrom, angles(90., 20.)),
        (PositionSegmentOperand::TransitionTo, angles(30., 40.)),
    ] {
        let mut child =
            PositionSegmentEvaluation::begin(&base, None, &samples, &[0, 1, 2], &base, false);
        child.stop_at(1, role).unwrap();
        let mut scratch = ComponentCompositionScratch::default();
        let mut trace = FamilyTraceArena::default();
        assert_eq!(
            ready(
                child
                    .advance(&context, &frame, &mut scratch, &mut trace)
                    .unwrap()
            ),
            expected
        );
        assert_eq!(
            child.value,
            angles(90., 20.),
            "the stopped transition has not blended"
        );
        assert_eq!(
            ready(
                child
                    .advance(&context, &frame, &mut scratch, &mut trace)
                    .unwrap()
            ),
            expected
        );
        assert!(scratch.components.is_empty());
    }
    assert_eq!(frame.0.get(), 0);
}

#[test]
fn adoption_stop_precedes_blocked_callback_and_abandoned_child_does_not_change_parent() {
    let base = target();
    let samples = [
        sample(
            Some(ProgrammingComponent::Pan),
            DynamicValue::Scalar(90.),
            1,
            1.,
        ),
        sample(
            Some(ProgrammingComponent::Tilt),
            DynamicValue::Scalar(40.),
            2,
            1.,
        ),
    ];
    let calls = Cell::new(0);
    let adoption = |from: &AttributeValue, _: &DynamicValueAddress| {
        calls.set(calls.get() + 1);
        assert_eq!(from, &base);
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&adoption),
        ..Default::default()
    };
    let frame = Frame::default();
    let mut child = PositionSegmentEvaluation::begin(&base, None, &samples, &[0, 1], &base, false);
    child
        .stop_at(0, PositionSegmentOperand::AdoptionInput)
        .unwrap();
    let mut scratch = ComponentCompositionScratch::default();
    let mut trace = FamilyTraceArena::default();
    for _ in 0..3 {
        assert_eq!(
            ready(
                child
                    .advance(&context, &frame, &mut scratch, &mut trace)
                    .unwrap()
            ),
            base
        );
    }
    assert_eq!(calls.get(), 0);
    assert!(child.resume(0, angles(10., 20.), None).is_err());
    drop(child);
    let mut parent = PositionSegmentEvaluation::begin(&base, None, &samples, &[0, 1], &base, false);
    let request = needed(
        parent
            .advance(&context, &frame, &mut scratch, &mut trace)
            .unwrap(),
    );
    assert!(matches!(
        request.operation,
        PositionSegmentOperation::Adoption { .. }
    ));
    assert_eq!(calls.get(), 1);
    parent.resume(request.node, angles(10., 20.), None).unwrap();
    assert_eq!(
        complete(
            parent
                .advance(&context, &frame, &mut scratch, &mut trace)
                .unwrap()
        ),
        angles(90., 40.)
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn transition_operands_wait_for_required_adoption_once_then_stop_before_blend() {
    let base = target();
    let samples = [
        sample(None, DynamicValue::Family(angles(90., 40.)), 1, 0.5),
        sample(
            Some(ProgrammingComponent::Tilt),
            DynamicValue::Scalar(80.),
            2,
            1.,
        ),
    ];
    let calls = Cell::new(0);
    let adoption = |_: &AttributeValue, _: &DynamicValueAddress| {
        calls.set(calls.get() + 1);
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&adoption),
        ..Default::default()
    };
    let frame = Frame::default();
    for (role, expected) in [
        (PositionSegmentOperand::TransitionFrom, angles(10., 20.)),
        (PositionSegmentOperand::TransitionTo, angles(90., 40.)),
    ] {
        let before = calls.get();
        let mut child =
            PositionSegmentEvaluation::begin(&base, None, &samples, &[0, 1], &base, false);
        child.stop_at(0, role).unwrap();
        let mut scratch = ComponentCompositionScratch::default();
        let mut trace = FamilyTraceArena::default();
        let request = needed(
            child
                .advance(&context, &frame, &mut scratch, &mut trace)
                .unwrap(),
        );
        assert!(matches!(
            request.operation,
            PositionSegmentOperation::Adoption { .. }
        ));
        assert_eq!(
            needed(
                child
                    .advance(&context, &frame, &mut scratch, &mut trace)
                    .unwrap()
            )
            .node,
            request.node
        );
        child.resume(request.node, angles(10., 20.), None).unwrap();
        assert_eq!(
            ready(
                child
                    .advance(&context, &frame, &mut scratch, &mut trace)
                    .unwrap()
            ),
            expected
        );
        assert_eq!(
            ready(
                child
                    .advance(&context, &frame, &mut scratch, &mut trace)
                    .unwrap()
            ),
            expected
        );
        assert_eq!(child.value, angles(10., 20.));
        assert_eq!(calls.get(), before + 1);
        assert_eq!(frame.0.get(), 0);
        assert!(scratch.components.is_empty());
    }
}

#[test]
fn covered_removed_and_direct_replacement_samples_do_not_authorize_a_stop() {
    let base = target();
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
    ];
    let frame = Frame::default();
    let context = FamilyCompositionContext::default();
    for (ordered, index, role) in [
        (&[0, 1, 2][..], 0, PositionSegmentOperand::AdoptionInput),
        (&[1, 2][..], 0, PositionSegmentOperand::AdoptionInput),
        (&[0, 1, 2][..], 1, PositionSegmentOperand::TransitionTo),
    ] {
        let mut child =
            PositionSegmentEvaluation::begin(&base, None, &samples, ordered, &base, false);
        child.stop_at(index, role).unwrap();
        assert_eq!(
            complete(
                child
                    .advance(
                        &context,
                        &frame,
                        &mut ComponentCompositionScratch::default(),
                        &mut FamilyTraceArena::default()
                    )
                    .unwrap()
            ),
            angles(120., 40.)
        );
        assert!(child.operand_ready.is_none());
    }
    assert_eq!(frame.0.get(), 0);
}

#[test]
fn stop_configuration_cannot_redirect_running_or_completed_work() {
    let base = angles(10., 20.);
    let samples = prefix_samples();
    let mut child =
        PositionSegmentEvaluation::begin(&base, None, &samples, &[0, 1, 2], &base, false);
    assert!(
        child
            .stop_at(3, PositionSegmentOperand::AdoptionInput)
            .is_err()
    );
    child
        .stop_at(1, PositionSegmentOperand::TransitionTo)
        .unwrap();
    assert!(
        child
            .stop_at(0, PositionSegmentOperand::AdoptionInput)
            .is_err()
    );
    let frame = Frame::default();
    assert_eq!(
        ready(
            child
                .advance(
                    &FamilyCompositionContext::default(),
                    &frame,
                    &mut ComponentCompositionScratch::default(),
                    &mut FamilyTraceArena::default()
                )
                .unwrap()
        ),
        angles(30., 40.)
    );
    assert!(
        child
            .stop_at(0, PositionSegmentOperand::AdoptionInput)
            .is_err()
    );
    assert!(child.resume(0, angles(90., 90.), None).is_err());
}
