//! Retained Position endpoint and activation envelope after expression evaluation.
//! Current belongs to the captured context. A mechanical cut never re-reads it or the graph.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PositionCompletionKind {
    Whole,
    Coupled,
    Known,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PositionCompletionStage {
    EndpointOutput,
    Activation,
}
#[derive(Clone)]
pub struct CompletionRequest {
    pub stage: PositionCompletionStage,
    pub requirement: TransitionRequirement,
    pub from: AttributeValue,
    pub to: AttributeValue,
    pub progress: f32,
}
pub(super) enum CompletionProgress {
    Complete(TracedValue),
    Needs(CompletionRequest),
}
thread_local! {
    static ENVELOPE_PROBE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// Stage-aware base evaluation (operand replay/discovery) holds this guard across one
/// synchronous task. Completions constructed inside it stop before each reached envelope
/// operation until their owning evaluation passes or stops it. It carries no route or
/// authority; ordinary composition constructs completions with no boundary stops.
pub(super) struct EnvelopeProbeScope {
    previous: bool,
}
impl EnvelopeProbeScope {
    pub(super) fn enter(enabled: bool) -> Self {
        Self {
            previous: ENVELOPE_PROBE.with(|probe| probe.replace(enabled)),
        }
    }
}
impl Drop for EnvelopeProbeScope {
    fn drop(&mut self) {
        ENVELOPE_PROBE.with(|probe| probe.set(self.previous));
    }
}
struct CapturedOutput {
    control: FamilyEndpointOutputControl,
    current: Option<AttributeValue>,
    occurrence: Option<crate::DynamicSourceOccurrenceId>,
}
pub(super) struct PositionSourceCompletion {
    target: TracedValue,
    rank: FamilySampleRank,
    activation_mix: f32,
    underlay: Option<TracedValue>,
    kind: PositionCompletionKind,
    captured: Option<Result<CapturedOutput, TransitionError>>,
    endpoint: Option<TracedValue>,
    waiting: Option<CompletionRequest>,
    resumed: Option<(AttributeValue, Option<ProgrammingTransitionTrace>)>,
    completed: Option<TracedValue>,
    tracing: Option<bool>,
    failed: bool,
    probe: bool,
    boundary: Option<PositionCompletionStage>,
    passed: [bool; 2],
}
impl PositionSourceCompletion {
    pub(super) fn new(
        target: TracedValue,
        rank: FamilySampleRank,
        activation_mix: f32,
        underlay: Option<TracedValue>,
        kind: PositionCompletionKind,
    ) -> Result<Self, TransitionError> {
        validate_position(&target.value)?;
        ensure(
            activation_mix.is_finite() && (0.0..=1.0).contains(&activation_mix),
            "Position activation influence must be between zero and one",
        )?;
        if activation_mix < 1.0 {
            validate_position(
                &underlay
                    .as_ref()
                    .ok_or(TransitionError::Requires(
                        TransitionRequirement::MaterializedEndpoints,
                    ))?
                    .value,
            )?;
        }
        Ok(Self {
            target,
            rank,
            activation_mix,
            underlay,
            kind,
            captured: None,
            endpoint: None,
            waiting: None,
            resumed: None,
            completed: None,
            tracing: None,
            failed: false,
            probe: ENVELOPE_PROBE.with(std::cell::Cell::get),
            boundary: None,
            passed: [false; 2],
        })
    }
    /// The original operation this probing completion is stopped before, if any. Its
    /// operands are the waiting request's from/to; no sampling has occurred for it.
    pub(super) fn pending_boundary(&self) -> Option<PositionCompletionStage> {
        self.boundary
    }
    /// Let the owner continue through an unmatched boundary into the ordinary operation.
    pub(super) fn pass_boundary(
        &mut self,
        stage: PositionCompletionStage,
    ) -> Result<(), TransitionError> {
        ensure(!self.failed, "failed Position completion must be discarded")?;
        ensure(
            self.boundary == Some(stage),
            "Position completion is not stopped before this stage",
        )?;
        self.boundary = None;
        self.waiting = None;
        self.passed[stage as usize] = true;
        Ok(())
    }
    fn stops_before(&self, stage: PositionCompletionStage) -> bool {
        self.probe && !self.passed[stage as usize]
    }
    fn stop_before(
        &mut self,
        stage: PositionCompletionStage,
        from: AttributeValue,
        to: AttributeValue,
        progress: f32,
    ) -> CompletionProgress {
        // Never surfaced as a materialization: the owning base evaluation intercepts it.
        let request = CompletionRequest {
            stage,
            requirement: TransitionRequirement::MaterializedEndpoints,
            from,
            to,
            progress,
        };
        self.boundary = Some(stage);
        self.waiting = Some(request.clone());
        CompletionProgress::Needs(request)
    }
    pub(super) fn advance(
        &mut self,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        trace: &mut FamilyTraceArena,
        tracing: bool,
    ) -> Result<CompletionProgress, TransitionError> {
        if self.failed {
            return Err(IntentError("failed Position completion must be discarded".into()).into());
        }
        if let Some(previous) = self.tracing {
            ensure(
                previous == tracing,
                "Position completion tracing changed during suspension",
            )?;
        } else {
            self.tracing = Some(tracing);
        }
        if let Some(value) = &self.completed {
            return Ok(CompletionProgress::Complete(value.clone()));
        }
        if let Some(request) = &self.waiting {
            return Ok(CompletionProgress::Needs(request.clone()));
        }
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.advance_inner(context, frame, trace, tracing)
        }));
        match outcome {
            Ok(result) => {
                if matches!(&result, Err(error) if !matches!(error, TransitionError::Requires(_))) {
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
        trace: &mut FamilyTraceArena,
        tracing: bool,
    ) -> Result<CompletionProgress, TransitionError> {
        if self.captured.is_none() {
            self.captured = Some(self.capture_output(context, tracing));
        }
        let captured = self
            .captured
            .as_ref()
            .expect("captured output")
            .as_ref()
            .map_err(Clone::clone)?;
        let controlled = matches!(
            captured.control,
            FamilyEndpointOutputControl::CrossfadeCurrent { .. }
        );
        if self.endpoint.is_none() {
            let crossfade = match captured.control {
                FamilyEndpointOutputControl::CrossfadeCurrent { mix } if mix < 1.0 => Some((
                    captured.current.clone().expect("captured Current"),
                    mix,
                    captured.occurrence,
                )),
                _ => None,
            };
            if let Some(progress) = self.complete_endpoint(crossfade, frame, trace, tracing)? {
                return Ok(progress);
            }
        }
        let endpoint = self.endpoint.as_ref().expect("completed endpoint").clone();
        self.complete_activation(endpoint, controlled, context, frame, trace, tracing)
    }
    fn capture_output(
        &self,
        context: &FamilyCompositionContext<'_>,
        tracing: bool,
    ) -> Result<CapturedOutput, TransitionError> {
        let control = endpoint_output::control(self.rank, context);
        endpoint_output::validate(control)?;
        let (current, occurrence) = if matches!(control,
            FamilyEndpointOutputControl::CrossfadeCurrent { mix } if mix < 1.0)
        {
            let output = context.endpoint_output.expect("captured control context");
            let address =
                DynamicValueAddress::whole_family(ProgrammingOwner::Position, &self.target.value)?;
            let current = output
                .current
                .try_current_family_base(output.target, &address)?
                .ok_or(TransitionError::Requires(requirement(&address)))?;
            validate_position(&current)?;
            let occurrence = tracing
                .then(|| {
                    output
                        .current
                        .current_family_occurrence(output.target, &address)
                })
                .flatten();
            (Some(current), occurrence)
        } else {
            (None, None)
        };
        Ok(CapturedOutput {
            control,
            current,
            occurrence,
        })
    }
    /// Resolve the endpoint-output stage once; `Some` is a stop the caller must surface.
    fn complete_endpoint(
        &mut self,
        crossfade: Option<(
            AttributeValue,
            f32,
            Option<crate::DynamicSourceOccurrenceId>,
        )>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        trace: &mut FamilyTraceArena,
        tracing: bool,
    ) -> Result<Option<CompletionProgress>, TransitionError> {
        let endpoint = if let Some((current, mix, occurrence)) = crossfade {
            if self.stops_before(PositionCompletionStage::EndpointOutput) {
                return Ok(Some(self.stop_before(
                    PositionCompletionStage::EndpointOutput,
                    current,
                    self.target.value.clone(),
                    mix,
                )));
            }
            let (value, transfer) = if mix == 0.0 {
                (current, None)
            } else if let Some(result) = self.resumed.take() {
                result
            } else {
                let request = CompletionRequest {
                    stage: PositionCompletionStage::EndpointOutput,
                    requirement: TransitionRequirement::LiveJointAngles,
                    from: current,
                    to: self.target.value.clone(),
                    progress: mix,
                };
                let Some(result) = self.sample(request, frame, tracing)? else {
                    return Ok(Some(CompletionProgress::Needs(
                        self.waiting.clone().expect("pending endpoint"),
                    )));
                };
                result
            };
            let node = if tracing {
                let current = trace.source(FamilyTraceSource {
                    rank: self.rank,
                    footprint: FamilyTraceFootprint::Whole,
                    role: FamilyTraceRole::CalculationDependency,
                    occurrence,
                });
                Some(if mix == 0.0 {
                    current
                } else {
                    trace.mapped_blend(
                        current,
                        self.target.trace.expect("traced endpoint"),
                        transfer,
                    )
                })
            } else {
                None
            };
            TracedValue { value, trace: node }
        } else {
            self.target.clone()
        };
        self.endpoint = Some(endpoint);
        Ok(None)
    }
    /// Apply the activation envelope over the underlay and record the completed value.
    fn complete_activation(
        &mut self,
        endpoint: TracedValue,
        controlled: bool,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        trace: &mut FamilyTraceArena,
        tracing: bool,
    ) -> Result<CompletionProgress, TransitionError> {
        if self.activation_mix < 1.0 && self.stops_before(PositionCompletionStage::Activation) {
            let underlay = self.underlay.as_ref().expect("activation underlay");
            return Ok(self.stop_before(
                PositionCompletionStage::Activation,
                underlay.value.clone(),
                endpoint.value,
                self.activation_mix,
            ));
        }
        let (value, transfer) = if self.activation_mix == 1.0 {
            (endpoint.value, None)
        } else if let Some(result) = self.resumed.take() {
            result
        } else {
            let request = CompletionRequest {
                stage: PositionCompletionStage::Activation,
                requirement: TransitionRequirement::LiveJointAngles,
                from: self
                    .underlay
                    .as_ref()
                    .expect("activation underlay")
                    .value
                    .clone(),
                to: endpoint.value,
                progress: self.activation_mix,
            };
            // Coupled uncontrolled activation uses ordinary transition without trace.
            let Some(result) = self.sample(
                request,
                frame,
                tracing && (controlled || self.kind != PositionCompletionKind::Coupled),
            )?
            else {
                return Ok(CompletionProgress::Needs(
                    self.waiting.clone().expect("pending activation"),
                ));
            };
            result
        };
        let node = if tracing {
            let appearance = if self.activation_mix == 1.0 {
                endpoint.trace.expect("traced endpoint")
            } else {
                let prior = self
                    .underlay
                    .as_ref()
                    .and_then(|value| value.trace)
                    .expect("traced activation underlay");
                let incoming = endpoint.trace.expect("traced endpoint");
                if self.kind == PositionCompletionKind::Coupled && !controlled {
                    trace.write(prior, incoming, FamilyTraceFootprint::Whole, true)
                } else {
                    trace.mapped_blend(prior, incoming, transfer)
                }
            };
            Some(endpoint_output::control_trace(
                self.rank,
                FamilyTraceFootprint::Whole,
                appearance,
                self.underlay
                    .as_ref()
                    .and_then(|value| value.trace)
                    .filter(|_| self.activation_mix < 1.0),
                context,
                trace,
            ))
        } else {
            None
        };
        let value = TracedValue { value, trace: node };
        self.completed = Some(value.clone());
        Ok(CompletionProgress::Complete(value))
    }
    fn sample(
        &mut self,
        mut request: CompletionRequest,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        traced: bool,
    ) -> Result<Option<(AttributeValue, Option<ProgrammingTransitionTrace>)>, TransitionError> {
        let compiled =
            CompiledProgrammingTransition::new(request.from.clone(), request.to.clone(), None)?;
        let sample = if traced {
            compiled.sample_with_trace(ProgrammingOwner::Position, request.progress)
        } else {
            compiled.sample(request.progress).map(|value| (value, None))
        };
        let result = match sample {
            Ok(result) => result,
            Err(TransitionError::Requires(
                requirement @ (TransitionRequirement::LiveTargetPoints
                | TransitionRequirement::LiveJointAngles),
            )) => {
                request.requirement = requirement;
                let operation = FamilyExpressionOperation::Transition {
                    progress: request.progress,
                };
                let result = if traced {
                    frame.resolve_with_trace(requirement, &request.from, &request.to, operation)
                } else {
                    frame
                        .resolve(requirement, &request.from, &request.to, operation)
                        .map(|value| (value, None))
                };
                match result {
                    Ok(result) => result,
                    Err(TransitionError::Requires(requirement)) => {
                        request.requirement = requirement;
                        self.waiting = Some(request);
                        return Ok(None);
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        };
        validate_position(&result.0)?;
        validate_transfer(result.1.as_ref())?;
        Ok(Some(result))
    }
    pub(super) fn resume(
        &mut self,
        stage: PositionCompletionStage,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        ensure(!self.failed, "failed Position completion must be discarded")?;
        ensure(
            self.boundary.is_none(),
            "Position completion boundary is not a materialization request",
        )?;
        let request = self
            .waiting
            .as_ref()
            .ok_or_else(|| IntentError("Position completion is not waiting".into()))?;
        ensure(
            request.stage == stage,
            "Position completion response names a different stage",
        )?;
        validate_position(&value)?;
        validate_transfer(transfer.as_ref())?;
        self.resumed = Some((value, transfer));
        self.waiting = None;
        Ok(())
    }
}
pub(super) fn validate_position(value: &AttributeValue) -> Result<(), TransitionError> {
    let address = DynamicValueAddress::whole_family(ProgrammingOwner::Position, value)?;
    address.validate_family(value)?;
    ensure(
        !matches!(value, AttributeValue::GroupFamily(_)),
        "Position completion requires a materialized family",
    )?;
    Ok(())
}
pub(super) fn validate_transfer(
    transfer: Option<&ProgrammingTransitionTrace>,
) -> Result<(), TransitionError> {
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
