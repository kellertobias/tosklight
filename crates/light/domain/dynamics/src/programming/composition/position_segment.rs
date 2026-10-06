//! Position-only continuation for a ranked component segment. The exact intermediate owner,
//! edit batch and trace are retained while its pinned-frame adoption is unavailable.
use super::*;
use crate::{FamilyExpressionOperation, WholeFamilyExpressionFrameResolver};

#[derive(Clone)]
pub struct PositionSegmentRequest {
    /// Local continuation identity. Captured frame/cut identity belongs to the caller.
    pub node: usize,
    pub(crate) sample_index: usize,
    pub requirement: TransitionRequirement,
    /// Actual captured source rank; independent of the local continuation node.
    pub rank: FamilySampleRank,
    pub operation: PositionSegmentOperation,
}

#[derive(Clone)]
pub enum PositionSegmentOperation {
    Adoption {
        from: AttributeValue,
        address: DynamicValueAddress,
    },
    Transition {
        from: AttributeValue,
        to: AttributeValue,
        progress: f32,
    },
}

/// Internal operand role at an actual active segment operation. Local sample indices are
/// execution locators, never captured source authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PositionSegmentOperand {
    AdoptionInput,
    TransitionFrom,
    TransitionTo,
}

pub(super) enum PositionSegmentProgress {
    Complete(retained_family::TracedValue),
    NeedsMaterialization(PositionSegmentRequest),
    OperandReady(AttributeValue),
}

#[derive(Clone, Copy)]
enum Next {
    Component(usize),
    Whole(usize),
}

struct Pending {
    request: PositionSegmentRequest,
    next: Next,
    response: Option<(AttributeValue, Option<ProgrammingTransitionTrace>)>,
}

pub(super) struct PositionSegmentEvaluation {
    value: AttributeValue,
    trace: Option<FamilyTraceNodeId>,
    original_base: AttributeValue,
    samples: Box<[FamilySample]>,
    ordered: Box<[usize]>,
    winner: Option<usize>,
    covered: Vec<bool>,
    cursor: usize,
    initialized: bool,
    tracing: bool,
    next_node: usize,
    pending: Option<Pending>,
    completed: Option<retained_family::TracedValue>,
    failed: bool,
    stop_target: Option<(usize, PositionSegmentOperand)>,
    operand_ready: Option<AttributeValue>,
}

impl PositionSegmentEvaluation {
    pub(super) fn begin(
        base: &AttributeValue,
        base_trace: Option<FamilyTraceNodeId>,
        samples: &[FamilySample],
        ordered: &[usize],
        original_base: &AttributeValue,
        tracing: bool,
    ) -> Self {
        Self {
            value: base.clone(),
            trace: base_trace,
            original_base: original_base.clone(),
            samples: samples.into(),
            ordered: ordered.into(),
            winner: None,
            covered: Vec::new(),
            cursor: 0,
            initialized: false,
            tracing,
            next_node: 0,
            pending: None,
            completed: None,
            failed: false,
            stop_target: None,
            operand_ready: None,
        }
    }

    /// Configure a child-only operand view before execution starts. An active sample that
    /// never reaches this role (including covered/replaced samples) completes ordinarily.
    pub(super) fn stop_at(
        &mut self,
        sample_index: usize,
        operand: PositionSegmentOperand,
    ) -> Result<(), TransitionError> {
        ensure(
            !self.initialized
                && !self.failed
                && self.pending.is_none()
                && self.completed.is_none()
                && self.operand_ready.is_none(),
            "Position segment operand stop must be configured before execution",
        )?;
        ensure(
            sample_index < self.samples.len(),
            "Position segment operand sample is absent",
        )?;
        ensure(
            self.stop_target.is_none(),
            "Position segment operand stop is already configured",
        )?;
        self.stop_target = Some((sample_index, operand));
        Ok(())
    }

    fn stop_operand(
        &mut self,
        index: usize,
        operand: PositionSegmentOperand,
        value: AttributeValue,
    ) -> Option<PositionSegmentProgress> {
        if self.stop_target == Some((index, operand)) {
            self.operand_ready = Some(value.clone());
            Some(PositionSegmentProgress::OperandReady(value))
        } else {
            None
        }
    }

    pub(super) fn advance(
        &mut self,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut ComponentCompositionScratch,
        trace: &mut FamilyTraceArena,
    ) -> Result<PositionSegmentProgress, TransitionError> {
        ensure(!self.failed, "failed Position segment must be discarded")?;
        if let Some(value) = &self.operand_ready {
            return Ok(PositionSegmentProgress::OperandReady(value.clone()));
        }
        if let Some(result) = &self.completed {
            return Ok(PositionSegmentProgress::Complete(result.clone()));
        }
        if let Some(pending) = &self.pending
            && pending.response.is_none()
        {
            return Ok(PositionSegmentProgress::NeedsMaterialization(
                pending.request.clone(),
            ));
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.advance_inner(context, frame, scratch, trace)
        }));
        match result {
            Ok(result) => {
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

    fn advance_inner(
        &mut self,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut ComponentCompositionScratch,
        trace: &mut FamilyTraceArena,
    ) -> Result<PositionSegmentProgress, TransitionError> {
        if !self.initialized {
            ensure(
                self.value.programming_owner() == Some(ProgrammingOwner::Position),
                "Position segment requires a Position base",
            )?;
            ensure(
                !self.tracing || self.trace.is_some(),
                "traced Position segment has no base trace",
            )?;
            for &index in &self.ordered {
                let sample = self
                    .samples
                    .get(index)
                    .ok_or_else(|| IntentError("Position segment source index is absent".into()))?;
                ensure(
                    sample.address.address().owner() == ProgrammingOwner::Position,
                    "Position segment contains another family",
                )?;
                sample.validate_value()?;
            }
            self.winner = self.ordered.last().copied();
            if let Some(winner) = self.winner {
                let start = self
                    .ordered
                    .iter()
                    .rposition(|&index| {
                        let sample = &self.samples[index];
                        compatible(sample, &self.samples[winner])
                            && sample.address.address().component.is_none()
                            && sample.activation_mix == 1.0
                    })
                    .unwrap_or(0);
                self.ordered = self.ordered[start..].into();
                mark_covered_components(&self.samples, &self.ordered, winner, false, scratch);
                self.covered.clone_from(&scratch.covered);
            }
            self.initialized = true;
        }
        if let Some(mut pending) = self.pending.take() {
            let (value, transfer) = pending.response.take().expect("ready response");
            match pending.request.operation {
                PositionSegmentOperation::Adoption { .. } => {
                    self.value = value;
                    if self.tracing {
                        let prior = self.trace.expect("traced adoption");
                        self.trace = Some(trace.mapped_blend(prior, prior, transfer));
                    }
                    match pending.next {
                        Next::Component(index) => self.component(index, context, scratch, trace)?,
                        Next::Whole(index) => {
                            if let Some(progress) = self.whole(index, context, frame, trace)? {
                                return Ok(progress);
                            }
                        }
                    }
                }
                PositionSegmentOperation::Transition { .. } => {
                    self.value = value;
                    let Next::Whole(index) = pending.next else {
                        unreachable!("whole transition")
                    };
                    if self.tracing {
                        let incoming = sample_trace(&self.samples[index], trace);
                        self.trace = Some(trace.mapped_blend(
                            self.trace.expect("whole prefix"),
                            incoming,
                            transfer,
                        ));
                    }
                }
            }
            self.cursor += 1;
        }
        while self.cursor < self.ordered.len() {
            let index = self.ordered[self.cursor];
            let sample = &self.samples[index];
            if self.covered[self.cursor]
                || !compatible(sample, &self.samples[self.winner.expect("source winner")])
            {
                self.cursor += 1;
                continue;
            }
            if let Some(DynamicValue::Family(target)) = sample.materialized_value() {
                if sample.activation_mix == 1.0 {
                    self.value = target.clone();
                    scratch.components.clear();
                    scratch.pending_native_model = None;
                    self.write_whole(index, trace);
                    self.cursor += 1;
                    continue;
                }
                flush(&mut self.value, context, scratch)?;
                if let Some(progress) = self.adoption(index, Next::Whole(index), context, trace)? {
                    return Ok(progress);
                }
                if let Some(progress) = self.whole(index, context, frame, trace)? {
                    return Ok(progress);
                }
            } else {
                if scratch.components.is_empty() {
                    if let Some(progress) =
                        self.adoption(index, Next::Component(index), context, trace)?
                    {
                        return Ok(progress);
                    }
                    scratch.pending_native_model = None;
                }
                self.component(index, context, scratch, trace)?;
            }
            self.cursor += 1;
        }
        flush(&mut self.value, context, scratch)?;
        let result = retained_family::TracedValue {
            value: self.value.clone(),
            trace: self.trace,
        };
        self.completed = Some(result.clone());
        Ok(PositionSegmentProgress::Complete(result))
    }

    fn adoption(
        &mut self,
        index: usize,
        next: Next,
        context: &FamilyCompositionContext<'_>,
        trace: &mut FamilyTraceArena,
    ) -> Result<Option<PositionSegmentProgress>, TransitionError> {
        if let Some(progress) = self.stop_operand(
            index,
            PositionSegmentOperand::AdoptionInput,
            self.value.clone(),
        ) {
            return Ok(Some(progress));
        }
        let address = self.samples[index].address.address();
        let converted = !address.matches_authored_source(&self.value);
        match adopt(self.value.clone(), address, context, &self.original_base) {
            Ok(value) => {
                self.value = value;
                if converted && self.tracing {
                    let prior = self.trace.expect("traced adoption base");
                    self.trace = Some(trace.mapped_blend(prior, prior, None));
                }
                Ok(None)
            }
            Err(TransitionError::Requires(requirement)) => {
                let operation = PositionSegmentOperation::Adoption {
                    from: self.value.clone(),
                    address: address.clone(),
                };
                Ok(Some(self.wait(requirement, operation, next)))
            }
            Err(error) => Err(error),
        }
    }

    fn whole(
        &mut self,
        index: usize,
        _context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        trace: &mut FamilyTraceArena,
    ) -> Result<Option<PositionSegmentProgress>, TransitionError> {
        let sample = &self.samples[index];
        let Some(DynamicValue::Family(target)) = sample.materialized_value() else {
            unreachable!("whole sample")
        };
        let progress = sample.activation_mix;
        let target = target.clone();
        if let Some(ready) = self.stop_operand(
            index,
            PositionSegmentOperand::TransitionFrom,
            self.value.clone(),
        ) {
            return Ok(Some(ready));
        }
        if let Some(ready) =
            self.stop_operand(index, PositionSegmentOperand::TransitionTo, target.clone())
        {
            return Ok(Some(ready));
        }
        let result = CompiledProgrammingTransition::new(self.value.clone(), target.clone(), None)
            .and_then(|compiled| compiled.sample(progress));
        match result {
            Ok(value) => {
                self.value = value;
                self.write_whole(index, trace);
                Ok(None)
            }
            Err(TransitionError::Requires(requirement)) => {
                let from = self.value.clone();
                let to = target.clone();
                let operation = FamilyExpressionOperation::Transition { progress };
                let result = if self.tracing {
                    frame.resolve_with_trace(requirement, &from, &to, operation)
                } else {
                    frame
                        .resolve(requirement, &from, &to, operation)
                        .map(|value| (value, None))
                };
                match result {
                    Ok((value, transfer)) => {
                        validate_position(&value)?;
                        validate_transfer(transfer.as_ref())?;
                        self.value = value;
                        if self.tracing {
                            let incoming = sample_trace(&self.samples[index], trace);
                            self.trace = Some(trace.mapped_blend(
                                self.trace.expect("whole prefix"),
                                incoming,
                                transfer,
                            ));
                        }
                        Ok(None)
                    }
                    Err(TransitionError::Requires(requirement)) => Ok(Some(self.wait(
                        requirement,
                        PositionSegmentOperation::Transition { from, to, progress },
                        Next::Whole(index),
                    ))),
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }

    fn write_whole(&mut self, index: usize, trace: &mut FamilyTraceArena) {
        if self.tracing {
            let sample = &self.samples[index];
            let incoming = sample_trace(sample, trace);
            self.trace = Some(trace.write(
                self.trace.expect("whole prefix"),
                incoming,
                FamilyTraceFootprint::Whole,
                sample.activation_mix < 1.0,
            ));
        }
    }

    fn component(
        &mut self,
        index: usize,
        context: &FamilyCompositionContext<'_>,
        scratch: &mut ComponentCompositionScratch,
        trace: &mut FamilyTraceArena,
    ) -> Result<(), TransitionError> {
        let sample = &self.samples[index];
        let component = sample
            .address
            .address()
            .component
            .expect("component sample");
        let existing = scratch
            .components
            .iter()
            .position(|(key, _)| *key == component);
        let underlay = if sample.component_needs_underlay() {
            Some(if let Some(index) = existing {
                scratch.components[index].1.clone()
            } else {
                extract_compatible_dynamic_value(
                    &self.value,
                    sample.address.address(),
                    &context.edit,
                )?
                .ok_or_else(|| TransitionError::Requires(requirement(sample.address.address())))?
            })
        } else {
            None
        };
        let (target, source_trace) = component_value_and_trace(
            sample,
            underlay.as_ref(),
            self.tracing.then_some(&mut *trace),
            self.trace,
        )?;
        let raw_context;
        let output_context = if sample.endpoint_output_exempt || sample.fix_at {
            raw_context = endpoint_output::with_control(context, None);
            &raw_context
        } else {
            context
        };
        let (target, source_trace) = endpoint_output::component(
            sample.rank,
            &sample.address,
            target,
            source_trace,
            output_context,
            self.tracing.then_some(&mut *trace),
        )?;
        let mixed = if sample.activation_mix == 1.0 {
            target
        } else {
            sample
                .address
                .transition(underlay.expect("partial component underlay"), target)?
                .sample(sample.activation_mix)?
        };
        if self.tracing {
            let prefix = self.trace.expect("component prefix");
            let source = endpoint_output::control_trace(
                sample.rank,
                FamilyTraceFootprint::Component(component),
                source_trace.expect("component source"),
                (sample.activation_mix < 1.0).then_some(prefix),
                output_context,
                trace,
            );
            self.trace = Some(trace.write(
                prefix,
                source,
                FamilyTraceFootprint::Component(component),
                sample.component_needs_underlay(),
            ));
        }
        if let Some(index) = existing {
            scratch.components[index].1 = mixed;
        } else {
            scratch.components.push((component, mixed));
        }
        Ok(())
    }

    fn wait(
        &mut self,
        requirement: TransitionRequirement,
        operation: PositionSegmentOperation,
        next: Next,
    ) -> PositionSegmentProgress {
        let index = match next {
            Next::Component(index) | Next::Whole(index) => index,
        };
        let request = PositionSegmentRequest {
            node: self.next_node,
            sample_index: index,
            requirement,
            rank: self.samples[index].rank,
            operation,
        };
        self.next_node += 1;
        self.pending = Some(Pending {
            request: request.clone(),
            next,
            response: None,
        });
        PositionSegmentProgress::NeedsMaterialization(request)
    }

    /// Invalid/misaddressed responses leave the original request and completed prefix intact.
    pub(super) fn resume(
        &mut self,
        node: usize,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        ensure(!self.failed, "failed Position segment must be discarded")?;
        let pending = self
            .pending
            .as_mut()
            .ok_or_else(|| IntentError("Position segment is not waiting".into()))?;
        ensure(
            pending.request.node == node && pending.response.is_none(),
            "Position segment response names another or completed request",
        )?;
        validate_position(&value)?;
        validate_transfer(transfer.as_ref())?;
        if let PositionSegmentOperation::Adoption { address, .. } = &pending.request.operation {
            ensure(
                address.matches_authored_source(&value),
                "Position adoption has another representation or reference",
            )?;
        }
        pending.response = Some((value, transfer));
        Ok(())
    }
}

fn validate_position(value: &AttributeValue) -> Result<(), TransitionError> {
    ensure(
        value.programming_owner() == Some(ProgrammingOwner::Position)
            && value.spread_control_points() == 0
            && !matches!(value, AttributeValue::GroupFamily(_)),
        "Position materialization must be a complete materialized Position owner",
    )?;
    value.validate_programming_address(ProgrammingOwner::Position.key_ref())?;
    Ok(())
}

fn validate_transfer(transfer: Option<&ProgrammingTransitionTrace>) -> Result<(), TransitionError> {
    if let Some(transfer) = transfer {
        for transfer in [&transfer.from, &transfer.to] {
            transfer.identity.validate(ProgrammingOwner::Position)?;
            ProgrammingFieldScope::new(transfer.remap.iter().flat_map(|(from, to)| [*from, *to]))
                .validate(ProgrammingOwner::Position)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod operand_tests;
