//! Heap-task steps that walk the ordered source stack: prefix composition, source
//! resolution, winner/candidate scanning and batch scheduling. None of them suspends.
use super::*;

impl BaseEvaluation {
    pub(super) fn collect_coupled(
        &mut self,
        index: usize,
        cursor: usize,
        step: usize,
        mut inputs: coupled::BaseInputs,
        uses: Vec<PreparedPositionStageUse>,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) {
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

    #[allow(clippy::too_many_arguments)]
    pub(super) fn schedule_cohort(
        &mut self,
        end: usize,
        candidates: Vec<FamilySample>,
        origins: Vec<Vec<PreparedSourceOrigin>>,
        uses: Vec<PreparedPositionStageUse>,
        context: &FamilyCompositionContext<'_>,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<(), TransitionError> {
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
        Ok(())
    }

    pub(super) fn compose_prefix(
        &mut self,
        end: usize,
        uses: Vec<PreparedPositionStageUse>,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) {
        if let Some(cursor) = self.ordered[..end]
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
                value: self.base.clone(),
                trace: self.base_trace,
            }));
        }
    }

    pub(super) fn resolve_source(
        &mut self,
        cursor: usize,
        uses: Vec<PreparedPositionStageUse>,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> Result<(), TransitionError> {
        let index = self.ordered[cursor];
        if let Some(value) = &scratch.resolved[index] {
            self.last = Some(BaseResult::Sample(value.clone()));
            return Ok(());
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
                if sample.address.address().component.is_none() && sample.activation_mix < 1.0 =>
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
        Ok(())
    }

    pub(super) fn resolve_over_underlay(
        &mut self,
        index: usize,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> Result<(), TransitionError> {
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
            let completion = begin_known_position_completion(index, Some(underlay), scratch)?;
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
        Ok(())
    }

    pub(super) fn start_winner_batch(
        &mut self,
        cursor: usize,
        uses: Vec<PreparedPositionStageUse>,
        context: &FamilyCompositionContext<'_>,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<(), TransitionError> {
        let winner = self.last.take().expect("resolved winner").sample();
        let mut batch = scratch.batches.pop().unwrap_or_default();
        batch.samples.clear();
        batch.uses = uses;
        batch.samples.push(winner.clone());
        batch.origins.clear();
        batch.origins.push(
            scratch
                .source_origins
                .get(self.ordered[cursor])
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
        Ok(())
    }

    pub(super) fn scan_for_candidate(
        &mut self,
        winner: FamilySample,
        cursor: usize,
        batch: ResolvedBatch,
        context: &FamilyCompositionContext<'_>,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<(), TransitionError> {
        let candidate = (0..cursor).rev().find(|&cursor| {
            let source = &scratch.sources[self.ordered[cursor]];
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
        Ok(())
    }

    pub(super) fn accept_candidate(
        &mut self,
        winner: FamilySample,
        cursor: usize,
        mut batch: ResolvedBatch,
        context: &FamilyCompositionContext<'_>,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<(), TransitionError> {
        let sample = self.last.take().expect("resolved candidate").sample();
        let mut covered = false;
        if compatible(&sample, &winner) {
            covered = covers_lower(&sample);
            batch.samples.push(sample);
            batch.origins.push(
                scratch
                    .source_origins
                    .get(self.ordered[cursor])
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
        Ok(())
    }
}
