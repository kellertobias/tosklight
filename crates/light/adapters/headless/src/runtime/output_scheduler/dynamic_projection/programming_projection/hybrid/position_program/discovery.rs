//! Captured registry-local candidate discovery. This speculative bridge never observes,
//! fits, accepts, or exposes a parent family value. Physical destination remains metadata.
use super::*;
use light_dynamics::{
    PositionStageDiscoveryContinuation, PositionStageDiscoveryProgress,
    PositionStageDiscoveryReport,
};

pub(in crate::runtime) struct HybridPositionDiscoveryEvaluation {
    token: CapturedFrameToken,
    target: FixtureId,
    destination: FixtureId,
    registry: CapturedPositionProgram,
    continuation: PositionStageDiscoveryContinuation,
    pending: Option<PositionCompositionRequest>,
    failed: bool,
}
impl HybridPositionDiscoveryEvaluation {
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
    pub(in super::super) fn into_scratch(self) -> RetainedFamilyCompositionScratch {
        self.continuation.into_scratch()
    }
    fn validate_request(
        &self,
        request: &PositionCompositionRequest,
    ) -> Result<(), TransitionError> {
        if request.capture_id != self.registry.capture_id() {
            return Err(IntentError(
                "owned Position discovery request belongs to another registry capture".into(),
            )
            .into());
        }
        let origin = request.origin().ok_or_else(|| {
            IntentError("owned Position discovery request has no original program binding".into())
        })?;
        self.registry.source_node_for_origin(origin)?;
        Ok(())
    }
    fn validate_report(
        &self,
        report: &PositionStageDiscoveryReport,
    ) -> Result<(), TransitionError> {
        let branch = self.registry.branch();
        for locator in &report.candidates {
            let operand = match locator.kind() {
                light_dynamics::PositionStageKind::ComponentAdoption { .. }
                | light_dynamics::PositionStageKind::WholeAdoption => {
                    light_dynamics::PositionStageOperand::AdoptionInput
                }
                light_dynamics::PositionStageKind::WholeSegmentTransition => {
                    light_dynamics::PositionStageOperand::TransitionFrom
                }
            };
            branch.validate_stage_operand(locator, operand)?;
        }
        Ok(())
    }
}

pub(in super::super) fn begin_discovery_branch<S: DynamicTickSource>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    program: &HybridCapturedPositionProgram,
    branch: &PositionProgramBranch,
    destination: FixtureId,
    adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    route_limit: usize,
) -> Result<HybridPositionDiscoveryEvaluation, TransitionError> {
    program.registry.validate_branch(branch)?;
    guard(composer, &program.token, program.target)?;
    if route_limit == 0 {
        return Err(
            IntentError("owned Position discovery requires a positive route limit".into()).into(),
        );
    }
    composer.typed.check()?;
    let scratch = std::mem::take(composer.scratch);
    let continuation =
        branch.begin_stage_discovery(&context(composer, adoption), scratch, true, route_limit)?;
    if let Err(error) = composer.typed.check() {
        *composer.scratch = continuation.into_scratch();
        return Err(error);
    }
    Ok(HybridPositionDiscoveryEvaluation {
        token: program.token.clone(),
        target: program.target,
        destination,
        registry: program.registry.clone(),
        continuation,
        pending: None,
        failed: false,
    })
}

pub(in super::super) fn advance_discovery<S: DynamicTickSource>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    evaluation: &mut HybridPositionDiscoveryEvaluation,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
) -> Result<PositionStageDiscoveryProgress, TransitionError> {
    guard(composer, &evaluation.token, evaluation.target)?;
    ensure_discovery_active(evaluation)?;
    evaluation.failed = true;
    composer.typed.check()?;
    let progress = evaluation.continuation.advance(
        evaluation.registry.capture_id(),
        &context(composer, adoption),
        frame,
    )?;
    composer.typed.check()?;
    match &progress {
        PositionStageDiscoveryProgress::NeedsMaterialization(request) => {
            evaluation.validate_request(request)?;
            evaluation.pending = Some(request.clone());
        }
        PositionStageDiscoveryProgress::Complete(report) => {
            evaluation.validate_report(report)?;
            evaluation.pending = None;
        }
    }
    evaluation.failed = false;
    Ok(progress)
}

pub(in super::super) fn resume_discovery<S: DynamicTickSource>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    evaluation: &mut HybridPositionDiscoveryEvaluation,
    request_id: uuid::Uuid,
    value: AttributeValue,
    transfer: Option<ProgrammingTransitionTrace>,
) -> Result<(), TransitionError> {
    guard(composer, &evaluation.token, evaluation.target)?;
    ensure_discovery_active(evaluation)?;
    let request = evaluation
        .pending
        .as_ref()
        .ok_or_else(|| IntentError("owned Position discovery has no pending response".into()))?;
    if request.request_id != request_id {
        return Err(IntentError(
            "owned Position discovery response names another outstanding request".into(),
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
    // Invalid ordinary responses preserve the exact pending UUID. Source failure/unwind is
    // terminal and skips resetting failed, so later freshness cannot resurrect authority.
    evaluation.failed = false;
    result
}

pub(in super::super) fn discovery_report<S: DynamicTickSource>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    evaluation: &mut HybridPositionDiscoveryEvaluation,
) -> Result<PositionStageDiscoveryReport, TransitionError> {
    guard(composer, &evaluation.token, evaluation.target)?;
    ensure_discovery_active(evaluation)?;
    evaluation.failed = true;
    composer.typed.check()?;
    let report = evaluation.continuation.report()?;
    composer.typed.check()?;
    evaluation.validate_report(&report)?;
    evaluation.failed = false;
    Ok(report)
}

pub(in super::super) fn recycle_discovery<S>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    evaluation: HybridPositionDiscoveryEvaluation,
) -> Result<(), TransitionError> {
    guard(composer, &evaluation.token, evaluation.target)?;
    *composer.scratch = evaluation.into_scratch();
    Ok(())
}
fn ensure_discovery_active(
    evaluation: &HybridPositionDiscoveryEvaluation,
) -> Result<(), TransitionError> {
    if evaluation.failed {
        Err(IntentError("failed owned Position discovery must be discarded".into()).into())
    } else {
        Ok(())
    }
}
