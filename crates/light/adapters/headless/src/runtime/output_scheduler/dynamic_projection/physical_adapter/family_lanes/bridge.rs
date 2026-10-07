//! Sidecar re-wrapping between the all-family composers (`FamilySidecar`) and the Position
//! observer's own composers (`PhysicalHeadResult<PositionAdapter>`). Every call is forwarded
//! unchanged; only observation closures and returned rows change their sidecar wrapper. A row
//! that comes back as another family is corruption, never silently dropped.
use super::super::programming_projection::hybrid::{
    HybridCapturedPositionProgram, HybridPositionBatchComposer, HybridPositionDiscoveryEvaluation,
    HybridPositionEvaluation, HybridPositionGraphEvaluation, HybridPositionResumeEvaluation,
    HybridPositionStageEvaluation, HybridProgramComposer, OwnedHybridProjection,
};
use super::*;
use light_dynamics::{
    FamilyAdoptionResolver, PositionCompositionProgress, PositionProgramBranch,
    PositionStageDiscoveryProgress, PositionStageDiscoveryReport, PositionStageLocator,
    PositionStageOperand, PositionStageOperandProgress, WholeFamilyExpressionFrameResolver,
};

type PositionObserve<'o> = dyn FnMut(
        HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, PositionSidecar), TransitionError>
    + 'o;

/// A composed row split by family: Position rows unwrapped, every other family handed back.
#[allow(clippy::large_enum_variant)] // Moved once per row, like `FamilySidecar` itself.
pub(super) enum SplitRow {
    Position(OwnedHybridProjection<PositionSidecar>),
    Other(OwnedHybridProjection<FamilySidecar>),
}

pub(super) fn position_row(row: OwnedHybridProjection<FamilySidecar>) -> SplitRow {
    let OwnedHybridProjection {
        target,
        owner,
        value,
        metadata,
        sidecar,
    } = row;
    match sidecar {
        FamilySidecar::Position(sidecar) => SplitRow::Position(OwnedHybridProjection {
            target,
            owner,
            value,
            metadata,
            sidecar: *sidecar,
        }),
        sidecar => SplitRow::Other(OwnedHybridProjection {
            target,
            owner,
            value,
            metadata,
            sidecar,
        }),
    }
}

pub(super) fn family_row(
    row: OwnedHybridProjection<PositionSidecar>,
) -> OwnedHybridProjection<FamilySidecar> {
    OwnedHybridProjection {
        target: row.target,
        owner: row.owner,
        value: row.value,
        metadata: row.metadata,
        sidecar: FamilySidecar::Position(Box::new(row.sidecar)),
    }
}

fn expect_position(
    row: OwnedHybridProjection<FamilySidecar>,
) -> Result<OwnedHybridProjection<PositionSidecar>, TransitionError> {
    match position_row(row) {
        SplitRow::Position(row) => Ok(row),
        SplitRow::Other(_) => {
            Err(IntentError("Position composition returned another family's sidecar".into()).into())
        }
    }
}

/// Observe through the family composer, wrapping the Position observer's sidecar.
fn wrapped(
    observe: &mut PositionObserve<'_>,
    observation: HybridFamilyObservation<'_>,
) -> Result<(FamilyProjectionMetadata, FamilySidecar), TransitionError> {
    observe(observation)
        .map(|(metadata, sidecar)| (metadata, FamilySidecar::Position(Box::new(sidecar))))
}

/// The family program composer, seen by the Position observer as a Position composer.
pub(super) struct PositionProgramBridge<'c> {
    pub inner: &'c mut dyn HybridProgramComposer<FamilySidecar>,
}

impl HybridProgramComposer<PositionSidecar> for PositionProgramBridge<'_> {
    fn begin_position(
        &mut self,
        program: &HybridCapturedPositionProgram,
        destination: FixtureId,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionEvaluation, TransitionError> {
        self.inner.begin_position(program, destination, adoption)
    }

    fn begin_position_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        destination: FixtureId,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionEvaluation, TransitionError> {
        self.inner
            .begin_position_branch(program, branch, destination, adoption)
    }

    fn advance_position(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<PositionCompositionProgress, TransitionError> {
        self.inner.advance_position(evaluation, frame, adoption)
    }

    fn observe_position(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        observe: &mut PositionObserve<'_>,
    ) -> Result<OwnedHybridProjection<PositionSidecar>, TransitionError> {
        let row = self
            .inner
            .observe_position(evaluation, &mut |o| wrapped(observe, o))?;
        expect_position(row)
    }

    fn resume_position(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.inner
            .resume_position(evaluation, request_id, value, transfer)
    }

    fn recycle_position(&mut self, evaluation: HybridPositionEvaluation) {
        self.inner.recycle_position(evaluation);
    }

    fn compose(
        &mut self,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &FamilyAdoptionResolver<'_>,
        observe: &mut PositionObserve<'_>,
    ) -> Result<OwnedHybridProjection<PositionSidecar>, TransitionError> {
        let row = self
            .inner
            .compose(frame, adoption, &mut |o| wrapped(observe, o))?;
        expect_position(row)
    }
}

/// The family Position batch composer, seen by the Position observer as its own.
pub(super) struct PositionBatchBridge<'c> {
    pub inner: &'c mut dyn HybridPositionBatchComposer<FamilySidecar>,
}

impl HybridPositionBatchComposer<PositionSidecar> for PositionBatchBridge<'_> {
    fn eligible_targets(&self) -> Vec<FixtureId> {
        self.inner.eligible_targets()
    }

    fn unchanged_controls(
        &self,
        program: &HybridCapturedPositionProgram,
    ) -> Result<bool, TransitionError> {
        self.inner.unchanged_controls(program)
    }

    fn noninterpolating_controls(
        &self,
        program: &HybridCapturedPositionProgram,
    ) -> Result<bool, TransitionError> {
        self.inner.noninterpolating_controls(program)
    }

    fn original_sources_suppressed(&self, target: FixtureId) -> Result<bool, TransitionError> {
        self.inner.original_sources_suppressed(target)
    }

    fn begin_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        destination: FixtureId,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionEvaluation, TransitionError> {
        self.inner
            .begin_branch(program, branch, destination, adoption)
    }

    fn advance(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<PositionCompositionProgress, TransitionError> {
        self.inner.advance(evaluation, frame, adoption)
    }

    fn resume(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.inner.resume(evaluation, request_id, value, transfer)
    }

    fn begin_stage_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &PositionStageLocator,
        operand: PositionStageOperand,
        destination: FixtureId,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionStageEvaluation, TransitionError> {
        self.inner
            .begin_stage_branch(program, branch, locator, operand, destination, adoption)
    }

    fn begin_envelope_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionEnvelopeLocator,
        operand: light_dynamics::PositionEnvelopeOperand,
        destination: FixtureId,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionStageEvaluation, TransitionError> {
        self.inner
            .begin_envelope_branch(program, branch, locator, operand, destination, adoption)
    }

    fn begin_mask_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionMaskLocator,
        operand: light_dynamics::PositionMaskOperand,
        destination: FixtureId,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionStageEvaluation, TransitionError> {
        self.inner
            .begin_mask_branch(program, branch, locator, operand, destination, adoption)
    }

    fn advance_stage(
        &mut self,
        evaluation: &mut HybridPositionStageEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<PositionStageOperandProgress, TransitionError> {
        self.inner.advance_stage(evaluation, frame, adoption)
    }

    fn resume_stage(
        &mut self,
        evaluation: &mut HybridPositionStageEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.inner
            .resume_stage(evaluation, request_id, value, transfer)
    }

    fn recycle_stage(
        &mut self,
        evaluation: HybridPositionStageEvaluation,
    ) -> Result<(), TransitionError> {
        self.inner.recycle_stage(evaluation)
    }

    fn begin_graph_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionGraphOperationLocator,
        operand: light_dynamics::PositionGraphOperationOperand,
        destination: FixtureId,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionGraphEvaluation, TransitionError> {
        self.inner
            .begin_graph_branch(program, branch, locator, operand, destination, adoption)
    }

    fn advance_graph(
        &mut self,
        evaluation: &mut HybridPositionGraphEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<light_dynamics::PositionGraphOperationOperandProgress, TransitionError> {
        self.inner.advance_graph(evaluation, frame, adoption)
    }

    fn resume_graph(
        &mut self,
        evaluation: &mut HybridPositionGraphEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.inner
            .resume_graph(evaluation, request_id, value, transfer)
    }

    fn recycle_graph(
        &mut self,
        evaluation: HybridPositionGraphEvaluation,
    ) -> Result<(), TransitionError> {
        self.inner.recycle_graph(evaluation)
    }

    fn begin_resume_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        locator: &light_dynamics::PositionResumeOperandLocator,
        endpoint: light_dynamics::PositionResumeEndpoint,
        destination: FixtureId,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionResumeEvaluation, TransitionError> {
        self.inner
            .begin_resume_branch(program, branch, locator, endpoint, destination, adoption)
    }

    fn advance_resume(
        &mut self,
        evaluation: &mut HybridPositionResumeEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<light_dynamics::PositionResumeOperandProgress, TransitionError> {
        self.inner.advance_resume(evaluation, frame, adoption)
    }

    fn resume_resume(
        &mut self,
        evaluation: &mut HybridPositionResumeEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.inner
            .resume_resume(evaluation, request_id, value, transfer)
    }

    fn recycle_resume(
        &mut self,
        evaluation: HybridPositionResumeEvaluation,
    ) -> Result<(), TransitionError> {
        self.inner.recycle_resume(evaluation)
    }

    fn begin_discovery_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &PositionProgramBranch,
        destination: FixtureId,
        adoption: &FamilyAdoptionResolver<'_>,
        route_limit: usize,
    ) -> Result<HybridPositionDiscoveryEvaluation, TransitionError> {
        self.inner
            .begin_discovery_branch(program, branch, destination, adoption, route_limit)
    }

    fn advance_discovery(
        &mut self,
        evaluation: &mut HybridPositionDiscoveryEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &FamilyAdoptionResolver<'_>,
    ) -> Result<PositionStageDiscoveryProgress, TransitionError> {
        self.inner.advance_discovery(evaluation, frame, adoption)
    }

    fn resume_discovery(
        &mut self,
        evaluation: &mut HybridPositionDiscoveryEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        self.inner
            .resume_discovery(evaluation, request_id, value, transfer)
    }

    fn discovery_report(
        &mut self,
        evaluation: &mut HybridPositionDiscoveryEvaluation,
    ) -> Result<PositionStageDiscoveryReport, TransitionError> {
        self.inner.discovery_report(evaluation)
    }

    fn recycle_discovery(
        &mut self,
        evaluation: HybridPositionDiscoveryEvaluation,
    ) -> Result<(), TransitionError> {
        self.inner.recycle_discovery(evaluation)
    }

    fn observe(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        observe: &mut PositionObserve<'_>,
    ) -> Result<OwnedHybridProjection<PositionSidecar>, TransitionError> {
        let row = self
            .inner
            .observe(evaluation, &mut |o| wrapped(observe, o))?;
        expect_position(row)
    }

    fn recycle(&mut self, evaluation: HybridPositionEvaluation) -> Result<(), TransitionError> {
        self.inner.recycle(evaluation)
    }
}
