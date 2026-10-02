//! MaskAdoption/MaskTransition stages of actual bound nested programs: exact original mask
//! locators, composed/adopted prefix operands, coverage, release, Resume and response unwind.
use super::*;
use crate::{
    CapturedPositionProgram, DynamicPresetSourceBinding, DynamicSampleExpression,
    DynamicTransitionReason, DynamicValueSourceResolver, PositionMaskLocator, PositionMaskOperand,
    PositionMaskStage, PositionResumeEndpoint, PositionResumeScope, PositionStageOperandProgress,
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
/// Deterministic test geometry: Angles (pan, tilt) <-> Target offset (pan, tilt, 1).
fn as_target(value: &AttributeValue) -> AttributeValue {
    match value {
        AttributeValue::Position(position) => match position.as_ref() {
            PositionIntent::Target { offset_metres, .. } => {
                target(0, offset_metres.each_ref().map(scalar))
            }
            PositionIntent::Angles {
                pan_degrees,
                tilt_degrees,
            } => target(0, [scalar(pan_degrees), scalar(tilt_degrees), 1.]),
        },
        _ => panic!("Position test value"),
    }
}
fn as_angles(value: &AttributeValue) -> AttributeValue {
    match value {
        AttributeValue::Position(position) => match position.as_ref() {
            PositionIntent::Angles { .. } => value.clone(),
            PositionIntent::Target { offset_metres, .. } => {
                let [pan, tilt, _] = offset_metres.each_ref().map(scalar);
                angles(pan, tilt)
            }
        },
        _ => panic!("Position test value"),
    }
}
/// Frame adoption into the mask's authored representation (the coordinator's role).
fn adopt_into(value: &AttributeValue, address: &DynamicValueAddress) -> AttributeValue {
    let candidate = as_angles(value);
    if address.matches_authored_source(&candidate) {
        candidate
    } else {
        as_target(value)
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
fn known_sample(value: AttributeValue, order: u128, mix: f32) -> FamilySample {
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
}
fn known(value: AttributeValue, order: u128, mix: f32) -> FamilyCompositionSample {
    known_sample(value, order, mix).into()
}
fn mask(value: AttributeValue, order: u128, mix: f32) -> FamilyCompositionSample {
    known_sample(value, order, mix).into_fix_at().into()
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
fn resume(from: AttributeValue, to: AttributeValue, order: u128) -> FamilyCompositionSample {
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
        activation_mix: 0.5,
    }
}

const R: u128 = 5;
const M1: usize = 2;
const RESUME: usize = 3;
const M2: usize = 4;
fn s0_target() -> AttributeValue {
    target(0, [10., 20., 1.])
}
fn s1_target() -> AttributeValue {
    target(51, [30., 40., 1.])
}
fn m1_value() -> AttributeValue {
    target(71, [5., 6., 1.])
}
fn m2_value() -> AttributeValue {
    angles(70., 30.)
}
fn current_value() -> AttributeValue {
    target(52, [0., 0., 1.])
}
fn full_value() -> AttributeValue {
    angles(15., 25.)
}
/// Root order: S0 Known, S1 Whole (Current crossfade), M1 partial Target mask (frame-resolved
/// transition), independent Resume (Coupled), M2 partial Angle mask, S5 Known suffix.
fn masked() -> Vec<FamilyCompositionSample> {
    vec![
        known(s0_target(), 1, 0.5),
        whole(leaf(s1_target()), 2, 0.5),
        mask(m1_value(), 3, 0.5),
        resume(target(61, [2., 4., 1.]), target(62, [6., 8., 1.]), R),
        mask(m2_value(), 6, 0.5),
        known(target(81, [1., 1., 1.]), 7, 0.5),
    ]
}
/// The same program with a full mask between M1 and the Resume.
fn covered() -> Vec<FamilyCompositionSample> {
    let mut sources = masked();
    sources.insert(RESUME, mask(full_value(), 4, 1.));
    sources
}
fn resume_scope() -> PositionResumeScope {
    PositionResumeScope {
        instance_id: Uuid::from_u128(1000 + R),
        controller_id: Uuid::from_u128(2000 + R),
        occurrence_id: Uuid::from_u128(5000 + R),
    }
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
fn resolve_adoption(
    value: &AttributeValue,
    address: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    Ok(adopt_into(value, address))
}
/// `resolving` adopts synchronously (as the oracle needs); otherwise every
/// representation-changing MaskAdoption suspends as a real request.
fn context<'a>(
    control: &'a dyn Fn(FamilySampleRank) -> FamilyEndpointOutputControl,
    current: &'a Current,
    resolving: bool,
) -> FamilyCompositionContext<'a> {
    FamilyCompositionContext {
        endpoint_output: Some(FamilyEndpointOutputContext {
            control,
            target: FixtureId::new(),
            current,
            native_models: None,
        }),
        resolve_adoption: resolving.then_some(&resolve_adoption as &FamilyAdoptionResolver),
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
            (as_target(from), as_target(to))
        } else {
            (as_angles(from), as_angles(to))
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
                    panic!("mask tests only transition")
                };
                Ok(Self::materialize(requirement, from, to, progress))
            }
        }
    }
}

/// The coordinator's answer to any request of these programs.
fn respond(request: &PositionCompositionRequest) -> AttributeValue {
    match &request.operation {
        PositionCompositionOperation::Base { request, .. } => {
            let base_evaluation::BaseMaterializationOperation::Completion(operation) =
                &request.operation
            else {
                panic!("only envelope operations are blocked below the masks")
            };
            Frame::materialize(
                operation.requirement,
                &operation.from,
                &operation.to,
                operation.progress,
            )
        }
        PositionCompositionOperation::MaskAdoption { from, address, .. } => {
            adopt_into(from, address)
        }
        PositionCompositionOperation::MaskTransition {
            from, to, progress, ..
        } => Frame::materialize(request.requirement, from, to, *progress),
    }
}
fn is_mask(request: &PositionCompositionRequest) -> bool {
    !matches!(request.operation, PositionCompositionOperation::Base { .. })
}

fn ready(progress: PositionStageOperandProgress) -> AttributeValue {
    match progress {
        PositionStageOperandProgress::OperandReady(value) => value,
        PositionStageOperandProgress::NeedsMaterialization(_) => panic!("unexpected suspension"),
        PositionStageOperandProgress::Inactive => panic!("original mask remains reachable"),
    }
}
fn suspended(progress: PositionStageOperandProgress) -> PositionCompositionRequest {
    match progress {
        PositionStageOperandProgress::NeedsMaterialization(request) => request,
        PositionStageOperandProgress::OperandReady(_) => panic!("prefix must suspend first"),
        PositionStageOperandProgress::Inactive => panic!("original mask remains reachable"),
    }
}
fn inactive(progress: PositionStageOperandProgress) {
    assert!(matches!(progress, PositionStageOperandProgress::Inactive));
}
fn operands(stage: PositionMaskStage) -> &'static [PositionMaskOperand] {
    match stage {
        PositionMaskStage::Adoption => &[PositionMaskOperand::AdoptionInput],
        PositionMaskStage::Transition => &[
            PositionMaskOperand::TransitionFrom,
            PositionMaskOperand::TransitionTo,
        ],
    }
}
fn discover(
    branch: &PositionProgramBranch,
    context: &FamilyCompositionContext<'_>,
    capture: Uuid,
    limit: usize,
) -> Result<PositionStageDiscoveryReport, TransitionError> {
    let mut discovery = branch.begin_stage_discovery(
        context,
        RetainedFamilyCompositionScratch::default(),
        true,
        limit,
    )?;
    match discovery.advance(capture, context, &Frame::default())? {
        PositionStageDiscoveryProgress::Complete(report) => Ok(report),
        PositionStageDiscoveryProgress::NeedsMaterialization(_) => {
            panic!("resolving context and available frame complete discovery")
        }
    }
}
/// Replay one operand to completion with an available frame and the resolving context.
fn operand(
    branch: &PositionProgramBranch,
    locator: &PositionMaskLocator,
    operand: PositionMaskOperand,
    context: &FamilyCompositionContext<'_>,
    capture: Uuid,
) -> PositionStageOperandProgress {
    let mut replay = branch
        .begin_mask_operand(
            locator,
            operand,
            context,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    let progress = replay.advance(capture, context, &Frame::default()).unwrap();
    replay.into_scratch();
    progress
}
/// Independent oracle: the unbound ordinary Position compositor (no registry, no matcher)
/// over exactly the given root sources. The legacy one-shot compositor cannot resolve a
/// frame-materialized Position mask transition, so it is not used for these programs.
fn reference(
    base: &AttributeValue,
    sources: &[FamilyCompositionSample],
    context: &FamilyCompositionContext<'_>,
) -> (AttributeValue, Option<crate::FamilyTraceQuery>) {
    let capture = Uuid::new_v4();
    let mut parent = begin_retained_position_composition(
        capture,
        base,
        sources,
        context,
        RetainedFamilyCompositionScratch::default(),
        true,
    )
    .unwrap();
    let PositionCompositionProgress::Complete(value) =
        parent.advance(capture, context, &Frame::default()).unwrap()
    else {
        panic!("resolving context and available frame complete")
    };
    let query = fields_query(parent.family_trace(), &value);
    (value, query)
}
fn fields_query(
    trace: &FamilyTraceArena,
    value: &AttributeValue,
) -> Option<crate::FamilyTraceQuery> {
    let fields = ProgrammingFieldScope::for_value(ProgrammingOwner::Position, value).unwrap();
    trace.query_fields_with_base(trace.root().unwrap(), &fields)
}
fn ordinary(
    branch: &PositionProgramBranch,
    context: &FamilyCompositionContext<'_>,
    capture: Uuid,
    scratch: RetainedFamilyCompositionScratch,
) -> (AttributeValue, Option<crate::FamilyTraceQuery>) {
    let mut parent = branch.begin_composition(context, scratch, true).unwrap();
    let PositionCompositionProgress::Complete(value) =
        parent.advance(capture, context, &Frame::default()).unwrap()
    else {
        panic!("resolving context and available frame complete")
    };
    let query = fields_query(parent.family_trace(), &value);
    (value, query)
}
fn present(sources: &[Option<FamilyCompositionSample>]) -> Vec<FamilyCompositionSample> {
    sources.iter().flatten().cloned().collect()
}
/// Every mask operand of a branch equals the composed/adopted prefix of exactly the lower
/// conditioned root sources and the original materialized mask value.
fn assert_operands_match_prefix(
    branch: &PositionProgramBranch,
    locators: &[PositionMaskLocator],
    slots: &[usize],
    context: &FamilyCompositionContext<'_>,
    capture: Uuid,
) {
    let conditioned = branch.conditioned_sources().unwrap();
    let base = angles(0., 0.);
    for (locator, &slot) in locators.iter().zip(slots) {
        let (prefix, _) = reference(&base, &present(&conditioned[..slot]), context);
        let Some(FamilyCompositionSample::Known(sample)) = &conditioned[slot] else {
            panic!("the mask slot stays present")
        };
        let Some(DynamicValue::Family(value)) = sample.materialized_value() else {
            panic!("materialized mask")
        };
        for &role in operands(locator.stage()) {
            let expected = match role {
                PositionMaskOperand::AdoptionInput => prefix.clone(),
                PositionMaskOperand::TransitionFrom => {
                    adopt_into(&prefix, sample.address.address())
                }
                PositionMaskOperand::TransitionTo => value.clone(),
            };
            assert_eq!(
                ready(operand(branch, locator, role, context, capture)),
                expected,
                "{role:?} of the mask at slot {slot}"
            );
        }
    }
}
struct Original {
    /// Discovery's reached mask stages: M1 Adoption, M1 Transition, M2 Adoption, M2 Transition.
    masks: [PositionMaskLocator; 4],
    value: AttributeValue,
    query: Option<crate::FamilyTraceQuery>,
}
fn original(
    registry: &CapturedPositionProgram,
    context: &FamilyCompositionContext<'_>,
    capture: Uuid,
) -> Original {
    let report = discover(&registry.branch(), context, capture, 64).unwrap();
    assert!(report.complete);
    let stages = report
        .mask_candidates
        .iter()
        .map(PositionMaskLocator::stage)
        .collect::<Vec<_>>();
    assert_eq!(
        stages,
        [
            PositionMaskStage::Adoption,
            PositionMaskStage::Transition,
            PositionMaskStage::Adoption,
            PositionMaskStage::Transition,
        ]
    );
    let (value, query) = ordinary(
        &registry.branch(),
        context,
        capture,
        RetainedFamilyCompositionScratch::default(),
    );
    Original {
        masks: report.mask_candidates.try_into().unwrap(),
        value,
        query,
    }
}

#[test]
fn actual_nested_program_exposes_exact_mask_locators_and_prefix_operands() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let sources = masked();
    let registry = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    let branch = registry.branch();
    let current = Current::new(true);
    let control = crossfade_s1;
    let resolving = context(&control, &current, true);
    let deferred = context(&control, &current, false);

    // Bound ordinary composition equals the unbound ordinary compositor, value and trace.
    let original = original(&registry, &resolving, capture);
    let (expected, query) = reference(&base, &sources, &resolving);
    assert_eq!(original.value, expected);
    assert_eq!(original.query, query);
    for (index, locator) in original.masks.iter().enumerate() {
        assert_eq!(locator.capture_id(), capture);
        for other in &original.masks[index + 1..] {
            assert_ne!(
                locator, other,
                "each original mask operation has its own route"
            );
        }
    }

    // The ordinary parent suspends at genuine mask requests; only those issue mask locators,
    // and they are the authentic originals that discovery reached.
    let frame = Frame::blocking(&[m1_value()]);
    let mut parent = branch
        .begin_composition(&deferred, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let mut issued = Vec::new();
    let value = loop {
        match parent.advance(capture, &deferred, &frame).unwrap() {
            PositionCompositionProgress::Complete(value) => break value,
            PositionCompositionProgress::NeedsMaterialization(request) => {
                assert!(parent.pending_mask_locator(Uuid::new_v4()).is_err());
                let locator = parent.pending_mask_locator(request.request_id).unwrap();
                assert!(
                    parent
                        .pending_stage_locator(request.request_id)
                        .unwrap()
                        .is_none()
                );
                if is_mask(&request) {
                    assert!(
                        parent
                            .pending_envelope_locator(request.request_id)
                            .unwrap()
                            .is_none()
                    );
                    let locator = locator.expect("bound mask request has its original locator");
                    assert!(original.masks.contains(&locator));
                    issued.push((locator, request.operation.clone()));
                } else {
                    assert!(locator.is_none(), "Base requests never issue mask locators");
                }
                parent
                    .resume_materialization(capture, request.request_id, respond(&request), None)
                    .unwrap();
            }
        }
    };
    assert_eq!(
        value, original.value,
        "suspension retains the original output"
    );
    assert_eq!(
        fields_query(parent.family_trace(), &value),
        original.query,
        "...and its field trace"
    );
    let issued_positions = issued
        .iter()
        .map(|(locator, _)| {
            original
                .masks
                .iter()
                .position(|mask| mask == locator)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(
        issued_positions.starts_with(&[0, 1]),
        "M1 Adoption, then the blocked M1 Transition suspend first: {issued_positions:?}"
    );
    assert!(issued_positions.windows(2).all(|pair| pair[0] < pair[1]));
    // Each issued request's own operands are what replay returns for that locator.
    for (locator, operation) in &issued {
        let expected: Vec<AttributeValue> = match operation {
            PositionCompositionOperation::MaskAdoption { from, .. } => vec![from.clone()],
            PositionCompositionOperation::MaskTransition { from, to, .. } => {
                vec![from.clone(), to.clone()]
            }
            PositionCompositionOperation::Base { .. } => unreachable!(),
        };
        for (&role, expected) in operands(locator.stage()).iter().zip(&expected) {
            assert_eq!(
                &ready(operand(&branch, locator, role, &resolving, capture)),
                expected
            );
        }
    }
    // Every operand is the actual composed/adopted prefix of the lower sources and the
    // original materialized mask before partial application.
    assert_operands_match_prefix(
        &branch,
        &original.masks,
        &[M1, M1, M2, M2],
        &resolving,
        capture,
    );
    // Mask locators never leak into segment or envelope candidates.
    let report = discover(&branch, &resolving, capture, 64).unwrap();
    assert!(
        report
            .candidates
            .iter()
            .all(|candidate| candidate.envelope_stage().is_none()
                && candidate.mask_stage().is_none())
    );
    assert!(!report.envelope_candidates.is_empty());

    // Unbound legacy composition has no original mask authority.
    let mut unbound = begin_retained_position_composition(
        capture,
        &base,
        &sources,
        &deferred,
        RetainedFamilyCompositionScratch::default(),
        true,
    )
    .unwrap();
    let request = loop {
        let PositionCompositionProgress::NeedsMaterialization(request) = unbound
            .advance(capture, &deferred, &Frame::default())
            .unwrap()
        else {
            panic!("the Angle mask adoption suspends")
        };
        if is_mask(&request) {
            break request;
        }
        unbound
            .resume_materialization(capture, request.request_id, respond(&request), None)
            .unwrap();
    };
    assert!(
        unbound
            .pending_mask_locator(request.request_id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn full_coverage_and_release_make_masks_inactive_or_reveal_the_continuing_phase() {
    let base = angles(0., 0.);
    let current = Current::new(true);
    let control = crossfade_s1;
    let resolving = context(&control, &current, true);

    // Full coverage: the full mask suppresses every covered lower stage, including M1.
    let capture = Uuid::new_v4();
    let registry = CapturedPositionProgram::new(capture, &base, &covered()).unwrap();
    let report = discover(&registry.branch(), &resolving, capture, 64).unwrap();
    let [m2_adoption, m2_transition] = report.mask_candidates.clone().try_into().unwrap();
    assert_eq!(m2_adoption.stage(), PositionMaskStage::Adoption);
    let mut released = registry.branch();
    released.replace_source(RESUME, None).unwrap();
    let revealed = discover(&released, &resolving, capture, 64).unwrap();
    let [
        m1_adoption,
        m1_transition,
        released_m2_adoption,
        released_m2_transition,
    ] = revealed.mask_candidates.clone().try_into().unwrap();
    assert_eq!(
        released_m2_adoption, m2_adoption,
        "M2 keeps its original identity"
    );
    assert_eq!(released_m2_transition, m2_transition);
    assert!(revealed.envelope_candidates.len() > report.envelope_candidates.len());
    for locator in [&m1_adoption, &m1_transition] {
        for &role in operands(locator.stage()) {
            inactive(operand(
                &registry.branch(),
                locator,
                role,
                &resolving,
                capture,
            ));
        }
    }
    // Lower envelope stages are covered too; the full mask has no stage of its own.
    for locator in &revealed.envelope_candidates {
        if !report.envelope_candidates.contains(locator) {
            let mut replay = registry
                .branch()
                .begin_envelope_operand(
                    locator,
                    match locator.stage() {
                        PositionCompletionStage::EndpointOutput => {
                            crate::PositionEnvelopeOperand::EndpointTarget
                        }
                        PositionCompletionStage::Activation => {
                            crate::PositionEnvelopeOperand::ActivationEndpoint
                        }
                    },
                    &resolving,
                    RetainedFamilyCompositionScratch::default(),
                    true,
                )
                .unwrap();
            inactive(
                replay
                    .advance(capture, &resolving, &Frame::default())
                    .unwrap(),
            );
        }
    }
    // M2 above the full mask stays active over the full mask's value and the Resume.
    let full_slots = [M2 + 1, M2 + 1];
    assert_operands_match_prefix(
        &registry.branch(),
        &[m2_adoption.clone(), m2_transition.clone()],
        &full_slots,
        &resolving,
        capture,
    );
    let calls = current.calls.get();
    let _ = ordinary(
        &registry.branch(),
        &resolving,
        capture,
        RetainedFamilyCompositionScratch::default(),
    );
    assert_eq!(
        current.calls.get(),
        calls,
        "covered S1 never samples Current"
    );
    // Releasing the full mask reveals the continuing original phase: the same captured lower
    // sources, not restarted ones, equal to the program that never had the full mask.
    let (value, _) = ordinary(
        &released,
        &resolving,
        capture,
        RetainedFamilyCompositionScratch::default(),
    );
    assert_eq!(value, reference(&base, &masked(), &resolving).0);
    assert_operands_match_prefix(
        &released,
        &[m1_adoption, m1_transition, m2_adoption, m2_transition],
        &[M1, M1, M2 + 1, M2 + 1],
        &resolving,
        capture,
    );

    // Released partial mask: its stages are Inactive; M2's prefix now omits M1.
    let capture = Uuid::new_v4();
    let registry = CapturedPositionProgram::new(capture, &base, &masked()).unwrap();
    let original = original(&registry, &resolving, capture);
    let mut released = registry.branch();
    released.replace_source(M1, None).unwrap();
    for locator in &original.masks[..2] {
        for &role in operands(locator.stage()) {
            inactive(operand(&released, locator, role, &resolving, capture));
        }
    }
    assert_operands_match_prefix(
        &released,
        &original.masks[2..],
        &[M2, M2],
        &resolving,
        capture,
    );
    let mut without_m1 = masked();
    without_m1.remove(M1);
    assert_eq!(
        ready(operand(
            &released,
            &original.masks[2],
            PositionMaskOperand::AdoptionInput,
            &resolving,
            capture
        )),
        reference(&base, &without_m1[..M2 - 1], &resolving).0
    );
    let report = discover(&released, &resolving, capture, 64).unwrap();
    assert_eq!(report.mask_candidates, original.masks[2..]);
}

#[test]
fn masks_above_and_below_an_independent_resume_keep_identity_prefix_and_coverage() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let registry = CapturedPositionProgram::new(capture, &base, &masked()).unwrap();
    let current = Current::new(true);
    let control = crossfade_s1;
    let resolving = context(&control, &current, true);
    let original = original(&registry, &resolving, capture);
    let unchosen = registry.branch();
    let before = original
        .masks
        .iter()
        .map(|locator| {
            operands(locator.stage())
                .iter()
                .map(|&role| ready(operand(&unchosen, locator, role, &resolving, capture)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for endpoint in [
        PositionResumeEndpoint::Outgoing,
        PositionResumeEndpoint::Incoming,
    ] {
        let mut chosen = registry.branch();
        chosen.choose_resume(resume_scope(), endpoint).unwrap();
        assert!(
            chosen.conditioned_sources().unwrap()[RESUME]
                .as_ref()
                .is_some(),
            "the Resume source stays present"
        );
        // The decision changes only the Resume: discovery reaches the same original masks.
        let report = discover(&chosen, &resolving, capture, 64).unwrap();
        assert_eq!(report.mask_candidates, original.masks);
        // Below the Resume: M1's operands are unchanged.
        for (locator, expected) in original.masks[..2].iter().zip(&before[..2]) {
            let actual = operands(locator.stage())
                .iter()
                .map(|&role| ready(operand(&chosen, locator, role, &resolving, capture)))
                .collect::<Vec<_>>();
            assert_eq!(&actual, expected);
        }
        // Above it: M2's prefix is the newly composed prefix, not stale lower history.
        let chosen_input = ready(operand(
            &chosen,
            &original.masks[2],
            PositionMaskOperand::AdoptionInput,
            &resolving,
            capture,
        ));
        assert_ne!(
            chosen_input, before[2][0],
            "{endpoint:?} changes M2's prefix"
        );
        assert_operands_match_prefix(
            &chosen,
            &original.masks,
            &[M1, M1, M2, M2],
            &resolving,
            capture,
        );
        let (value, _) = ordinary(
            &chosen,
            &resolving,
            capture,
            RetainedFamilyCompositionScratch::default(),
        );
        assert_eq!(
            value,
            reference(
                &base,
                &present(&chosen.conditioned_sources().unwrap()),
                &resolving
            )
            .0
        );

        // An unrelated descendant Resume does not authenticate a released mask: with M2
        // released, the chosen Resume below it is still reached, but M2 is Inactive.
        let mut released = chosen.clone();
        released.replace_source(M2, None).unwrap();
        for locator in &original.masks[2..] {
            for &role in operands(locator.stage()) {
                inactive(operand(&released, locator, role, &resolving, capture));
            }
        }
        // Releasing the lower mask makes exactly M1's stages Inactive.
        let mut lower_released = chosen.clone();
        lower_released.replace_source(M1, None).unwrap();
        for locator in &original.masks[..2] {
            inactive(operand(
                &lower_released,
                locator,
                operands(locator.stage())[0],
                &resolving,
                capture,
            ));
        }
        assert_eq!(
            discover(&released, &resolving, capture, 64)
                .unwrap()
                .mask_candidates,
            original.masks[..2]
        );
    }
    // Releasing the unrelated descendant Resume changes M2's prefix, never its identity:
    // the mask is authenticated by its own original slot alone.
    let mut without_resume = registry.branch();
    without_resume.replace_source(RESUME, None).unwrap();
    assert_eq!(
        discover(&without_resume, &resolving, capture, 64)
            .unwrap()
            .mask_candidates,
        original.masks
    );
    assert_operands_match_prefix(
        &without_resume,
        &original.masks,
        &[M1, M1, M2, M2],
        &resolving,
        capture,
    );
    // The Resume's own reached stages are never mask locators.
    let report = discover(&unchosen, &resolving, capture, 64).unwrap();
    assert!(
        report
            .candidates
            .iter()
            .all(|candidate| candidate.mask_stage().is_none())
    );
}

#[test]
fn prefix_current_is_captured_once_and_the_parent_suffix_is_not_applied() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let registry = CapturedPositionProgram::new(capture, &base, &masked()).unwrap();
    let current = Current::new(true);
    let control = crossfade_s1;
    let resolving = context(&control, &current, true);
    let deferred = context(&control, &current, false);
    let original = original(&registry, &resolving, capture);
    let [m1_adoption, m1_transition, m2_adoption, m2_transition] = original.masks.clone();

    // Every mask is above S1: one Current read per replay evaluator, none on repeated reads.
    for locator in &original.masks {
        for &role in operands(locator.stage()) {
            let calls = current.calls.get();
            let frame = Frame::default();
            let mut replay = registry
                .branch()
                .begin_mask_operand(
                    locator,
                    role,
                    &resolving,
                    RetainedFamilyCompositionScratch::default(),
                    true,
                )
                .unwrap();
            let value = ready(replay.advance(capture, &resolving, &frame).unwrap());
            assert_eq!(current.calls.get() - calls, 1);
            let frame_calls = frame.calls.get();
            for _ in 0..3 {
                assert_eq!(
                    ready(replay.advance(capture, &resolving, &frame).unwrap()),
                    value
                );
                assert_eq!(current.calls.get() - calls, 1);
                assert_eq!(frame.calls.get(), frame_calls);
            }
        }
    }

    // M1 TransitionFrom: the prefix suspends at the S0 envelope, then at M1's own adoption.
    // A distinctive adoption response is the operand itself: neither M1's transition nor
    // the Resume, M2 or S5 suffix is applied.
    let frame = Frame::blocking(&[s0_target()]);
    let calls = current.calls.get();
    let mut replay = registry
        .branch()
        .begin_mask_operand(
            &m1_transition,
            PositionMaskOperand::TransitionFrom,
            &deferred,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    let envelope = suspended(replay.advance(capture, &deferred, &frame).unwrap());
    assert!(!is_mask(&envelope));
    assert_eq!(
        current.calls.get(),
        calls,
        "Current is read only when S1 is reached"
    );
    replay
        .resume_materialization(capture, envelope.request_id, respond(&envelope), None)
        .unwrap();
    let adoption = suspended(replay.advance(capture, &deferred, &frame).unwrap());
    assert!(matches!(
        adoption.operation,
        PositionCompositionOperation::MaskAdoption { .. }
    ));
    assert_eq!(
        adoption.origin().unwrap().original_source_indices(),
        [M1],
        "M1's own adoption request"
    );
    assert_eq!(current.calls.get() - calls, 1);
    let distinctive = target(0, [11., 22., 1.]);
    replay
        .resume_materialization(capture, adoption.request_id, distinctive.clone(), None)
        .unwrap();
    let frame_calls = frame.calls.get();
    for _ in 0..3 {
        assert_eq!(
            ready(replay.advance(capture, &deferred, &frame).unwrap()),
            distinctive
        );
        assert_eq!(frame.calls.get(), frame_calls);
        assert_eq!(current.calls.get() - calls, 1, "Current captured once");
    }
    assert!(
        replay
            .resume_materialization(capture, adoption.request_id, distinctive, None)
            .is_err(),
        "a stopped replay has no pending response"
    );

    // M1 AdoptionInput stops before the adoption: no MaskAdoption request is ever issued.
    let mut replay = registry
        .branch()
        .begin_mask_operand(
            &m1_adoption,
            PositionMaskOperand::AdoptionInput,
            &deferred,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    let value = ready(
        replay
            .advance(capture, &deferred, &Frame::default())
            .unwrap(),
    );
    assert_eq!(value, as_angles(&value), "the unadopted Angle prefix");

    // M1 TransitionTo is the original materialized mask: its blocked transition is never
    // requested, while the prefix's own M1 adoption is answered once.
    let frame = Frame::blocking(&[m1_value()]);
    let mut replay = registry
        .branch()
        .begin_mask_operand(
            &m1_transition,
            PositionMaskOperand::TransitionTo,
            &deferred,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    let mut answered = Vec::new();
    let value = loop {
        match replay.advance(capture, &deferred, &frame).unwrap() {
            PositionStageOperandProgress::OperandReady(value) => break value,
            PositionStageOperandProgress::Inactive => panic!("M2 is reachable"),
            PositionStageOperandProgress::NeedsMaterialization(request) => {
                assert!(
                    !matches!(
                        request.operation,
                        PositionCompositionOperation::MaskTransition { .. }
                    ),
                    "the located transition itself is never requested"
                );
                answered.push(request.request_id);
                replay
                    .resume_materialization(capture, request.request_id, respond(&request), None)
                    .unwrap();
            }
        }
    };
    assert_eq!(value, m1_value());
    assert_eq!(answered.len(), 1, "only M1's adoption, once");
    // AdoptionInput and TransitionFrom differ exactly by the adoption.
    for (adoption, transition, adopt) in [
        (
            &m1_adoption,
            &m1_transition,
            as_target as fn(&AttributeValue) -> AttributeValue,
        ),
        (&m2_adoption, &m2_transition, as_angles),
    ] {
        let input = ready(operand(
            &registry.branch(),
            adoption,
            PositionMaskOperand::AdoptionInput,
            &resolving,
            capture,
        ));
        let from = ready(operand(
            &registry.branch(),
            transition,
            PositionMaskOperand::TransitionFrom,
            &resolving,
            capture,
        ));
        assert_eq!(from, adopt(&input));
    }
    assert_ne!(
        ready(operand(
            &registry.branch(),
            &m1_adoption,
            PositionMaskOperand::AdoptionInput,
            &resolving,
            capture,
        )),
        ready(operand(
            &registry.branch(),
            &m1_transition,
            PositionMaskOperand::TransitionFrom,
            &resolving,
            capture,
        )),
        "M1 adoption changes the representation"
    );

    // The consumer applies the suffix once: the parent output is still the original.
    assert_eq!(
        ordinary(
            &registry.branch(),
            &resolving,
            capture,
            RetainedFamilyCompositionScratch::default()
        )
        .0,
        original.value
    );
}

#[test]
fn foreign_registry_unsupported_roles_and_unavailable_current_reject_without_fallback() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let sources = masked();
    let registry = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    let current = Current::new(true);
    let control = crossfade_s1;
    let resolving = context(&control, &current, true);
    let original = original(&registry, &resolving, capture);

    // Equal capture UUID and equal sources are still another registry.
    let foreign = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    let all = [
        PositionMaskOperand::AdoptionInput,
        PositionMaskOperand::TransitionFrom,
        PositionMaskOperand::TransitionTo,
    ];
    let calls = current.calls.get();
    for locator in &original.masks {
        for role in all {
            let supported = role.stage() == locator.stage();
            assert_eq!(
                registry
                    .branch()
                    .validate_mask_operand(locator, role)
                    .is_ok(),
                supported
            );
            assert!(
                foreign
                    .branch()
                    .validate_mask_operand(locator, role)
                    .is_err()
            );
            assert!(
                foreign
                    .branch()
                    .begin_mask_operand(
                        locator,
                        role,
                        &resolving,
                        RetainedFamilyCompositionScratch::default(),
                        true,
                    )
                    .is_err()
            );
            if !supported {
                assert!(
                    registry
                        .branch()
                        .begin_mask_operand(
                            locator,
                            role,
                            &resolving,
                            RetainedFamilyCompositionScratch::default(),
                            true,
                        )
                        .is_err()
                );
            }
        }
    }
    assert_eq!(
        current.calls.get(),
        calls,
        "rejection precedes any evaluation"
    );
    let foreign_report = discover(&foreign.branch(), &resolving, capture, 64).unwrap();
    assert_eq!(foreign_report.mask_candidates.len(), 4);
    for (foreign, original) in foreign_report.mask_candidates.iter().zip(&original.masks) {
        assert_eq!(foreign.stage(), original.stage());
        assert_ne!(foreign, original);
    }

    // Unavailable Current: Requires, read once, no Live fallback, terminal replay.
    let missing = Current::new(false);
    let unavailable = context(&control, &missing, true);
    let frame = Frame::default();
    let mut replay = registry
        .branch()
        .begin_mask_operand(
            &original.masks[0],
            PositionMaskOperand::AdoptionInput,
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
}

#[test]
fn bad_responses_unwind_and_bounded_discovery_retain_the_original_output() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let registry = CapturedPositionProgram::new(capture, &base, &masked()).unwrap();
    let current = Current::new(true);
    let control = crossfade_s1;
    let resolving = context(&control, &current, true);
    let deferred = context(&control, &current, false);
    let original = original(&registry, &resolving, capture);
    let m1_transition = &original.masks[1];
    let begin = |context: &FamilyCompositionContext<'_>,
                 scratch: RetainedFamilyCompositionScratch| {
        registry
            .branch()
            .begin_mask_operand(
                m1_transition,
                PositionMaskOperand::TransitionFrom,
                context,
                scratch,
                true,
            )
            .unwrap()
    };

    // Retryable: rejected before mutation; the same MaskAdoption stays pending.
    let mut replay = begin(&deferred, RetainedFamilyCompositionScratch::default());
    let request = suspended(
        replay
            .advance(capture, &deferred, &Frame::default())
            .unwrap(),
    );
    assert!(is_mask(&request));
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
    for wrong in [
        AttributeValue::Normalized(0.5),
        as_angles(&good), // valid Position, wrong representation for the Target mask
    ] {
        assert!(
            replay
                .resume_materialization(capture, request.request_id, wrong, None)
                .is_err()
        );
    }
    assert_eq!(
        suspended(
            replay
                .advance(capture, &deferred, &Frame::default())
                .unwrap()
        )
        .request_id,
        request.request_id
    );
    replay
        .resume_materialization(capture, request.request_id, good.clone(), None)
        .unwrap();
    assert_eq!(
        ready(
            replay
                .advance(capture, &deferred, &Frame::default())
                .unwrap()
        ),
        good
    );
    replay.into_scratch();

    // Terminal error inside the prefix: nothing is observed afterwards.
    let frame = Frame::default();
    frame.block(s1_target(), Block::Error);
    let mut replay = begin(&resolving, RetainedFamilyCompositionScratch::default());
    assert!(replay.advance(capture, &resolving, &frame).is_err());
    let calls = frame.calls.get();
    assert!(replay.advance(capture, &resolving, &frame).is_err());
    assert_eq!(frame.calls.get(), calls);
    replay.into_scratch();

    // Terminal unwind: only this replay is poisoned; its recovered scratch reproduces the
    // operand and then the original parent output and trace.
    let frame = Frame::default();
    frame.block(s0_target(), Block::Panic);
    let mut replay = begin(&resolving, RetainedFamilyCompositionScratch::default());
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        replay.advance(capture, &resolving, &frame)
    }));
    assert!(unwound.is_err());
    assert!(
        replay
            .advance(capture, &resolving, &Frame::default())
            .is_err()
    );
    let mut fresh = begin(&resolving, replay.into_scratch());
    assert_eq!(
        ready(
            fresh
                .advance(capture, &resolving, &Frame::default())
                .unwrap()
        ),
        good
    );
    let (value, query) = ordinary(
        &registry.branch(),
        &resolving,
        capture,
        fresh.into_scratch(),
    );
    assert_eq!(value, original.value);
    assert_eq!(query, original.query);

    // Bounded discovery: one route short fails with no report; the exact budget completes.
    let full = discover(&registry.branch(), &resolving, capture, 64).unwrap();
    assert_eq!(full.unmapped_routes, 0);
    let mut short = registry
        .branch()
        .begin_stage_discovery(
            &resolving,
            RetainedFamilyCompositionScratch::default(),
            true,
            full.inspected_routes - 1,
        )
        .unwrap();
    assert!(
        short
            .advance(capture, &resolving, &Frame::default())
            .is_err()
    );
    assert!(short.report().is_err(), "no partial enumeration survives");
    let exact = discover(
        &registry.branch(),
        &resolving,
        capture,
        full.inspected_routes,
    )
    .unwrap();
    assert_eq!(exact.mask_candidates, original.masks);
    assert_eq!(exact.inspected_routes, full.inspected_routes);
    let (value, query) = ordinary(
        &registry.branch(),
        &resolving,
        capture,
        short.into_scratch(),
    );
    assert_eq!(value, original.value);
    assert_eq!(query, original.query);

    // Discovery through real mask suspensions records each mask stage exactly once.
    let frame = Frame::blocking(&[m1_value()]);
    let mut discovery = registry
        .branch()
        .begin_stage_discovery(
            &deferred,
            RetainedFamilyCompositionScratch::default(),
            true,
            full.inspected_routes,
        )
        .unwrap();
    let mut suspended_masks = 0;
    let report = loop {
        match discovery.advance(capture, &deferred, &frame).unwrap() {
            PositionStageDiscoveryProgress::Complete(report) => break report,
            PositionStageDiscoveryProgress::NeedsMaterialization(request) => {
                suspended_masks += usize::from(is_mask(&request));
                discovery
                    .resume_materialization(capture, request.request_id, respond(&request), None)
                    .unwrap();
            }
        }
    };
    assert_eq!(
        suspended_masks, 3,
        "M1 Adoption, M1 Transition and M2 Adoption"
    );
    assert_eq!(report.mask_candidates, original.masks);
    assert_eq!(report.inspected_routes, full.inspected_routes);
    let (value, query) = ordinary(
        &registry.branch(),
        &resolving,
        capture,
        discovery.into_scratch(),
    );
    assert_eq!(value, original.value);
    assert_eq!(query, original.query);
}
