//! Resume operand replay retains the enclosing original goal and atomic member boundaries.
//! A locator is owner-local; only the captured runtime occurrence can correlate peers.
use super::super::position_conditioning::origins::{ResumeGoal, ResumeStop};
use super::*;
use std::cell::Cell;

pub enum PositionResumeOperandProgress {
    NeedsMaterialization(PositionCompositionRequest),
    OperandReady(AttributeValue),
    Inactive,
}
pub struct PositionResumeOperandContinuation {
    inner: PositionCompositionContinuation,
}
pub(super) struct ResumeReplayState {
    stops: Vec<ResumeStop>,
    eligible: Vec<bool>,
    stopped_depth: Cell<Option<usize>>,
}
impl ResumeReplayState {
    pub(super) fn graph_plan(
        &self,
        route: &PreparedPositionGraphRoute,
        binding: &super::super::position_conditioning::origins::PositionOriginBinding,
    ) -> Result<Vec<GraphOperationSelection>, TransitionError> {
        let mut selections = Vec::new();
        for (depth, stop) in self.stops.iter().enumerate() {
            if !self.eligible[depth] {
                continue;
            }
            let operand = match stop {
                ResumeStop::Graph(locator, operand)
                    if binding.graph_locator(route)?.as_ref() == Some(locator) =>
                {
                    Some(operand.internal())
                }
                ResumeStop::Resume(locator, endpoint) if !locator.is_cohort() => {
                    let candidate = binding.resume_locator(route, ResumeGoal::Full)?;
                    candidate
                        .filter(|candidate| locator.same_boundary(candidate))
                        .map(|_| match endpoint {
                            PositionResumeEndpoint::Outgoing => {
                                GraphOperationOperand::ResumeOutgoing
                            }
                            PositionResumeEndpoint::Incoming => {
                                GraphOperationOperand::ResumeIncoming
                            }
                        })
                }
                _ => None,
            };
            if let Some(operand) = operand {
                selections.push(GraphOperationSelection { operand, depth });
            }
        }
        Ok(selections)
    }
    pub(super) fn stage(
        &self,
        route: &PreparedPositionStageRoute,
        binding: &super::super::position_conditioning::origins::PositionOriginBinding,
    ) -> Result<Option<position_segment::PositionSegmentOperand>, TransitionError> {
        for (depth, stop) in self.stops.iter().enumerate() {
            if let ResumeStop::Stage(locator, operand) = stop {
                if self.eligible[depth] && binding.stage_locator(route)?.as_ref() == Some(locator) {
                    self.stopped_depth.set(Some(depth));
                    return Ok(Some(operand.segment()));
                }
            }
        }
        Ok(None)
    }
    pub(super) fn cohort(
        &self,
        uses: &[PreparedPositionStageUse],
        origins: &[PreparedSourceOrigin],
        expression: &Arc<CompiledCoupledExpression>,
        endpoints: &[usize],
        binding: &super::super::position_conditioning::origins::PositionOriginBinding,
    ) -> Result<Option<usize>, TransitionError> {
        let mut selected = None;
        for (depth, stop) in self.stops.iter().enumerate() {
            if let ResumeStop::Resume(locator, _) = stop {
                if self.eligible[depth]
                    && binding
                        .resume_cohort_matches(locator, uses, origins, expression, endpoints)?
                {
                    selected = Some(depth);
                }
            }
        }
        Ok(selected)
    }
    pub(super) fn complete_stop(&self, depth: usize) {
        self.stopped_depth.set(Some(depth));
    }
    pub(super) fn requested_stopped(&self) -> bool {
        self.stopped_depth.get() == Some(self.stops.len() - 1)
    }
    pub(super) fn enclosing_stage(&self) -> Option<(PositionStageLocator, PositionStageOperand)> {
        self.stops.iter().find_map(|stop| {
            if let ResumeStop::Stage(locator, operand) = stop {
                Some((locator.clone(), *operand))
            } else {
                None
            }
        })
    }
}
impl PositionCompositionContinuation {
    pub fn pending_resume_operand_locator(
        &self,
        request_id: Uuid,
    ) -> Result<Option<PositionResumeOperandLocator>, TransitionError> {
        ensure(
            !self.failed,
            "failed Position composition has no Resume operand authority",
        )?;
        let request = self.pending.as_ref().ok_or_else(|| {
            IntentError("Position composition has no pending Resume operand".into())
        })?;
        ensure(
            request.request_id == request_id,
            "Position Resume locator names another outstanding request",
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
        binding.resume_locator(route, self.resume_goal.clone())
    }
}
impl PositionProgramBranch {
    pub fn begin_resume_operand(
        &self,
        locator: &PositionResumeOperandLocator,
        endpoint: PositionResumeEndpoint,
        context: &FamilyCompositionContext<'_>,
        scratch: RetainedFamilyCompositionScratch,
        tracing: bool,
    ) -> Result<PositionResumeOperandContinuation, TransitionError> {
        self.validate_resume_operand(locator, endpoint)?;
        let stops = locator.stops(endpoint)?;
        let mut branch = self.clone();
        let mut eligible = Vec::with_capacity(stops.len());
        let mut enclosing_inactive = false;
        for stop in &stops {
            let active = match stop {
                ResumeStop::Stage(locator, _) => {
                    let active =
                        PositionResumeOperandLocator::enclosing_stage_active(locator, &branch)?;
                    if !active {
                        enclosing_inactive = true;
                    }
                    active
                }
                ResumeStop::Graph(locator, _) => {
                    let active = branch.original_node_active(locator.operation_node())?;
                    if !active {
                        enclosing_inactive = true;
                    }
                    active
                }
                ResumeStop::Resume(locator, _) if !locator.is_cohort() => {
                    let active = branch.original_node_active(locator.operation_node())?;
                    if !active {
                        enclosing_inactive = true;
                    }
                    active
                }
                ResumeStop::Resume(locator, endpoint) if locator.is_cohort() => {
                    branch.prepare_resume_cohort(locator, *endpoint)?
                }
                _ => true,
            };
            if !active {
                enclosing_inactive = true;
            }
            eligible.push(active);
        }
        let mut inner = branch.begin_composition(context, scratch, tracing)?;
        inner.operand_only = true;
        if enclosing_inactive {
            inner.completed = true;
        }
        inner.resume_goal = ResumeGoal::Resume(Box::new(locator.clone()), endpoint);
        inner.resume_replay = Some(ResumeReplayState {
            stops,
            eligible,
            stopped_depth: Cell::new(None),
        });
        Ok(PositionResumeOperandContinuation { inner })
    }
}
impl PositionResumeOperandContinuation {
    pub fn advance(
        &mut self,
        capture_id: Uuid,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<PositionResumeOperandProgress, TransitionError> {
        ensure(
            capture_id == self.inner.capture_id,
            "Position Resume operand belongs to another capture",
        )?;
        ensure(
            !self.inner.failed,
            "failed Position Resume operand must be discarded",
        )?;
        if let Some(value) = &self.inner.stopped_operand {
            return Ok(PositionResumeOperandProgress::OperandReady(value.clone()));
        }
        let stage = self
            .inner
            .resume_replay
            .as_ref()
            .expect("Resume replay")
            .enclosing_stage();
        self.inner.failed = true;
        let result = self.inner.advance_impl(
            context,
            frame,
            stage.as_ref().map(|(locator, operand)| (locator, *operand)),
        );
        if result.is_ok() {
            self.inner.failed = false;
        }
        Ok(match result? {
            PositionCompositionProgress::NeedsMaterialization(request) => {
                PositionResumeOperandProgress::NeedsMaterialization(request)
            }
            PositionCompositionProgress::Complete(_) => match &self.inner.stopped_operand {
                Some(value) => PositionResumeOperandProgress::OperandReady(value.clone()),
                None => PositionResumeOperandProgress::Inactive,
            },
        })
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
    pub fn pending_resume_operand_locator(
        &self,
        request_id: Uuid,
    ) -> Result<Option<PositionResumeOperandLocator>, TransitionError> {
        self.inner.pending_resume_operand_locator(request_id)
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
