//! Retained outer dependency stack for Position expression suspension. This driver owns
//! the pending tasks and last completed result; the caller must keep its scratch, captured
//! context and source membership unchanged until completion. Coupled and nested whole-source
//! graphs, component segments and source completion can suspend. Complete shared mechanical
//! cuts still require a coordinator owning the immutable captured peers.
use super::*;

#[derive(Clone)]
pub struct BaseMaterializationRequest {
    pub node: usize,
    pub(super) source_origins: Vec<PreparedSourceOrigin>,
    pub(super) stage_route: Option<PreparedPositionStageRoute>,
    pub(super) graph_route: Option<PreparedPositionGraphRoute>,
    pub requirement: TransitionRequirement,
    pub operation: BaseMaterializationOperation,
}

#[derive(Clone)]
pub enum BaseMaterializationOperation {
    Coupled(crate::PositionMaterializationRequest),
    Whole(crate::FamilyMaterializationRequest),
    Completion(position_completion::CompletionRequest),
    Segment(PositionSegmentRequest),
    SourceCohort {
        endpoint_index: usize,
        child_source_index: usize,
        source_rank: FamilySampleRank,
        request: Box<BaseMaterializationRequest>,
    },
}

impl BaseMaterializationRequest {
    pub(super) fn source_rank(
        &self,
        source_index: usize,
        scratch: &RetainedFamilyCompositionScratch,
    ) -> FamilySampleRank {
        match &self.operation {
            BaseMaterializationOperation::Segment(inner) => inner.rank,
            BaseMaterializationOperation::SourceCohort { source_rank, .. } => *source_rank,
            _ => scratch.sources[source_index].rank(),
        }
    }
}

impl From<crate::PositionMaterializationRequest> for BaseMaterializationRequest {
    fn from(request: crate::PositionMaterializationRequest) -> Self {
        Self {
            node: request.node,
            source_origins: Vec::new(),
            stage_route: None,
            graph_route: None,
            requirement: request.requirement,
            operation: BaseMaterializationOperation::Coupled(request),
        }
    }
}
impl From<crate::FamilyMaterializationRequest> for BaseMaterializationRequest {
    fn from(request: crate::FamilyMaterializationRequest) -> Self {
        Self {
            node: request.node,
            source_origins: Vec::new(),
            stage_route: None,
            graph_route: None,
            requirement: request.requirement,
            operation: BaseMaterializationOperation::Whole(request),
        }
    }
}

impl From<position_completion::CompletionRequest> for BaseMaterializationRequest {
    fn from(request: position_completion::CompletionRequest) -> Self {
        let node = match request.stage {
            position_completion::PositionCompletionStage::EndpointOutput => 0,
            position_completion::PositionCompletionStage::Activation => 1,
        };
        Self {
            node,
            source_origins: Vec::new(),
            stage_route: None,
            graph_route: None,
            requirement: request.requirement,
            operation: BaseMaterializationOperation::Completion(request),
        }
    }
}

pub(super) enum BaseEvaluationProgress {
    Pending,
    NeedsMaterialization {
        source_index: usize,
        request: BaseMaterializationRequest,
    },
    Complete(TracedValue),
    OperandReady(AttributeValue),
}

pub(super) struct BaseEvaluation {
    base: AttributeValue,
    base_trace: Option<FamilyTraceNodeId>,
    original_base: AttributeValue,
    ordered: Box<[usize]>,
    tasks: Vec<BaseTask>,
    last: Option<BaseResult>,
    completed: Option<TracedValue>,
    operand_ready: Option<AttributeValue>,
    materialization: Option<(usize, BaseMaterializationRequest)>,
    failed: bool,
    /// Lexical use of each Known/whole source at its first resolution. Envelope routes need
    /// it after the consuming task is gone; the use vector is moved, never re-derived.
    envelope_uses: Vec<(usize, Vec<PreparedPositionStageUse>)>,
}

impl BaseEvaluation {
    pub(super) fn begin(
        base: &AttributeValue,
        base_trace: Option<FamilyTraceNodeId>,
        ordered: &[usize],
        original_base: &AttributeValue,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> Self {
        Self::begin_with_route(
            base,
            base_trace,
            ordered,
            original_base,
            scratch,
            vec![PreparedPositionStageUse::RootFinalSegment],
        )
    }

    pub(super) fn begin_with_route(
        base: &AttributeValue,
        base_trace: Option<FamilyTraceNodeId>,
        ordered: &[usize],
        original_base: &AttributeValue,
        scratch: &mut RetainedFamilyCompositionScratch,
        uses: Vec<PreparedPositionStageUse>,
    ) -> Self {
        let mut tasks = std::mem::take(&mut scratch.tasks);
        tasks.clear();
        tasks.push(BaseTask::Compose {
            end: ordered.len(),
            uses,
        });
        Self {
            base: base.clone(),
            base_trace,
            original_base: original_base.clone(),
            ordered: ordered.into(),
            tasks,
            last: None,
            completed: None,
            operand_ready: None,
            materialization: None,
            failed: false,
            envelope_uses: Vec::new(),
        }
    }

    /// Execute one heap task. Completed child values, batches and coupled endpoint inputs
    /// remain owned by this evaluation/scratch; no source sampling or graph preparation occurs.
    pub(super) fn advance(
        &mut self,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> Result<BaseEvaluationProgress, TransitionError> {
        self.advance_with_stage(
            context,
            frame,
            scratch,
            &mut PreparedPositionStageMatcher::disabled(),
        )
    }

    pub(super) fn advance_with_stage(
        &mut self,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<BaseEvaluationProgress, TransitionError> {
        if self.failed {
            return Err(IntentError("failed base evaluation must be discarded".into()).into());
        }
        if let Some(value) = &self.operand_ready {
            return Ok(BaseEvaluationProgress::OperandReady(value.clone()));
        }
        if let Some(value) = &self.completed {
            return Ok(BaseEvaluationProgress::Complete(value.clone()));
        }
        if let Some((source_index, request)) = &self.materialization {
            return Ok(BaseEvaluationProgress::NeedsMaterialization {
                source_index: *source_index,
                request: request.clone(),
            });
        }
        // Existing coupled scheduling uses scratch.tasks. Temporarily lend this evaluation's
        // stack, then take it back on success and error; a nested scratch has a distinct stack.
        std::mem::swap(&mut self.tasks, &mut scratch.tasks);
        let probe = position_completion::EnvelopeProbeScope::enter(matcher.is_enabled());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.advance_task(context, frame, scratch, matcher)
        }));
        drop(probe);
        std::mem::swap(&mut self.tasks, &mut scratch.tasks);
        match result {
            Ok(result) => {
                if let Ok(BaseEvaluationProgress::OperandReady(value)) = &result {
                    self.operand_ready = Some(value.clone());
                }
                if result.is_err() {
                    self.failed = true;
                }
                result
            }
            Err(payload) => {
                self.failed = true;
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn advance_task(
        &mut self,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<BaseEvaluationProgress, TransitionError> {
        let base = &self.base;
        let base_trace = self.base_trace;
        let ordered = &self.ordered;
        let Some(task) = scratch.tasks.pop() else {
            let value = self
                .last
                .take()
                .expect("completed base composition")
                .value();
            self.completed = Some(value.clone());
            return Ok(BaseEvaluationProgress::Complete(value));
        };
        match task {
            BaseTask::EvaluatePositionSegment {
                mut evaluation,
                batch,
            } => {
                match evaluation.advance(
                    context,
                    frame,
                    &mut scratch.components,
                    &mut scratch.trace,
                )? {
                    position_segment::PositionSegmentProgress::OperandReady(value) => {
                        scratch
                            .tasks
                            .push(BaseTask::EvaluatePositionSegment { evaluation, batch });
                        return Ok(BaseEvaluationProgress::OperandReady(value));
                    }
                    position_segment::PositionSegmentProgress::Complete(value) => {
                        scratch.batches.push(batch);
                        self.last = Some(BaseResult::Value(value));
                    }
                    position_segment::PositionSegmentProgress::NeedsMaterialization(inner) => {
                        let request = BaseMaterializationRequest {
                            node: inner.node,
                            source_origins: batch
                                .origins
                                .get(inner.sample_index)
                                .cloned()
                                .ok_or_else(|| {
                                    IntentError("Position segment source origin is absent".into())
                                })?,
                            graph_route: None,
                            stage_route: Some(segment_route(
                                &batch,
                                inner.sample_index,
                                match &inner.operation {
                                    PositionSegmentOperation::Adoption { address, .. } => {
                                        PreparedPositionStageKind::Adoption {
                                            component: address.component,
                                        }
                                    }
                                    PositionSegmentOperation::Transition { .. } => {
                                        PreparedPositionStageKind::WholeSegmentTransition
                                    }
                                },
                            )?),
                            requirement: inner.requirement,
                            operation: BaseMaterializationOperation::Segment(inner),
                        };
                        scratch
                            .tasks
                            .push(BaseTask::EvaluatePositionSegment { evaluation, batch });
                        self.materialization = Some((usize::MAX, request.clone()));
                        return Ok(BaseEvaluationProgress::NeedsMaterialization {
                            source_index: usize::MAX,
                            request,
                        });
                    }
                }
            }

            BaseTask::CompleteKnownPosition {
                index,
                mut completion,
            } => {
                let context = if matches!(&scratch.sources[index], FamilyCompositionSample::Known(sample) if sample.fix_at || sample.endpoint_output_exempt)
                {
                    endpoint_output::with_control(context, None)
                } else {
                    endpoint_output::with_control(context, context.endpoint_output)
                };
                match completion.advance(
                    &context,
                    frame,
                    &mut scratch.trace,
                    scratch.trace_enabled,
                )? {
                    position_completion::CompletionProgress::Complete(value) => {
                        self.last = Some(BaseResult::Sample(completed_position_sample(
                            index, value, scratch,
                        )?))
                    }
                    position_completion::CompletionProgress::Needs(request) => {
                        let mut request: BaseMaterializationRequest = request.into();
                        request.source_origins = scratch
                            .source_origins
                            .get(index)
                            .cloned()
                            .unwrap_or_default();
                        scratch
                            .tasks
                            .push(BaseTask::CompleteKnownPosition { index, completion });
                        if let Some(progress) =
                            self.envelope(index, &mut request, scratch, matcher)?
                        {
                            return Ok(progress);
                        }
                        self.materialization = Some((index, request.clone()));
                        return Ok(BaseEvaluationProgress::NeedsMaterialization {
                            source_index: index,
                            request,
                        });
                    }
                }
            }

            BaseTask::EvaluateWhole { index, inputs } => {
                let uses = self
                    .envelope_uses
                    .iter()
                    .find(|(source, _)| *source == index)
                    .map(|(_, uses)| uses.clone())
                    .unwrap_or_default();
                match whole::advance_with_graph(
                    index, inputs, uses, context, frame, scratch, matcher,
                )? {
                    // Required/Size and Resume operands share this opaque lexical stop.
                    // The enclosing driver decides which goal finished; no parent is accepted.
                    whole::AdvanceProgress::OperandReady(value) => {
                        return Ok(BaseEvaluationProgress::OperandReady(value));
                    }
                    whole::AdvanceProgress::Complete(sample) => {
                        self.last = Some(BaseResult::Sample(sample))
                    }
                    whole::AdvanceProgress::NeedsMaterialization(request) => {
                        let mut request: BaseMaterializationRequest = request.into();
                        request.source_origins = scratch
                            .source_origins
                            .get(index)
                            .cloned()
                            .unwrap_or_default();
                        if let Some(progress) =
                            self.envelope(index, &mut request, scratch, matcher)?
                        {
                            return Ok(progress);
                        }
                        self.materialization = Some((index, request.clone()));
                        return Ok(BaseEvaluationProgress::NeedsMaterialization {
                            source_index: index,
                            request,
                        });
                    }
                }
            }

            BaseTask::Coupled {
                index,
                cursor,
                step,
                inputs,
                uses,
            } => match coupled::advance_with_stage(
                index, cursor, step, inputs, uses, context, frame, scratch, matcher,
            )? {
                coupled::AdvanceProgress::Scheduled => {}
                coupled::AdvanceProgress::OperandReady(value) => {
                    return Ok(BaseEvaluationProgress::OperandReady(value));
                }
                coupled::AdvanceProgress::Complete(sample) => {
                    self.last = Some(BaseResult::Sample(sample))
                }
                coupled::AdvanceProgress::NeedsMaterialization(request) => {
                    let mut request: BaseMaterializationRequest = request.into();
                    request.source_origins = scratch
                        .source_origins
                        .get(index)
                        .cloned()
                        .unwrap_or_default();
                    if let Some(progress) = self.envelope(index, &mut request, scratch, matcher)? {
                        return Ok(progress);
                    }
                    self.materialization = Some((index, request.clone()));
                    return Ok(BaseEvaluationProgress::NeedsMaterialization {
                        source_index: index,
                        request,
                    });
                }
            },
            BaseTask::CollectCoupled {
                index,
                cursor,
                step,
                mut inputs,
                uses,
            } => {
                let value = self
                    .last
                    .take()
                    .expect("completed coupled endpoint cohort")
                    .value();
                if step == 0 {
                    inputs.underlay = Some(value);
                } else {
                    inputs.values.push(Some(value));
                }
                scratch.tasks.push(BaseTask::Coupled {
                    index,
                    cursor,
                    step: step + 1,
                    inputs,
                    uses,
                });
            }
            BaseTask::Cohort {
                end,
                candidates,
                origins,
                uses,
            } => {
                let mut batch = scratch.batches.pop().unwrap_or_default();
                batch.samples.clear();
                batch.origins.clear();
                batch.uses = uses;
                ensure(
                    candidates.len() == origins.len(),
                    "Position cohort batch origins differ from source membership",
                )?;
                let mut paired = candidates.into_iter().zip(origins).collect::<Vec<_>>();
                paired.sort_unstable_by_key(|(sample, _)| sample.order_key());
                paired.reverse();
                for (mut sample, origins) in paired {
                    sample.endpoint_output_exempt = true;
                    batch.samples.push(sample);
                    batch.origins.push(origins);
                }
                // Source and origin sidecars share the exact rank ordering above.
                let winner = batch
                    .samples
                    .first()
                    .expect("nonempty coupled endpoint cohort")
                    .clone();
                if batch.samples.iter().any(covers_lower) {
                    self.finish_or_schedule_batch(context, batch, scratch, matcher)?;
                } else {
                    scratch.tasks.push(BaseTask::Scan {
                        winner,
                        cursor: end,
                        batch,
                    });
                }
            }
            BaseTask::Compose { end, uses } => {
                if let Some(cursor) = ordered[..end]
                    .iter()
                    .rposition(|&index| !scratch.sources[index].is_orthogonal())
                {
                    scratch.tasks.push(BaseTask::Winner {
                        cursor,
                        uses: uses.clone(),
                    });
                    scratch.tasks.push(BaseTask::Resolve { cursor, uses });
                } else {
                    self.last = Some(BaseResult::Value(TracedValue {
                        value: base.clone(),
                        trace: base_trace,
                    }));
                }
            }
            BaseTask::Resolve { cursor, uses } => {
                let index = ordered[cursor];
                if let Some(value) = &scratch.resolved[index] {
                    self.last = Some(BaseResult::Sample(value.clone()));
                    return Ok(BaseEvaluationProgress::Pending);
                }
                match &scratch.sources[index] {
                    FamilyCompositionSample::CoupledExpression { .. } => {
                        scratch.tasks.push(BaseTask::Coupled {
                            index,
                            cursor,
                            step: 0,
                            inputs: coupled::BaseInputs::default(),
                            uses,
                        });
                    }
                    FamilyCompositionSample::Known(sample)
                        if sample.address.address().component.is_none()
                            && sample.activation_mix < 1.0 =>
                    {
                        let underlay = underlay_route(&uses, index, scratch);
                        self.record_envelope_uses(index, uses);
                        scratch.tasks.push(BaseTask::Whole { index });
                        scratch.tasks.push(BaseTask::Compose {
                            end: cursor,
                            uses: underlay,
                        });
                    }
                    FamilyCompositionSample::Known(sample)
                        if sample.address.address().component.is_none()
                            && matches!(sample.endpoint_control(context), FamilyEndpointOutputControl::CrossfadeCurrent { mix } if mix < 1.0) =>
                    {
                        if sample.address.address().owner() == ProgrammingOwner::Position {
                            self.record_envelope_uses(index, uses);
                            let completion = begin_known_position_completion(index, None, scratch)?;
                            scratch
                                .tasks
                                .push(BaseTask::CompleteKnownPosition { index, completion });
                        } else {
                            self.last = Some(BaseResult::Sample(resolve_whole(
                                index, None, context, frame, scratch,
                            )?));
                        }
                    }
                    FamilyCompositionSample::Known(sample) => {
                        self.last = Some(BaseResult::Sample(sample.clone()))
                    }
                    FamilyCompositionSample::WholeExpression {
                        expression,
                        activation_mix,
                        ..
                    } if expression.needs_underlay() || *activation_mix < 1.0 => {
                        let underlay = underlay_route(&uses, index, scratch);
                        self.record_envelope_uses(index, uses);
                        scratch.tasks.push(BaseTask::Whole { index });
                        scratch.tasks.push(BaseTask::Compose {
                            end: cursor,
                            uses: underlay,
                        });
                    }
                    FamilyCompositionSample::WholeExpression { .. } => {
                        self.record_envelope_uses(index, uses);
                        let inputs = whole::begin(index, None, scratch)?;
                        scratch
                            .tasks
                            .push(BaseTask::EvaluateWhole { index, inputs });
                    }
                }
            }
            BaseTask::Whole { index } => {
                let underlay = self.last.take().expect("completed lower prefix").value();
                if matches!(
                    scratch.sources[index],
                    FamilyCompositionSample::WholeExpression { .. }
                ) {
                    let inputs = whole::begin(index, Some(underlay), scratch)?;
                    scratch
                        .tasks
                        .push(BaseTask::EvaluateWhole { index, inputs });
                } else if matches!(&scratch.sources[index], FamilyCompositionSample::Known(sample) if sample.address.address().owner() == ProgrammingOwner::Position)
                {
                    let completion =
                        begin_known_position_completion(index, Some(underlay), scratch)?;
                    scratch
                        .tasks
                        .push(BaseTask::CompleteKnownPosition { index, completion });
                } else {
                    self.last = Some(BaseResult::Sample(resolve_whole(
                        index,
                        Some(&underlay),
                        context,
                        frame,
                        scratch,
                    )?));
                }
            }
            BaseTask::Winner { cursor, uses } => {
                let winner = self.last.take().expect("resolved winner").sample();
                let mut batch = scratch.batches.pop().unwrap_or_default();
                batch.samples.clear();
                batch.uses = uses;
                batch.samples.push(winner.clone());
                batch.origins.clear();
                batch.origins.push(
                    scratch
                        .source_origins
                        .get(ordered[cursor])
                        .cloned()
                        .unwrap_or_default(),
                );
                if covers_lower(&winner) {
                    self.finish_or_schedule_batch(context, batch, scratch, matcher)?;
                } else {
                    scratch.tasks.push(BaseTask::Scan {
                        winner,
                        cursor,
                        batch,
                    });
                }
            }
            BaseTask::Scan {
                winner,
                cursor,
                batch,
            } => {
                let candidate = (0..cursor).rev().find(|&cursor| {
                    let source = &scratch.sources[ordered[cursor]];
                    !source.is_orthogonal() && could_match(source, &winner, context)
                });
                if let Some(cursor) = candidate {
                    let uses = batch.uses.clone();
                    scratch.tasks.push(BaseTask::Candidate {
                        winner,
                        cursor,
                        batch,
                    });
                    scratch.tasks.push(BaseTask::Resolve { cursor, uses });
                } else {
                    self.finish_or_schedule_batch(context, batch, scratch, matcher)?;
                }
            }
            BaseTask::Candidate {
                winner,
                cursor,
                mut batch,
            } => {
                let sample = self.last.take().expect("resolved candidate").sample();
                let mut covered = false;
                if compatible(&sample, &winner) {
                    covered = covers_lower(&sample);
                    batch.samples.push(sample);
                    batch.origins.push(
                        scratch
                            .source_origins
                            .get(ordered[cursor])
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                if covered {
                    self.finish_or_schedule_batch(context, batch, scratch, matcher)?;
                } else {
                    scratch.tasks.push(BaseTask::Scan {
                        winner,
                        cursor,
                        batch,
                    });
                }
            }
        }
        Ok(BaseEvaluationProgress::Pending)
    }

    fn finish_or_schedule_batch(
        &mut self,
        context: &FamilyCompositionContext<'_>,
        mut batch: ResolvedBatch,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<(), TransitionError> {
        if self.base.programming_owner() == Some(ProgrammingOwner::Position) {
            batch.samples.reverse();
            batch.origins.reverse();
            batch.ordered.clear();
            batch.ordered.extend(0..batch.samples.len());
            let mut evaluation = position_segment::PositionSegmentEvaluation::begin(
                &self.base,
                self.base_trace,
                &batch.samples,
                &batch.ordered,
                &self.original_base,
                scratch.trace_enabled,
            );
            let mut stop = None;
            if matcher.is_enabled() {
                for (index, sample) in batch.samples.iter().enumerate() {
                    let adoption = segment_route(
                        &batch,
                        index,
                        PreparedPositionStageKind::Adoption {
                            component: sample.address.address().component,
                        },
                    )?;
                    if let Some(operand) = matcher.match_route(&adoption)? {
                        ensure(
                            stop.is_none(),
                            "Position segment matches multiple stage stops",
                        )?;
                        ensure(
                            operand == position_segment::PositionSegmentOperand::AdoptionInput,
                            "Position adoption stop has another operand role",
                        )?;
                        stop = Some((index, operand));
                    }
                    if sample.address.address().component.is_none() {
                        let transition = segment_route(
                            &batch,
                            index,
                            PreparedPositionStageKind::WholeSegmentTransition,
                        )?;
                        if let Some(operand) = matcher.match_route(&transition)? {
                            ensure(
                                stop.is_none(),
                                "Position segment matches multiple stage stops",
                            )?;
                            ensure(
                                operand != position_segment::PositionSegmentOperand::AdoptionInput,
                                "Position transition stop has an adoption operand role",
                            )?;
                            stop = Some((index, operand));
                        }
                    }
                }
            }
            if let Some((index, operand)) = stop {
                evaluation.stop_at(index, operand)?;
            }
            scratch
                .tasks
                .push(BaseTask::EvaluatePositionSegment { evaluation, batch });
        } else {
            self.last = Some(BaseResult::Value(finish_batch(
                &self.base,
                self.base_trace,
                context,
                &self.original_base,
                batch,
                scratch,
            )?));
        }
        Ok(())
    }

    fn record_envelope_uses(&mut self, index: usize, uses: Vec<PreparedPositionStageUse>) {
        if !self
            .envelope_uses
            .iter()
            .any(|(source, _)| *source == index)
        {
            self.envelope_uses.push((index, uses));
        }
    }

    /// Route a completion request of `index` to its exact original envelope operation. A
    /// probing completion stopped before that operation is either a matched operand (left
    /// unexecuted, with no parent suffix) or passed back into ordinary execution. A real
    /// materialization only gains the route needed for its pending envelope locator.
    fn envelope(
        &mut self,
        index: usize,
        request: &mut BaseMaterializationRequest,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<Option<BaseEvaluationProgress>, TransitionError> {
        let BaseMaterializationOperation::Completion(operation) = &request.operation else {
            return Ok(None);
        };
        let stage = operation.stage;
        let (uses, completion) = match scratch.tasks.last_mut() {
            Some(BaseTask::Coupled {
                index: task,
                inputs,
                uses,
                ..
            }) if *task == index => (Some(uses.clone()), inputs.completion.as_mut()),
            Some(BaseTask::EvaluateWhole {
                index: task,
                inputs,
            }) if *task == index => (None, inputs.completion.as_mut()),
            Some(BaseTask::CompleteKnownPosition {
                index: task,
                completion,
            }) if *task == index => (None, Some(completion)),
            _ => (None, None),
        };
        let completion =
            completion.ok_or_else(|| IntentError("Position completion task is absent".into()))?;
        let uses = match uses {
            Some(uses) => uses,
            None => self
                .envelope_uses
                .iter()
                .find(|(source, _)| *source == index)
                .map(|(_, uses)| uses.clone())
                .ok_or_else(|| IntentError("Position completion has no lexical use".into()))?,
        };
        let route = PreparedPositionStageRoute {
            uses,
            trigger_origins: request.source_origins.clone(),
            kind: PreparedPositionStageKind::Envelope(stage),
        };
        if completion.pending_boundary() != Some(stage) {
            request.stage_route = Some(route);
            return Ok(None);
        }
        if let Some(operand) = matcher.match_route(&route)? {
            return Ok(Some(BaseEvaluationProgress::OperandReady(match operand {
                position_segment::PositionSegmentOperand::TransitionFrom => operation.from.clone(),
                position_segment::PositionSegmentOperand::TransitionTo => operation.to.clone(),
                position_segment::PositionSegmentOperand::AdoptionInput => {
                    return Err(IntentError(
                        "Position envelope stop has an adoption operand role".into(),
                    )
                    .into());
                }
            })));
        }
        completion.pass_boundary(stage)?;
        Ok(Some(BaseEvaluationProgress::Pending))
    }

    pub(super) fn resume_materialization(
        &mut self,
        source_index: usize,
        node: usize,
        value: AttributeValue,
    ) -> Result<(), TransitionError> {
        self.resume_materialization_traced(source_index, node, value, None)
    }

    pub(super) fn resume_materialization_traced(
        &mut self,
        source_index: usize,
        node: usize,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        if self.failed {
            return Err(IntentError("failed base evaluation must be discarded".into()).into());
        }
        self.failed = true;
        let result = self.resume_materialization_impl(source_index, node, value, transfer);
        // Invalid responses remain retryable; unwind skips this reset and is terminal.
        self.failed = false;
        result
    }

    fn resume_materialization_impl(
        &mut self,
        source_index: usize,
        node: usize,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        let (pending_index, request) = self
            .materialization
            .as_ref()
            .ok_or_else(|| IntentError("base evaluation has no pending materialization".into()))?;
        if *pending_index != source_index || request.node != node {
            return Err(IntentError(
                "base materialization response names another source or graph node".into(),
            )
            .into());
        }
        match (&request.operation, self.tasks.last_mut()) {
            (
                BaseMaterializationOperation::Segment(inner),
                Some(BaseTask::EvaluatePositionSegment { evaluation, .. }),
            ) => {
                evaluation.resume(inner.node, value, transfer)?;
            }

            (
                BaseMaterializationOperation::Coupled(inner),
                Some(BaseTask::Coupled { index, inputs, .. }),
            ) if *index == source_index => {
                inputs
                    .evaluation
                    .as_mut()
                    .ok_or_else(|| IntentError("pending coupled evaluation is absent".into()))?
                    .resume_materialization(inner.node, value)?;
            }
            (
                BaseMaterializationOperation::Whole(inner),
                Some(BaseTask::EvaluateWhole { index, inputs }),
            ) if *index == source_index => {
                inputs
                    .evaluation
                    .resume_materialization(inner.node, value, transfer)?;
            }
            (
                BaseMaterializationOperation::Completion(request),
                Some(BaseTask::Coupled { index, inputs, .. }),
            ) if *index == source_index => {
                inputs
                    .completion
                    .as_mut()
                    .ok_or_else(|| IntentError("pending Position completion is absent".into()))?
                    .resume(request.stage, value, transfer)?;
            }
            (
                BaseMaterializationOperation::Completion(request),
                Some(BaseTask::EvaluateWhole { index, inputs }),
            ) if *index == source_index => {
                inputs
                    .completion
                    .as_mut()
                    .ok_or_else(|| IntentError("pending Position completion is absent".into()))?
                    .resume(request.stage, value, transfer)?;
            }
            (
                BaseMaterializationOperation::Completion(request),
                Some(BaseTask::CompleteKnownPosition { index, completion }),
            ) if *index == source_index => {
                completion.resume(request.stage, value, transfer)?;
            }
            (
                BaseMaterializationOperation::SourceCohort {
                    endpoint_index,
                    child_source_index,
                    request: child_request,
                    ..
                },
                Some(BaseTask::Coupled {
                    index,
                    step,
                    inputs,
                    ..
                }),
            ) if *index == source_index && *step - 1 == *endpoint_index => {
                let child = inputs
                    .source_cohort
                    .as_mut()
                    .ok_or_else(|| IntentError("pending source cohort is absent".into()))?;
                child.evaluation.resume_materialization_traced(
                    *child_source_index,
                    child_request.node,
                    value,
                    transfer,
                )?;
            }
            _ => {
                return Err(
                    IntentError("pending base materialization lost its task".into()).into(),
                );
            }
        }
        self.materialization = None;
        Ok(())
    }

    pub(super) fn recycle(mut self, scratch: &mut RetainedFamilyCompositionScratch) {
        // A synchronous caller may abandon a materialization suspension. Recover its active
        // nested workspace before dropping tasks, just as the former error path did.
        for task in &mut self.tasks {
            if let BaseTask::Coupled { inputs, .. } | BaseTask::CollectCoupled { inputs, .. } = task
                && let Some(mut child) = inputs.source_cohort.take()
            {
                child.evaluation.recycle(&mut child.scratch);
                scratch.source_cohort_scratch = Some(child.scratch);
            }
        }
        for task in &mut self.tasks {
            if let BaseTask::EvaluatePositionSegment { batch, .. } = task {
                scratch.batches.push(std::mem::take(batch));
            }
        }
        // The completed/discarded evaluation no longer exposes task state; retain capacity.
        scratch.tasks = self.tasks;
        scratch.tasks.clear();
    }
}

#[cfg(test)]
mod tests;

fn underlay_route(
    uses: &[PreparedPositionStageUse],
    consumer: usize,
    scratch: &RetainedFamilyCompositionScratch,
) -> Vec<PreparedPositionStageUse> {
    let mut next = uses.to_vec();
    next.push(PreparedPositionStageUse::UnderlayFor {
        consumer_origins: scratch
            .source_origins
            .get(consumer)
            .cloned()
            .unwrap_or_default(),
    });
    next
}
fn segment_route(
    batch: &ResolvedBatch,
    sample: usize,
    kind: PreparedPositionStageKind,
) -> Result<PreparedPositionStageRoute, TransitionError> {
    Ok(PreparedPositionStageRoute {
        uses: batch.uses.clone(),
        trigger_origins: batch
            .origins
            .get(sample)
            .cloned()
            .ok_or_else(|| IntentError("Position stage trigger membership is absent".into()))?,
        kind,
    })
}
