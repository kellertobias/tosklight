use super::*;
use crate::{DynamicPresetSourceBinding, DynamicValueSourceResolver};
use light_core::FixtureId;
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
fn rank() -> FamilySampleRank {
    FamilySampleRank {
        priority: 10,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: 1,
        identity: FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(2),
            lane_id: Uuid::from_u128(3),
        },
    }
}
struct Current {
    value: Option<AttributeValue>,
    calls: Cell<usize>,
    occurrences: Cell<usize>,
}
impl DynamicValueSourceResolver for Current {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        None
    }
    fn try_current_family_base(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        Ok(self.value.clone())
    }
    fn current_family_occurrence(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Option<crate::DynamicSourceOccurrenceId> {
        self.occurrences.set(self.occurrences.get() + 1);
        None
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
    calls: Cell<usize>,
    available: bool,
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
        if self.available {
            Ok(angles(25., 35.))
        } else {
            Err(TransitionError::Requires(requirement))
        }
    }
}
fn source(arena: &mut FamilyTraceArena, value: AttributeValue) -> TracedValue {
    TracedValue {
        value,
        trace: Some(arena.source(FamilyTraceSource {
            rank: rank(),
            footprint: FamilyTraceFootprint::Whole,
            role: FamilyTraceRole::Authored,
            occurrence: None,
        })),
    }
}
#[test]
fn two_stage_cut_captures_current_once_and_keeps_invalid_resume_waiting() {
    let current = Current {
        value: Some(angles(5., 10.)),
        calls: Cell::new(0),
        occurrences: Cell::new(0),
    };
    let controls = Cell::new(0);
    let control = |_| {
        controls.set(controls.get() + 1);
        FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.5 }
    };
    let context = FamilyCompositionContext {
        endpoint_output: Some(FamilyEndpointOutputContext {
            control: &control,
            target: FixtureId::new(),
            current: &current,
            native_models: None,
        }),
        ..Default::default()
    };
    let frame = Frame {
        calls: Cell::new(0),
        available: false,
    };
    let mut arena = FamilyTraceArena::default();
    let incoming = source(&mut arena, target());
    let underlay = source(&mut arena, angles(0., 0.));
    let mut completion = PositionSourceCompletion::new(
        incoming,
        rank(),
        0.5,
        Some(underlay),
        PositionCompletionKind::Whole,
    )
    .unwrap();
    let CompletionProgress::Needs(request) = completion
        .advance(&context, &frame, &mut arena, true)
        .unwrap()
    else {
        panic!("expected endpoint cut")
    };
    assert_eq!(request.stage, PositionCompletionStage::EndpointOutput);
    assert_eq!(request.from, angles(5., 10.));
    assert!(
        completion
            .resume(PositionCompletionStage::Activation, angles(1., 2.), None)
            .is_err()
    );
    assert!(
        completion
            .resume(request.stage, AttributeValue::Normalized(0.5), None)
            .is_err()
    );
    for _ in 0..3 {
        assert!(matches!(
            completion
                .advance(&context, &frame, &mut arena, true)
                .unwrap(),
            CompletionProgress::Needs(_)
        ));
    }
    assert_eq!(
        (
            controls.get(),
            current.calls.get(),
            current.occurrences.get(),
            frame.calls.get()
        ),
        (1, 1, 1, 1)
    );
    // Endpoint materialization can remain a Target; the separate activation then needs a cut.
    completion.resume(request.stage, target(), None).unwrap();
    let CompletionProgress::Needs(request) = completion
        .advance(&context, &frame, &mut arena, true)
        .unwrap()
    else {
        panic!("expected activation cut")
    };
    assert_eq!(request.stage, PositionCompletionStage::Activation);
    assert_eq!(request.from, angles(0., 0.));
    completion
        .resume(request.stage, angles(45., 55.), None)
        .unwrap();
    for _ in 0..3 {
        let CompletionProgress::Complete(value) = completion
            .advance(&context, &frame, &mut arena, true)
            .unwrap()
        else {
            panic!("expected completion")
        };
        assert_eq!(value.value, angles(45., 55.));
        assert!(value.trace.is_some());
    }
    assert_eq!(
        (
            controls.get(),
            current.calls.get(),
            current.occurrences.get(),
            frame.calls.get()
        ),
        (1, 1, 1, 2)
    );
}
#[test]
fn missing_captured_current_is_passive_and_is_not_replaced_by_underlay() {
    let current = Current {
        value: None,
        calls: Cell::new(0),
        occurrences: Cell::new(0),
    };
    let control = |_| FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.5 };
    let context = FamilyCompositionContext {
        endpoint_output: Some(FamilyEndpointOutputContext {
            control: &control,
            target: FixtureId::new(),
            current: &current,
            native_models: None,
        }),
        ..Default::default()
    };
    let frame = Frame {
        calls: Cell::new(0),
        available: true,
    };
    let mut arena = FamilyTraceArena::default();
    let incoming = source(&mut arena, target());
    let underlay = source(&mut arena, angles(0., 0.));
    let mut completion = PositionSourceCompletion::new(
        incoming,
        rank(),
        0.5,
        Some(underlay),
        PositionCompletionKind::Known,
    )
    .unwrap();
    for _ in 0..3 {
        assert!(matches!(
            completion.advance(&context, &frame, &mut arena, true),
            Err(TransitionError::Requires(_))
        ));
    }
    assert_eq!(current.calls.get(), 1);
    assert_eq!(frame.calls.get(), 0);
    assert!(
        completion
            .resume(
                PositionCompletionStage::EndpointOutput,
                angles(3., 4.),
                None
            )
            .is_err()
    );
}
#[test]
fn immediate_position_completion_matches_existing_endpoint_and_activation_trace() {
    for kind in [
        PositionCompletionKind::Whole,
        PositionCompletionKind::Known,
        PositionCompletionKind::Coupled,
    ] {
        for controlled in [false, true] {
            let current = Current {
                value: Some(angles(5., 10.)),
                calls: Cell::new(0),
                occurrences: Cell::new(0),
            };
            let control = |_| {
                if controlled {
                    FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.4 }
                } else {
                    FamilyEndpointOutputControl::Unchanged
                }
            };
            let context = FamilyCompositionContext {
                endpoint_output: Some(FamilyEndpointOutputContext {
                    control: &control,
                    target: FixtureId::new(),
                    current: &current,
                    native_models: None,
                }),
                ..Default::default()
            };
            let frame = Frame {
                calls: Cell::new(0),
                available: true,
            };
            let mut actual = FamilyTraceArena::default();
            let target = source(&mut actual, angles(90., 45.));
            let underlay = source(&mut actual, angles(0., 0.));
            let mut expected = FamilyTraceArena::default();
            let _ = source(&mut expected, target.value.clone());
            let _ = source(&mut expected, underlay.value.clone());
            let (endpoint, endpoint_node) = endpoint_output::whole(
                rank(),
                ProgrammingOwner::Position,
                target.value.clone(),
                target.trace,
                &context,
                &frame,
                &|_| {
                    Err(TransitionError::Requires(
                        TransitionRequirement::NativeColorModel,
                    ))
                },
                Some(&mut expected),
            )
            .unwrap();
            let transition =
                CompiledProgrammingTransition::new(underlay.value.clone(), endpoint, None).unwrap();
            let (expected_value, transfer) = transition
                .sample_with_trace(ProgrammingOwner::Position, 0.5)
                .unwrap();
            let appearance = if kind == PositionCompletionKind::Coupled && !controlled {
                expected.write(
                    underlay.trace.unwrap(),
                    endpoint_node.unwrap(),
                    FamilyTraceFootprint::Whole,
                    true,
                )
            } else {
                expected.mapped_blend(underlay.trace.unwrap(), endpoint_node.unwrap(), transfer)
            };
            let expected_node = endpoint_output::control_trace(
                rank(),
                FamilyTraceFootprint::Whole,
                appearance,
                underlay.trace,
                &context,
                &mut expected,
            );
            let mut completion =
                PositionSourceCompletion::new(target, rank(), 0.5, Some(underlay), kind).unwrap();
            let CompletionProgress::Complete(value) = completion
                .advance(&context, &frame, &mut actual, true)
                .unwrap()
            else {
                panic!("unexpected cut")
            };
            assert_eq!(value.value, expected_value);
            let fields =
                ProgrammingFieldScope::for_value(ProgrammingOwner::Position, &value.value).unwrap();
            assert_eq!(
                actual.sources_for_fields(value.trace.unwrap(), &fields),
                expected.sources_for_fields(expected_node, &fields)
            );
            assert_eq!(
                actual.control_sources_for_fields(value.trace.unwrap(), &fields),
                expected.control_sources_for_fields(expected_node, &fields)
            );
            assert_eq!(value.trace, Some(expected_node));
        }
    }
}
#[test]
fn foreign_transfer_response_is_rejected_without_consuming_cut() {
    let context = FamilyCompositionContext::default();
    let frame = Frame {
        calls: Cell::new(0),
        available: false,
    };
    let mut trace = FamilyTraceArena::default();
    let incoming = source(&mut trace, target());
    let underlay = source(&mut trace, angles(0., 0.));
    let mut completion = PositionSourceCompletion::new(
        incoming,
        rank(),
        0.5,
        Some(underlay),
        PositionCompletionKind::Known,
    )
    .unwrap();
    let CompletionProgress::Needs(request) = completion
        .advance(&context, &frame, &mut trace, true)
        .unwrap()
    else {
        panic!("expected cut")
    };
    let foreign = ProgrammingTransitionTrace {
        from: ProgrammingFieldTransfer {
            identity: ProgrammingFieldScope::new([ProgrammingTraceField::ColorXyz]),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        completion
            .resume(request.stage, angles(1., 2.), Some(foreign))
            .is_err()
    );
    assert!(matches!(
        completion
            .advance(&context, &frame, &mut trace, true)
            .unwrap(),
        CompletionProgress::Needs(_)
    ));
    assert_eq!(frame.calls.get(), 1);
    completion
        .resume(request.stage, angles(1., 2.), None)
        .unwrap();
    assert!(matches!(
        completion
            .advance(&context, &frame, &mut trace, true)
            .unwrap(),
        CompletionProgress::Complete(_)
    ));
}
#[test]
fn panicking_frame_is_terminal_and_is_never_replayed() {
    struct PanicFrame(Cell<usize>);
    impl WholeFamilyExpressionFrameResolver for PanicFrame {
        fn resolve(
            &self,
            _: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            self.0.set(self.0.get() + 1);
            panic!("fixture resolver panic");
        }
    }
    let context = FamilyCompositionContext::default();
    let frame = PanicFrame(Cell::new(0));
    let mut trace = FamilyTraceArena::default();
    let incoming = source(&mut trace, target());
    let underlay = source(&mut trace, angles(0., 0.));
    let mut completion = PositionSourceCompletion::new(
        incoming,
        rank(),
        0.5,
        Some(underlay),
        PositionCompletionKind::Whole,
    )
    .unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || completion.advance(&context, &frame, &mut trace, true)
        ))
        .is_err()
    );
    assert!(
        completion
            .advance(&context, &frame, &mut trace, true)
            .is_err()
    );
    assert!(
        completion
            .resume(PositionCompletionStage::Activation, angles(3., 4.), None)
            .is_err()
    );
    assert_eq!(frame.0.get(), 1);
}
