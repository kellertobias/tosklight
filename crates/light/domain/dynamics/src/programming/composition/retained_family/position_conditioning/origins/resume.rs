//! Original Resume boundaries retain the exact lexical use and enclosing operand goal.
use super::super::super::{
    PreparedPositionGraphRoute, PreparedPositionStageUse, PreparedSourceOrigin,
};
use super::stage::{OriginalSource, OriginalUse};
use super::*;

pub(in crate::programming::composition) const MAX_RESUME_GOALS: usize = 16;
pub(in crate::programming::composition) const MAX_RESUME_MEMBERS: usize = 256;
#[derive(Clone, Eq, Hash, PartialEq)]
pub(in crate::programming::composition) enum ResumeGoal {
    Full,
    Graph(PositionGraphOperationLocator, PositionGraphOperationOperand),
    Stage(PositionStageLocator, PositionStageOperand),
    Resume(Box<PositionResumeOperandLocator>, PositionResumeEndpoint),
}
impl ResumeGoal {
    fn inside_cohort_members(&self, cohort: &PositionSourceNode) -> bool {
        let in_member = |node: &PositionSourceNode| {
            node.source_index == cohort.source_index
                && matches!((node.node, cohort.node), (NodeLocation::MemberTape(forest, _, _), NodeLocation::Forest(other)) if forest == other)
        };
        match self {
            Self::Full => false,
            Self::Graph(locator, _) => in_member(locator.operation_node()),
            Self::Stage(locator, _) => locator.inside_cohort_members(cohort),
            Self::Resume(locator, _) => locator.cohort.is_none() && in_member(&locator.node),
        }
    }
}
#[derive(Clone)]
pub(in crate::programming::composition) enum ResumeStop {
    Graph(PositionGraphOperationLocator, PositionGraphOperationOperand),
    Stage(PositionStageLocator, PositionStageOperand),
    Resume(PositionResumeOperandLocator, PositionResumeEndpoint),
}
#[derive(Clone, Eq, Hash, PartialEq)]
struct CohortBoundary {
    node: PositionSourceNode,
    members: Vec<PositionSourceNode>,
}
#[derive(Clone)]
pub struct PositionResumeOperandLocator {
    captured: Arc<CapturedProgram>,
    uses: Vec<OriginalUse>,
    trigger: Vec<OriginalSource>,
    node: PositionSourceNode,
    scope: PositionResumeScope,
    goal: ResumeGoal,
    cohort: Option<CohortBoundary>,
}
impl PartialEq for PositionResumeOperandLocator {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.captured, &other.captured)
            && self.uses == other.uses
            && self.trigger == other.trigger
            && self.node == other.node
            && self.scope == other.scope
            && self.goal == other.goal
            && self.cohort == other.cohort
    }
}
impl Eq for PositionResumeOperandLocator {}
impl Hash for PositionResumeOperandLocator {
    fn hash<H: Hasher>(&self, h: &mut H) {
        Arc::as_ptr(&self.captured).hash(h);
        self.uses.hash(h);
        self.trigger.hash(h);
        self.node.hash(h);
        self.scope.hash(h);
        self.goal.hash(h);
        self.cohort.hash(h);
    }
}
impl std::fmt::Debug for PositionResumeOperandLocator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PositionResumeOperandLocator")
            .field("capture_id", &self.capture_id())
            .field("scope", &self.scope)
            .field("lexical_depth", &self.uses.len())
            .finish_non_exhaustive()
    }
}
impl PositionResumeOperandLocator {
    pub fn capture_id(&self) -> Uuid {
        self.captured.capture_id
    }
    pub fn scope(&self) -> PositionResumeScope {
        self.scope
    }
    pub fn operation_node(&self) -> &PositionSourceNode {
        &self.node
    }
    pub fn source_node(&self) -> &PositionSourceNode {
        &self.node
    }
    pub(in crate::programming::composition) fn enclosing_stage_active(
        locator: &PositionStageLocator,
        branch: &PositionProgramBranch,
    ) -> Result<bool, TransitionError> {
        locator.active_in_branch(branch)
    }
    pub(in crate::programming::composition) fn same_boundary(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.captured, &other.captured)
            && self.uses == other.uses
            && self.trigger == other.trigger
            && self.node == other.node
            && self.scope == other.scope
            && self.cohort == other.cohort
    }
    pub(in crate::programming::composition) fn is_cohort(&self) -> bool {
        self.cohort.is_some()
    }
    pub(in crate::programming::composition) fn stops(
        &self,
        endpoint: PositionResumeEndpoint,
    ) -> Result<Vec<ResumeStop>, TransitionError> {
        let mut pending = Vec::new();
        let mut goal = &self.goal;
        loop {
            ensure(
                pending.len() < MAX_RESUME_GOALS,
                "Position Resume goal chain limit exceeded",
            )?;
            match goal {
                ResumeGoal::Full => break,
                ResumeGoal::Graph(locator, operand) => {
                    pending.push(ResumeStop::Graph(locator.clone(), *operand));
                    break;
                }
                ResumeGoal::Stage(locator, operand) => {
                    pending.push(ResumeStop::Stage(locator.clone(), *operand));
                    break;
                }
                ResumeGoal::Resume(locator, operand) => {
                    pending.push(ResumeStop::Resume((**locator).clone(), *operand));
                    goal = &locator.goal;
                }
            }
        }
        pending.reverse();
        ensure(
            pending.len() < MAX_RESUME_GOALS,
            "Position Resume goal chain limit exceeded",
        )?;
        pending.push(ResumeStop::Resume(self.clone(), endpoint));
        Ok(pending)
    }
}
impl PositionOriginBinding {
    pub(in crate::programming::composition) fn resume_locator(
        &self,
        route: &PreparedPositionGraphRoute,
        goal: ResumeGoal,
    ) -> Result<Option<PositionResumeOperandLocator>, TransitionError> {
        if route.kind != super::super::super::GraphOperationKind::Resume {
            return Ok(None);
        }
        let Some((uses, trigger, node)) = self.graph_identity(route)? else {
            return Ok(None);
        };
        let registry = CapturedPositionProgram {
            captured: self.captured.clone(),
        };
        let Some(scope) = registry.resume_scope(&node)? else {
            return Ok(None);
        };
        let cohort = match node.node {
            NodeLocation::MemberTape(forest, _, _) => {
                let Some(OriginalUse::SourceCohort { node: cohort, .. }) = uses
                    .iter()
                    .rev()
                    .find(|entry| matches!(entry, OriginalUse::SourceCohort { .. }))
                else {
                    return Ok(None);
                };
                if cohort.source_index != node.source_index
                    || cohort.node != NodeLocation::Forest(forest)
                {
                    return Ok(None);
                }
                let NodeDescription::SourceCohort {
                    members: original_members,
                } = nodes::describe(&self.captured, cohort.source_index, cohort.node)?
                else {
                    return Ok(None);
                };
                if original_members.len() > MAX_RESUME_MEMBERS {
                    return Ok(None);
                }
                let members = registry.resume_nodes(scope)?;
                if members.len() > MAX_RESUME_MEMBERS || members.iter().any(|member| member.source_index != node.source_index || !matches!(member.node, NodeLocation::MemberTape(other, _, _) if other == forest)) { return Ok(None); }
                if goal.inside_cohort_members(cohort) {
                    None
                } else {
                    Some(CohortBoundary {
                        node: cohort.clone(),
                        members,
                    })
                }
            }
            NodeLocation::Tape(_) | NodeLocation::WholeTape(_, _) | NodeLocation::Forest(_) => None,
            _ => return Ok(None),
        };
        let locator = PositionResumeOperandLocator {
            captured: self.captured.clone(),
            uses,
            trigger,
            node,
            scope,
            goal,
            cohort,
        };
        locator.stops(PositionResumeEndpoint::Outgoing)?;
        Ok(Some(locator))
    }
    pub(in crate::programming::composition) fn resume_cohort_matches(
        &self,
        locator: &PositionResumeOperandLocator,
        uses: &[PreparedPositionStageUse],
        origins: &[PreparedSourceOrigin],
        expression: &Arc<CompiledCoupledExpression>,
        endpoints: &[usize],
    ) -> Result<bool, TransitionError> {
        let Some(boundary) = &locator.cohort else {
            return Ok(false);
        };
        if endpoints.len() != 1 {
            return Ok(false);
        }
        let Some((mut mapped, consumer, scope)) = self.lexical_route(uses, origins)? else {
            return Ok(false);
        };
        if scope.is_some() || consumer.len() != 1 {
            return Ok(false);
        }
        let consumer = consumer.into_iter().next().unwrap();
        let Some(location) = Self::coupled_location(expression, endpoints[0]) else {
            return Ok(false);
        };
        let Some(node) = self.handle(self.source(consumer.slot)?, location)? else {
            return Ok(false);
        };
        mapped.push(OriginalUse::SourceCohort {
            consumer,
            node: node.clone(),
        });
        let Some(index) = locator
            .uses
            .iter()
            .rposition(|entry| matches!(entry, OriginalUse::SourceCohort { .. }))
        else {
            return Ok(false);
        };
        Ok(node == boundary.node && mapped == locator.uses[..=index])
    }
}
impl PositionProgramBranch {
    pub fn validate_resume_operand(
        &self,
        locator: &PositionResumeOperandLocator,
        endpoint: PositionResumeEndpoint,
    ) -> Result<(), TransitionError> {
        ensure(
            Arc::ptr_eq(&self.captured, &locator.captured),
            "Position Resume operand belongs to another captured registry",
        )?;
        let registry = CapturedPositionProgram {
            captured: self.captured.clone(),
        };
        ensure(
            registry.resume_scope(&locator.node)? == Some(locator.scope),
            "Position Resume operand has another original scope",
        )?;
        locator.stops(endpoint)?;
        Ok(())
    }
    pub(in crate::programming::composition) fn prepare_resume_cohort(
        &mut self,
        locator: &PositionResumeOperandLocator,
        endpoint: PositionResumeEndpoint,
    ) -> Result<bool, TransitionError> {
        let Some(boundary) = &locator.cohort else {
            return Ok(true);
        };
        let active = match self.resume_scope_membership(locator.scope)? {
            PositionResumeScopeMembership::Active(nodes) => nodes,
            PositionResumeScopeMembership::Absent | PositionResumeScopeMembership::Inactive => {
                return Ok(false);
            }
        };
        ensure(
            active.len() <= MAX_RESUME_MEMBERS,
            "Position Resume cohort member limit exceeded",
        )?;
        ensure(
            active.iter().all(|node| boundary.members.contains(node)),
            "Position Resume has an ambiguous atomic cohort boundary",
        )?;
        self.choose_resume(locator.scope, endpoint)?;
        for node in active {
            let NodeLocation::MemberTape(forest, member, root) = node.node else {
                return Err(IntentError(
                    "Position Resume atomic boundary is not a member tape".into(),
                )
                .into());
            };
            let selection = MemberRootSelection {
                source_index: node.source_index,
                forest,
                member,
                root,
            };
            if let Some(old) = self.member_roots.iter_mut().find(|old| {
                old.source_index == selection.source_index
                    && old.forest == forest
                    && old.member == member
            }) {
                *old = selection;
            } else {
                self.member_roots.push(selection);
            }
        }
        Ok(true)
    }
}
