//! Operand replay has its own terminal result type. It can never be observed as a completed
//! parent program or publish speculative continuity through the ordinary composition API.
use super::*;

pub enum PositionStageOperandProgress {
    NeedsMaterialization(PositionCompositionRequest),
    OperandReady(AttributeValue),
    Inactive,
}
pub struct PositionStageOperandContinuation {
    inner: PositionCompositionContinuation,
    locator: PositionStageLocator,
    operand: PositionStageOperand,
}
impl PositionStageOperandContinuation {
    pub fn pending_resume_operand_locator(
        &self,
        request_id: Uuid,
    ) -> Result<Option<PositionResumeOperandLocator>, TransitionError> {
        self.inner.pending_resume_operand_locator(request_id)
    }
    pub fn enable_graph_discovery(&mut self, route_limit: usize) -> Result<(), TransitionError> {
        self.inner.enable_graph_discovery(route_limit)
    }
    pub fn graph_discovery_report(&self) -> Result<PositionGraphDiscoveryReport, TransitionError> {
        self.inner.graph_discovery_report()
    }
    pub fn enable_graph_materialization(
        &mut self,
        site: &PositionGraphOperationLocator,
    ) -> Result<(), TransitionError> {
        self.inner.enable_graph_materialization(site)
    }
    pub fn graph_materialization_status(
        &self,
    ) -> Result<PositionGraphMaterializationStatus, TransitionError> {
        self.inner.graph_materialization_status()
    }
    pub(crate) fn new(
        mut inner: PositionCompositionContinuation,
        locator: PositionStageLocator,
        operand: PositionStageOperand,
    ) -> Self {
        inner.operand_only = true;
        inner.resume_goal = ResumeGoal::Stage(locator.clone(), operand);
        Self {
            inner,
            locator,
            operand,
        }
    }
    pub fn advance(
        &mut self,
        capture_id: Uuid,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<PositionStageOperandProgress, TransitionError> {
        if capture_id != self.inner.capture_id {
            return Err(IntentError("Position operand belongs to another capture".into()).into());
        }
        ensure(
            !self.inner.failed,
            "failed Position operand must be discarded",
        )?;
        if let Some(value) = &self.inner.stopped_operand {
            return Ok(PositionStageOperandProgress::OperandReady(value.clone()));
        }
        self.inner.failed = true;
        let result = self
            .inner
            .advance_impl(context, frame, Some((&self.locator, self.operand)));
        if result.is_ok() {
            self.inner.failed = false;
        }
        match result? {
            PositionCompositionProgress::NeedsMaterialization(request) => {
                Ok(PositionStageOperandProgress::NeedsMaterialization(request))
            }
            PositionCompositionProgress::Complete(_) => Ok(match &self.inner.stopped_operand {
                Some(value) => PositionStageOperandProgress::OperandReady(value.clone()),
                None => PositionStageOperandProgress::Inactive,
            }),
        }
    }
    /// Nested prefix requests retain this driver's original registry and exact lexical use.
    pub fn pending_graph_operation_locator(
        &self,
        request_id: Uuid,
    ) -> Result<Option<PositionGraphOperationLocator>, TransitionError> {
        self.inner.pending_graph_operation_locator(request_id)
    }
    pub fn pending_stage_locator(
        &self,
        request_id: Uuid,
    ) -> Result<Option<PositionStageLocator>, TransitionError> {
        self.inner.pending_stage_locator(request_id)
    }
    pub fn pending_envelope_locator(
        &self,
        request_id: Uuid,
    ) -> Result<Option<PositionEnvelopeLocator>, TransitionError> {
        self.inner.pending_envelope_locator(request_id)
    }
    pub fn pending_mask_locator(
        &self,
        request_id: Uuid,
    ) -> Result<Option<PositionMaskLocator>, TransitionError> {
        self.inner.pending_mask_locator(request_id)
    }
    pub fn resume_materialization(
        &mut self,
        capture_id: Uuid,
        request_id: Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.inner
            .resume_materialization(capture_id, request_id, value, transfer)
    }
    pub fn into_scratch(self) -> RetainedFamilyCompositionScratch {
        self.inner.into_scratch()
    }
}
impl PositionCompositionContinuation {
    /// Only the actual outstanding request can issue a structural stage locator. Legacy
    /// unbound composition intentionally supplies no original-registry authority.
    pub fn pending_stage_locator(
        &self,
        request_id: Uuid,
    ) -> Result<Option<PositionStageLocator>, TransitionError> {
        ensure(
            !self.failed,
            "failed Position composition has no stage authority",
        )?;
        let request = self
            .pending
            .as_ref()
            .ok_or_else(|| IntentError("Position composition has no pending stage".into()))?;
        ensure(
            request.request_id == request_id,
            "Position stage names another outstanding request",
        )?;
        let Some(binding) = &self.origin_binding else {
            return Ok(None);
        };
        let PositionCompositionOperation::Base { request, .. } = &request.operation else {
            return Ok(None);
        };
        let Some(route) = &request.stage_route else {
            return Ok(None);
        };
        // Envelope routes have their own public locator; segment consumers never see them.
        Ok(binding
            .stage_locator(route)?
            .filter(|locator| locator.envelope_stage().is_none()))
    }
    /// The outstanding EndpointOutput/Activation materialization's exact original locator.
    /// Like `pending_stage_locator`, only this request ID and an original binding issue it.
    pub fn pending_envelope_locator(
        &self,
        request_id: Uuid,
    ) -> Result<Option<PositionEnvelopeLocator>, TransitionError> {
        ensure(
            !self.failed,
            "failed Position composition has no stage authority",
        )?;
        let request = self
            .pending
            .as_ref()
            .ok_or_else(|| IntentError("Position composition has no pending stage".into()))?;
        ensure(
            request.request_id == request_id,
            "Position stage names another outstanding request",
        )?;
        let Some(binding) = &self.origin_binding else {
            return Ok(None);
        };
        let PositionCompositionOperation::Base { request, .. } = &request.operation else {
            return Ok(None);
        };
        let Some(route) = &request.stage_route else {
            return Ok(None);
        };
        Ok(binding
            .stage_locator(route)?
            .and_then(PositionEnvelopeLocator::from_stage))
    }
}

/// Opaque owner-local locator of one original post-expression envelope operation: the
/// EndpointOutput Current crossfade or the Activation blend of one exact original source at
/// one lexical composition use. It names no physical cut and no cross-peer correspondence.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PositionEnvelopeLocator {
    inner: PositionStageLocator,
}
impl PositionEnvelopeLocator {
    pub(super) fn from_stage(inner: PositionStageLocator) -> Option<Self> {
        inner.envelope_stage().map(|_| Self { inner })
    }
    pub fn capture_id(&self) -> Uuid {
        self.inner.capture_id()
    }
    pub fn stage(&self) -> PositionCompletionStage {
        self.inner.envelope_stage().expect("envelope locator")
    }
}
/// Pre-operation operands, in the original operation's from/to order.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PositionEnvelopeOperand {
    /// EndpointOutput `from`: the evaluator's once-captured Current.
    EndpointCurrent,
    /// EndpointOutput `to`: the original raw evaluated target before its envelope.
    EndpointTarget,
    /// Activation `from`: the original resolved lower underlay.
    ActivationUnderlay,
    /// Activation `to`: the output-completed endpoint before activation.
    ActivationEndpoint,
}
impl PositionEnvelopeOperand {
    pub fn stage(self) -> PositionCompletionStage {
        match self {
            Self::EndpointCurrent | Self::EndpointTarget => PositionCompletionStage::EndpointOutput,
            Self::ActivationUnderlay | Self::ActivationEndpoint => {
                PositionCompletionStage::Activation
            }
        }
    }
    fn role(self) -> PositionStageOperand {
        match self {
            Self::EndpointCurrent | Self::ActivationUnderlay => {
                PositionStageOperand::TransitionFrom
            }
            Self::EndpointTarget | Self::ActivationEndpoint => PositionStageOperand::TransitionTo,
        }
    }
}
impl PositionProgramBranch {
    /// Registry identity (pointer, never capture UUID) and stage/operand role authority.
    pub fn validate_envelope_operand(
        &self,
        locator: &PositionEnvelopeLocator,
        operand: PositionEnvelopeOperand,
    ) -> Result<(), TransitionError> {
        ensure(
            operand.stage() == locator.stage(),
            "Position envelope operand does not match the original stage",
        )?;
        self.validate_stage_operand(&locator.inner, operand.role())
    }
    /// Replay this branch's original prefix and stop before the exact original envelope
    /// operation, leaving it and the parent suffix unexecuted. Unreached is Inactive.
    pub fn begin_envelope_operand(
        &self,
        locator: &PositionEnvelopeLocator,
        operand: PositionEnvelopeOperand,
        context: &FamilyCompositionContext<'_>,
        scratch: RetainedFamilyCompositionScratch,
        tracing: bool,
    ) -> Result<PositionStageOperandContinuation, TransitionError> {
        self.validate_envelope_operand(locator, operand)?;
        self.begin_stage_operand(&locator.inner, operand.role(), context, scratch, tracing)
    }
}

/// The two operations of one original partial whole mask: adoption of the composed prefix
/// into the mask's authored representation, then the activation transition to the mask.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PositionMaskStage {
    Adoption,
    Transition,
}
/// Opaque owner-local locator of one original MaskAdoption or MaskTransition: exact captured
/// registry (Arc pointer) + root lexical use + the mask's original source slot + stage.
/// It is a local replay recipe, not shared cross-peer mask authority or a physical cut.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PositionMaskLocator {
    inner: PositionStageLocator,
}
impl PositionMaskLocator {
    pub(super) fn from_stage(inner: PositionStageLocator) -> Option<Self> {
        inner.mask_stage().map(|_| Self { inner })
    }
    pub fn capture_id(&self) -> Uuid {
        self.inner.capture_id()
    }
    pub fn stage(&self) -> PositionMaskStage {
        self.inner.mask_stage().expect("mask locator")
    }
}
/// Pre-operation mask operands. The prefix is the actual composed (and, for Transition,
/// adopted) value; the mask is its original materialized value before partial application.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PositionMaskOperand {
    /// MaskAdoption input: the composed prefix of every lower root source.
    AdoptionInput,
    /// MaskTransition `from`: the prefix adopted into the mask's representation.
    TransitionFrom,
    /// MaskTransition `to`: the original materialized mask value.
    TransitionTo,
}
impl PositionMaskOperand {
    pub fn stage(self) -> PositionMaskStage {
        match self {
            Self::AdoptionInput => PositionMaskStage::Adoption,
            Self::TransitionFrom | Self::TransitionTo => PositionMaskStage::Transition,
        }
    }
    fn role(self) -> PositionStageOperand {
        match self {
            Self::AdoptionInput => PositionStageOperand::AdoptionInput,
            Self::TransitionFrom => PositionStageOperand::TransitionFrom,
            Self::TransitionTo => PositionStageOperand::TransitionTo,
        }
    }
}
impl PositionCompositionContinuation {
    /// The outstanding MaskAdoption/MaskTransition request's exact original locator. Like
    /// `pending_stage_locator`, only this request ID and an original binding issue it.
    pub fn pending_mask_locator(
        &self,
        request_id: Uuid,
    ) -> Result<Option<PositionMaskLocator>, TransitionError> {
        ensure(
            !self.failed,
            "failed Position composition has no stage authority",
        )?;
        let request = self
            .pending
            .as_ref()
            .ok_or_else(|| IntentError("Position composition has no pending stage".into()))?;
        ensure(
            request.request_id == request_id,
            "Position stage names another outstanding request",
        )?;
        let Some(binding) = &self.origin_binding else {
            return Ok(None);
        };
        let (source_index, stage) = match &request.operation {
            PositionCompositionOperation::MaskAdoption { source_index, .. } => {
                (*source_index, PositionMaskStage::Adoption)
            }
            PositionCompositionOperation::MaskTransition { source_index, .. } => {
                (*source_index, PositionMaskStage::Transition)
            }
            PositionCompositionOperation::Base { .. } => return Ok(None),
        };
        let origins = self
            .scratch
            .source_origins
            .get(source_index)
            .ok_or_else(|| IntentError("Position mask source origins are absent".into()))?;
        Ok(binding
            .mask_locator(origins, stage)?
            .and_then(PositionMaskLocator::from_stage))
    }
}
impl PositionProgramBranch {
    /// Registry identity (pointer, never capture UUID) and mask stage/operand role authority.
    pub fn validate_mask_operand(
        &self,
        locator: &PositionMaskLocator,
        operand: PositionMaskOperand,
    ) -> Result<(), TransitionError> {
        ensure(
            operand.stage() == locator.stage(),
            "Position mask operand does not match the original stage",
        )?;
        self.validate_stage_operand(&locator.inner, operand.role())
    }
    /// Replay this branch's original prefix and stop before the exact original mask
    /// operation, leaving it and the parent suffix unexecuted. Unreached is Inactive.
    pub fn begin_mask_operand(
        &self,
        locator: &PositionMaskLocator,
        operand: PositionMaskOperand,
        context: &FamilyCompositionContext<'_>,
        scratch: RetainedFamilyCompositionScratch,
        tracing: bool,
    ) -> Result<PositionStageOperandContinuation, TransitionError> {
        self.validate_mask_operand(locator, operand)?;
        self.begin_stage_operand(&locator.inner, operand.role(), context, scratch, tracing)
    }
}
