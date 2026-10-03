//! One captured Position program with owned masks, source caches and pending base tasks.
//! Request IDs protect response routing; they never establish a shared mechanical cut.
use super::*;
use base_evaluation::{BaseEvaluation, BaseEvaluationProgress, BaseMaterializationRequest};
use std::ops::ControlFlow;
mod stage_recipe;
pub use stage_recipe::*;
mod stage_discovery;
pub use stage_discovery::*;
mod multi_source_resume;
use super::position_conditioning::origins::ResumeGoal;
pub use multi_source_resume::*;
mod graph_discovery;
mod graph_materialization;
mod graph_recipe;
pub use super::position_conditioning::origins::{
    PositionGraphOperationKind, PositionGraphOperationLocator, PositionGraphOperationOperand,
};
pub use graph_discovery::*;
pub use graph_materialization::*;
pub use graph_recipe::*;

#[derive(Clone)]
pub struct PositionCompositionRequest {
    pub request_id: Uuid,
    pub capture_id: Uuid,
    pub requirement: TransitionRequirement,
    pub operation: PositionCompositionOperation,
    origin: Option<PositionCompositionOrigin>,
    materialized_reached_site: bool,
}
impl PositionCompositionRequest {
    pub fn origin(&self) -> Option<&PositionCompositionOrigin> {
        self.origin.as_ref()
    }
}

#[derive(Clone)]
pub enum PositionCompositionOperation {
    Base {
        source_index: usize,
        rank: FamilySampleRank,
        request: BaseMaterializationRequest,
    },
    MaskAdoption {
        source_index: usize,
        rank: FamilySampleRank,
        from: AttributeValue,
        address: DynamicValueAddress,
    },
    MaskTransition {
        source_index: usize,
        rank: FamilySampleRank,
        from: AttributeValue,
        to: AttributeValue,
        progress: f32,
    },
}

pub enum PositionCompositionProgress {
    Complete(AttributeValue),
    NeedsMaterialization(PositionCompositionRequest),
}

struct MaskApplication {
    source_index: usize,
    mask: FamilySample,
    from: TracedValue,
    adopted: Option<AttributeValue>,
    response: Option<(AttributeValue, Option<ProgrammingTransitionTrace>)>,
}

/// How a settled base step continues the Position advance loop when it does not return.
enum BaseStep {
    /// The base driver needs another pass.
    Pending,
    /// The base segment completed before a whole mask that must be applied next.
    Mask,
}

/// The coordinator owns the immutable frame, context and mechanical peer registry. Supply
/// its exact capture_id on every advance/response and retain the same context/arena. This
/// driver samples no clocks/Random and neither fits a fixture nor publishes continuity.
pub struct PositionCompositionContinuation {
    capture_id: Uuid,
    original_base: AttributeValue,
    scratch: RetainedFamilyCompositionScratch,
    current: TracedValue,
    segment_start: usize,
    segment_end: usize,
    next_mask: Option<usize>,
    base: Option<BaseEvaluation>,
    mask: Option<MaskApplication>,
    pending: Option<PositionCompositionRequest>,
    completed: bool,
    failed: bool,
    origin_binding: Option<super::position_conditioning::origins::PositionOriginBinding>,
    operand_only: bool,
    stopped_operand: Option<AttributeValue>,
    discovery: Option<stage_discovery::DiscoveryState>,
    graph_replay: Option<(PositionGraphOperationLocator, PositionGraphOperationOperand)>,
    started: bool,
    graph_discovery: Option<graph_discovery::GraphDiscoveryState>,
    graph_materialization: Option<graph_materialization::GraphMaterializationState>,
    resume_goal: ResumeGoal,
    resume_replay: Option<multi_source_resume::ResumeReplayState>,
}

pub fn begin_retained_position_composition(
    capture_id: Uuid,
    base: &AttributeValue,
    samples: &[FamilyCompositionSample],
    context: &FamilyCompositionContext<'_>,
    mut scratch: RetainedFamilyCompositionScratch,
    tracing: bool,
) -> Result<PositionCompositionContinuation, TransitionError> {
    ensure(
        !capture_id.is_nil(),
        "Position composition requires an immutable capture identity",
    )?;
    let base_trace = prepare_family_inputs(
        ProgrammingOwner::Position,
        base,
        samples.iter().cloned(),
        context,
        &mut scratch,
        tracing,
    )?;
    PositionCompositionContinuation::prepared(capture_id, base, base_trace, scratch)
}

impl PositionCompositionContinuation {
    pub(super) fn prepared(
        capture_id: Uuid,
        base: &AttributeValue,
        base_trace: Option<FamilyTraceNodeId>,
        mut scratch: RetainedFamilyCompositionScratch,
    ) -> Result<Self, TransitionError> {
        let last_full = scratch.ordered.iter().rposition(|&index| {
            scratch.sources[index]
                .whole_mask()
                .is_some_and(|sample| sample.activation_mix == 1.)
        });
        let (current, segment_start) = if let Some(cursor) = last_full {
            let mask = scratch.sources[scratch.ordered[cursor]]
                .whole_mask()
                .expect("whole mask")
                .clone();
            let Some(DynamicValue::Family(value)) = mask.materialized_value() else {
                return Err(IntentError("Position mask is not a complete family".into()).into());
            };
            let trace = scratch.trace_enabled.then(|| {
                whole_mask_trace(
                    &mask,
                    base_trace.expect("trace base"),
                    None,
                    &mut scratch.trace,
                )
            });
            (
                TracedValue {
                    value: value.clone(),
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
        let mut evaluation = Self {
            capture_id,
            original_base: base.clone(),
            scratch,
            current,
            segment_start,
            segment_end: segment_start,
            next_mask: None,
            base: None,
            mask: None,
            pending: None,
            completed: false,
            failed: false,
            origin_binding: None,
            operand_only: false,
            stopped_operand: None,
            discovery: None,
            graph_replay: None,
            started: false,
            graph_discovery: None,
            graph_materialization: None,
            resume_goal: ResumeGoal::Full,
            resume_replay: None,
        };
        evaluation.schedule_segment();
        Ok(evaluation)
    }

    pub(super) fn bind_origins(
        &mut self,
        binding: super::position_conditioning::origins::PositionOriginBinding,
    ) {
        self.origin_binding = Some(binding);
    }

    fn schedule_segment(&mut self) {
        self.next_mask = (self.segment_start..self.scratch.ordered.len()).find(|&cursor| {
            self.scratch.sources[self.scratch.ordered[cursor]]
                .whole_mask()
                .is_some()
        });
        self.segment_end = self.next_mask.unwrap_or(self.scratch.ordered.len());
        let ordered = std::mem::take(&mut self.scratch.ordered);
        self.base = Some(BaseEvaluation::begin(
            &self.current.value,
            self.current.trace,
            &ordered[self.segment_start..self.segment_end],
            &self.original_base,
            &mut self.scratch,
        ));
        self.scratch.ordered = ordered;
    }

    pub fn family_trace(&self) -> &FamilyTraceArena {
        &self.scratch.trace
    }
    pub fn pending_materialization(&self) -> Option<&PositionCompositionRequest> {
        self.pending.as_ref()
    }

    pub fn advance(
        &mut self,
        capture_id: Uuid,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<PositionCompositionProgress, TransitionError> {
        ensure(
            !self.operand_only,
            "operand replay cannot use ordinary parent composition",
        )?;
        if capture_id != self.capture_id {
            return Err(
                IntentError("Position composition belongs to another capture".into()).into(),
            );
        }
        if self.failed {
            return Err(IntentError("failed Position composition must be discarded".into()).into());
        }
        self.failed = true;
        let result = self.advance_impl(context, frame, None);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }

    fn wait(
        &mut self,
        requirement: TransitionRequirement,
        operation: PositionCompositionOperation,
    ) -> Result<PositionCompositionProgress, TransitionError> {
        let origin = self
            .origin_binding
            .as_ref()
            .map(|binding| binding.request_origin(&operation, &self.scratch))
            .transpose()?;
        let request_id = Uuid::new_v4();
        let materialized_reached_site = self.issue_graph_materialization(&operation, request_id)?;
        let request = PositionCompositionRequest {
            request_id,
            capture_id: self.capture_id,
            requirement,
            operation,
            origin,
            materialized_reached_site,
        };
        self.pending = Some(request.clone());
        Ok(PositionCompositionProgress::NeedsMaterialization(request))
    }

    fn advance_impl(
        &mut self,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        stage: Option<(&PositionStageLocator, PositionStageOperand)>,
    ) -> Result<PositionCompositionProgress, TransitionError> {
        self.started = true;
        if let Some(request) = &self.pending {
            return Ok(PositionCompositionProgress::NeedsMaterialization(
                request.clone(),
            ));
        }
        if self.completed {
            return Ok(PositionCompositionProgress::Complete(
                self.current.value.clone(),
            ));
        }
        loop {
            if let Some(progress) = self.advance_base(context, frame, stage)? {
                match self.settle_base(progress)? {
                    ControlFlow::Continue(BaseStep::Pending) => continue,
                    ControlFlow::Continue(BaseStep::Mask) => {}
                    ControlFlow::Break(progress) => return Ok(progress),
                }
            }
            if let Some(progress) = self.adopt_mask(context, stage)? {
                return Ok(progress);
            }
            if let Some(progress) = self.apply_mask(frame, stage)? {
                return Ok(progress);
            }
        }
    }

    /// Advance the pending base segment through the matcher selected by the replay mode.
    fn advance_base(
        &mut self,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        stage: Option<(&PositionStageLocator, PositionStageOperand)>,
    ) -> Result<Option<BaseEvaluationProgress>, TransitionError> {
        let Some(base) = &mut self.base else {
            return Ok(None);
        };
        let reached_enabled =
            self.graph_discovery.is_some() || self.graph_materialization.is_some();
        let binding = self.origin_binding.as_ref();
        let mut reached = |route: &PreparedPositionGraphRoute| {
            let state = (&mut self.graph_discovery, &mut self.graph_materialization);
            graph_materialization::reached_action(state, binding, route)
        };
        let progress = if let Some(replay) = &self.resume_replay {
            let binding = self.origin_binding.as_ref().ok_or_else(|| {
                IntentError("Position Resume operand lost its original binding".into())
            })?;
            let mut graph = |route: &PreparedPositionGraphRoute| replay.graph_plan(route, binding);
            let mut stages = |route: &PreparedPositionStageRoute| replay.stage(route, binding);
            let mut stopped = |depth| replay.complete_stop(depth);
            let mut cohort = |uses: &[PreparedPositionStageUse],
                              origins: &[PreparedSourceOrigin],
                              expression: &Arc<CompiledCoupledExpression>,
                              endpoints: &[usize]| {
                replay.cohort(uses, origins, expression, endpoints, binding)
            };
            let matcher = if replay.enclosing_stage().is_some() {
                PreparedPositionStageMatcher::enabled(&mut stages)
            } else {
                PreparedPositionStageMatcher::disabled()
            };
            base.advance_with_stage(
                context,
                frame,
                &mut self.scratch,
                &mut matcher
                    .with_resume_plan(&mut graph, &mut stopped, &mut cohort)
                    .with_graph_reached(reached_enabled.then_some(&mut reached)),
            )?
        } else if let Some((locator, operand)) = &self.graph_replay {
            let binding = self.origin_binding.as_ref().ok_or_else(|| {
                IntentError("Position graph operand lost its original binding".into())
            })?;
            let mut matcher = |route: &PreparedPositionGraphRoute| {
                Ok(binding
                    .graph_locator(route)?
                    .filter(|candidate| candidate == locator)
                    .map(|_| operand.internal()))
            };
            base.advance_with_stage(
                context,
                frame,
                &mut self.scratch,
                &mut PreparedPositionStageMatcher::enabled_graph(&mut matcher)
                    .with_graph_reached(reached_enabled.then_some(&mut reached)),
            )?
        } else if let Some((locator, operand)) = stage {
            let binding = self
                .origin_binding
                .as_ref()
                .ok_or_else(|| IntentError("Position operand lost its original binding".into()))?;
            let mut matcher = |route: &PreparedPositionStageRoute| {
                Ok(binding
                    .stage_locator(route)?
                    .filter(|candidate| candidate == locator)
                    .map(|_| operand.segment()))
            };
            base.advance_with_stage(
                context,
                frame,
                &mut self.scratch,
                &mut PreparedPositionStageMatcher::enabled(&mut matcher)
                    .with_graph_reached(reached_enabled.then_some(&mut reached)),
            )?
        } else if let Some(discovery) = &mut self.discovery {
            let binding = self.origin_binding.as_ref().ok_or_else(|| {
                IntentError("Position discovery lost its original binding".into())
            })?;
            let mut matcher = |route: &PreparedPositionStageRoute| {
                discovery.record(binding.stage_locator(route)?)?;
                Ok(None)
            };
            base.advance_with_stage(
                context,
                frame,
                &mut self.scratch,
                &mut PreparedPositionStageMatcher::enabled(&mut matcher)
                    .with_graph_reached(reached_enabled.then_some(&mut reached)),
            )?
        } else {
            base.advance_with_stage(
                context,
                frame,
                &mut self.scratch,
                &mut PreparedPositionStageMatcher::disabled()
                    .with_graph_reached(reached_enabled.then_some(&mut reached)),
            )?
        };
        Ok(Some(progress))
    }

    fn settle_base(
        &mut self,
        progress: BaseEvaluationProgress,
    ) -> Result<ControlFlow<PositionCompositionProgress, BaseStep>, TransitionError> {
        match progress {
            BaseEvaluationProgress::Pending => {
                return Ok(ControlFlow::Continue(BaseStep::Pending));
            }
            BaseEvaluationProgress::OperandReady(value) => {
                ensure(
                    self.operand_only,
                    "speculative operand escaped into ordinary parent composition",
                )?;
                if self
                    .resume_replay
                    .as_ref()
                    .is_some_and(|replay| !replay.requested_stopped())
                {
                    self.completed = true;
                } else {
                    self.stopped_operand = Some(value.clone());
                }
                return Ok(ControlFlow::Break(PositionCompositionProgress::Complete(
                    value,
                )));
            }
            BaseEvaluationProgress::NeedsMaterialization {
                source_index,
                request,
            } => {
                let rank = request.source_rank(source_index, &self.scratch);
                return self
                    .wait(
                        request.requirement,
                        PositionCompositionOperation::Base {
                            source_index,
                            rank,
                            request,
                        },
                    )
                    .map(ControlFlow::Break);
            }
            BaseEvaluationProgress::Complete(value) => {
                self.current = value;
                self.base
                    .take()
                    .expect("completed base driver")
                    .recycle(&mut self.scratch);
                if let Some(cursor) = self.next_mask {
                    let source_index = self.scratch.ordered[cursor];
                    let mask = self.scratch.sources[source_index]
                        .whole_mask()
                        .expect("partial mask")
                        .clone();
                    self.mask = Some(MaskApplication {
                        source_index,
                        mask,
                        from: self.current.clone(),
                        adopted: None,
                        response: None,
                    });
                } else {
                    self.completed = true;
                    if let Some(root) = self.current.trace {
                        self.scratch.trace.set_root(root);
                    }
                    return Ok(ControlFlow::Break(PositionCompositionProgress::Complete(
                        self.current.value.clone(),
                    )));
                }
            }
        }
        Ok(ControlFlow::Continue(BaseStep::Mask))
    }

    fn adopt_mask(
        &mut self,
        context: &FamilyCompositionContext<'_>,
        stage: Option<(&PositionStageLocator, PositionStageOperand)>,
    ) -> Result<Option<PositionCompositionProgress>, TransitionError> {
        let application = self.mask.as_mut().expect("pending Position mask");
        if application.adopted.is_some() {
            return Ok(None);
        }
        if mask_stage(
            self.origin_binding.as_ref(),
            &self.scratch,
            self.discovery.as_mut(),
            application.source_index,
            PositionMaskStage::Adoption,
            stage,
        )?
        .is_some()
        {
            // Only AdoptionInput belongs to this stage: the composed prefix.
            let value = application.from.value.clone();
            return self.stop_at_mask(value).map(Some);
        }
        match adopt(
            application.from.value.clone(),
            application.mask.address.address(),
            context,
            &self.original_base,
        ) {
            Ok(value) => {
                if self.scratch.trace_enabled
                    && !application
                        .mask
                        .address
                        .address()
                        .matches_authored_source(&application.from.value)
                {
                    let prior = application.from.trace.expect("traced mask adoption");
                    application.from.trace =
                        Some(self.scratch.trace.mapped_blend(prior, prior, None));
                }
                application.adopted = Some(value);
            }
            Err(TransitionError::Requires(requirement)) => {
                let operation = PositionCompositionOperation::MaskAdoption {
                    source_index: application.source_index,
                    rank: application.mask.rank,
                    from: application.from.value.clone(),
                    address: application.mask.address.address().clone(),
                };
                return self.wait(requirement, operation).map(Some);
            }
            Err(error) => return Err(error),
        }
        Ok(None)
    }

    fn apply_mask(
        &mut self,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        stage: Option<(&PositionStageLocator, PositionStageOperand)>,
    ) -> Result<Option<PositionCompositionProgress>, TransitionError> {
        let application = self.mask.as_mut().expect("pending Position mask");
        let from = application.adopted.as_ref().expect("adopted mask prefix");
        let Some(DynamicValue::Family(to)) = application.mask.materialized_value() else {
            unreachable!("validated whole mask")
        };
        if application.response.is_none()
            && let Some(operand) = mask_stage(
                self.origin_binding.as_ref(),
                &self.scratch,
                self.discovery.as_mut(),
                application.source_index,
                PositionMaskStage::Transition,
                stage,
            )?
        {
            let value = match operand {
                PositionStageOperand::TransitionFrom => from.clone(),
                PositionStageOperand::TransitionTo => to.clone(),
                PositionStageOperand::AdoptionInput => {
                    return Err(IntentError(
                        "Position mask transition has no adoption operand".into(),
                    )
                    .into());
                }
            };
            return self.stop_at_mask(value).map(Some);
        }
        let mut appearance = None;
        let value = if let Some((value, transfer)) = application.response.take() {
            appearance = Some(transfer);
            value
        } else {
            let compiled = CompiledProgrammingTransition::new(from.clone(), to.clone(), None)?;
            match compiled.sample(application.mask.activation_mix) {
                Ok(value) => value,
                Err(TransitionError::Requires(requirement)) => {
                    match frame.resolve_with_trace(
                        requirement,
                        from,
                        to,
                        FamilyExpressionOperation::Transition {
                            progress: application.mask.activation_mix,
                        },
                    ) {
                        Ok((value, transfer)) => {
                            position_completion::validate_position(&value)?;
                            position_completion::validate_transfer(transfer.as_ref())?;
                            appearance = Some(transfer);
                            value
                        }
                        Err(TransitionError::Requires(requirement)) => {
                            let operation = PositionCompositionOperation::MaskTransition {
                                source_index: application.source_index,
                                rank: application.mask.rank,
                                from: from.clone(),
                                to: to.clone(),
                                progress: application.mask.activation_mix,
                            };
                            return self.wait(requirement, operation).map(Some);
                        }
                        Err(error) => return Err(error),
                    }
                }
                Err(error) => return Err(error),
            }
        };
        let trace = self.scratch.trace_enabled.then(|| {
            whole_mask_trace(
                &application.mask,
                application.from.trace.expect("trace mask prefix"),
                appearance,
                &mut self.scratch.trace,
            )
        });
        self.current = TracedValue { value, trace };
        self.mask = None;
        self.segment_start = self.next_mask.expect("completed mask cursor") + 1;
        self.schedule_segment();
        Ok(None)
    }

    /// Operand replay stops before the located mask operation: the operation itself and every
    /// parent suffix stay unexecuted for the consumer to apply once.
    fn stop_at_mask(
        &mut self,
        value: AttributeValue,
    ) -> Result<PositionCompositionProgress, TransitionError> {
        ensure(
            self.operand_only,
            "speculative operand escaped into ordinary parent composition",
        )?;
        if self.resume_replay.is_some() {
            self.completed = true;
        } else {
            self.stopped_operand = Some(value.clone());
        }
        Ok(PositionCompositionProgress::Complete(value))
    }

    pub fn resume_materialization(
        &mut self,
        capture_id: Uuid,
        request_id: Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        if capture_id != self.capture_id {
            return Err(IntentError("Position response belongs to another capture".into()).into());
        }
        if self.failed {
            return Err(IntentError("failed Position composition must be discarded".into()).into());
        }
        let request = self
            .pending
            .as_ref()
            .ok_or_else(|| IntentError("Position composition has no pending request".into()))?;
        if request.request_id != request_id {
            return Err(IntentError("Position response names another request".into()).into());
        }
        self.failed = true;
        let result = self.resume_impl(value, transfer);
        self.failed = false;
        result
    }

    fn resume_impl(
        &mut self,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        let request = self.pending.as_ref().expect("guarded pending response");
        position_completion::validate_position(&value)?;
        position_completion::validate_transfer(transfer.as_ref())?;
        match &request.operation {
            PositionCompositionOperation::Base {
                source_index,
                request,
                ..
            } => {
                self.base
                    .as_mut()
                    .expect("pending base")
                    .resume_materialization_traced(*source_index, request.node, value, transfer)?;
            }
            PositionCompositionOperation::MaskAdoption { address, .. } => {
                ensure(
                    address.matches_authored_source(&value),
                    "Position mask adoption returned another representation",
                )?;
                let application = self.mask.as_mut().expect("pending mask");
                if self.scratch.trace_enabled {
                    let prior = application.from.trace.expect("traced mask adoption");
                    application.from.trace =
                        Some(self.scratch.trace.mapped_blend(prior, prior, transfer));
                }
                application.adopted = Some(value);
            }
            PositionCompositionOperation::MaskTransition { .. } => {
                self.mask.as_mut().expect("pending mask").response = Some((value, transfer));
            }
        }
        self.pending = None;
        Ok(())
    }

    /// Consume either a completed or deliberately abandoned evaluation and return its buffers.
    /// No speculative result or continuity is published by recycling.
    pub fn into_scratch(mut self) -> RetainedFamilyCompositionScratch {
        if let Some(base) = self.base.take() {
            base.recycle(&mut self.scratch);
        }
        self.scratch
    }
}

/// Explicit replay/discovery only: ordinary composition performs no mask locator work. The
/// mask's own original slot is the only authority; lower routes never name it.
fn mask_stage(
    binding: Option<&super::position_conditioning::origins::PositionOriginBinding>,
    scratch: &RetainedFamilyCompositionScratch,
    discovery: Option<&mut stage_discovery::DiscoveryState>,
    source_index: usize,
    kind: PositionMaskStage,
    stage: Option<(&PositionStageLocator, PositionStageOperand)>,
) -> Result<Option<PositionStageOperand>, TransitionError> {
    if stage.is_none() && discovery.is_none() {
        return Ok(None);
    }
    let binding = binding
        .ok_or_else(|| IntentError("Position mask stage lost its original binding".into()))?;
    let origins = scratch
        .source_origins
        .get(source_index)
        .ok_or_else(|| IntentError("Position mask source origins are absent".into()))?;
    let candidate = binding.mask_locator(origins, kind)?;
    if let Some((locator, operand)) = stage {
        return Ok(candidate
            .filter(|candidate| candidate == locator)
            .map(|_| operand));
    }
    if let Some(discovery) = discovery {
        discovery.record(candidate)?;
    }
    Ok(None)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "position_program/segment_tests.rs"]
mod segment_tests;

#[cfg(test)]
#[path = "position_program/stage_tests.rs"]
mod stage_tests;

#[cfg(test)]
#[path = "position_program/stage_discovery_tests.rs"]
mod stage_discovery_tests;

#[cfg(test)]
#[path = "position_program/envelope_stage_tests.rs"]
mod envelope_stage_tests;

#[cfg(test)]
#[path = "position_program/mask_stage_tests.rs"]
mod mask_stage_tests;

#[cfg(test)]
mod operation_stage_tests;

#[cfg(test)]
#[path = "position_program/stage_nested_authority_tests.rs"]
mod stage_nested_authority_tests;
