//! Actual retained program suspension across both post-expression envelope stages.
use super::*;
use crate::{DynamicPresetSourceBinding, DynamicValueSourceResolver};
use light_core::FixtureId;

struct CapturedCurrent {
    calls: Cell<usize>,
    occurrences: Cell<usize>,
}
impl DynamicValueSourceResolver for CapturedCurrent {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        None
    }
    fn try_current_family_base(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        Ok(Some(target(41, [0., 0., 1.])))
    }
    fn current_family_occurrence(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Option<crate::DynamicSourceOccurrenceId> {
        self.occurrences.set(self.occurrences.get() + 1);
        Some(crate::DynamicSourceOccurrenceId::new(Uuid::from_u128(740)).unwrap())
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
fn source(whole: bool) -> FamilyCompositionSample {
    let value = target(42, [20., 10., 1.]);
    if whole {
        FamilyCompositionSample::WholeExpression {
            expression: Arc::new(
                CompiledProgrammingFamilyExpression::new(
                    leaf(value),
                    ProgrammingOwner::Position,
                    None,
                    None,
                )
                .unwrap(),
            ),
            rank: rank(1),
            activation_mix: 0.5,
        }
    } else {
        FamilySample::new(
            Arc::new(
                CompiledDynamicValueAddress::new(
                    DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
                    None,
                )
                .unwrap(),
            ),
            DynamicValue::Family(value),
            rank(1),
            0.5,
        )
        .unwrap()
        .into()
    }
}
fn completion_request(
    request: &PositionCompositionRequest,
) -> &position_completion::CompletionRequest {
    let PositionCompositionOperation::Base {
        source_index,
        rank: actual_rank,
        request,
    } = &request.operation
    else {
        panic!("completion must route through Base")
    };
    assert_eq!(*source_index, 0);
    assert_eq!(*actual_rank, rank(1));
    let base_evaluation::BaseMaterializationOperation::Completion(completion) = &request.operation
    else {
        panic!("expected completion operation")
    };
    completion
}
fn drive_two_stage(whole: bool) {
    let capture = Uuid::from_u128(if whole { 742 } else { 741 });
    let base = angles(0., 0.);
    let samples = [source(whole)];
    let current = CapturedCurrent {
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
    let endpoint = completion_request(&first);
    assert_eq!(
        endpoint.stage,
        position_completion::PositionCompletionStage::EndpointOutput
    );
    assert_eq!(
        endpoint.requirement,
        TransitionRequirement::LiveTargetPoints
    );
    assert_eq!(endpoint.from, target(41, [0., 0., 1.]));
    assert_eq!(endpoint.to, target(42, [20., 10., 1.]));
    assert_eq!(endpoint.progress, 0.5);
    assert_eq!(current.calls.get(), 1);
    assert_eq!(current.occurrences.get(), 1);
    let callbacks = frame.calls.get();
    let controls_before_wait = controls.get();
    assert_eq!(callbacks, 1);
    for _ in 0..3 {
        assert_eq!(
            needed(evaluation.advance(capture, &context, &frame).unwrap()).request_id,
            first.request_id
        );
        assert_eq!(frame.calls.get(), callbacks);
        assert_eq!(current.calls.get(), 1);
        assert_eq!(current.occurrences.get(), 1);
        assert_eq!(controls.get(), controls_before_wait);
    }
    assert!(
        evaluation
            .resume_materialization(
                capture,
                first.request_id,
                AttributeValue::Normalized(0.4),
                None
            )
            .is_err()
    );
    assert_eq!(
        evaluation.pending_materialization().unwrap().request_id,
        first.request_id
    );
    let endpoint_value = Frame::materialize(
        endpoint.requirement,
        &endpoint.from,
        &endpoint.to,
        FamilyExpressionOperation::Transition {
            progress: endpoint.progress,
        },
    );
    assert_eq!(endpoint_value, target(0, [10., 5., 1.]));
    evaluation
        .resume_materialization(capture, first.request_id, endpoint_value, None)
        .unwrap();
    let second = needed(evaluation.advance(capture, &context, &frame).unwrap());
    let activation = completion_request(&second);
    assert_ne!(second.request_id, first.request_id);
    assert_eq!(
        activation.stage,
        position_completion::PositionCompletionStage::Activation
    );
    assert_eq!(
        activation.requirement,
        TransitionRequirement::LiveJointAngles
    );
    assert_eq!(activation.from, base);
    assert_eq!(activation.to, target(0, [10., 5., 1.]));
    assert_eq!(activation.progress, 0.5);
    assert_eq!(frame.calls.get(), 2);
    assert_eq!(current.calls.get(), 1);
    assert_eq!(current.occurrences.get(), 1);
    let controls_before_activation_wait = controls.get();
    assert!(
        evaluation
            .resume_materialization(capture, first.request_id, angles(99., 99.), None)
            .is_err()
    );
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
    for _ in 0..3 {
        assert_eq!(
            needed(evaluation.advance(capture, &context, &frame).unwrap()).request_id,
            second.request_id
        );
        assert_eq!(frame.calls.get(), 2);
        assert_eq!(current.calls.get(), 1);
        assert_eq!(controls.get(), controls_before_activation_wait);
    }
    let activation_value = Frame::materialize(
        activation.requirement,
        &activation.from,
        &activation.to,
        FamilyExpressionOperation::Transition {
            progress: activation.progress,
        },
    );
    evaluation
        .resume_materialization(capture, second.request_id, activation_value, None)
        .unwrap();
    let actual = complete(evaluation.advance(capture, &context, &frame).unwrap());
    assert_eq!(actual, angles(5., 2.5));
    let current_calls = current.calls.get();
    for _ in 0..3 {
        assert_eq!(
            complete(evaluation.advance(capture, &context, &frame).unwrap()),
            actual
        );
        assert_eq!(current.calls.get(), current_calls);
        assert_eq!(frame.calls.get(), 2);
    }
    let mut reference = RetainedFamilyCompositionScratch::default();
    let expected = compose_retained_dynamic_family_traced(
        ProgrammingOwner::Position,
        &base,
        &samples,
        &context,
        &Frame::available(),
        &mut reference,
    )
    .unwrap();
    assert_eq!(actual, expected);
    let fields = ProgrammingFieldScope::for_value(ProgrammingOwner::Position, &actual).unwrap();
    assert_eq!(
        query(evaluation.family_trace(), &fields),
        query(reference.family_trace(), &fields)
    );
    assert_eq!(
        evaluation
            .family_trace()
            .control_sources_for_fields(evaluation.family_trace().root().unwrap(), &fields),
        reference
            .family_trace()
            .control_sources_for_fields(reference.family_trace().root().unwrap(), &fields)
    );
}
#[test]
fn known_source_retains_endpoint_and_activation_materialization() {
    drive_two_stage(false);
}
#[test]
fn whole_source_retains_endpoint_and_activation_materialization() {
    drive_two_stage(true);
}
