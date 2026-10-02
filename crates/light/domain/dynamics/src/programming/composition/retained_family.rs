//! Deferred whole-owner sources participate alongside fixed-address component samples.
//! Resolve only visible dependencies, then reuse the component compositor. A whole expression
//! is classified by its actual result, including any live-frame conversion during activation.
use super::*;
use crate::programming::expression_family::{
    GraphOperationKind, GraphOperationOperand, GraphOperationProgress,
    GraphOperationReachedCallback, GraphOperationSelection, GraphReachedAction,
};
use crate::{
    CompiledCoupledExpression, CompiledProgrammingFamilyExpression, FamilyExpressionObserver,
    FamilyExpressionOperation, FamilyExpressionStep, WholeFamilyExpressionFrameResolver,
};

mod base_evaluation;
mod position_program;
pub use super::position_segment::{PositionSegmentOperation, PositionSegmentRequest};
mod position_completion;
pub use base_evaluation::{
    BaseMaterializationOperation as PositionCompositionBaseOperation,
    BaseMaterializationRequest as PositionCompositionBaseRequest,
};
pub use position_completion::{
    CompletionRequest as PositionCompositionCompletionRequest, PositionCompletionStage,
};
pub use position_program::*;
mod conditioning;
mod position_conditioning;
pub use position_conditioning::*;
mod coupled;
mod orthogonal;
#[cfg(test)]
mod preparation_origins_tests;
#[cfg(test)]
mod scratch_tests;
mod whole;

#[derive(Clone)]
pub enum FamilyCompositionSample {
    Known(FamilySample),
    WholeExpression {
        expression: Arc<CompiledProgrammingFamilyExpression>,
        rank: FamilySampleRank,
        activation_mix: f32,
    },
    CoupledExpression {
        expression: Arc<CompiledCoupledExpression>,
        rank: FamilySampleRank,
        activation_mix: f32,
    },
}

impl From<FamilySample> for FamilyCompositionSample {
    fn from(sample: FamilySample) -> Self {
        Self::Known(sample)
    }
}

/// Existing materialized callers use their coherent adoption context through the same
/// transition engine. Retained callers provide a full frame resolver at the public boundary.
pub(super) fn compose_known_dynamic_family(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: &[FamilySample],
    context: &FamilyCompositionContext<'_>,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<AttributeValue, TransitionError> {
    struct ContextFrame<'a, 'b> {
        base: &'a AttributeValue,
        samples: &'a [FamilySample],
        context: &'a FamilyCompositionContext<'b>,
    }
    impl WholeFamilyExpressionFrameResolver for ContextFrame<'_, '_> {
        fn resolve(
            &self,
            _: TransitionRequirement,
            from: &AttributeValue,
            to: &AttributeValue,
            operation: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            let owner = to.programming_owner().unwrap_or(ProgrammingOwner::Focus);
            let address = DynamicValueAddress::whole_family(owner, to)?;
            let adopted = adopt(from.clone(), &address, self.context, self.base)?;
            let model = self.samples.iter().find_map(|sample| {
                (sample.address.address().representation == address.representation)
                    .then(|| sample.address.native_model())
                    .flatten()
            });
            let compiled = CompiledProgrammingTransition::new(adopted, to.clone(), model)?;
            match operation {
                FamilyExpressionOperation::Transition { progress } => compiled.sample(progress),
                FamilyExpressionOperation::Scale { factor } => compiled.scale(factor),
            }
        }
    }
    compose_family_inputs(
        owner,
        base,
        samples.iter().cloned().map(Into::into),
        context,
        &ContextFrame {
            base,
            samples,
            context,
        },
        None,
        scratch,
    )
}

impl FamilyCompositionSample {
    fn rank(&self) -> FamilySampleRank {
        match self {
            Self::Known(sample) => sample.rank,
            Self::WholeExpression { rank, .. } | Self::CoupledExpression { rank, .. } => *rank,
        }
    }

    fn endpoint_control(
        &self,
        context: &FamilyCompositionContext<'_>,
    ) -> FamilyEndpointOutputControl {
        match self {
            Self::Known(sample) => sample.endpoint_control(context),
            _ => endpoint_output::control(self.rank(), context),
        }
    }

    fn order_key(&self) -> (FamilySampleRank, Option<ProgrammingComponent>) {
        match self {
            Self::Known(sample) => sample.order_key(),
            Self::WholeExpression { rank, .. } | Self::CoupledExpression { rank, .. } => {
                (*rank, None)
            }
        }
    }

    fn is_orthogonal(&self) -> bool {
        match self {
            Self::Known(sample) => orthogonal(sample.address.address()),
            Self::CoupledExpression { expression, .. } => matches!(
                expression.footprint(crate::CoupledExpressionRole::Base),
                crate::CoupledExpressionFootprint::Inactive
            ),
            Self::WholeExpression { .. } => false,
        }
    }

    fn whole_mask(&self) -> Option<&FamilySample> {
        match self {
            Self::Known(sample)
                if sample.fix_at && sample.address.address().component.is_none() =>
            {
                Some(sample)
            }
            _ => None,
        }
    }
}

/// Lexical use metadata before original-registry binding. Source/member indices belong to
/// their route level's prepared scratch and are never public boundary authority.
#[derive(Clone)]
pub(super) enum PreparedPositionStageUse {
    RootFinalSegment,
    /// Legacy materialized coupled endpoints have no original forest/member locator yet.
    /// Keep their lexical use distinct so they cannot alias a final segment boundary.
    UnsupportedCoupledEndpoint,
    UnderlayFor {
        consumer_origins: Vec<PreparedSourceOrigin>,
    },
    SourceCohort {
        consumer_origins: Vec<PreparedSourceOrigin>,
        expression: Arc<CompiledCoupledExpression>,
        endpoint_nodes: Vec<usize>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PreparedPositionStageKind {
    Adoption {
        component: Option<ProgrammingComponent>,
    },
    WholeSegmentTransition,
    /// Post-expression envelope operation of the source named by the route trigger.
    Envelope(position_completion::PositionCompletionStage),
}
#[derive(Clone)]
pub(super) struct PreparedPositionStageRoute {
    pub uses: Vec<PreparedPositionStageUse>,
    pub trigger_origins: Vec<PreparedSourceOrigin>,
    pub kind: PreparedPositionStageKind,
}
#[derive(Clone)]
pub(super) enum PreparedPositionGraphExpression {
    Whole(Arc<CompiledProgrammingFamilyExpression>),
    Coupled(Arc<CompiledCoupledExpression>),
}
#[derive(Clone)]
pub(super) struct PreparedPositionGraphRoute {
    pub uses: Vec<PreparedPositionStageUse>,
    pub trigger_origins: Vec<PreparedSourceOrigin>,
    pub expression: PreparedPositionGraphExpression,
    pub node: usize,
    pub kind: GraphOperationKind,
}
type PreparedPositionGraphPlanCallback<'a> = dyn FnMut(&PreparedPositionGraphRoute) -> Result<Vec<GraphOperationSelection>, TransitionError>
    + 'a;
type PreparedPositionStopCallback<'a> = dyn FnMut(usize) + 'a;
type PreparedPositionCohortCallback<'a> = dyn FnMut(
        &[PreparedPositionStageUse],
        &[PreparedSourceOrigin],
        &Arc<CompiledCoupledExpression>,
        &[usize],
    ) -> Result<Option<usize>, TransitionError>
    + 'a;
type PreparedPositionGraphReachedCallback<'a> =
    dyn FnMut(&PreparedPositionGraphRoute) -> Result<GraphReachedAction, TransitionError> + 'a;
type PreparedPositionGraphCallback<'a> = dyn FnMut(&PreparedPositionGraphRoute) -> Result<Option<GraphOperationOperand>, TransitionError>
    + 'a;
type PreparedPositionStageCallback<'a> = dyn FnMut(
        &PreparedPositionStageRoute,
    ) -> Result<Option<position_segment::PositionSegmentOperand>, TransitionError>
    + 'a;
/// Ordinary composition carries routing only when needed; it never scans/clones per-sample
/// stop candidates. Explicit operand replay opts into the matching work.
pub(super) struct PreparedPositionStageMatcher<'a> {
    callback: Option<&'a mut PreparedPositionStageCallback<'a>>,
    graph_callback: Option<&'a mut PreparedPositionGraphCallback<'a>>,
    graph_reached_callback: Option<&'a mut PreparedPositionGraphReachedCallback<'a>>,
    graph_plan_callback: Option<&'a mut PreparedPositionGraphPlanCallback<'a>>,
    stop_callback: Option<&'a mut PreparedPositionStopCallback<'a>>,
    cohort_callback: Option<&'a mut PreparedPositionCohortCallback<'a>>,
}
impl<'a> PreparedPositionStageMatcher<'a> {
    pub(super) fn disabled() -> Self {
        Self {
            callback: None,
            graph_callback: None,
            graph_reached_callback: None,
            graph_plan_callback: None,
            stop_callback: None,
            cohort_callback: None,
        }
    }
    pub(super) fn enabled(callback: &'a mut PreparedPositionStageCallback<'a>) -> Self {
        Self {
            callback: Some(callback),
            graph_callback: None,
            graph_reached_callback: None,
            graph_plan_callback: None,
            stop_callback: None,
            cohort_callback: None,
        }
    }
    pub(super) fn enabled_graph(callback: &'a mut PreparedPositionGraphCallback<'a>) -> Self {
        Self {
            callback: None,
            graph_callback: Some(callback),
            graph_reached_callback: None,
            graph_plan_callback: None,
            stop_callback: None,
            cohort_callback: None,
        }
    }
    pub(super) fn with_graph_reached(
        mut self,
        callback: Option<&'a mut PreparedPositionGraphReachedCallback<'a>>,
    ) -> Self {
        self.graph_reached_callback = callback;
        self
    }
    pub(super) fn with_resume_plan(
        mut self,
        plan: &'a mut PreparedPositionGraphPlanCallback<'a>,
        stop: &'a mut PreparedPositionStopCallback<'a>,
        cohort: &'a mut PreparedPositionCohortCallback<'a>,
    ) -> Self {
        self.graph_plan_callback = Some(plan);
        self.stop_callback = Some(stop);
        self.cohort_callback = Some(cohort);
        self
    }
    pub(super) fn completed_stop(&mut self, depth: usize) {
        if let Some(callback) = self.stop_callback.as_deref_mut() {
            callback(depth);
        }
    }
    pub(super) fn graph_reached_is_enabled(&self) -> bool {
        self.graph_reached_callback.is_some()
    }
    pub(super) fn graph_is_enabled(&self) -> bool {
        self.graph_callback.is_some() || self.graph_plan_callback.is_some()
    }
    pub(super) fn match_graph(
        &mut self,
        route: &PreparedPositionGraphRoute,
    ) -> Result<Option<GraphOperationOperand>, TransitionError> {
        match &mut self.graph_callback {
            Some(callback) => callback(route),
            None => Ok(None),
        }
    }
    pub(super) fn is_enabled(&self) -> bool {
        self.callback.is_some()
    }
    pub(super) fn match_route(
        &mut self,
        route: &PreparedPositionStageRoute,
    ) -> Result<Option<position_segment::PositionSegmentOperand>, TransitionError> {
        match &mut self.callback {
            Some(callback) => callback(route),
            None => Ok(None),
        }
    }
}

#[derive(Default)]
struct ResolvedBatch {
    samples: Vec<FamilySample>,
    origins: Vec<Vec<PreparedSourceOrigin>>,
    ordered: Vec<usize>,
    uses: Vec<PreparedPositionStageUse>,
}

enum BaseTask {
    EvaluatePositionSegment {
        evaluation: position_segment::PositionSegmentEvaluation,
        batch: ResolvedBatch,
    },
    CompleteKnownPosition {
        index: usize,
        completion: position_completion::PositionSourceCompletion,
    },
    EvaluateWhole {
        index: usize,
        inputs: whole::Inputs,
    },
    Coupled {
        index: usize,
        cursor: usize,
        step: usize,
        inputs: coupled::BaseInputs,
        uses: Vec<PreparedPositionStageUse>,
    },
    CollectCoupled {
        index: usize,
        cursor: usize,
        step: usize,
        inputs: coupled::BaseInputs,
        uses: Vec<PreparedPositionStageUse>,
    },
    Cohort {
        end: usize,
        candidates: Vec<FamilySample>,
        origins: Vec<Vec<PreparedSourceOrigin>>,
        uses: Vec<PreparedPositionStageUse>,
    },
    Compose {
        end: usize,
        uses: Vec<PreparedPositionStageUse>,
    },
    Resolve {
        cursor: usize,
        uses: Vec<PreparedPositionStageUse>,
    },
    Winner {
        cursor: usize,
        uses: Vec<PreparedPositionStageUse>,
    },
    Whole {
        index: usize,
    },
    Scan {
        winner: FamilySample,
        cursor: usize,
        batch: ResolvedBatch,
    },
    Candidate {
        winner: FamilySample,
        cursor: usize,
        batch: ResolvedBatch,
    },
}

enum BaseResult {
    Value(TracedValue),
    Sample(FamilySample),
}

#[derive(Clone)]
pub(super) struct TracedValue {
    pub value: AttributeValue,
    pub trace: Option<FamilyTraceNodeId>,
}

impl BaseResult {
    fn value(self) -> TracedValue {
        let Self::Value(value) = self else {
            unreachable!("composition task expected a family")
        };
        value
    }
    fn sample(self) -> FamilySample {
        let Self::Sample(value) = self else {
            unreachable!("composition task expected a source")
        };
        value
    }
}

/// An original caller slot and, after an exact cohort expands, its original member path.
/// A synthetic pair can contain multiple members of the same slot without losing identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreparedSourceOrigin {
    pub(super) original_index: usize,
    pub(super) member: Option<usize>,
}

/// Reuse across frames. The resolution cache is invalidated for each coherent frame; original
/// expressions, models and source membership remain in the caller's compiled sources.
#[derive(Default)]
pub struct RetainedFamilyCompositionScratch {
    sources: Vec<FamilyCompositionSample>,
    // Original caller slots survive filtering, expansion and synthetic Angle bundling.
    source_origins: Vec<Vec<PreparedSourceOrigin>>,
    known: Vec<FamilySample>,
    known_origins: Vec<Vec<PreparedSourceOrigin>>,
    bundled: Vec<FamilySample>,
    bundled_membership: Vec<Vec<usize>>,
    resolved: Vec<Option<FamilySample>>,
    ordered: Vec<usize>,
    batches: Vec<ResolvedBatch>,
    tasks: Vec<BaseTask>,
    components: ComponentCompositionScratch,
    trace: FamilyTraceArena,
    trace_enabled: bool,
    // Deferred Position SourceCohorts are composed one at a time over a captured lower prefix.
    // Their members are ordinary Known/WholeExpression samples, so this one nested scratch is
    // sufficient and keeps its buffers and trace arena across frames and error retries.
    source_cohort_scratch: Option<Box<RetainedFamilyCompositionScratch>>,
    source_cohort_candidates: Vec<FamilyCompositionSample>,
}

impl RetainedFamilyCompositionScratch {
    pub fn family_trace(&self) -> &FamilyTraceArena {
        &self.trace
    }
}

/// Whole expressions own a complete family, while Known component expressions keep their
/// component masks. A missing whole endpoint reads the eligible lower base stack. Explicit
/// Color orthogonals are applied once, after base composition; whole FixAT cuts both passes.
pub fn compose_retained_dynamic_family(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: &[FamilyCompositionSample],
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<AttributeValue, TransitionError> {
    compose_family_inputs(
        owner,
        base,
        samples.iter().cloned(),
        context,
        frame,
        None,
        scratch,
    )
}

/// Trace and value are produced by one compositor pass. The returned root belongs to `scratch`;
/// read it before the next composition. Unsupported deferred paths return an explicit error.
pub fn compose_retained_dynamic_family_traced(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: &[FamilyCompositionSample],
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<AttributeValue, TransitionError> {
    let result = compose_family_inputs_impl(
        owner,
        base,
        samples.iter().cloned(),
        context,
        frame,
        None,
        scratch,
        true,
    )?;
    if let Some(root) = result.trace {
        scratch.trace.set_root(root);
    }
    Ok(result.value)
}

fn compose_family_inputs(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: impl Iterator<Item = FamilyCompositionSample>,
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    base_only: Option<&AttributeValue>,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<AttributeValue, TransitionError> {
    compose_family_inputs_impl(
        owner, base, samples, context, frame, base_only, scratch, false,
    )
    .map(|result| result.value)
}

fn compose_family_inputs_impl(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: impl Iterator<Item = FamilyCompositionSample>,
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    base_only: Option<&AttributeValue>,
    scratch: &mut RetainedFamilyCompositionScratch,
    tracing: bool,
) -> Result<TracedValue, TransitionError> {
    let base_trace = prepare_family_inputs(owner, base, samples, context, scratch, tracing)?;
    let ordered = std::mem::take(&mut scratch.ordered);
    let result = if let Some(original_base) = base_only {
        compose_base(
            base,
            base_trace,
            &ordered,
            context,
            frame,
            original_base,
            scratch,
        )
    } else {
        compose_masks(base, base_trace, &ordered, context, frame, scratch)
    };
    scratch.ordered = ordered;
    if result.is_err() {
        scratch.resolved.fill(None);
        scratch.tasks.clear();
        scratch.components.components.clear();
        scratch.components.edits.clear();
        scratch.components.pending_native_model = None;
    }
    result
}

fn prepare_family_inputs(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: impl Iterator<Item = FamilyCompositionSample>,
    context: &FamilyCompositionContext<'_>,
    scratch: &mut RetainedFamilyCompositionScratch,
    tracing: bool,
) -> Result<Option<FamilyTraceNodeId>, TransitionError> {
    prepare_family_inputs_with_origins(
        owner,
        base,
        samples.map(|sample| (sample, Vec::new())),
        context,
        scratch,
        tracing,
    )
}

/// Retain explicit original slot membership through the same preparation as ordinary callers.
/// Slots may contain holes after release; no rank or value matching reconstructs membership.
fn prepare_family_inputs_with_origins(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: impl Iterator<Item = (FamilyCompositionSample, Vec<usize>)>,
    context: &FamilyCompositionContext<'_>,
    scratch: &mut RetainedFamilyCompositionScratch,
    tracing: bool,
) -> Result<Option<FamilyTraceNodeId>, TransitionError> {
    scratch.sources.clear();
    scratch.source_origins.clear();
    scratch.known.clear();
    scratch.known_origins.clear();
    scratch.bundled.clear();
    scratch.bundled_membership.clear();
    scratch.trace_enabled = tracing;
    scratch.trace.clear();
    let base_trace = tracing.then(|| scratch.trace.base());
    base.validate_programming_address(&owner.key())?;
    if base.spread_control_points() != 0 || matches!(base, AttributeValue::GroupFamily(_)) {
        return Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints,
        ));
    }
    scratch.components.components.clear();
    scratch.components.edits.clear();
    scratch.components.pending_native_model = None;
    for (sample, original_slots) in samples {
        let origins = original_slots
            .into_iter()
            .map(|original_index| PreparedSourceOrigin {
                original_index,
                member: None,
            })
            .collect::<Vec<_>>();
        sample.rank().validate()?;
        endpoint_output::validate(sample.endpoint_control(context))?;
        match &sample {
            FamilyCompositionSample::CoupledExpression {
                expression,
                rank,
                activation_mix,
            } => {
                for (sample, member) in coupled::prepare_with_membership(
                    expression.clone(),
                    *rank,
                    *activation_mix,
                    owner,
                )? {
                    let origins = origins
                        .iter()
                        .map(|origin| PreparedSourceOrigin {
                            original_index: origin.original_index,
                            member,
                        })
                        .collect::<Vec<_>>();
                    match sample {
                        FamilyCompositionSample::Known(sample) => {
                            scratch.known.push(sample);
                            scratch.known_origins.push(origins.clone());
                        }
                        sample => {
                            scratch.sources.push(sample);
                            scratch.source_origins.push(origins.clone());
                        }
                    }
                }
            }
            FamilyCompositionSample::Known(sample) => {
                ensure(
                    sample.address.address().owner() == owner,
                    "composition contains a different owner",
                )?;
                ensure(
                    sample.activation_mix.is_finite()
                        && (0.0..=1.0).contains(&sample.activation_mix),
                    "Dynamic activation influence must be between zero and one",
                )?;
                sample.validate_value()?;
                if sample.participates() {
                    scratch.known.push(sample.clone());
                    scratch.known_origins.push(origins);
                }
            }
            FamilyCompositionSample::WholeExpression {
                expression,
                activation_mix,
                ..
            } => {
                ensure(
                    expression.owner() == owner,
                    "composition contains a different owner",
                )?;
                ensure(
                    activation_mix.is_finite() && (0.0..=1.0).contains(activation_mix),
                    "Dynamic activation influence must be between zero and one",
                )?;
                if *activation_mix > 0.0 && expression.participates() {
                    scratch.sources.push(sample.clone());
                    scratch.source_origins.push(origins);
                }
            }
        }
    }
    if owner == ProgrammingOwner::Position {
        if scratch.known_origins.iter().all(Vec::is_empty) {
            bundle_angles(
                &scratch.known,
                &mut scratch.bundled,
                &mut scratch.components,
                tracing,
            )?;
            scratch
                .source_origins
                .extend(scratch.bundled.iter().map(|_| Vec::new()));
        } else {
            bundle_angles_with_membership(
                &scratch.known,
                &mut scratch.bundled,
                &mut scratch.components,
                tracing,
                Some(&mut scratch.bundled_membership),
            )?;
            for members in &scratch.bundled_membership {
                let mut origins = Vec::new();
                for &member in members {
                    for &origin in &scratch.known_origins[member] {
                        if !origins.contains(&origin) {
                            origins.push(origin);
                        }
                    }
                }
                scratch.source_origins.push(origins);
            }
        }
        scratch
            .sources
            .extend(scratch.bundled.iter().cloned().map(Into::into));
    } else {
        scratch
            .sources
            .extend(scratch.known.iter().cloned().map(Into::into));
        scratch
            .source_origins
            .extend(scratch.known_origins.iter().cloned());
    }
    scratch.ordered.clear();
    scratch.ordered.extend(0..scratch.sources.len());
    scratch
        .ordered
        .sort_unstable_by_key(|&index| scratch.sources[index].order_key());
    for pair in scratch.ordered.windows(2) {
        match (&scratch.sources[pair[0]], &scratch.sources[pair[1]]) {
            (FamilyCompositionSample::Known(a), FamilyCompositionSample::Known(b)) => {
                validate_source_order(a, b)?;
            }
            (a, b) => ensure(
                a.rank() != b.rank(),
                "Dynamic composition requires distinct stable source/lane identities",
            )?,
        }
    }
    scratch.ordered.retain(|&index| {
        !matches!(
            scratch.sources[index].endpoint_control(context),
            FamilyEndpointOutputControl::Suppressed
        )
    });
    scratch.resolved.clear();
    scratch.resolved.resize(scratch.sources.len(), None);
    Ok(base_trace)
}

fn compose_masks(
    base: &AttributeValue,
    base_trace: Option<FamilyTraceNodeId>,
    ordered: &[usize],
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<TracedValue, TransitionError> {
    let last_full = ordered.iter().rposition(|&index| {
        scratch.sources[index]
            .whole_mask()
            .is_some_and(|sample| sample.activation_mix == 1.0)
    });
    let (mut value, mut start) = if let Some(cursor) = last_full {
        let mask = scratch.sources[ordered[cursor]]
            .whole_mask()
            .expect("whole mask")
            .clone();
        let output = apply_whole(
            base.clone(),
            &mask,
            context,
            base,
            Some(frame),
            scratch.trace_enabled,
        )?;
        let trace = scratch.trace_enabled.then(|| {
            whole_mask_trace(
                &mask,
                base_trace.expect("enabled trace base"),
                output.appearance.clone(),
                &mut scratch.trace,
            )
        });
        (
            TracedValue {
                value: output.value,
                trace,
            },
            cursor + 1,
        )
    } else {
        (
            TracedValue {
                value: base.clone(),
                trace: base_trace,
            },
            0,
        )
    };
    for cursor in start..ordered.len() {
        let Some(mask) = scratch.sources[ordered[cursor]].whole_mask().cloned() else {
            continue;
        };
        value = compose_segment_with_orthogonals(
            &value.value,
            value.trace,
            &ordered[start..cursor],
            context,
            frame,
            base,
            scratch,
        )?;
        // A value-only Position adoption cannot certify how old Angle/Target fields map
        // into the new representation. Preserve that uncertainty before the partial mask.
        if scratch.trace_enabled
            && mask.address.address().owner() == ProgrammingOwner::Position
            && !mask.address.address().matches_authored_source(&value.value)
        {
            let prior = value.trace.expect("traced Position mask adoption");
            value.trace = Some(scratch.trace.mapped_blend(prior, prior, None));
        }
        let output = apply_whole(
            value.value,
            &mask,
            context,
            base,
            Some(frame),
            scratch.trace_enabled,
        )?;
        let trace = scratch.trace_enabled.then(|| {
            whole_mask_trace(
                &mask,
                value.trace.expect("enabled trace prefix"),
                output.appearance.clone(),
                &mut scratch.trace,
            )
        });
        value = TracedValue {
            value: output.value,
            trace,
        };
        start = cursor + 1;
    }
    compose_segment_with_orthogonals(
        &value.value,
        value.trace,
        &ordered[start..],
        context,
        frame,
        base,
        scratch,
    )
}

/// A native whole activation is a Whole write over its prefix. A frame-sampled representation
/// crossing (TL-600) blends the prefix and the mask through the resolver's own transfer
/// evidence instead: portable appearance interpolation is not exact field attribution.
fn whole_mask_trace(
    mask: &FamilySample,
    prior: FamilyTraceNodeId,
    appearance: Option<Option<ProgrammingTransitionTrace>>,
    trace: &mut FamilyTraceArena,
) -> FamilyTraceNodeId {
    let incoming = sample_trace(mask, trace);
    match appearance {
        Some(transfer) => trace.mapped_blend(prior, incoming, transfer),
        None => trace.write(
            prior,
            incoming,
            FamilyTraceFootprint::Whole,
            mask.activation_mix < 1.0,
        ),
    }
}

fn compose_segment_with_orthogonals(
    base: &AttributeValue,
    base_trace: Option<FamilyTraceNodeId>,
    ordered: &[usize],
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    original_base: &AttributeValue,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<TracedValue, TransitionError> {
    let value = compose_base(
        base,
        base_trace,
        ordered,
        context,
        frame,
        original_base,
        scratch,
    )?;
    if ordered.iter().any(|&index| match &scratch.sources[index] {
        FamilyCompositionSample::CoupledExpression { expression, .. } => expression
            .roles()
            .iter()
            .any(|role| matches!(role, crate::CoupledExpressionRole::ColorOrthogonal(_))),
        FamilyCompositionSample::Known(sample) => {
            orthogonal(sample.address.address())
                && matches!(sample.body, FamilySampleBody::ComponentExpression(_))
        }
        FamilyCompositionSample::WholeExpression { .. } => false,
    }) {
        let sources = ordered
            .iter()
            .map(|&index| scratch.sources[index].clone())
            .collect::<Vec<_>>();
        return orthogonal::compose(
            base,
            base_trace,
            &value.value,
            value.trace,
            &sources,
            context,
            frame,
            original_base,
            scratch.trace_enabled.then_some(&mut scratch.trace),
        );
    }
    let mut batch = scratch.batches.pop().unwrap_or_default();
    batch.samples.clear();
    for &index in ordered {
        if scratch.sources[index].is_orthogonal()
            && let FamilyCompositionSample::Known(sample) = &scratch.sources[index]
        {
            batch.samples.push(sample.clone());
        }
    }
    let result = run_batch(
        &value.value,
        value.trace,
        context,
        original_base,
        &mut batch,
        &mut scratch.components,
        scratch.trace_enabled.then_some(&mut scratch.trace),
    );
    scratch.batches.push(batch);
    result
}

fn run_batch(
    base: &AttributeValue,
    base_trace: Option<FamilyTraceNodeId>,
    context: &FamilyCompositionContext<'_>,
    original_base: &AttributeValue,
    batch: &mut ResolvedBatch,
    scratch: &mut ComponentCompositionScratch,
    trace: Option<&mut FamilyTraceArena>,
) -> Result<TracedValue, TransitionError> {
    batch.ordered.clear();
    batch.ordered.extend(0..batch.samples.len());
    compose_segment(
        base,
        base_trace,
        &batch.samples,
        &batch.ordered,
        context,
        original_base,
        scratch,
        trace,
    )
}

fn compose_base(
    base: &AttributeValue,
    base_trace: Option<FamilyTraceNodeId>,
    ordered: &[usize],
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    original_base: &AttributeValue,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<TracedValue, TransitionError> {
    let mut evaluation =
        base_evaluation::BaseEvaluation::begin(base, base_trace, ordered, original_base, scratch);
    let result = loop {
        match evaluation.advance(context, frame, scratch) {
            Ok(base_evaluation::BaseEvaluationProgress::Pending) => {}
            Ok(base_evaluation::BaseEvaluationProgress::Complete(value)) => break Ok(value),
            Ok(base_evaluation::BaseEvaluationProgress::OperandReady(_)) => {
                break Err(
                    IntentError("ordinary base evaluation yielded a stage operand".into()).into(),
                );
            }
            Ok(base_evaluation::BaseEvaluationProgress::NeedsMaterialization {
                request, ..
            }) => {
                break Err(TransitionError::Requires(request.requirement));
            }
            Err(error) => break Err(error),
        }
    };
    evaluation.recycle(scratch);
    result
}

fn covers_lower(sample: &FamilySample) -> bool {
    sample.address.address().component.is_none() && sample.activation_mix == 1.0
}

fn finish_batch(
    base: &AttributeValue,
    base_trace: Option<FamilyTraceNodeId>,
    context: &FamilyCompositionContext<'_>,
    original_base: &AttributeValue,
    mut batch: ResolvedBatch,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<TracedValue, TransitionError> {
    batch.samples.reverse();
    let result = run_batch(
        base,
        base_trace,
        context,
        original_base,
        &mut batch,
        &mut scratch.components,
        scratch.trace_enabled.then_some(&mut scratch.trace),
    );
    scratch.batches.push(batch);
    result
}

fn could_match(
    source: &FamilyCompositionSample,
    winner: &FamilySample,
    context: &FamilyCompositionContext<'_>,
) -> bool {
    match source {
        FamilyCompositionSample::Known(sample)
            if sample.address.address().component.is_some()
                || (sample.activation_mix == 1.0
                    && !matches!(sample.endpoint_control(context), FamilyEndpointOutputControl::CrossfadeCurrent { mix } if mix < 1.0)) =>
        {
            compatible(sample, winner)
        }
        // A whole Color can resolve to Semantic Whole or a pinned Direct recipe. Neither is
        // an RGB/HS component basis: an opaque newer component takeover excludes both without
        // asking invisible old target/appearance dependencies to resolve.
        FamilyCompositionSample::WholeExpression { .. }
        | FamilyCompositionSample::CoupledExpression { .. }
        | FamilyCompositionSample::Known(_) => !matches!(
            winner.address.address().representation,
            DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Recipe | DynamicSemanticColorBasis::HueSaturation
            }
        ),
    }
}

fn begin_known_position_completion(
    index: usize,
    underlay: Option<TracedValue>,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<position_completion::PositionSourceCompletion, TransitionError> {
    let FamilyCompositionSample::Known(sample) = scratch.sources[index].clone() else {
        unreachable!("known source")
    };
    let Some(DynamicValue::Family(value)) = sample.materialized_value() else {
        unreachable!("whole known source")
    };
    let trace = scratch
        .trace_enabled
        .then(|| sample_trace(&sample, &mut scratch.trace));
    position_completion::PositionSourceCompletion::new(
        TracedValue {
            value: value.clone(),
            trace,
        },
        sample.rank,
        sample.activation_mix,
        underlay,
        position_completion::PositionCompletionKind::Known,
    )
}

fn completed_position_sample(
    index: usize,
    value: TracedValue,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<FamilySample, TransitionError> {
    let source = &scratch.sources[index];
    let trace_sources = match source {
        FamilyCompositionSample::Known(sample) => sample.trace_sources.clone(),
        _ => None,
    };
    let address = DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value.value)?;
    let mut sample = coupled::verified_endpoint(
        Arc::new(CompiledDynamicValueAddress::new(address, None)?),
        DynamicValue::Family(value.value),
        source.rank(),
        1.,
    )?;
    sample.trace_node = value.trace;
    sample.trace_sources = trace_sources;
    scratch.resolved[index] = Some(sample.clone());
    Ok(sample)
}

fn resolve_whole(
    index: usize,
    underlay: Option<&TracedValue>,
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<FamilySample, TransitionError> {
    let source = scratch.sources[index].clone();
    if let FamilyCompositionSample::Known(sample) = &source {
        let Some(DynamicValue::Family(target)) = sample.materialized_value() else {
            unreachable!("known whole source")
        };
        let model = sample.address.native_model();
        let models = model
            .as_ref()
            .map(|model| vec![(model.source().clone(), model.clone())])
            .unwrap_or_default();
        let retained_model = |source: &light_core::NativeColorIdentity| {
            model
                .clone()
                .filter(|model| model.source() == source)
                .ok_or(TransitionError::Requires(
                    TransitionRequirement::NativeColorModel,
                ))
        };
        let incoming = scratch
            .trace_enabled
            .then(|| sample_trace(sample, &mut scratch.trace));
        let (target, incoming) = endpoint_output::whole(
            sample.rank,
            sample.address.address().owner(),
            target.clone(),
            incoming,
            context,
            frame,
            &retained_model,
            scratch.trace_enabled.then_some(&mut scratch.trace),
        )?;
        let (value, transfer) = if sample.activation_mix == 1.0 {
            (target, None)
        } else if matches!(
            sample.endpoint_control(context),
            FamilyEndpointOutputControl::CrossfadeCurrent { .. }
        ) {
            endpoint_output::transition(
                sample.address.address().owner(),
                &underlay.expect("partial activation prefix").value,
                &target,
                sample.activation_mix,
                context,
                frame,
                &retained_model,
                scratch.trace_enabled,
            )?
        } else {
            let underlay = underlay.expect("partial whole activation has an eligible prefix");
            let compiled = CompiledProgrammingTransition::new(
                underlay.value.clone(),
                target.clone(),
                model.clone(),
            )?;
            let owner = sample.address.address().owner();
            let operation = FamilyExpressionOperation::Transition {
                progress: sample.activation_mix,
            };
            if scratch.trace_enabled {
                super::super::expression_family::resolve_operation_with_trace(
                    owner,
                    compiled.sample_with_trace(owner, sample.activation_mix),
                    &underlay.value,
                    &target,
                    operation,
                    &models,
                    frame,
                )?
            } else {
                (
                    super::super::expression_family::resolve_operation(
                        owner,
                        compiled.sample(sample.activation_mix),
                        &underlay.value,
                        &target,
                        operation,
                        &models,
                        frame,
                    )?,
                    None,
                )
            }
        };
        let address = DynamicValueAddress::whole_family(sample.address.address().owner(), &value)?;
        let model = match &address.representation {
            DynamicFamilyRepresentation::DirectColor { source } => Some(
                endpoint_output::resolve_native_model(source, context, &retained_model)?,
            ),
            _ => None,
        };
        let address = Arc::new(CompiledDynamicValueAddress::new(address, model)?);
        address.validate_value(&DynamicValue::Family(value.clone()))?;
        let trace_node = scratch.trace_enabled.then(|| {
            let appearance = if sample.activation_mix == 1.0 {
                incoming.expect("traced endpoint")
            } else {
                scratch.trace.mapped_blend(
                    underlay
                        .and_then(|value| value.trace)
                        .expect("traced whole underlay"),
                    incoming.expect("traced endpoint"),
                    transfer,
                )
            };
            endpoint_output::control_trace(
                sample.rank,
                FamilyTraceFootprint::Whole,
                appearance,
                underlay
                    .and_then(|value| value.trace)
                    .filter(|_| sample.activation_mix < 1.0),
                context,
                &mut scratch.trace,
            )
        });
        let resolved = FamilySample {
            address,
            body: FamilySampleBody::Materialized(DynamicValue::Family(value)),
            projection: None,
            fix_at: false,
            endpoint_output_exempt: false,
            trace_sources: sample.trace_sources.clone(),
            trace_node,
            rank: sample.rank,
            activation_mix: 1.0,
        };
        scratch.resolved[index] = Some(resolved.clone());
        return Ok(resolved);
    }
    let inputs = whole::begin(index, underlay.cloned(), scratch)?;
    match whole::advance(index, inputs, context, frame, scratch)? {
        whole::AdvanceProgress::OperandReady(_) => {
            Err(IntentError("speculative graph operand escaped ordinary composition".into()).into())
        }
        whole::AdvanceProgress::Complete(sample) => Ok(sample),
        whole::AdvanceProgress::NeedsMaterialization(request) => {
            Err(TransitionError::Requires(request.requirement))
        }
    }
}
