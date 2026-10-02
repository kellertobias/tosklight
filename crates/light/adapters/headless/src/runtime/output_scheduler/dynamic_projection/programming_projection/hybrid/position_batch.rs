//! Borrowed complete Position composition service. Peer requests coexist while their immutable
//! source/context loans remain pinned. Adapter ownership and fitting belong to the observer.
use super::*;
// Named here because hybrid.rs re-exports are frozen at its size limit in this chunk.
use super::position_program::HybridPositionResumeEvaluation;
use light_dynamics::{
    PositionCompositionProgress, PositionProgramBranch, PositionStageDiscoveryProgress,
    PositionStageDiscoveryReport, PositionStageLocator, PositionStageOperand,
    PositionStageOperandProgress,
};

pub(in crate::runtime) struct HybridPositionBatchResult<T> {
    pub handled: Vec<FixtureId>,
    pub projections: Vec<OwnedHybridProjection<T>>,
    pub requirements: Vec<HybridFamilyRequirement>,
}

pub(in crate::runtime) trait HybridPositionBatchComposer<T> {
    fn eligible_targets(&self) -> Vec<FixtureId>;
    fn unchanged_controls(
        &self,
        program: &HybridCapturedPositionProgram,
    ) -> Result<bool, TransitionError>;
    /// Classify this original program's captured gates only. This introduces no cross-owner
    /// operation identity: suppression removes participation, and a full master is identity.
    fn noninterpolating_controls(
        &self,
        program: &HybridCapturedPositionProgram,
    ) -> Result<bool, TransitionError>;
    /// All original sources are removed by this captured batch's actual endpoint gates.
    /// An empty/static program, exempt mask, or zero crossfade is not this proof.
    fn original_sources_suppressed(&self, target: FixtureId) -> Result<bool, TransitionError>;
    fn begin_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionEvaluation, TransitionError>;
    fn advance(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<PositionCompositionProgress, TransitionError>;
    fn resume(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError>;
    /// Replay an owner-local original stage without observing or accepting a family result.
    fn begin_stage_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &PositionStageLocator,
        operand: PositionStageOperand,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionStageEvaluation, TransitionError>;
    /// Replay an exact original envelope; its operands carry no observation authority.
    fn begin_envelope_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionEnvelopeLocator,
        operand: light_dynamics::PositionEnvelopeOperand,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionStageEvaluation, TransitionError>;
    /// Replay an exact original mask; its operands carry no observation authority.
    fn begin_mask_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionMaskLocator,
        operand: light_dynamics::PositionMaskOperand,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionStageEvaluation, TransitionError>;
    fn advance_stage(
        &mut self,
        evaluation: &mut HybridPositionStageEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<PositionStageOperandProgress, TransitionError>;
    fn resume_stage(
        &mut self,
        evaluation: &mut HybridPositionStageEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError>;
    fn recycle_stage(
        &mut self,
        evaluation: HybridPositionStageEvaluation,
    ) -> Result<(), TransitionError>;
    /// Replay the exact original graph operation through its captured lexical prefix.
    fn begin_graph_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionGraphOperationLocator,
        operand: light_dynamics::PositionGraphOperationOperand,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionGraphEvaluation, TransitionError>;
    fn advance_graph(
        &mut self,
        evaluation: &mut HybridPositionGraphEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<light_dynamics::PositionGraphOperationOperandProgress, TransitionError>;
    fn resume_graph(
        &mut self,
        evaluation: &mut HybridPositionGraphEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError>;
    fn recycle_graph(
        &mut self,
        evaluation: HybridPositionGraphEvaluation,
    ) -> Result<(), TransitionError>;
    /// Replay one endpoint of an exact original Resume through its issued enclosing goal.
    /// The locator is owner-local authority from this program's own outstanding request.
    fn begin_resume_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionResumeOperandLocator,
        endpoint: light_dynamics::PositionResumeEndpoint,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionResumeEvaluation, TransitionError>;
    fn advance_resume(
        &mut self,
        evaluation: &mut HybridPositionResumeEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<light_dynamics::PositionResumeOperandProgress, TransitionError>;
    fn resume_resume(
        &mut self,
        evaluation: &mut HybridPositionResumeEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError>;
    fn recycle_resume(
        &mut self,
        evaluation: HybridPositionResumeEvaluation,
    ) -> Result<(), TransitionError>;
    /// Discover exact owner-local segment candidates; no result observation or physical acceptance.
    fn begin_discovery_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
        route_limit: usize,
    ) -> Result<HybridPositionDiscoveryEvaluation, TransitionError>;
    fn advance_discovery(
        &mut self,
        evaluation: &mut HybridPositionDiscoveryEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<PositionStageDiscoveryProgress, TransitionError>;
    fn resume_discovery(
        &mut self,
        evaluation: &mut HybridPositionDiscoveryEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError>;
    fn discovery_report(
        &mut self,
        evaluation: &mut HybridPositionDiscoveryEvaluation,
    ) -> Result<PositionStageDiscoveryReport, TransitionError>;
    fn recycle_discovery(
        &mut self,
        evaluation: HybridPositionDiscoveryEvaluation,
    ) -> Result<(), TransitionError>;
    fn observe(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        observe: &mut dyn FnMut(
            HybridFamilyObservation<'_>,
        ) -> Result<(FamilyProjectionMetadata, T), TransitionError>,
    ) -> Result<OwnedHybridProjection<T>, TransitionError>;
    fn recycle(&mut self, evaluation: HybridPositionEvaluation) -> Result<(), TransitionError>;
}

pub(super) struct CapturedHybridPositionBatchComposer<'a, 'sources, S> {
    typed: &'a CapturedProgrammingSources<'sources, S>,
    groups: Vec<&'a light_dynamics::DynamicFamilySampleGroup>,
    frame: HybridFrameContext<'a>,
    baseline: &'a PreparedStaticFamilyFrame,
    control:
        &'a dyn Fn(light_dynamics::FamilySampleRank) -> light_dynamics::FamilyEndpointOutputControl,
    pool: Vec<RetainedFamilyCompositionScratch>,
}
impl<'a, 'sources, S: DynamicTickSource> CapturedHybridPositionBatchComposer<'a, 'sources, S> {
    #[cfg(test)]
    pub fn new(
        typed: &'a CapturedProgrammingSources<'sources, S>,
        groups: &[&'a light_dynamics::DynamicFamilySampleGroup],
        frame: HybridFrameContext<'a>,
        baseline: &'a PreparedStaticFamilyFrame,
        control: &'a dyn Fn(
            light_dynamics::FamilySampleRank,
        ) -> light_dynamics::FamilyEndpointOutputControl,
        scratch: RetainedFamilyCompositionScratch,
    ) -> Self {
        Self::new_with_pool(typed, groups, frame, baseline, control, scratch, Vec::new())
    }
    pub fn new_with_pool(
        typed: &'a CapturedProgrammingSources<'sources, S>,
        groups: &[&'a light_dynamics::DynamicFamilySampleGroup],
        frame: HybridFrameContext<'a>,
        baseline: &'a PreparedStaticFamilyFrame,
        control: &'a dyn Fn(
            light_dynamics::FamilySampleRank,
        ) -> light_dynamics::FamilyEndpointOutputControl,
        scratch: RetainedFamilyCompositionScratch,
        mut pool: Vec<RetainedFamilyCompositionScratch>,
    ) -> Self {
        pool.truncate(256);
        pool.push(scratch);
        Self {
            typed,
            groups: groups.to_vec(),
            frame,
            baseline,
            control,
            pool,
        }
    }
    fn with_composer<R>(
        &mut self,
        target: FixtureId,
        retain_local: bool,
        action: impl FnOnce(
            &mut CapturedHybridProgramComposer<'_, 'sources, S>,
        ) -> Result<R, TransitionError>,
    ) -> Result<R, TransitionError> {
        let group = self
            .groups
            .iter()
            .copied()
            .find(|group| group.target == target)
            .ok_or_else(|| {
                IntentError("Position batch target is unavailable in this captured context".into())
            })?;
        if group.owner != ProgrammingOwner::Position
            || self
                .groups
                .iter()
                .filter(|group| group.target == target)
                .count()
                != 1
        {
            return Err(IntentError(
                "Position batch has duplicate or foreign owner membership".into(),
            )
            .into());
        }
        let mut scratch = self.pool.pop().unwrap_or_default();
        let mut composer = CapturedHybridProgramComposer {
            typed: self.typed,
            group,
            frame: self.frame,
            baseline: self.baseline,
            control: self.control,
            scratch: &mut scratch,
        };
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| action(&mut composer)));
        // A successful begin transfers this workspace to its owned evaluation. Do not
        // retain the empty placeholder, which would grow the pool on every frame.
        if retain_local || !matches!(&result, Ok(Ok(_))) {
            self.pool.push(scratch);
        }
        match result {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
    #[cfg(test)]
    pub fn into_scratch(mut self) -> RetainedFamilyCompositionScratch {
        self.pool.pop().unwrap_or_default()
    }
    pub fn into_workspaces(
        mut self,
    ) -> (
        RetainedFamilyCompositionScratch,
        Vec<RetainedFamilyCompositionScratch>,
    ) {
        let local = self.pool.pop().unwrap_or_default();
        self.pool.truncate(256);
        (local, self.pool)
    }
    fn controls_match(
        &self,
        program: &HybridCapturedPositionProgram,
        accepts: impl Fn(light_dynamics::FamilyEndpointOutputControl) -> bool,
    ) -> Result<bool, TransitionError> {
        if program.frame_token() != self.frame.token
            || !self
                .groups
                .iter()
                .any(|group| group.target == program.target())
        {
            return Err(
                IntentError("Position batch control query names a foreign program".into()).into(),
            );
        }
        for index in 0..program.registry().source_count() {
            if !accepts((self.control)(program.registry().source_rank(index)?)) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
impl<S: DynamicTickSource, T> HybridPositionBatchComposer<T>
    for CapturedHybridPositionBatchComposer<'_, '_, S>
{
    fn eligible_targets(&self) -> Vec<FixtureId> {
        self.groups.iter().map(|group| group.target).collect()
    }
    fn unchanged_controls(
        &self,
        program: &HybridCapturedPositionProgram,
    ) -> Result<bool, TransitionError> {
        self.controls_match(program, |control| {
            matches!(
                control,
                light_dynamics::FamilyEndpointOutputControl::Unchanged
            )
        })
    }
    fn noninterpolating_controls(
        &self,
        program: &HybridCapturedPositionProgram,
    ) -> Result<bool, TransitionError> {
        self.controls_match(program, |control| {
            matches!(
                control,
                light_dynamics::FamilyEndpointOutputControl::Unchanged
                    | light_dynamics::FamilyEndpointOutputControl::Suppressed
            ) || matches!(control,
                    light_dynamics::FamilyEndpointOutputControl::CrossfadeCurrent { mix }
                        if mix == 1.0)
        })
    }
    fn original_sources_suppressed(&self, target: FixtureId) -> Result<bool, TransitionError> {
        self.typed.check()?;
        if !self.frame.token.matches_geometry(self.frame.geometry)
            || !self.frame.token.matches_static_frame(self.baseline)
            || !self.frame.token.matches_static_frame(self.frame.scalar)
        {
            return Err(IntentError(
                "Position gate query has a foreign captured frame lane".into(),
            )
            .into());
        }
        let mut matches = self
            .groups
            .iter()
            .copied()
            .filter(|group| group.target == target);
        let group = matches
            .next()
            .ok_or_else(|| IntentError("Position gate query target is absent".into()))?;
        if group.owner != ProgrammingOwner::Position || matches.next().is_some() {
            return Err(IntentError(
                "Position gate query has duplicate or foreign owner membership".into(),
            )
            .into());
        }
        let suppressed = !group.samples.is_empty()
            && group.samples.iter().all(|sample| {
                let control = match sample {
                    light_dynamics::FamilyCompositionSample::Known(sample) => {
                        sample.captured_endpoint_control(self.control)
                    }
                    light_dynamics::FamilyCompositionSample::WholeExpression { rank, .. }
                    | light_dynamics::FamilyCompositionSample::CoupledExpression { rank, .. } => {
                        (self.control)(*rank)
                    }
                };
                matches!(
                    control,
                    light_dynamics::FamilyEndpointOutputControl::Suppressed
                )
            });
        self.typed.check()?;
        Ok(suppressed)
    }
    fn begin_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionEvaluation, TransitionError> {
        self.with_composer(program.target(), false, |composer| {
            position_program::begin_branch(composer, program, branch, destination, adoption)
        })
    }
    fn advance(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<PositionCompositionProgress, TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::advance(composer, evaluation, frame, adoption)
        })
    }
    fn resume(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::resume(composer, evaluation, request_id, value, transfer)
        })
    }
    fn begin_stage_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &PositionStageLocator,
        operand: PositionStageOperand,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionStageEvaluation, TransitionError> {
        self.with_composer(program.target(), false, |composer| {
            position_program::begin_stage_branch(
                composer,
                program,
                branch,
                locator,
                operand,
                destination,
                adoption,
            )
        })
    }
    fn begin_envelope_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionEnvelopeLocator,
        operand: light_dynamics::PositionEnvelopeOperand,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionStageEvaluation, TransitionError> {
        self.with_composer(program.target(), false, |composer| {
            position_program::begin_envelope_branch(
                composer,
                program,
                branch,
                locator,
                operand,
                destination,
                adoption,
            )
        })
    }
    fn begin_mask_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionMaskLocator,
        operand: light_dynamics::PositionMaskOperand,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionStageEvaluation, TransitionError> {
        self.with_composer(program.target(), false, |composer| {
            position_program::begin_mask_branch(
                composer,
                program,
                branch,
                locator,
                operand,
                destination,
                adoption,
            )
        })
    }
    fn advance_stage(
        &mut self,
        evaluation: &mut HybridPositionStageEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<PositionStageOperandProgress, TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::advance_stage(composer, evaluation, frame, adoption)
        })
    }
    fn resume_stage(
        &mut self,
        evaluation: &mut HybridPositionStageEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::resume_stage(composer, evaluation, request_id, value, transfer)
        })
    }
    fn recycle_stage(
        &mut self,
        evaluation: HybridPositionStageEvaluation,
    ) -> Result<(), TransitionError> {
        if evaluation.frame_token() != self.frame.token
            || !self
                .groups
                .iter()
                .any(|group| group.target == evaluation.target())
        {
            return Err(IntentError(
                "Position batch recycled a foreign operand frame or target".into(),
            )
            .into());
        }
        self.pool.push(evaluation.into_scratch());
        Ok(())
    }
    fn begin_graph_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionGraphOperationLocator,
        operand: light_dynamics::PositionGraphOperationOperand,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionGraphEvaluation, TransitionError> {
        program.registry().validate_branch(branch)?;
        branch.validate_graph_operation_operand(locator, operand)?;
        self.typed.check()?;
        if program.frame_token() != self.frame.token
            || !self.frame.token.matches_geometry(self.frame.geometry)
            || !self.frame.token.matches_static_frame(self.baseline)
            || !self.frame.token.matches_static_frame(self.frame.scalar)
        {
            return Err(IntentError(
                "Position graph operand belongs to another captured frame lane".into(),
            )
            .into());
        }
        self.with_composer(program.target(), false, |composer| {
            position_program::begin_graph_branch(
                composer,
                program,
                branch,
                locator,
                operand,
                destination,
                adoption,
            )
        })
    }
    fn advance_graph(
        &mut self,
        evaluation: &mut HybridPositionGraphEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<light_dynamics::PositionGraphOperationOperandProgress, TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::advance_graph(composer, evaluation, frame, adoption)
        })
    }
    fn resume_graph(
        &mut self,
        evaluation: &mut HybridPositionGraphEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::resume_graph(composer, evaluation, request_id, value, transfer)
        })
    }
    fn recycle_graph(
        &mut self,
        evaluation: HybridPositionGraphEvaluation,
    ) -> Result<(), TransitionError> {
        if evaluation.frame_token() != self.frame.token
            || !self
                .groups
                .iter()
                .any(|group| group.target == evaluation.target())
        {
            return Err(IntentError(
                "Position batch recycled a foreign graph operand frame or target".into(),
            )
            .into());
        }
        self.pool.push(evaluation.into_scratch());
        Ok(())
    }
    fn begin_resume_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionResumeOperandLocator,
        endpoint: light_dynamics::PositionResumeEndpoint,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionResumeEvaluation, TransitionError> {
        program.registry().validate_branch(branch)?;
        branch.validate_resume_operand(locator, endpoint)?;
        self.typed.check()?;
        if program.frame_token() != self.frame.token
            || !self.frame.token.matches_geometry(self.frame.geometry)
            || !self.frame.token.matches_static_frame(self.baseline)
            || !self.frame.token.matches_static_frame(self.frame.scalar)
        {
            return Err(IntentError(
                "Position Resume operand belongs to another captured frame lane".into(),
            )
            .into());
        }
        self.with_composer(program.target(), false, |composer| {
            position_program::begin_resume_branch(
                composer,
                program,
                branch,
                locator,
                endpoint,
                destination,
                adoption,
            )
        })
    }
    fn advance_resume(
        &mut self,
        evaluation: &mut HybridPositionResumeEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<light_dynamics::PositionResumeOperandProgress, TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::advance_resume(composer, evaluation, frame, adoption)
        })
    }
    fn resume_resume(
        &mut self,
        evaluation: &mut HybridPositionResumeEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::resume_resume(composer, evaluation, request_id, value, transfer)
        })
    }
    fn recycle_resume(
        &mut self,
        evaluation: HybridPositionResumeEvaluation,
    ) -> Result<(), TransitionError> {
        if evaluation.frame_token() != self.frame.token
            || !self
                .groups
                .iter()
                .any(|group| group.target == evaluation.target())
        {
            return Err(IntentError(
                "Position batch recycled a foreign Resume operand frame or target".into(),
            )
            .into());
        }
        self.pool.push(evaluation.into_scratch());
        Ok(())
    }
    fn begin_discovery_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
        route_limit: usize,
    ) -> Result<HybridPositionDiscoveryEvaluation, TransitionError> {
        self.with_composer(program.target(), false, |composer| {
            position_program::begin_discovery_branch(
                composer,
                program,
                branch,
                destination,
                adoption,
                route_limit,
            )
        })
    }
    fn advance_discovery(
        &mut self,
        evaluation: &mut HybridPositionDiscoveryEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<PositionStageDiscoveryProgress, TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::advance_discovery(composer, evaluation, frame, adoption)
        })
    }
    fn resume_discovery(
        &mut self,
        evaluation: &mut HybridPositionDiscoveryEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::resume_discovery(composer, evaluation, request_id, value, transfer)
        })
    }
    fn discovery_report(
        &mut self,
        evaluation: &mut HybridPositionDiscoveryEvaluation,
    ) -> Result<PositionStageDiscoveryReport, TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::discovery_report(composer, evaluation)
        })
    }
    fn recycle_discovery(
        &mut self,
        evaluation: HybridPositionDiscoveryEvaluation,
    ) -> Result<(), TransitionError> {
        if evaluation.frame_token() != self.frame.token
            || !self
                .groups
                .iter()
                .any(|group| group.target == evaluation.target())
        {
            return Err(IntentError(
                "Position batch recycled a foreign discovery frame or target".into(),
            )
            .into());
        }
        self.pool.push(evaluation.into_scratch());
        Ok(())
    }
    fn observe(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        observe: &mut dyn FnMut(
            HybridFamilyObservation<'_>,
        ) -> Result<(FamilyProjectionMetadata, T), TransitionError>,
    ) -> Result<OwnedHybridProjection<T>, TransitionError> {
        self.with_composer(evaluation.target(), true, |composer| {
            position_program::observe(composer, evaluation, observe)
        })
    }
    fn recycle(&mut self, evaluation: HybridPositionEvaluation) -> Result<(), TransitionError> {
        if evaluation.frame_token() != self.frame.token
            || !self
                .groups
                .iter()
                .any(|group| group.target == evaluation.target())
        {
            return Err(
                IntentError("Position batch recycled a foreign frame or target".into()).into(),
            );
        }
        self.pool.push(evaluation.into_scratch());
        Ok(())
    }
}

pub(super) fn validate_result<T>(
    result: &HybridPositionBatchResult<T>,
    groups: &[&light_dynamics::DynamicFamilySampleGroup],
) -> Result<(), TransitionError> {
    let mut handled = FxHashSet::default();
    for target in &result.handled {
        if !handled.insert(*target)
            || !groups
                .iter()
                .any(|group| group.owner == ProgrammingOwner::Position && group.target == *target)
        {
            return Err(IntentError(
                "Position batch handled a duplicate or unavailable owner".into(),
            )
            .into());
        }
    }
    let mut rows = FxHashSet::default();
    for row in &result.projections {
        if row.owner != ProgrammingOwner::Position
            || !handled.contains(&row.target)
            || !rows.insert(row.target)
        {
            return Err(IntentError(
                "Position batch projection is duplicated or outside handled membership".into(),
            )
            .into());
        }
    }
    let mut requirements = FxHashSet::default();
    for requirement in &result.requirements {
        if requirement.owner != ProgrammingOwner::Position || !handled.contains(&requirement.target)
        {
            return Err(IntentError(
                "Position batch requirement is outside handled membership".into(),
            )
            .into());
        }
        requirements.insert(requirement.target);
    }
    if handled
        .iter()
        .any(|target| !rows.contains(target) && !requirements.contains(target))
    {
        return Err(IntentError(
            "Position batch omitted a handled owner without a passive requirement".into(),
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
