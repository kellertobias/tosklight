//! Heap-task steps that advance one suspended source evaluation (component segment, Known
//! Position completion, whole-source graph or coupled cohort) and surface its stops.
use super::*;

impl BaseEvaluation {
    pub(super) fn advance_position_segment(
        &mut self,
        mut evaluation: position_segment::PositionSegmentEvaluation,
        batch: ResolvedBatch,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> Result<BaseEvaluationProgress, TransitionError> {
        match evaluation.advance(context, frame, &mut scratch.components, &mut scratch.trace)? {
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
                    source_origins: batch.origins.get(inner.sample_index).cloned().ok_or_else(
                        || IntentError("Position segment source origin is absent".into()),
                    )?,
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
        Ok(BaseEvaluationProgress::Pending)
    }

    pub(super) fn advance_known_position(
        &mut self,
        index: usize,
        mut completion: position_completion::PositionSourceCompletion,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<BaseEvaluationProgress, TransitionError> {
        let context = if matches!(&scratch.sources[index], FamilyCompositionSample::Known(sample) if sample.fix_at || sample.endpoint_output_exempt)
        {
            endpoint_output::with_control(context, None)
        } else {
            endpoint_output::with_control(context, context.endpoint_output)
        };
        match completion.advance(&context, frame, &mut scratch.trace, scratch.trace_enabled)? {
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
                if let Some(progress) = self.envelope(index, &mut request, scratch, matcher)? {
                    return Ok(progress);
                }
                self.materialization = Some((index, request.clone()));
                return Ok(BaseEvaluationProgress::NeedsMaterialization {
                    source_index: index,
                    request,
                });
            }
        }
        Ok(BaseEvaluationProgress::Pending)
    }

    pub(super) fn advance_evaluate_whole(
        &mut self,
        index: usize,
        inputs: whole::Inputs,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<BaseEvaluationProgress, TransitionError> {
        let uses = self
            .envelope_uses
            .iter()
            .find(|(source, _)| *source == index)
            .map(|(_, uses)| uses.clone())
            .unwrap_or_default();
        match whole::advance_with_graph(index, inputs, uses, context, frame, scratch, matcher)? {
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
                if let Some(progress) = self.envelope(index, &mut request, scratch, matcher)? {
                    return Ok(progress);
                }
                self.materialization = Some((index, request.clone()));
                return Ok(BaseEvaluationProgress::NeedsMaterialization {
                    source_index: index,
                    request,
                });
            }
        }
        Ok(BaseEvaluationProgress::Pending)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn advance_coupled(
        &mut self,
        index: usize,
        cursor: usize,
        step: usize,
        inputs: coupled::BaseInputs,
        uses: Vec<PreparedPositionStageUse>,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<BaseEvaluationProgress, TransitionError> {
        match coupled::advance_with_stage(
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
        }
        Ok(BaseEvaluationProgress::Pending)
    }
}
