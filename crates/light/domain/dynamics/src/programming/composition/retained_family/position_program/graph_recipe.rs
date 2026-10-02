//! Original Required/Size operand replay is speculative and has no ordinary observation or
//! acceptance API. Its retained prefix can issue authentic nested materialization requests.
use super::*;

pub enum PositionGraphOperationOperandProgress {
    NeedsMaterialization(PositionCompositionRequest),
    OperandReady(AttributeValue),
    Inactive,
}
pub struct PositionGraphOperationOperandContinuation {
    inner: PositionCompositionContinuation,
}
impl PositionCompositionContinuation {
    /// Only this outstanding request, bound to its original captured registry and lexical use,
    /// can issue a graph locator. Legacy unbound or ambiguous synthetic requests remain None.
    pub fn pending_graph_operation_locator(
        &self,
        request_id: Uuid,
    ) -> Result<Option<PositionGraphOperationLocator>, TransitionError> {
        ensure(
            !self.failed,
            "failed Position composition has no graph operation authority",
        )?;
        let request = self.pending.as_ref().ok_or_else(|| {
            IntentError("Position composition has no pending graph operation".into())
        })?;
        ensure(
            request.request_id == request_id,
            "Position graph locator names another outstanding request",
        )?;
        let Some(binding) = &self.origin_binding else {
            return Ok(None);
        };
        let PositionCompositionOperation::Base { request, .. } = &request.operation else {
            return Ok(None);
        };
        let Some(route) = &request.graph_route else {
            return Ok(None);
        };
        binding.graph_locator(route)
    }
}
impl PositionProgramBranch {
    /// Replay the actual original compositor prefix under this changed branch. Stop before
    /// the named graph operation and every parent suffix; removed/covered operations are Inactive.
    /// Required outgoing/Size baseline do not evaluate their unselected incoming/value sibling.
    pub fn begin_graph_operation_operand(
        &self,
        locator: &PositionGraphOperationLocator,
        operand: PositionGraphOperationOperand,
        context: &FamilyCompositionContext<'_>,
        scratch: RetainedFamilyCompositionScratch,
        tracing: bool,
    ) -> Result<PositionGraphOperationOperandContinuation, TransitionError> {
        self.validate_graph_operation_operand(locator, operand)?;
        let mut inner = self.begin_composition(context, scratch, tracing)?;
        inner.operand_only = true;
        inner.graph_replay = Some((locator.clone(), operand));
        inner.resume_goal = ResumeGoal::Graph(locator.clone(), operand);
        Ok(PositionGraphOperationOperandContinuation { inner })
    }
}
impl PositionGraphOperationOperandContinuation {
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
    pub fn advance(
        &mut self,
        capture_id: Uuid,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<PositionGraphOperationOperandProgress, TransitionError> {
        ensure(
            capture_id == self.inner.capture_id,
            "Position graph operand belongs to another capture",
        )?;
        ensure(
            !self.inner.failed,
            "failed Position graph operand must be discarded",
        )?;
        if let Some(value) = &self.inner.stopped_operand {
            return Ok(PositionGraphOperationOperandProgress::OperandReady(
                value.clone(),
            ));
        }
        self.inner.failed = true;
        let result = self.inner.advance_impl(context, frame, None);
        if result.is_ok() {
            self.inner.failed = false;
        }
        match result? {
            PositionCompositionProgress::NeedsMaterialization(request) => Ok(
                PositionGraphOperationOperandProgress::NeedsMaterialization(request),
            ),
            PositionCompositionProgress::Complete(_) => Ok(match &self.inner.stopped_operand {
                Some(value) => PositionGraphOperationOperandProgress::OperandReady(value.clone()),
                None => PositionGraphOperationOperandProgress::Inactive,
            }),
        }
    }
    /// Nested prefix requests keep the same original registry and exact lexical authority.
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
