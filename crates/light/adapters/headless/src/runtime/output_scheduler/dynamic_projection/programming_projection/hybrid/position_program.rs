//! Owned Position program/evaluation bridge. It retains one captured program without
//! a frame loan, sampler, physical solve, or accepted-continuity publication.
use super::*;
mod discovery;
mod graph;
mod resume;
mod stage;
pub(in crate::runtime) use discovery::HybridPositionDiscoveryEvaluation;
pub(super) use discovery::{
    advance_discovery, begin_discovery_branch, discovery_report, recycle_discovery,
    resume_discovery,
};
pub(in crate::runtime) use graph::HybridPositionGraphEvaluation;
pub(super) use graph::{advance_graph, begin_graph_branch, resume_graph};
pub(in crate::runtime) use resume::HybridPositionResumeEvaluation;
pub(super) use resume::{advance_resume, begin_resume_branch, resume_resume};
// Composer-scoped recycle mirrors recycle_stage; the batch returns workspaces to its pool.
use light_dynamics::{
    CapturedPositionProgram, FamilyCompositionSample, PositionCompositionContinuation,
    PositionCompositionProgress, PositionCompositionRequest, PositionProgramBranch,
};
#[allow(unused_imports)]
pub(super) use resume::recycle_resume;
pub(in crate::runtime) use stage::HybridPositionStageEvaluation;
pub(super) use stage::{
    advance_stage, begin_envelope_branch, begin_mask_branch, begin_stage_branch, recycle_stage,
    resume_stage,
};

#[derive(Clone)]
pub(in crate::runtime) struct HybridCapturedPositionProgram {
    token: CapturedFrameToken,
    target: FixtureId,
    registry: CapturedPositionProgram,
}
impl HybridCapturedPositionProgram {
    pub fn new(
        token: &CapturedFrameToken,
        target: FixtureId,
        base: &AttributeValue,
        samples: &[FamilyCompositionSample],
    ) -> Result<Self, TransitionError> {
        Ok(Self {
            token: token.clone(),
            target,
            registry: CapturedPositionProgram::new(uuid::Uuid::new_v4(), base, samples)?,
        })
    }
    pub fn frame_token(&self) -> &CapturedFrameToken {
        &self.token
    }
    pub fn target(&self) -> FixtureId {
        self.target
    }
    pub fn registry(&self) -> &CapturedPositionProgram {
        &self.registry
    }
}

pub(in crate::runtime) struct HybridPositionEvaluation {
    token: CapturedFrameToken,
    target: FixtureId,
    destination: FixtureId,
    registry: CapturedPositionProgram,
    continuation: PositionCompositionContinuation,
    completed: Option<AttributeValue>,
    observed: bool,
    failed: bool,
}
impl HybridPositionEvaluation {
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
        self.continuation.pending_materialization()
    }
    /// The exact outstanding normal request can issue a lexical operand locator. A registry
    /// capture UUID alone never grants authority to a separate program or another request.
    pub fn pending_stage_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<light_dynamics::PositionStageLocator>, TransitionError> {
        if self.failed || self.observed {
            return Err(IntentError(
                "failed or observed Position evaluation has no stage authority".into(),
            )
            .into());
        }
        let request = self
            .continuation
            .pending_materialization()
            .ok_or_else(|| IntentError("owned Position evaluation has no pending stage".into()))?;
        if request.request_id != request_id || request.capture_id != self.registry.capture_id() {
            return Err(IntentError(
                "Position stage names another outstanding request or capture".into(),
            )
            .into());
        }
        let origin = request.origin().ok_or_else(|| {
            IntentError("owned Position stage has no original program binding".into())
        })?;
        self.registry.source_node_for_origin(origin)?;
        self.continuation.pending_stage_locator(request_id)
    }
    /// Only the actual outstanding request grants an original owner-local envelope locator.
    pub fn pending_envelope_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<light_dynamics::PositionEnvelopeLocator>, TransitionError> {
        if self.failed || self.observed {
            return Err(IntentError(
                "failed or observed Position evaluation has no envelope authority".into(),
            )
            .into());
        }
        let request = self.continuation.pending_materialization().ok_or_else(|| {
            IntentError("owned Position evaluation has no pending envelope".into())
        })?;
        if request.request_id != request_id || request.capture_id != self.registry.capture_id() {
            return Err(IntentError(
                "Position envelope names another outstanding request or capture".into(),
            )
            .into());
        }
        let origin = request.origin().ok_or_else(|| {
            IntentError("owned Position envelope has no original program binding".into())
        })?;
        self.registry.source_node_for_origin(origin)?;
        self.continuation.pending_envelope_locator(request_id)
    }
    /// Only the actual outstanding request grants an original owner-local mask locator.
    pub fn pending_mask_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<light_dynamics::PositionMaskLocator>, TransitionError> {
        if self.failed || self.observed {
            return Err(IntentError(
                "failed or observed Position evaluation has no mask authority".into(),
            )
            .into());
        }
        let request = self
            .continuation
            .pending_materialization()
            .ok_or_else(|| IntentError("owned Position evaluation has no pending mask".into()))?;
        if request.request_id != request_id || request.capture_id != self.registry.capture_id() {
            return Err(IntentError(
                "Position mask names another outstanding request or capture".into(),
            )
            .into());
        }
        let origin = request.origin().ok_or_else(|| {
            IntentError("owned Position mask has no original program binding".into())
        })?;
        self.registry.source_node_for_origin(origin)?;
        self.continuation.pending_mask_locator(request_id)
    }
    /// Only the actual outstanding request grants an original owner-local graph-operation locator.
    pub fn pending_graph_operation_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<light_dynamics::PositionGraphOperationLocator>, TransitionError> {
        if self.failed || self.observed {
            return Err(IntentError(
                "failed or observed Position evaluation has no graph-operation authority".into(),
            )
            .into());
        }
        let request = self.continuation.pending_materialization().ok_or_else(|| {
            IntentError("owned Position evaluation has no pending graph operation".into())
        })?;
        if request.request_id != request_id || request.capture_id != self.registry.capture_id() {
            return Err(IntentError(
                "Position graph operation names another outstanding request or capture".into(),
            )
            .into());
        }
        let origin = request.origin().ok_or_else(|| {
            IntentError("owned Position graph operation has no original program binding".into())
        })?;
        self.registry.source_node_for_origin(origin)?;
        self.continuation
            .pending_graph_operation_locator(request_id)
    }
    /// Only the actual outstanding request grants an original owner-local Resume operand
    /// locator. Its enclosing goal is this Full evaluation; it never names another owner.
    pub fn pending_resume_operand_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<light_dynamics::PositionResumeOperandLocator>, TransitionError> {
        if self.failed || self.observed {
            return Err(IntentError(
                "failed or observed Position evaluation has no Resume operand authority".into(),
            )
            .into());
        }
        let request = self.continuation.pending_materialization().ok_or_else(|| {
            IntentError("owned Position evaluation has no pending Resume operation".into())
        })?;
        if request.request_id != request_id || request.capture_id != self.registry.capture_id() {
            return Err(IntentError(
                "Position Resume operation names another outstanding request or capture".into(),
            )
            .into());
        }
        let origin = request.origin().ok_or_else(|| {
            IntentError("owned Position Resume operation has no original program binding".into())
        })?;
        self.registry.source_node_for_origin(origin)?;
        self.continuation.pending_resume_operand_locator(request_id)
    }
    /// Opt into same-driver reports of actual reached Required/Size sites before the first
    /// advance. Reports are owner-local evidence, never cross-owner or output authority.
    pub fn enable_graph_discovery(&mut self, route_limit: usize) -> Result<(), TransitionError> {
        self.ensure_graph_authority()?;
        self.continuation.enable_graph_discovery(route_limit)
    }
    pub fn graph_discovery_report(
        &self,
    ) -> Result<light_dynamics::PositionGraphDiscoveryReport, TransitionError> {
        self.ensure_graph_authority()?;
        self.continuation.graph_discovery_report()
    }
    /// Arm one exact original reached site of this same captured goal before its first advance.
    pub fn enable_graph_materialization(
        &mut self,
        site: &light_dynamics::PositionGraphOperationLocator,
    ) -> Result<(), TransitionError> {
        self.ensure_graph_authority()?;
        if site.capture_id() != self.registry.capture_id() {
            return Err(IntentError(
                "Position graph materialization names another registry capture".into(),
            )
            .into());
        }
        self.continuation.enable_graph_materialization(site)
    }
    pub fn graph_materialization_status(
        &self,
    ) -> Result<light_dynamics::PositionGraphMaterializationStatus, TransitionError> {
        self.ensure_graph_authority()?;
        self.continuation.graph_materialization_status()
    }
    fn ensure_graph_authority(&self) -> Result<(), TransitionError> {
        if self.failed || self.observed {
            return Err(IntentError(
                "failed or observed Position evaluation has no reached-graph authority".into(),
            )
            .into());
        }
        Ok(())
    }
    pub fn completed_value(&self) -> Option<&AttributeValue> {
        if self.failed {
            None
        } else {
            self.completed.as_ref()
        }
    }
    pub(super) fn into_scratch(self) -> RetainedFamilyCompositionScratch {
        self.continuation.into_scratch()
    }
}

fn guard<S>(
    composer: &CapturedHybridProgramComposer<'_, '_, S>,
    token: &CapturedFrameToken,
    target: FixtureId,
) -> Result<(), TransitionError> {
    if composer.frame.token != token {
        return Err(IntentError(
            "owned Position program belongs to another captured frame or lane".into(),
        )
        .into());
    }
    if !token.matches_geometry(composer.frame.geometry)
        || !token.matches_static_frame(composer.baseline)
        || !token.matches_static_frame(composer.frame.scalar)
    {
        return Err(IntentError(
            "owned Position geometry or baseline does not belong to its captured frame lane".into(),
        )
        .into());
    }
    if composer.group.owner != ProgrammingOwner::Position || composer.group.target != target {
        return Err(IntentError(
            "owned Position program belongs to another logical target or owner".into(),
        )
        .into());
    }
    Ok(())
}
fn context<'a, S: DynamicTickSource>(
    composer: &'a CapturedHybridProgramComposer<'_, '_, S>,
    adoption: &'a light_dynamics::FamilyAdoptionResolver<'_>,
) -> FamilyCompositionContext<'a> {
    FamilyCompositionContext {
        edit: FamilyEditContext {
            color_model: Some(&VirtualColorAuthoringV1),
            ..Default::default()
        },
        resolve_adoption: Some(adoption),
        endpoint_output: Some(FamilyEndpointOutputContext {
            control: composer.control,
            target: composer.group.target,
            current: composer.typed,
            native_models: Some(composer.frame.native_models),
        }),
        ..Default::default()
    }
}

pub(super) fn begin<S: DynamicTickSource>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    program: &HybridCapturedPositionProgram,
    destination: FixtureId,
    adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
) -> Result<HybridPositionEvaluation, TransitionError> {
    begin_branch(
        composer,
        program,
        &program.registry.branch(),
        destination,
        adoption,
    )
}

pub(super) fn begin_branch<S: DynamicTickSource>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    program: &HybridCapturedPositionProgram,
    branch: &PositionProgramBranch,
    destination: FixtureId,
    adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
) -> Result<HybridPositionEvaluation, TransitionError> {
    program.registry.validate_branch(branch)?;
    guard(composer, &program.token, program.target)?;
    composer.typed.check()?;
    let scratch = std::mem::take(composer.scratch);
    let continuation = branch.begin_composition(&context(composer, adoption), scratch, true)?;
    composer.typed.check()?;
    Ok(HybridPositionEvaluation {
        token: program.token.clone(),
        target: program.target,
        destination,
        registry: program.registry.clone(),
        continuation,
        completed: None,
        observed: false,
        failed: false,
    })
}

pub(super) fn advance<S: DynamicTickSource>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    evaluation: &mut HybridPositionEvaluation,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
) -> Result<PositionCompositionProgress, TransitionError> {
    guard(composer, &evaluation.token, evaluation.target)?;
    if evaluation.failed || evaluation.observed {
        return Err(IntentError(
            "failed or observed owned Position evaluation must be discarded".into(),
        )
        .into());
    }
    // An unwind or initial freshness failure leaves the evaluation terminal. A failed typed
    // source or origin proof cannot expose its partial/completed result or stage authority.
    evaluation.failed = true;
    composer.typed.check()?;
    let result = evaluation.continuation.advance(
        evaluation.registry.capture_id(),
        &context(composer, adoption),
        frame,
    )?;
    composer.typed.check()?;
    match &result {
        PositionCompositionProgress::NeedsMaterialization(request) => {
            if request.capture_id != evaluation.registry.capture_id() {
                return Err(IntentError(
                    "owned Position request belongs to another registry capture".into(),
                )
                .into());
            }
            let origin = request.origin().ok_or_else(|| {
                IntentError("owned Position request has no original program binding".into())
            })?;
            evaluation.registry.source_node_for_origin(origin)?;
        }
        PositionCompositionProgress::Complete(value) => evaluation.completed = Some(value.clone()),
    }
    evaluation.failed = false;
    Ok(result)
}

pub(super) fn resume<S: DynamicTickSource>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    evaluation: &mut HybridPositionEvaluation,
    request_id: uuid::Uuid,
    value: AttributeValue,
    transfer: Option<ProgrammingTransitionTrace>,
) -> Result<(), TransitionError> {
    guard(composer, &evaluation.token, evaluation.target)?;
    if evaluation.failed || evaluation.observed {
        return Err(IntentError(
            "failed or observed owned Position evaluation must be discarded".into(),
        )
        .into());
    }
    let request = evaluation
        .continuation
        .pending_materialization()
        .ok_or_else(|| IntentError("owned Position evaluation has no pending response".into()))?;
    if request.capture_id != evaluation.registry.capture_id() {
        return Err(IntentError(
            "owned Position response belongs to another registry capture".into(),
        )
        .into());
    }
    let origin = request.origin().ok_or_else(|| {
        IntentError("owned Position response has no original program binding".into())
    })?;
    evaluation.registry.source_node_for_origin(origin)?;
    evaluation.failed = true;
    composer.typed.check()?;
    let result = evaluation.continuation.resume_materialization(
        evaluation.registry.capture_id(),
        request_id,
        value,
        transfer,
    );
    composer.typed.check()?;
    // Invalid ordinary responses retain the driver's pending cut and completed children.
    evaluation.failed = false;
    result
}

pub(super) fn observe<S: DynamicTickSource, T>(
    composer: &mut CapturedHybridProgramComposer<'_, '_, S>,
    evaluation: &mut HybridPositionEvaluation,
    observe: &mut dyn FnMut(
        HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, T), TransitionError>,
) -> Result<OwnedHybridProjection<T>, TransitionError> {
    guard(composer, &evaluation.token, evaluation.target)?;
    if evaluation.failed || evaluation.observed {
        return Err(IntentError(
            "owned Position evaluation cannot be observed again or after failure".into(),
        )
        .into());
    }
    let value = evaluation
        .completed
        .as_ref()
        .ok_or_else(|| IntentError("owned Position evaluation has not completed".into()))?;
    if evaluation.continuation.pending_materialization().is_some() {
        return Err(
            IntentError("pending owned Position evaluation cannot be observed".into()).into(),
        );
    }
    evaluation.failed = true;
    composer.typed.check()?;
    let observation = CapturedFamilyObservation {
        target: evaluation.target,
        owner: ProgrammingOwner::Position,
        value,
        trace: evaluation.continuation.family_trace(),
        sources: composer.typed,
    };
    let project = |fields: &ProgrammingFieldScope,
                   projection: &mut DynamicFamilySourceProjection| {
        observation.project_fields(fields, projection)
    };
    let controls = |fields: &ProgrammingFieldScope| {
        fields.validate(ProgrammingOwner::Position).ok()?;
        observation
            .trace
            .root()
            .and_then(|root| observation.trace.control_sources_for_fields(root, fields))
    };
    let output = observe(HybridFamilyObservation {
        target: evaluation.target,
        owner: ProgrammingOwner::Position,
        value: observation.value(),
        static_baseline: composer.baseline,
        frame: composer.frame,
        project: &project,
        controls: &controls,
    });
    composer.typed.check()?;
    let (metadata, sidecar) = output?;
    let projection = OwnedHybridProjection {
        target: evaluation.target,
        owner: ProgrammingOwner::Position,
        value: value.clone(),
        metadata,
        sidecar,
    };
    evaluation.observed = true;
    evaluation.failed = false;
    Ok(projection)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod stage_tests;

#[cfg(test)]
mod discovery_tests;

#[cfg(test)]
mod graph_tests;
