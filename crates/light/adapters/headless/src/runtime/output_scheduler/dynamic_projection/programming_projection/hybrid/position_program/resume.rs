//! Speculative original multi-source Resume operand bridge (TL-636). It owns a captured
//! registry and driver, never an observation, physical acceptance, producer clock, or
//! borrowed frame loan. A Resume locator is owner-local: only the exact outstanding request of
//! this registry's own evaluation issues it, and it never correlates another owner.
use super::*;
use light_dynamics::{
    PositionResumeEndpoint, PositionResumeOperandContinuation, PositionResumeOperandLocator,
    PositionResumeOperandProgress,
};

pub(in crate::runtime) struct HybridPositionResumeEvaluation {
    token: CapturedFrameToken,
    target: FixtureId,
    destination: FixtureId,
    registry: CapturedPositionProgram,
    continuation: PositionResumeOperandContinuation,
    pending: Option<PositionCompositionRequest>,
    failed: bool,
}
impl HybridPositionResumeEvaluation {
    pub fn frame_token(&self) -> &CapturedFrameToken {
        &self.token
    }
    pub fn target(&self) -> FixtureId {
        self.target
    }
    pub fn destination(&self) -> FixtureId {
        self.destination
    }
    pub fn registry(&self) -> &CapturedPositionProgram {
        &self.registry
    }
    pub fn pending_request(&self) -> Option<&PositionCompositionRequest> {
        if self.failed {
            None
        } else {
            self.pending.as_ref()
        }
    }
    /// Only the exact outstanding original request can issue nested operand authority.
    pub fn pending_graph_operation_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<light_dynamics::PositionGraphOperationLocator>, TransitionError> {
        self.validate_pending_locator(request_id)?;
        self.continuation
            .pending_graph_operation_locator(request_id)
    }
    pub fn pending_stage_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<light_dynamics::PositionStageLocator>, TransitionError> {
        self.validate_pending_locator(request_id)?;
        self.continuation.pending_stage_locator(request_id)
    }
    pub fn pending_envelope_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<light_dynamics::PositionEnvelopeLocator>, TransitionError> {
        self.validate_pending_locator(request_id)?;
        self.continuation.pending_envelope_locator(request_id)
    }
    pub fn pending_mask_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<light_dynamics::PositionMaskLocator>, TransitionError> {
        self.validate_pending_locator(request_id)?;
        self.continuation.pending_mask_locator(request_id)
    }
    pub fn pending_resume_operand_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<PositionResumeOperandLocator>, TransitionError> {
        self.validate_pending_locator(request_id)?;
        self.continuation.pending_resume_operand_locator(request_id)
    }
    /// Same-driver reached Required/Size reports; enable before the first advance.
    pub fn enable_graph_discovery(&mut self, route_limit: usize) -> Result<(), TransitionError> {
        self.ensure_reached_authority()?;
        self.continuation.enable_graph_discovery(route_limit)
    }
    pub fn graph_discovery_report(
        &self,
    ) -> Result<light_dynamics::PositionGraphDiscoveryReport, TransitionError> {
        self.ensure_reached_authority()?;
        self.continuation.graph_discovery_report()
    }
    /// Arm one exact original reached site of this same operand goal before its first advance.
    pub fn enable_graph_materialization(
        &mut self,
        site: &light_dynamics::PositionGraphOperationLocator,
    ) -> Result<(), TransitionError> {
        self.ensure_reached_authority()?;
        if site.capture_id() != self.registry.capture_id() {
            return Err(IntentError(
                "Position operand materialization names another registry capture".into(),
            )
            .into());
        }
        self.continuation.enable_graph_materialization(site)
    }
    pub fn graph_materialization_status(
        &self,
    ) -> Result<light_dynamics::PositionGraphMaterializationStatus, TransitionError> {
        self.ensure_reached_authority()?;
        self.continuation.graph_materialization_status()
    }
    fn ensure_reached_authority(&self) -> Result<(), TransitionError> {
        if self.failed {
            return Err(IntentError(
                "failed Position operand evaluation has no reached-graph authority".into(),
            )
            .into());
        }
        Ok(())
    }
    fn validate_pending_locator(&self, request_id: uuid::Uuid) -> Result<(), TransitionError> {
        if self.failed {
            return Err(IntentError(
                "failed Position Resume operand has no nested locator authority".into(),
            )
            .into());
        }
        let request = self.pending.as_ref().ok_or_else(|| {
            IntentError("owned Position Resume operand has no pending request".into())
        })?;
        if request.request_id != request_id || request.capture_id != self.registry.capture_id() {
            return Err(IntentError(
                "Position Resume locator names another outstanding request or capture".into(),
            )
            .into());
        }
        self.validate_request(request)
    }
    pub(in super::super) fn into_scratch(self) -> RetainedFamilyCompositionScratch {
        self.continuation.into_scratch()
    }
    fn validate_request(
        &self,
        request: &PositionCompositionRequest,
    ) -> Result<(), TransitionError> {
        if request.capture_id != self.registry.capture_id() {
            return Err(IntentError(
                "owned Position Resume request belongs to another registry capture".into(),
            )
            .into());
        }
        let origin = request.origin().ok_or_else(|| {
            IntentError("owned Position Resume request has no original program binding".into())
        })?;
        self.registry.source_node_for_origin(origin)?;
        Ok(())
    }
}

pub(in super::super) fn begin_resume_branch<S: DynamicTickSource>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    program: &HybridCapturedPositionProgram,
    branch: &PositionProgramBranch,
    locator: &PositionResumeOperandLocator,
    endpoint: PositionResumeEndpoint,
    destination: FixtureId,
    adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
) -> Result<HybridPositionResumeEvaluation, TransitionError> {
    // The program's own registry must own the branch; an equal capture UUID on another
    // registry is not authority. The domain then checks the locator against the branch.
    program.registry.validate_branch(branch)?;
    if locator.capture_id() != program.registry.capture_id() {
        return Err(IntentError(
            "Position Resume operand locator belongs to another registry capture".into(),
        )
        .into());
    }
    branch.validate_resume_operand(locator, endpoint)?;
    guard(composer, &program.token, program.target)?;
    composer.typed.check()?;
    let scratch = std::mem::take(composer.scratch);
    let continuation = branch.begin_resume_operand(
        locator,
        endpoint,
        &context(composer, adoption),
        scratch,
        true,
    )?;
    if let Err(error) = composer.typed.check() {
        *composer.scratch = continuation.into_scratch();
        return Err(error);
    }
    Ok(HybridPositionResumeEvaluation {
        token: program.token.clone(),
        target: program.target,
        destination,
        registry: program.registry.clone(),
        continuation,
        pending: None,
        failed: false,
    })
}

pub(in super::super) fn advance_resume<S: DynamicTickSource>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    evaluation: &mut HybridPositionResumeEvaluation,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
) -> Result<PositionResumeOperandProgress, TransitionError> {
    guard(composer, &evaluation.token, evaluation.target)?;
    ensure_resume_active(evaluation)?;
    // Source failure or unwind is terminal; wrong frame/target loans above leave the owned
    // original cut intact so the actual coordinator can retry it through a valid loan.
    evaluation.failed = true;
    composer.typed.check()?;
    let progress = evaluation.continuation.advance(
        evaluation.registry.capture_id(),
        &context(composer, adoption),
        frame,
    )?;
    composer.typed.check()?;
    match &progress {
        PositionResumeOperandProgress::NeedsMaterialization(request) => {
            evaluation.validate_request(request)?;
            evaluation.pending = Some(request.clone());
        }
        // Inactive is the original enclosing goal's answer; it never falls back to Full.
        PositionResumeOperandProgress::OperandReady(_)
        | PositionResumeOperandProgress::Inactive => {
            evaluation.pending = None;
        }
    }
    evaluation.failed = false;
    Ok(progress)
}

pub(in super::super) fn resume_resume<S: DynamicTickSource>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    evaluation: &mut HybridPositionResumeEvaluation,
    request_id: uuid::Uuid,
    value: AttributeValue,
    transfer: Option<ProgrammingTransitionTrace>,
) -> Result<(), TransitionError> {
    guard(composer, &evaluation.token, evaluation.target)?;
    ensure_resume_active(evaluation)?;
    let request = evaluation.pending.as_ref().ok_or_else(|| {
        IntentError("owned Position Resume operand has no pending response".into())
    })?;
    if request.request_id != request_id {
        return Err(IntentError(
            "owned Position Resume response names another outstanding request".into(),
        )
        .into());
    }
    evaluation.validate_request(request)?;
    evaluation.failed = true;
    composer.typed.check()?;
    let result = evaluation.continuation.resume_materialization(
        evaluation.registry.capture_id(),
        request_id,
        value,
        transfer,
    );
    composer.typed.check()?;
    if result.is_ok() {
        evaluation.pending = None;
    }
    // Ordinary invalid responses retain the exact original UUID/driver prefix and remain
    // retryable. An unwind or typed-source check skips this reset and is terminal.
    evaluation.failed = false;
    result
}

pub(in super::super) fn recycle_resume<S>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    evaluation: HybridPositionResumeEvaluation,
) -> Result<(), TransitionError> {
    guard(composer, &evaluation.token, evaluation.target)?;
    *composer.scratch = evaluation.into_scratch();
    Ok(())
}

fn ensure_resume_active(
    evaluation: &HybridPositionResumeEvaluation,
) -> Result<(), TransitionError> {
    if evaluation.failed {
        Err(IntentError("failed owned Position Resume operand must be discarded".into()).into())
    } else {
        Ok(())
    }
}
