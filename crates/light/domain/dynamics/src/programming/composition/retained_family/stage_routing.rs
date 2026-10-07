//! Prepared Position stage and graph routing: lexical use metadata and the optional
//! callbacks that let operand replay match routes without burdening ordinary composition.
use super::*;

/// Lexical use metadata before original-registry binding. Source/member indices belong to
/// their route level's prepared scratch and are never public boundary authority.
#[derive(Clone)]
pub(in super::super) enum PreparedPositionStageUse {
    RootFinalSegment,
    /// Legacy materialized coupled endpoints have no original forest/member locator yet.
    /// Keep their lexical use distinct so they cannot alias a final segment boundary.
    UnsupportedCoupledEndpoint,
    UnderlayFor {
        consumer_origins: Vec<PreparedSourceOrigin>,
    },
    SourceCohort {
        consumer_origins: Vec<PreparedSourceOrigin>,
        expression: Arc<CompiledCoupledExpression>,
        endpoint_nodes: Vec<usize>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum PreparedPositionStageKind {
    Adoption {
        component: Option<ProgrammingComponent>,
    },
    WholeSegmentTransition,
    /// Post-expression envelope operation of the source named by the route trigger.
    Envelope(position_completion::PositionCompletionStage),
}
#[derive(Clone)]
pub(in super::super) struct PreparedPositionStageRoute {
    pub uses: Vec<PreparedPositionStageUse>,
    pub trigger_origins: Vec<PreparedSourceOrigin>,
    pub kind: PreparedPositionStageKind,
}
#[derive(Clone)]
pub(in super::super) enum PreparedPositionGraphExpression {
    Whole(Arc<CompiledProgrammingFamilyExpression>),
    Coupled(Arc<CompiledCoupledExpression>),
}
#[derive(Clone)]
pub(in super::super) struct PreparedPositionGraphRoute {
    pub uses: Vec<PreparedPositionStageUse>,
    pub trigger_origins: Vec<PreparedSourceOrigin>,
    pub expression: PreparedPositionGraphExpression,
    pub node: usize,
    pub kind: GraphOperationKind,
}
pub(super) type PreparedPositionGraphPlanCallback<'a> = dyn FnMut(&PreparedPositionGraphRoute) -> Result<Vec<GraphOperationSelection>, TransitionError>
    + 'a;
pub(super) type PreparedPositionStopCallback<'a> = dyn FnMut(usize) + 'a;
pub(super) type PreparedPositionCohortCallback<'a> = dyn FnMut(
        &[PreparedPositionStageUse],
        &[PreparedSourceOrigin],
        &Arc<CompiledCoupledExpression>,
        &[usize],
    ) -> Result<Option<usize>, TransitionError>
    + 'a;
pub(super) type PreparedPositionGraphReachedCallback<'a> =
    dyn FnMut(&PreparedPositionGraphRoute) -> Result<GraphReachedAction, TransitionError> + 'a;
pub(super) type PreparedPositionGraphCallback<'a> = dyn FnMut(&PreparedPositionGraphRoute) -> Result<Option<GraphOperationOperand>, TransitionError>
    + 'a;
pub(super) type PreparedPositionStageCallback<'a> = dyn FnMut(
        &PreparedPositionStageRoute,
    ) -> Result<Option<position_segment::PositionSegmentOperand>, TransitionError>
    + 'a;
/// Ordinary composition carries routing only when needed; it never scans/clones per-sample
/// stop candidates. Explicit operand replay opts into the matching work.
pub(in super::super) struct PreparedPositionStageMatcher<'a> {
    pub(super) callback: Option<&'a mut PreparedPositionStageCallback<'a>>,
    pub(super) graph_callback: Option<&'a mut PreparedPositionGraphCallback<'a>>,
    pub(super) graph_reached_callback: Option<&'a mut PreparedPositionGraphReachedCallback<'a>>,
    pub(super) graph_plan_callback: Option<&'a mut PreparedPositionGraphPlanCallback<'a>>,
    pub(super) stop_callback: Option<&'a mut PreparedPositionStopCallback<'a>>,
    pub(super) cohort_callback: Option<&'a mut PreparedPositionCohortCallback<'a>>,
}
impl<'a> PreparedPositionStageMatcher<'a> {
    pub(in super::super) fn disabled() -> Self {
        Self {
            callback: None,
            graph_callback: None,
            graph_reached_callback: None,
            graph_plan_callback: None,
            stop_callback: None,
            cohort_callback: None,
        }
    }
    pub(in super::super) fn enabled(callback: &'a mut PreparedPositionStageCallback<'a>) -> Self {
        Self {
            callback: Some(callback),
            graph_callback: None,
            graph_reached_callback: None,
            graph_plan_callback: None,
            stop_callback: None,
            cohort_callback: None,
        }
    }
    pub(in super::super) fn enabled_graph(
        callback: &'a mut PreparedPositionGraphCallback<'a>,
    ) -> Self {
        Self {
            callback: None,
            graph_callback: Some(callback),
            graph_reached_callback: None,
            graph_plan_callback: None,
            stop_callback: None,
            cohort_callback: None,
        }
    }
    pub(in super::super) fn with_graph_reached(
        mut self,
        callback: Option<&'a mut PreparedPositionGraphReachedCallback<'a>>,
    ) -> Self {
        self.graph_reached_callback = callback;
        self
    }
    pub(in super::super) fn with_resume_plan(
        mut self,
        plan: &'a mut PreparedPositionGraphPlanCallback<'a>,
        stop: &'a mut PreparedPositionStopCallback<'a>,
        cohort: &'a mut PreparedPositionCohortCallback<'a>,
    ) -> Self {
        self.graph_plan_callback = Some(plan);
        self.stop_callback = Some(stop);
        self.cohort_callback = Some(cohort);
        self
    }
    pub(in super::super) fn completed_stop(&mut self, depth: usize) {
        if let Some(callback) = self.stop_callback.as_deref_mut() {
            callback(depth);
        }
    }
    pub(in super::super) fn graph_reached_is_enabled(&self) -> bool {
        self.graph_reached_callback.is_some()
    }
    pub(in super::super) fn graph_is_enabled(&self) -> bool {
        self.graph_callback.is_some() || self.graph_plan_callback.is_some()
    }
    pub(in super::super) fn match_graph(
        &mut self,
        route: &PreparedPositionGraphRoute,
    ) -> Result<Option<GraphOperationOperand>, TransitionError> {
        match &mut self.graph_callback {
            Some(callback) => callback(route),
            None => Ok(None),
        }
    }
    pub(in super::super) fn is_enabled(&self) -> bool {
        self.callback.is_some()
    }
    pub(in super::super) fn match_route(
        &mut self,
        route: &PreparedPositionStageRoute,
    ) -> Result<Option<position_segment::PositionSegmentOperand>, TransitionError> {
        match &mut self.callback {
            Some(callback) => callback(route),
            None => Ok(None),
        }
    }
}
