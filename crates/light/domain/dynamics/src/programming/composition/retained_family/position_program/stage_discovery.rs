//! Bounded enumeration of registry-local syntactic stage candidates. A candidate needs
//! operand replay to prove reachability; completing discovery does not certify an exhaustive
//! catalogue of operations, fitting, physical copies or cross-owner correspondence.
use super::*;

#[derive(Clone, Debug)]
pub struct PositionStageDiscoveryReport {
    /// Preserve emission order and repeated candidates. Equality cannot establish uniqueness.
    pub candidates: Vec<PositionStageLocator>,
    /// Reached EndpointOutput/Activation operations, in emission order. Each was reached
    /// in this replay but still needs operand replay in any changed branch.
    pub envelope_candidates: Vec<PositionEnvelopeLocator>,
    /// Reached MaskAdoption/MaskTransition operations of partial whole masks, in emission
    /// order. Full masks have no operation; covered lower masks are never reached.
    pub mask_candidates: Vec<PositionMaskLocator>,
    pub inspected_routes: usize,
    /// Inspected segment/envelope/mask routes lacking supported exact original identity.
    /// Operations outside the stage matcher are not enumerated.
    pub unmapped_routes: usize,
    /// True only after the original retained driver completes; candidates may still be inactive.
    pub complete: bool,
}

pub enum PositionStageDiscoveryProgress {
    NeedsMaterialization(PositionCompositionRequest),
    Complete(PositionStageDiscoveryReport),
}

pub(super) struct DiscoveryState {
    candidates: Vec<PositionStageLocator>,
    envelope_candidates: Vec<PositionEnvelopeLocator>,
    mask_candidates: Vec<PositionMaskLocator>,
    inspected_routes: usize,
    unmapped_routes: usize,
    route_limit: usize,
}
impl DiscoveryState {
    pub(super) fn record(
        &mut self,
        candidate: Option<PositionStageLocator>,
    ) -> Result<(), TransitionError> {
        ensure(
            self.inspected_routes < self.route_limit,
            "Position stage discovery route limit exhausted; enumeration is incomplete",
        )?;
        self.inspected_routes += 1;
        match candidate {
            Some(candidate) if candidate.envelope_stage().is_some() => self
                .envelope_candidates
                .extend(PositionEnvelopeLocator::from_stage(candidate)),
            Some(candidate) if candidate.mask_stage().is_some() => self
                .mask_candidates
                .extend(PositionMaskLocator::from_stage(candidate)),
            Some(candidate) => self.candidates.push(candidate),
            None => self.unmapped_routes += 1,
        }
        Ok(())
    }
    fn report(&self, complete: bool) -> PositionStageDiscoveryReport {
        PositionStageDiscoveryReport {
            candidates: self.candidates.clone(),
            envelope_candidates: self.envelope_candidates.clone(),
            mask_candidates: self.mask_candidates.clone(),
            inspected_routes: self.inspected_routes,
            unmapped_routes: self.unmapped_routes,
            complete,
        }
    }
}

/// Separate speculative driver: no parent value, trace observation or acceptance API.
/// Callers retain the exact immutable context/frame, as for ordinary Position composition.
pub struct PositionStageDiscoveryContinuation {
    inner: PositionCompositionContinuation,
}
impl PositionProgramBranch {
    pub fn begin_stage_discovery(
        &self,
        context: &FamilyCompositionContext<'_>,
        scratch: RetainedFamilyCompositionScratch,
        tracing: bool,
        route_limit: usize,
    ) -> Result<PositionStageDiscoveryContinuation, TransitionError> {
        ensure(
            route_limit > 0,
            "Position stage discovery requires a positive route limit",
        )?;
        let mut inner = self.begin_composition(context, scratch, tracing)?;
        // Prevent the ordinary advance entry point from exposing this speculative result.
        inner.operand_only = true;
        inner.discovery = Some(DiscoveryState {
            candidates: Vec::new(),
            envelope_candidates: Vec::new(),
            mask_candidates: Vec::new(),
            inspected_routes: 0,
            unmapped_routes: 0,
            route_limit,
        });
        Ok(PositionStageDiscoveryContinuation { inner })
    }
}
impl PositionStageDiscoveryContinuation {
    pub fn advance(
        &mut self,
        capture_id: Uuid,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<PositionStageDiscoveryProgress, TransitionError> {
        ensure(
            capture_id == self.inner.capture_id,
            "Position discovery belongs to another capture",
        )?;
        ensure(
            !self.inner.failed,
            "failed Position discovery must be discarded",
        )?;
        self.inner.failed = true;
        let result = self.inner.advance_impl(context, frame, None);
        if result.is_ok() {
            self.inner.failed = false;
        }
        match result? {
            PositionCompositionProgress::NeedsMaterialization(request) => Ok(
                PositionStageDiscoveryProgress::NeedsMaterialization(request),
            ),
            PositionCompositionProgress::Complete(_) => {
                Ok(PositionStageDiscoveryProgress::Complete(self.report()?))
            }
        }
    }
    /// Snapshot of the supported candidate prefix. No report survives terminal failure.
    pub fn report(&self) -> Result<PositionStageDiscoveryReport, TransitionError> {
        ensure(
            !self.inner.failed,
            "failed Position discovery has no enumeration authority",
        )?;
        Ok(self
            .inner
            .discovery
            .as_ref()
            .expect("discovery state")
            .report(self.inner.completed))
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
