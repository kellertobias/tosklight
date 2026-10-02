//! EndpointOutput/Activation boundaries of actual nested captured programs: exact original
//! routes, pre-operation operands, suspension, inactivity and response unwind.
use super::*;
use crate::{
    CapturedPositionProgram, DynamicPresetSourceBinding, DynamicSampleExpression,
    DynamicTransitionReason, DynamicValueSourceResolver, PositionEnvelopeLocator,
    PositionEnvelopeOperand, PositionResumeEndpoint, PositionResumeScope,
    PositionStageOperandProgress,
};
use light_core::FixtureId;
use std::cell::{Cell, RefCell};

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
fn xyz(value: &AttributeValue) -> [f32; 3] {
    match value {
        AttributeValue::Position(value) => match value.as_ref() {
            PositionIntent::Target { offset_metres, .. } => offset_metres.each_ref().map(scalar),
            PositionIntent::Angles { .. } => panic!("Target endpoint"),
        },
        _ => panic!("Position endpoint"),
    }
}
fn angle_value(value: &AttributeValue) -> AttributeValue {
    match value {
        AttributeValue::Position(position) => match position.as_ref() {
            PositionIntent::Angles { .. } => value.clone(),
            PositionIntent::Target { .. } => {
                let [pan, tilt, _] = xyz(value);
                angles(pan, tilt)
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
fn known(value: AttributeValue, order: u128, mix: f32) -> FamilyCompositionSample {
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
    .into()
}
fn whole(
    expression: Arc<DynamicSampleExpression>,
    order: u128,
    mix: f32,
) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(
                expression,
                ProgrammingOwner::Position,
                None,
                None,
            )
            .unwrap(),
        ),
        rank: rank(order),
        activation_mix: mix,
    }
}
fn coupled(
    from: AttributeValue,
    to: AttributeValue,
    order: u128,
    mix: f32,
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
        activation_mix: mix,
    }
}

const S0_TARGET: [f32; 3] = [10., 20., 1.];
const S1_TARGET: [f32; 3] = [30., 40., 1.];
fn s0_target() -> AttributeValue {
    target(0, S0_TARGET)
}
fn s1_target() -> AttributeValue {
    target(51, S1_TARGET)
}
fn current_value() -> AttributeValue {
    target(52, [0., 0., 1.])
}
/// S2 (Coupled, partial) consumes S1 (Whole, Current crossfade, partial) which consumes S0
/// (Known, partial): three nested lexical uses, four envelope operations.
fn nested() -> Vec<FamilyCompositionSample> {
    vec![
        known(s0_target(), 1, 0.5),
        whole(leaf(s1_target()), 2, 0.5),
        coupled(target(61, [2., 4., 1.]), target(62, [6., 8., 1.]), 3, 0.5),
    ]
}

struct Current {
    available: bool,
    calls: Cell<usize>,
}
impl Current {
    fn new(available: bool) -> Self {
        Self {
            available,
            calls: Cell::new(0),
        }
    }
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
        Ok(self.available.then(current_value))
    }
    fn current_family_occurrence(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Option<crate::DynamicSourceOccurrenceId> {
        Some(crate::DynamicSourceOccurrenceId::new(Uuid::from_u128(990)).unwrap())
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
fn crossfade_s1(rank_value: FamilySampleRank) -> FamilyEndpointOutputControl {
    if rank_value == rank(2) {
        FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.5 }
    } else {
        FamilyEndpointOutputControl::Unchanged
    }
}
fn suppress_s1(rank_value: FamilySampleRank) -> FamilyEndpointOutputControl {
    if rank_value == rank(2) {
        FamilyEndpointOutputControl::Suppressed
    } else {
        FamilyEndpointOutputControl::Unchanged
    }
}
fn context<'a>(
    control: &'a dyn Fn(FamilySampleRank) -> FamilyEndpointOutputControl,
    current: &'a Current,
    fixture: FixtureId,
) -> FamilyCompositionContext<'a> {
    FamilyCompositionContext {
        endpoint_output: Some(FamilyEndpointOutputContext {
            control,
            target: fixture,
            current,
            native_models: None,
        }),
        ..Default::default()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Block {
    Requires,
    Error,
    Panic,
}
/// Materializes like a live frame, except for operations whose `to` operand is listed.
#[derive(Default)]
struct Frame {
    blocked: RefCell<Vec<(AttributeValue, Block)>>,
    calls: Cell<usize>,
}
impl Frame {
    fn blocking(to: &[AttributeValue]) -> Self {
        let frame = Self::default();
        for value in to {
            frame.block(value.clone(), Block::Requires);
        }
        frame
    }
    fn block(&self, to: AttributeValue, block: Block) {
        self.blocked.borrow_mut().push((to, block));
    }
    fn materialize(
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        progress: f32,
    ) -> AttributeValue {
        let (from, to) = if requirement == TransitionRequirement::LiveTargetPoints {
            (target(0, xyz(from)), target(0, xyz(to)))
        } else {
            (angle_value(from), angle_value(to))
        };
        CompiledProgrammingTransition::new(from, to, None)
            .unwrap()
            .sample(progress)
            .unwrap()
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
        let block = self
            .blocked
            .borrow()
            .iter()
            .find(|(value, _)| value == to)
            .map(|(_, block)| *block);
        match block {
            Some(Block::Requires) => Err(TransitionError::Requires(requirement)),
            Some(Block::Error) => Err(IntentError("frame rejected the operation".into()).into()),
            Some(Block::Panic) => panic!("frame unwound during the operation"),
            None => {
                let FamilyExpressionOperation::Transition { progress } = operation else {
                    panic!("envelope tests only transition")
                };
                Ok(Self::materialize(requirement, from, to, progress))
            }
        }
    }
}

fn completion(request: &PositionCompositionRequest) -> position_completion::CompletionRequest {
    let PositionCompositionOperation::Base { request, .. } = &request.operation else {
        panic!("envelope requests route through Base")
    };
    let base_evaluation::BaseMaterializationOperation::Completion(completion) = &request.operation
    else {
        panic!("expected an envelope operation")
    };
    completion.clone()
}
fn respond(request: &PositionCompositionRequest) -> AttributeValue {
    let operation = completion(request);
    Frame::materialize(
        operation.requirement,
        &operation.from,
        &operation.to,
        operation.progress,
    )
}
fn discover(
    branch: &PositionProgramBranch,
    context: &FamilyCompositionContext<'_>,
    frame: &Frame,
    capture: Uuid,
) -> PositionStageDiscoveryReport {
    let mut discovery = branch
        .begin_stage_discovery(
            context,
            RetainedFamilyCompositionScratch::default(),
            true,
            64,
        )
        .unwrap();
    match discovery.advance(capture, context, frame).unwrap() {
        PositionStageDiscoveryProgress::Complete(report) => report,
        PositionStageDiscoveryProgress::NeedsMaterialization(_) => {
            panic!("available frame completes discovery")
        }
    }
}
/// Original ordinary parent: each envelope operation suspends once, in execution order.
struct Original {
    locators: Vec<PositionEnvelopeLocator>,
    operations: Vec<position_completion::CompletionRequest>,
    value: AttributeValue,
}
fn original(
    branch: &PositionProgramBranch,
    context: &FamilyCompositionContext<'_>,
    capture: Uuid,
) -> Original {
    // Block exactly the four envelope operations (by their `to` operand); S2's own
    // coupled expression transition (to target 62) stays synchronous.
    let s1_endpoint = Frame::materialize(
        TransitionRequirement::LiveTargetPoints,
        &current_value(),
        &s1_target(),
        0.5,
    );
    let s2_expression = Frame::materialize(
        TransitionRequirement::LiveTargetPoints,
        &target(61, [2., 4., 1.]),
        &target(62, [6., 8., 1.]),
        0.5,
    );
    let frame = Frame::blocking(&[s0_target(), s1_target(), s1_endpoint, s2_expression]);
    let mut parent = branch
        .begin_composition(context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let mut locators = Vec::new();
    let mut operations = Vec::new();
    loop {
        match parent.advance(capture, context, &frame).unwrap() {
            PositionCompositionProgress::Complete(value) => {
                return Original {
                    locators,
                    operations,
                    value,
                };
            }
            PositionCompositionProgress::NeedsMaterialization(request) => {
                assert!(
                    parent
                        .pending_stage_locator(request.request_id)
                        .unwrap()
                        .is_none(),
                    "envelope routes never leak into the segment-only locator"
                );
                let locator = parent
                    .pending_envelope_locator(request.request_id)
                    .unwrap()
                    .expect("bound envelope operation has an exact original route");
                assert!(parent.pending_envelope_locator(Uuid::new_v4()).is_err());
                let operation = completion(&request);
                assert_eq!(locator.stage(), operation.stage);
                assert_eq!(locator.capture_id(), capture);
                let value = respond(&request);
                locators.push(locator);
                operations.push(operation);
                parent
                    .resume_materialization(capture, request.request_id, value, None)
                    .unwrap();
            }
        }
    }
}
fn ready(progress: PositionStageOperandProgress) -> AttributeValue {
    match progress {
        PositionStageOperandProgress::OperandReady(value) => value,
        PositionStageOperandProgress::NeedsMaterialization(_) => panic!("unexpected suspension"),
        PositionStageOperandProgress::Inactive => panic!("original operation remains reachable"),
    }
}
fn suspended(progress: PositionStageOperandProgress) -> PositionCompositionRequest {
    match progress {
        PositionStageOperandProgress::NeedsMaterialization(request) => request,
        PositionStageOperandProgress::OperandReady(_) => panic!("prefix must suspend first"),
        PositionStageOperandProgress::Inactive => panic!("original operation remains reachable"),
    }
}
fn inactive(progress: PositionStageOperandProgress) {
    assert!(matches!(progress, PositionStageOperandProgress::Inactive));
}
fn operands(stage: PositionCompletionStage) -> [PositionEnvelopeOperand; 2] {
    match stage {
        PositionCompletionStage::EndpointOutput => [
            PositionEnvelopeOperand::EndpointCurrent,
            PositionEnvelopeOperand::EndpointTarget,
        ],
        PositionCompletionStage::Activation => [
            PositionEnvelopeOperand::ActivationUnderlay,
            PositionEnvelopeOperand::ActivationEndpoint,
        ],
    }
}

#[test]
fn nested_program_discovers_and_replays_exact_envelope_operands_in_order() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let sources = nested();
    let registry = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    let branch = registry.branch();
    let fixture = FixtureId::new();
    let current = Current::new(true);
    let control = crossfade_s1;
    let context = context(&control, &current, fixture);
    let conditioned = branch.conditioned_sources().unwrap();
    assert!(matches!(
        conditioned[0],
        Some(FamilyCompositionSample::Known(_))
    ));
    assert!(matches!(
        conditioned[1],
        Some(FamilyCompositionSample::WholeExpression { .. })
    ));
    assert!(matches!(
        conditioned[2],
        Some(FamilyCompositionSample::CoupledExpression { .. })
    ));

    let original = original(&branch, &context, capture);
    let stages = original
        .locators
        .iter()
        .map(PositionEnvelopeLocator::stage)
        .collect::<Vec<_>>();
    assert_eq!(
        stages,
        [
            PositionCompletionStage::Activation,
            PositionCompletionStage::EndpointOutput,
            PositionCompletionStage::Activation,
            PositionCompletionStage::Activation,
        ],
        "S0 Activation, S1 EndpointOutput then Activation, S2 Activation"
    );
    for (index, locator) in original.locators.iter().enumerate() {
        for other in &original.locators[index + 1..] {
            assert_ne!(locator, other, "each original operation has its own route");
        }
    }
    assert_eq!(current.calls.get(), 1, "the parent sampled Current once");

    // Discovery reaches the same four routes, in the same order, without a suspension.
    let discovery_frame = Frame::default();
    let report = discover(&branch, &context, &discovery_frame, capture);
    assert!(report.complete);
    assert_eq!(report.envelope_candidates, original.locators);
    assert!(
        report
            .candidates
            .iter()
            .all(|candidate| candidate.envelope_stage().is_none())
    );
    assert_eq!(current.calls.get(), 2, "discovery is one more evaluator");

    // Ordinary parent output is unaffected by suspension at envelope operations.
    let mut ordinary = branch
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let PositionCompositionProgress::Complete(value) = ordinary
        .advance(capture, &context, &Frame::default())
        .unwrap()
    else {
        panic!("available frame completes")
    };
    assert_eq!(value, original.value);
    // ...and equals the legacy traced compositor, value and trace.
    let mut reference = RetainedFamilyCompositionScratch::default();
    let expected = compose_retained_dynamic_family_traced(
        ProgrammingOwner::Position,
        &base,
        &sources,
        &context,
        &Frame::default(),
        &mut reference,
    )
    .unwrap();
    assert_eq!(value, expected);
    let fields = ProgrammingFieldScope::for_value(ProgrammingOwner::Position, &value).unwrap();
    let query =
        |trace: &FamilyTraceArena| trace.query_fields_with_base(trace.root().unwrap(), &fields);
    assert_eq!(
        query(ordinary.family_trace()),
        query(reference.family_trace())
    );

    // Every operand equals the original operation's pre-operation from/to, in order.
    for (index, (locator, operation)) in original
        .locators
        .iter()
        .zip(&original.operations)
        .enumerate()
    {
        for (operand, expected) in operands(locator.stage())
            .into_iter()
            .zip([&operation.from, &operation.to])
        {
            let calls_before = current.calls.get();
            let frame = Frame::default();
            let mut replay = branch
                .begin_envelope_operand(
                    locator,
                    operand,
                    &context,
                    RetainedFamilyCompositionScratch::default(),
                    true,
                )
                .unwrap();
            let value = ready(replay.advance(capture, &context, &frame).unwrap());
            assert_eq!(&value, expected, "{operand:?} of {:?}", locator.stage());
            let sampled = current.calls.get() - calls_before;
            // Only S1 (and S2 above it) reach S1's Current crossfade; once per evaluator.
            assert_eq!(sampled, usize::from(index > 0));
            let frame_calls = frame.calls.get();
            for _ in 0..3 {
                assert_eq!(
                    ready(replay.advance(capture, &context, &frame).unwrap()),
                    value
                );
                assert_eq!(current.calls.get() - calls_before, sampled);
                assert_eq!(frame.calls.get(), frame_calls);
            }
            replay.into_scratch();
        }
    }
    // S0 stops below S1: Current is never sampled; S1/S2 stops sample it exactly once.
    let calls_before = current.calls.get();
    let mut below = branch
        .begin_envelope_operand(
            &original.locators[0],
            PositionEnvelopeOperand::ActivationEndpoint,
            &context,
            RetainedFamilyCompositionScratch::default(),
            false,
        )
        .unwrap();
    assert_eq!(
        ready(below.advance(capture, &context, &Frame::default()).unwrap()),
        s0_target()
    );
    assert_eq!(current.calls.get(), calls_before);
    assert_eq!(original.operations[0].from, base);
    assert_eq!(original.operations[1].from, current_value());
    assert_eq!(original.operations[1].to, s1_target());
}

#[test]
fn consecutive_boundaries_suspend_and_resume_without_applying_the_parent_suffix() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let registry = CapturedPositionProgram::new(capture, &base, &nested()).unwrap();
    let branch = registry.branch();
    let current = Current::new(true);
    let control = crossfade_s1;
    let context = context(&control, &current, FixtureId::new());
    let original = original(&branch, &context, capture);
    let s1_activation = &original.locators[2];

    // Prefix S0 Activation and S1 EndpointOutput both suspend; stop before S1 Activation.
    let frame = Frame::blocking(&[s0_target(), s1_target()]);
    let mut replay = branch
        .begin_envelope_operand(
            s1_activation,
            PositionEnvelopeOperand::ActivationEndpoint,
            &context,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    let first = suspended(replay.advance(capture, &context, &frame).unwrap());
    assert_eq!(
        completion(&first).stage,
        PositionCompletionStage::Activation
    );
    assert_eq!(completion(&first).to, s0_target());
    assert_eq!(
        current.calls.get(),
        1,
        "Current is not read before S1 is reached"
    );
    for _ in 0..2 {
        assert_eq!(
            suspended(replay.advance(capture, &context, &frame).unwrap()).request_id,
            first.request_id
        );
    }
    replay
        .resume_materialization(capture, first.request_id, respond(&first), None)
        .unwrap();
    let second = suspended(replay.advance(capture, &context, &frame).unwrap());
    assert_ne!(second.request_id, first.request_id);
    assert_eq!(
        completion(&second).stage,
        PositionCompletionStage::EndpointOutput
    );
    assert_eq!(completion(&second).from, current_value());
    assert_eq!(
        current.calls.get(),
        2,
        "the replay evaluator sampled Current once"
    );
    // A distinctive response proves the operand is the response itself, before Activation.
    let endpoint = target(0, [17., 23., 1.]);
    replay
        .resume_materialization(capture, second.request_id, endpoint.clone(), None)
        .unwrap();
    let frame_calls = frame.calls.get();
    let operand = ready(replay.advance(capture, &context, &frame).unwrap());
    assert_eq!(
        operand, endpoint,
        "Activation and the parent suffix are not applied"
    );
    for _ in 0..3 {
        assert_eq!(
            ready(replay.advance(capture, &context, &frame).unwrap()),
            endpoint
        );
        assert_eq!(frame.calls.get(), frame_calls);
        assert_eq!(current.calls.get(), 2);
    }
    assert!(
        replay
            .resume_materialization(capture, second.request_id, endpoint, None)
            .is_err(),
        "a stopped replay has no pending response"
    );

    // S1's underlay operand: only S0 suspends, S1's EndpointOutput then runs ordinarily.
    let frame = Frame::blocking(&[s0_target()]);
    let mut underlay = branch
        .begin_envelope_operand(
            s1_activation,
            PositionEnvelopeOperand::ActivationUnderlay,
            &context,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    let request = suspended(underlay.advance(capture, &context, &frame).unwrap());
    let s0 = respond(&request);
    underlay
        .resume_materialization(capture, request.request_id, s0.clone(), None)
        .unwrap();
    assert_eq!(
        ready(underlay.advance(capture, &context, &frame).unwrap()),
        s0,
        "the underlay operand is the resolved S0 result, before S1's envelope"
    );
    assert_eq!(current.calls.get(), 3);
}

#[test]
fn suppressed_released_and_covered_operations_are_inactive() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let registry = CapturedPositionProgram::new(capture, &base, &nested()).unwrap();
    let current = Current::new(true);
    let control = crossfade_s1;
    let context = context(&control, &current, FixtureId::new());
    let original = original(&registry.branch(), &context, capture);
    let [s0, s1_endpoint, s1_activation, s2] = original.locators.clone().try_into().unwrap();

    // Suppressed: the captured gate removes S1 entirely; Current is never sampled.
    let suppress = suppress_s1;
    let suppressed_current = Current::new(true);
    let suppressed = self::context(&suppress, &suppressed_current, FixtureId::new());
    for locator in [&s1_endpoint, &s1_activation] {
        let mut replay = registry
            .branch()
            .begin_envelope_operand(
                locator,
                operands(locator.stage())[0],
                &suppressed,
                RetainedFamilyCompositionScratch::default(),
                true,
            )
            .unwrap();
        inactive(
            replay
                .advance(capture, &suppressed, &Frame::default())
                .unwrap(),
        );
        inactive(
            replay
                .advance(capture, &suppressed, &Frame::default())
                .unwrap(),
        );
    }
    assert_eq!(suppressed_current.calls.get(), 0);

    // Released S1: its operations vanish and S0's lexical use (underlay of S1) is gone.
    let mut released = registry.branch();
    released.replace_source(1, None).unwrap();
    for locator in [&s0, &s1_endpoint, &s1_activation] {
        let mut replay = released
            .begin_envelope_operand(
                locator,
                operands(locator.stage())[1],
                &context,
                RetainedFamilyCompositionScratch::default(),
                true,
            )
            .unwrap();
        inactive(
            replay
                .advance(capture, &context, &Frame::default())
                .unwrap(),
        );
    }
    // S2 is unchanged and now receives S0 directly as its underlay.
    let mut replay = released
        .begin_envelope_operand(
            &s2,
            PositionEnvelopeOperand::ActivationUnderlay,
            &context,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        ready(
            replay
                .advance(capture, &context, &Frame::default())
                .unwrap()
        ),
        original.operations[2].from,
        "S0's completed result, which was S1's Activation underlay"
    );

    // Covered: Resume chooses the Incoming endpoint, so the top source no longer consumes
    // its underlay and covers the lower partial source.
    let covered_capture = Uuid::new_v4();
    let sources = vec![
        known(s0_target(), 1, 0.5),
        whole(
            Arc::new(DynamicSampleExpression::Transition {
                from: None,
                to: Some(leaf(target(0, [50., 60., 1.]))),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: Uuid::from_u128(7001),
                },
            }),
            2,
            1.,
        ),
    ];
    let registry = CapturedPositionProgram::new(covered_capture, &base, &sources).unwrap();
    let plain = Current::new(true);
    let unchanged = |_| FamilyEndpointOutputControl::Unchanged;
    let context = self::context(&unchanged, &plain, FixtureId::new());
    let report = discover(
        &registry.branch(),
        &context,
        &Frame::default(),
        covered_capture,
    );
    let [lower] = report.envelope_candidates.try_into().unwrap();
    assert_eq!(lower.stage(), PositionCompletionStage::Activation);
    let mut chosen = registry.branch();
    chosen
        .choose_resume(
            PositionResumeScope {
                instance_id: Uuid::from_u128(1002),
                controller_id: Uuid::from_u128(2002),
                occurrence_id: Uuid::from_u128(7001),
            },
            PositionResumeEndpoint::Incoming,
        )
        .unwrap();
    match &chosen.conditioned_sources().unwrap()[1] {
        Some(FamilyCompositionSample::WholeExpression { expression, .. }) => {
            assert!(
                !expression.needs_underlay(),
                "Incoming no longer consumes S0"
            )
        }
        Some(FamilyCompositionSample::Known(_)) => {}
        _ => panic!("chosen Resume keeps the covering source"),
    }
    for (branch, reachable) in [(registry.branch(), true), (chosen, false)] {
        let mut replay = branch
            .begin_envelope_operand(
                &lower,
                PositionEnvelopeOperand::ActivationEndpoint,
                &context,
                RetainedFamilyCompositionScratch::default(),
                true,
            )
            .unwrap();
        let progress = replay
            .advance(covered_capture, &context, &Frame::default())
            .unwrap();
        if reachable {
            assert_eq!(ready(progress), s0_target());
        } else {
            inactive(progress);
        }
    }
}

#[test]
fn unavailable_current_is_passive_and_foreign_registry_is_rejected() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let sources = nested();
    let registry = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    let current = Current::new(true);
    let control = crossfade_s1;
    let available = context(&control, &current, FixtureId::new());
    let original = original(&registry.branch(), &available, capture);
    let s1_endpoint = &original.locators[1];

    // Unavailable Current: no Live fallback, no operand, sampled once, terminal replay.
    let missing = Current::new(false);
    let unavailable = context(&control, &missing, FixtureId::new());
    let frame = Frame::default();
    let mut replay = registry
        .branch()
        .begin_envelope_operand(
            s1_endpoint,
            PositionEnvelopeOperand::EndpointCurrent,
            &unavailable,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    assert!(matches!(
        replay.advance(capture, &unavailable, &frame),
        Err(TransitionError::Requires(_))
    ));
    assert_eq!(missing.calls.get(), 1);
    let frame_calls = frame.calls.get();
    assert!(replay.advance(capture, &unavailable, &frame).is_err());
    assert_eq!(missing.calls.get(), 1, "no re-read and no Live fallback");
    assert_eq!(frame.calls.get(), frame_calls);
    replay.into_scratch();
    // A stop below the Current consumer never needs Current.
    let mut below = registry
        .branch()
        .begin_envelope_operand(
            &original.locators[0],
            PositionEnvelopeOperand::ActivationUnderlay,
            &unavailable,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        ready(
            below
                .advance(capture, &unavailable, &Frame::default())
                .unwrap()
        ),
        base
    );
    assert_eq!(missing.calls.get(), 1);

    // Equal capture UUID and equal sources are still another registry.
    let foreign = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    for locator in &original.locators {
        for operand in operands(locator.stage()) {
            assert!(
                foreign
                    .branch()
                    .validate_envelope_operand(locator, operand)
                    .is_err()
            );
            assert!(
                foreign
                    .branch()
                    .begin_envelope_operand(
                        locator,
                        operand,
                        &available,
                        RetainedFamilyCompositionScratch::default(),
                        true,
                    )
                    .is_err()
            );
        }
        // The operand role must belong to the located stage.
        let wrong = operands(match locator.stage() {
            PositionCompletionStage::EndpointOutput => PositionCompletionStage::Activation,
            PositionCompletionStage::Activation => PositionCompletionStage::EndpointOutput,
        })[0];
        assert!(
            registry
                .branch()
                .validate_envelope_operand(locator, wrong)
                .is_err()
        );
    }
    let foreign_report = discover(&foreign.branch(), &available, &Frame::default(), capture);
    assert_eq!(foreign_report.envelope_candidates.len(), 4);
    for (foreign, original) in foreign_report
        .envelope_candidates
        .iter()
        .zip(&original.locators)
    {
        assert_ne!(foreign, original);
    }
}

#[test]
fn bad_responses_are_retryable_or_terminal_without_partial_observation() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let registry = CapturedPositionProgram::new(capture, &base, &nested()).unwrap();
    let current = Current::new(true);
    let control = crossfade_s1;
    let context = context(&control, &current, FixtureId::new());
    let original = original(&registry.branch(), &context, capture);
    let s1_activation = &original.locators[2];
    let begin = || {
        registry
            .branch()
            .begin_envelope_operand(
                s1_activation,
                PositionEnvelopeOperand::ActivationEndpoint,
                &context,
                RetainedFamilyCompositionScratch::default(),
                true,
            )
            .unwrap()
    };

    // Retryable: rejected before mutation; the same request stays pending.
    let frame = Frame::blocking(&[s1_target()]);
    let mut replay = begin();
    let request = suspended(replay.advance(capture, &context, &frame).unwrap());
    assert_eq!(
        completion(&request).stage,
        PositionCompletionStage::EndpointOutput
    );
    let good = respond(&request);
    assert!(
        replay
            .resume_materialization(Uuid::new_v4(), request.request_id, good.clone(), None)
            .is_err()
    );
    assert!(
        replay
            .resume_materialization(capture, Uuid::new_v4(), good.clone(), None)
            .is_err()
    );
    assert!(
        replay
            .resume_materialization(
                capture,
                request.request_id,
                AttributeValue::Normalized(0.5),
                None
            )
            .is_err()
    );
    assert_eq!(
        suspended(replay.advance(capture, &context, &frame).unwrap()).request_id,
        request.request_id
    );
    replay
        .resume_materialization(capture, request.request_id, good.clone(), None)
        .unwrap();
    assert_eq!(
        ready(replay.advance(capture, &context, &frame).unwrap()),
        good
    );

    // Terminal error: the frame rejects S1's EndpointOutput; nothing is observed afterwards.
    let frame = Frame::default();
    frame.block(s1_target(), Block::Error);
    let mut replay = begin();
    assert!(replay.advance(capture, &context, &frame).is_err());
    let calls = frame.calls.get();
    assert!(replay.advance(capture, &context, &frame).is_err());
    assert_eq!(frame.calls.get(), calls);
    replay.into_scratch();

    // Terminal unwind: a panicking frame poisons only this replay; its scratch is recoverable.
    let frame = Frame::default();
    frame.block(s1_target(), Block::Panic);
    let mut replay = begin();
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        replay.advance(capture, &context, &frame)
    }));
    assert!(unwound.is_err());
    assert!(
        replay
            .advance(capture, &context, &Frame::default())
            .is_err()
    );
    let scratch = replay.into_scratch();
    let mut fresh = registry
        .branch()
        .begin_envelope_operand(
            s1_activation,
            PositionEnvelopeOperand::ActivationEndpoint,
            &context,
            scratch,
            true,
        )
        .unwrap();
    assert_eq!(
        ready(fresh.advance(capture, &context, &Frame::default()).unwrap()),
        original.operations[2].to
    );
    // The original parent's final output is still exactly reproducible.
    let mut parent = registry
        .branch()
        .begin_composition(&context, fresh.into_scratch(), true)
        .unwrap();
    assert!(matches!(
        parent.advance(capture, &context, &Frame::default()).unwrap(),
        PositionCompositionProgress::Complete(value) if value == original.value
    ));
}
