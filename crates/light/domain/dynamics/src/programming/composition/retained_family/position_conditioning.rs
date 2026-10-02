//! Cold, captured Position branch views. Original source slots, forest topology and opaque
//! subtree handles retain captured Current and source membership; fitting cannot author them.
use super::*;
use crate::programming::expression_coupled::{PositionForestLineage, PositionForestNode};
use crate::{
    DynamicTransitionReason, RetainedExpressionNode, RetainedExpressionTape, RetainedNodeId,
};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
mod forest;
mod forest_origins;
mod nodes;
pub(super) mod origins;
mod point_dependencies;
use nodes::{NodeDescription, NodeLocation};
pub use origins::{
    PositionResumeOperandLocator, PositionStageKind, PositionStageLocator, PositionStageOperand,
};
pub use point_dependencies::PositionPointDependencies;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PositionResumeScope {
    pub instance_id: Uuid,
    pub controller_id: Uuid,
    pub occurrence_id: Uuid,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PositionResumeEndpoint {
    Outgoing,
    Incoming,
}

/// Membership is evaluated against the actual selected original subtree and Resume ancestry.
/// An absent scope never authorizes a choice; an inactive scope must not revive its history.
#[derive(Clone, Eq, PartialEq)]
pub enum PositionResumeScopeMembership {
    Absent,
    Inactive,
    Active(Vec<PositionSourceNode>),
}

/// An exact original operation selection, local to one immutable Position registry.
/// These choices do not correlate other owners as a shared Resume occurrence would.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PositionLocalEndpoint {
    RequiredOutgoing,
    RequiredIncoming,
    ScaleBaseline,
    ScaleValue,
}
#[derive(Clone, Copy)]
pub(super) struct LocalNodeSelection {
    source_index: usize,
    node: NodeLocation,
    endpoint: PositionLocalEndpoint,
}

#[derive(Clone, Copy)]
pub(super) struct MemberRootSelection {
    source_index: usize,
    forest: usize,
    member: usize,
    root: RetainedNodeId,
}

fn tape_local_endpoints(
    selections: &[LocalNodeSelection],
    source_index: usize,
    prefix: NodeLocation,
) -> Vec<(RetainedNodeId, PositionLocalEndpoint)> {
    selections
        .iter()
        .filter_map(|selection| {
            if selection.source_index != source_index {
                return None;
            }
            let node = match (prefix, selection.node) {
                (NodeLocation::Tape(_), NodeLocation::Tape(node)) => node,
                (NodeLocation::WholeTape(a, _), NodeLocation::WholeTape(b, node)) if a == b => node,
                (NodeLocation::MemberTape(a, am, _), NodeLocation::MemberTape(b, bm, node))
                    if a == b && am == bm =>
                {
                    node
                }
                _ => return None,
            };
            Some((node, selection.endpoint))
        })
        .collect()
}

struct CapturedProgram {
    capture_id: Uuid,
    base: AttributeValue,
    sources: Arc<[FamilyCompositionSample]>,
    tapes: Vec<Option<Arc<RetainedExpressionTape>>>,
    forests: Vec<Option<Arc<PositionForestLineage>>>,
    active: Vec<HashSet<NodeLocation>>,
    point_dependencies: PositionPointDependencies,
}

/// Constructing another registry, even with equal input and capture IDs, creates another scope.
#[derive(Clone)]
pub struct CapturedPositionProgram {
    captured: Arc<CapturedProgram>,
}

/// Only original reachable nodes of this exact captured program can authorize replacements.
/// Compiled request indices cannot construct these handles.
#[derive(Clone)]
pub struct PositionSourceNode {
    captured: Arc<CapturedProgram>,
    source_index: usize,
    node: NodeLocation,
}
impl PartialEq for PositionSourceNode {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.captured, &other.captured)
            && self.source_index == other.source_index
            && self.node == other.node
    }
}
impl Eq for PositionSourceNode {}
impl Hash for PositionSourceNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.captured).hash(state);
        self.source_index.hash(state);
        self.node.hash(state);
    }
}
impl PositionSourceNode {
    pub fn source_index(&self) -> usize {
        self.source_index
    }
    pub fn capture_id(&self) -> Uuid {
        self.captured.capture_id
    }
}
#[derive(Clone)]
pub enum PositionSourceNodeView {
    /// Captured Angle pairs remain atomic; no independent Pan/Tilt child override is exposed.
    Leaf,
    Transition {
        progress: f32,
        reason: DynamicTransitionReason,
        from: Option<PositionSourceNode>,
        to: Option<PositionSourceNode>,
    },
    Scale {
        factor: f32,
        value: Option<PositionSourceNode>,
    },
    Whole {
        lane_id: Uuid,
        root: PositionSourceNode,
    },
    SourceCohort {
        members: Vec<PositionSourceNode>,
    },
}

/// Source routing for one deferred operation in this exact immutable program. Original
/// slot aggregates are not mechanical-cut proof; only a validated original node can witness
/// retained history. Synthetic masks, completion, activation and underlay have no such node.
#[derive(Clone)]
pub struct PositionCompositionOrigin {
    captured: Arc<CapturedProgram>,
    original_indices: Vec<usize>,
    node: Option<PositionSourceNode>,
}
impl PositionCompositionOrigin {
    pub fn capture_id(&self) -> Uuid {
        self.captured.capture_id
    }
    pub fn original_source_indices(&self) -> &[usize] {
        &self.original_indices
    }
}
impl CapturedPositionProgram {
    pub fn source_node_for_origin(
        &self,
        origin: &PositionCompositionOrigin,
    ) -> Result<Option<PositionSourceNode>, TransitionError> {
        ensure(
            Arc::ptr_eq(&self.captured, &origin.captured),
            "Position request origin belongs to another captured program",
        )?;
        ensure(
            origin
                .original_indices
                .iter()
                .all(|&index| index < self.source_count()),
            "Position request origin source is absent",
        )?;
        if let Some(node) = &origin.node {
            validate_handle(&self.captured, node)?;
        }
        Ok(origin.node.clone())
    }
}

/// Released source slots remain indexed holes. State is private and never persisted.
#[derive(Clone)]
pub struct PositionProgramBranch {
    captured: Arc<CapturedProgram>,
    state: conditioning::BranchState,
    selected_nodes: Vec<(usize, Option<NodeLocation>)>,
    local_endpoints: Vec<LocalNodeSelection>,
    member_roots: Vec<MemberRootSelection>,
}
impl CapturedPositionProgram {
    pub fn new(
        capture_id: Uuid,
        base: &AttributeValue,
        sources: &[FamilyCompositionSample],
    ) -> Result<Self, TransitionError> {
        ensure(
            !capture_id.is_nil(),
            "Position branch registry requires a captured frame identity",
        )?;
        position_completion::validate_position(base)?;
        let mut captured = CapturedProgram {
            capture_id,
            base: base.clone(),
            sources: sources.into(),
            tapes: Vec::with_capacity(sources.len()),
            forests: Vec::with_capacity(sources.len()),
            active: Vec::with_capacity(sources.len()),
            point_dependencies: PositionPointDependencies::default(),
        };
        let mut participates = Vec::with_capacity(sources.len());
        for source in sources {
            source.rank().validate()?;
            let (owner, activation, root, forest, active) = match source {
                FamilyCompositionSample::Known(known) => {
                    known.validate_value()?;
                    let root = match &known.body {
                        FamilySampleBody::ComponentExpression(expression) => {
                            Some(Arc::new(expression.expression().clone()))
                        }
                        FamilySampleBody::Materialized(_) => None,
                    };
                    (
                        known.address.address().owner(),
                        known.activation_mix,
                        root,
                        None,
                        known.participates(),
                    )
                }
                FamilyCompositionSample::WholeExpression {
                    expression,
                    activation_mix,
                    ..
                } => (
                    expression.owner(),
                    *activation_mix,
                    Some(Arc::new(expression.expression().clone())),
                    None,
                    *activation_mix > 0.0 && expression.participates(),
                ),
                FamilyCompositionSample::CoupledExpression {
                    expression,
                    activation_mix,
                    ..
                } => (
                    expression.owner(),
                    *activation_mix,
                    expression.expression().cloned(),
                    expression.position_forest_lineage().cloned(),
                    *activation_mix > 0.0 && !expression.roles().is_empty(),
                ),
            };
            ensure(
                owner == ProgrammingOwner::Position,
                "Position branch registry contains another owner",
            )?;
            ensure(
                activation.is_finite() && (0.0..=1.0).contains(&activation),
                "Position branch activation must be between zero and one",
            )?;
            captured.tapes.push(
                root.map(|root| RetainedExpressionTape::from_roots(&[root]).map(Arc::new))
                    .transpose()?,
            );
            captured.forests.push(forest);
            participates.push(active);
        }
        let empty = conditioning::BranchState::default();
        let mut progress = HashMap::new();
        for (index, active) in participates.into_iter().enumerate() {
            let nodes = if active {
                nodes::active_nodes(&captured, index, nodes::root(&captured, index), &empty)?
            } else {
                HashSet::new()
            };
            for &node in &nodes {
                if let NodeDescription::Transition {
                    progress: value,
                    reason,
                    ..
                } = nodes::describe(&captured, index, node)?
                {
                    ensure(
                        value.is_finite() && (0.0..=1.0).contains(&value),
                        "Position retained progress is invalid",
                    )?;
                    if let DynamicTransitionReason::Resume { occurrence_id } = reason {
                        ensure(!occurrence_id.is_nil(), "Position Resume occurrence is nil")?;
                        let identity = nodes::rank(&captured, index, node)?
                            .dynamic_identity()
                            .ok_or_else(|| {
                                IntentError(
                                    "Position Resume requires an actual Dynamic identity".into(),
                                )
                            })?;
                        let scope = PositionResumeScope {
                            instance_id: identity.instance_id,
                            controller_id: identity.controller_id,
                            occurrence_id,
                        };
                        if let Some(old) = progress.insert(scope, value) {
                            ensure(
                                old == value,
                                "one scoped Resume occurrence has different progress",
                            )?;
                        }
                    }
                }
            }
            captured.active.push(nodes);
        }
        captured.point_dependencies = point_dependencies::collect(&captured)?;
        Ok(Self {
            captured: Arc::new(captured),
        })
    }
    /// Conservative structural dependencies of the original captured program, including
    /// hidden and retained branches. Conditioning does not remove this cold census.
    pub fn point_dependencies(&self) -> &PositionPointDependencies {
        &self.captured.point_dependencies
    }
    pub fn capture_id(&self) -> Uuid {
        self.captured.capture_id
    }
    pub fn source_count(&self) -> usize {
        self.captured.sources.len()
    }
    pub fn source_rank(&self, index: usize) -> Result<FamilySampleRank, TransitionError> {
        self.captured
            .sources
            .get(index)
            .map(FamilyCompositionSample::rank)
            .ok_or_else(|| IntentError("Position source index is absent".into()).into())
    }
    /// Nested whole/cohort members use their actual original lane, not the outer representative.
    pub fn node_source_rank(
        &self,
        node: &PositionSourceNode,
    ) -> Result<FamilySampleRank, TransitionError> {
        validate_handle(&self.captured, node)?;
        nodes::rank(&self.captured, node.source_index, node.node)
    }
    /// Equal capture UUIDs do not make independently constructed registries interchangeable.
    pub fn validate_branch(&self, branch: &PositionProgramBranch) -> Result<(), TransitionError> {
        ensure(
            Arc::ptr_eq(&self.captured, &branch.captured),
            "Position branch belongs to another captured program",
        )?;
        Ok(())
    }
    pub fn branch(&self) -> PositionProgramBranch {
        PositionProgramBranch {
            captured: self.captured.clone(),
            state: Default::default(),
            selected_nodes: Vec::new(),
            local_endpoints: Vec::new(),
            member_roots: Vec::new(),
        }
    }
    pub fn source_root(&self, index: usize) -> Result<Option<PositionSourceNode>, TransitionError> {
        ensure(
            index < self.source_count(),
            "Position source index is absent",
        )?;
        Ok(nodes::root(&self.captured, index)
            .filter(|node| self.captured.active[index].contains(node))
            .map(|node| self.handle(index, node)))
    }
    fn handle(&self, source_index: usize, node: NodeLocation) -> PositionSourceNode {
        PositionSourceNode {
            captured: self.captured.clone(),
            source_index,
            node,
        }
    }
    pub fn node_view(
        &self,
        handle: &PositionSourceNode,
    ) -> Result<PositionSourceNodeView, TransitionError> {
        validate_handle(&self.captured, handle)?;
        let index = handle.source_index;
        let child = |node: Option<NodeLocation>| {
            node.filter(|node| self.captured.active[index].contains(node))
                .map(|node| self.handle(index, node))
        };
        Ok(match nodes::describe(&self.captured, index, handle.node)? {
            NodeDescription::Leaf => PositionSourceNodeView::Leaf,
            NodeDescription::Transition {
                progress,
                reason,
                from,
                to,
            } => PositionSourceNodeView::Transition {
                progress,
                reason,
                from: child(from),
                to: child(to),
            },
            NodeDescription::Scale { factor, value } => PositionSourceNodeView::Scale {
                factor,
                value: child(value),
            },
            NodeDescription::Whole { lane_id, root } => PositionSourceNodeView::Whole {
                lane_id,
                root: self.handle(index, root),
            },
            NodeDescription::SourceCohort { members } => PositionSourceNodeView::SourceCohort {
                members: members
                    .into_iter()
                    .filter_map(|node| child(Some(node)))
                    .collect(),
            },
        })
    }
    pub fn node_expression(
        &self,
        node: &PositionSourceNode,
    ) -> Result<Arc<DynamicSampleExpression>, TransitionError> {
        validate_handle(&self.captured, node)?;
        nodes::expression(&self.captured, node.source_index, node.node)
    }
    pub fn resume_scope(
        &self,
        node: &PositionSourceNode,
    ) -> Result<Option<PositionResumeScope>, TransitionError> {
        validate_handle(&self.captured, node)?;
        let NodeDescription::Transition {
            progress,
            reason: DynamicTransitionReason::Resume { occurrence_id },
            ..
        } = nodes::describe(&self.captured, node.source_index, node.node)?
        else {
            return Ok(None);
        };
        if progress <= 0.0 || progress >= 1.0 {
            return Ok(None);
        }
        let identity = self
            .node_source_rank(node)?
            .dynamic_identity()
            .ok_or_else(|| {
                IntentError("Position Resume requires an actual Dynamic identity".into())
            })?;
        Ok(Some(PositionResumeScope {
            instance_id: identity.instance_id,
            controller_id: identity.controller_id,
            occurrence_id,
        }))
    }
    pub fn resume_nodes(
        &self,
        scope: PositionResumeScope,
    ) -> Result<Vec<PositionSourceNode>, TransitionError> {
        let mut found = Vec::new();
        for (index, active) in self.captured.active.iter().enumerate() {
            for &node in active {
                let handle = self.handle(index, node);
                if self.resume_scope(&handle)? == Some(scope) {
                    found.push(handle)
                }
            }
        }
        found.sort_by_key(|node| (node.source_index, node.node));
        Ok(found)
    }
}
impl PositionProgramBranch {
    fn selected_root(&self, index: usize) -> Option<NodeLocation> {
        match self.selected_nodes.iter().find(|(key, _)| *key == index) {
            Some((_, root)) => *root,
            None => nodes::root(&self.captured, index),
        }
    }
    pub(super) fn original_node_active(
        &self,
        node: &PositionSourceNode,
    ) -> Result<bool, TransitionError> {
        validate_handle(&self.captured, node)?;
        Ok(nodes::active_nodes_with_boundary(
            &self.captured,
            node.source_index,
            self.selected_root(node.source_index),
            &self.state,
            &self.local_endpoints,
            &self.member_roots,
        )?
        .contains(&node.node))
    }
    pub fn resume_scope_membership(
        &self,
        scope: PositionResumeScope,
    ) -> Result<PositionResumeScopeMembership, TransitionError> {
        ensure(
            !scope.instance_id.is_nil()
                && !scope.controller_id.is_nil()
                && !scope.occurrence_id.is_nil(),
            "Position Resume scope requires nonnil instance, controller and occurrence identities",
        )?;
        let registry = CapturedPositionProgram {
            captured: self.captured.clone(),
        };
        let original = registry.resume_nodes(scope)?;
        if original.is_empty() {
            return Ok(PositionResumeScopeMembership::Absent);
        }
        let mut reachable = HashMap::new();
        let mut active = Vec::new();
        for node in original {
            if !reachable.contains_key(&node.source_index) {
                reachable.insert(
                    node.source_index,
                    nodes::active_nodes_with_boundary(
                        &self.captured,
                        node.source_index,
                        self.selected_root(node.source_index),
                        &self.state,
                        &self.local_endpoints,
                        &self.member_roots,
                    )?,
                );
            }
            if reachable[&node.source_index].contains(&node.node) {
                active.push(node);
            }
        }
        Ok(if active.is_empty() {
            PositionResumeScopeMembership::Inactive
        } else {
            PositionResumeScopeMembership::Active(active)
        })
    }
    /// Evaluate a Resume operand at its original cut boundary. Outer operations belong to
    /// the retained parent and must not be evaluated again by this child view.
    pub fn at_resume_operand(
        &self,
        scope: PositionResumeScope,
        endpoint: PositionResumeEndpoint,
    ) -> Result<Option<PositionProgramBranch>, TransitionError> {
        let active = match self.resume_scope_membership(scope)? {
            PositionResumeScopeMembership::Absent => return Ok(None),
            PositionResumeScopeMembership::Inactive => {
                return Err(TransitionError::Requires(
                    TransitionRequirement::CompatibleOwners,
                ));
            }
            PositionResumeScopeMembership::Active(active) => active,
        };
        // Additional original source slots require an explicit prefix-composition recipe.
        if self.captured.sources.len() != 1 {
            return Err(TransitionError::Requires(
                TransitionRequirement::CompatibleOwners,
            ));
        }
        let mut child = self.clone();
        match active.as_slice() {
            [node]
                if matches!(
                    node.node,
                    NodeLocation::Tape(_) | NodeLocation::Forest(_) | NodeLocation::WholeTape(_, _)
                ) =>
            {
                child.replace_source(node.source_index, Some(node))?;
            }
            _ => {
                let Some(first) = active.first() else {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::CompatibleOwners,
                    ));
                };
                let NodeLocation::MemberTape(forest, _, _) = first.node else {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::CompatibleOwners,
                    ));
                };
                let mut members = HashSet::new();
                for node in &active {
                    let NodeLocation::MemberTape(other_forest, member, root) = node.node else {
                        return Err(TransitionError::Requires(
                            TransitionRequirement::CompatibleOwners,
                        ));
                    };
                    if node.source_index != first.source_index
                        || other_forest != forest
                        || !members.insert(member)
                    {
                        return Err(TransitionError::Requires(
                            TransitionRequirement::CompatibleOwners,
                        ));
                    }
                    let selection = MemberRootSelection {
                        source_index: node.source_index,
                        forest,
                        member,
                        root,
                    };
                    if let Some(old) = child.member_roots.iter_mut().find(|old| {
                        old.source_index == node.source_index
                            && old.forest == forest
                            && old.member == member
                    }) {
                        *old = selection;
                    } else {
                        child.member_roots.push(selection);
                    }
                }
                let cohort = PositionSourceNode {
                    captured: self.captured.clone(),
                    source_index: first.source_index,
                    node: NodeLocation::Forest(forest),
                };
                // Original nearest SourceCohort retains every atomic sibling and its lane.
                child.replace_source(first.source_index, Some(&cohort))?;
            }
        }
        child.choose_resume(scope, endpoint)?;
        Ok(Some(child))
    }
    /// Exact owner-local cut view identity, independent of compiled graph aliases.
    pub fn operand_boundary_nodes(&self) -> Result<Vec<PositionSourceNode>, TransitionError> {
        let mut boundaries = self
            .selected_nodes
            .iter()
            .filter_map(|(index, root)| root.map(|node| (*index, node)))
            .collect::<Vec<_>>();
        boundaries.extend(self.member_roots.iter().map(|selection| {
            (
                selection.source_index,
                NodeLocation::MemberTape(selection.forest, selection.member, selection.root),
            )
        }));
        boundaries.sort_unstable();
        boundaries.dedup();
        boundaries
            .into_iter()
            .map(|(source_index, node)| {
                let handle = PositionSourceNode {
                    captured: self.captured.clone(),
                    source_index,
                    node,
                };
                validate_handle(&self.captured, &handle)?;
                Ok(handle)
            })
            .collect()
    }
    /// Select an endpoint at this original operation in place. Ancestor envelopes,
    /// sibling members, original slots and captured values remain owned by the registry.
    pub fn select_local_endpoint(
        &mut self,
        node: &PositionSourceNode,
        endpoint: PositionLocalEndpoint,
    ) -> Result<(), TransitionError> {
        validate_handle(&self.captured, node)?;
        ensure(
            nodes::active_nodes_with_boundary(
                &self.captured,
                node.source_index,
                self.selected_root(node.source_index),
                &self.state,
                &self.local_endpoints,
                &self.member_roots,
            )?
            .contains(&node.node),
            "Position local operation is outside the selected original branch",
        )?;
        match (
            nodes::describe(&self.captured, node.source_index, node.node)?,
            endpoint,
        ) {
            (
                NodeDescription::Transition {
                    progress,
                    reason: DynamicTransitionReason::Required { .. },
                    ..
                },
                PositionLocalEndpoint::RequiredOutgoing,
            ) => ensure(progress < 1.0, "outgoing Required endpoint is inactive")?,
            (
                NodeDescription::Transition {
                    progress,
                    reason: DynamicTransitionReason::Required { .. },
                    ..
                },
                PositionLocalEndpoint::RequiredIncoming,
            ) => ensure(progress > 0.0, "incoming Required endpoint is inactive")?,
            (NodeDescription::Scale { .. }, PositionLocalEndpoint::ScaleBaseline) => {}
            (NodeDescription::Scale { factor, .. }, PositionLocalEndpoint::ScaleValue) => {
                ensure(factor != 0.0, "zero Size value history is inactive")?
            }
            _ => {
                return Err(IntentError(
                    "Position local endpoint does not match an original Required or Size operation"
                        .into(),
                )
                .into());
            }
        }
        if let Some(old) = self
            .local_endpoints
            .iter()
            .find(|old| old.source_index == node.source_index && old.node == node.node)
        {
            ensure(
                old.endpoint == endpoint,
                "conflicting local endpoint choices for one original Position operation",
            )?;
            return Ok(());
        }
        let mut candidate = self.local_endpoints.clone();
        candidate.push(LocalNodeSelection {
            source_index: node.source_index,
            node: node.node,
            endpoint,
        });
        for &(index, selected) in &self.selected_nodes {
            if let Some(selected) = selected {
                ensure(
                    nodes::active_nodes_with_boundary(
                        &self.captured,
                        index,
                        nodes::root(&self.captured, index),
                        &self.state,
                        &candidate,
                        &self.member_roots,
                    )?
                    .contains(&selected),
                    "local endpoint would invalidate the selected original source subtree",
                )?;
            }
        }
        self.local_endpoints = candidate;
        Ok(())
    }
    pub fn choose_resume(
        &mut self,
        scope: PositionResumeScope,
        endpoint: PositionResumeEndpoint,
    ) -> Result<(), TransitionError> {
        let registry = CapturedPositionProgram {
            captured: self.captured.clone(),
        };
        let witness = registry
            .resume_nodes(scope)?
            .into_iter()
            .find(|node| {
                nodes::active_nodes_with_boundary(
                    &self.captured,
                    node.source_index,
                    self.selected_root(node.source_index),
                    &self.state,
                    &self.local_endpoints,
                    &self.member_roots,
                )
                .is_ok_and(|nodes| nodes.contains(&node.node))
            })
            .ok_or_else(|| {
                IntentError(
                    "Resume choice has no active interior occurrence in this Position branch"
                        .into(),
                )
            })?;
        let mut candidate = self.state.clone();
        candidate.choose(
            registry.node_source_rank(&witness)?,
            scope.occurrence_id,
            endpoint == PositionResumeEndpoint::Incoming,
        )?;
        for &(index, node) in &self.selected_nodes {
            if let Some(node) = node {
                ensure(
                    nodes::active_nodes_with_boundary(
                        &self.captured,
                        index,
                        nodes::root(&self.captured, index),
                        &candidate,
                        &self.local_endpoints,
                        &self.member_roots,
                    )?
                    .contains(&node),
                    "Resume choice would invalidate the selected original source subtree",
                )?;
            }
        }
        self.state = candidate;
        Ok(())
    }
    pub fn replace_source(
        &mut self,
        index: usize,
        node: Option<&PositionSourceNode>,
    ) -> Result<(), TransitionError> {
        ensure(
            index < self.captured.sources.len(),
            "Position source override index is absent",
        )?;
        if let Some(node) = node {
            validate_handle(&self.captured, node)?;
            ensure(
                node.source_index == index,
                "Position override belongs to another source index",
            )?;
            ensure(
                nodes::active_nodes_with_boundary(
                    &self.captured,
                    index,
                    nodes::root(&self.captured, index),
                    &self.state,
                    &self.local_endpoints,
                    &self.member_roots,
                )?
                .contains(&node.node),
                "Position override is outside the selected Resume branch",
            )?;
        }
        let node = node.map(|node| node.node);
        if let Some((_, old)) = self
            .selected_nodes
            .iter_mut()
            .find(|(key, _)| *key == index)
        {
            *old = node
        } else {
            self.selected_nodes.push((index, node))
        }
        Ok(())
    }
    /// Cold recompilation uses original sources/Current Arcs. Cache views; no producer is rerun.
    pub fn conditioned_sources(
        &self,
    ) -> Result<Vec<Option<FamilyCompositionSample>>, TransitionError> {
        if !self.local_endpoints.is_empty() || !self.member_roots.is_empty() {
            let mut sources = vec![None; self.captured.sources.len()];
            for source in self.conditioned_with_origins()? {
                sources[source.original_index] = Some(source.sample);
            }
            return Ok(sources);
        }
        self.captured
            .sources
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let selected = self.selected_nodes.iter().find(|(key, _)| *key == index);
                if matches!(selected, Some((_, None))) {
                    return Ok(None);
                }
                if self.state.decisions.is_empty() && selected.is_none() {
                    return Ok(Some(source.clone()));
                }
                if let Some(lineage) = &self.captured.forests[index] {
                    return forest::condition(
                        source,
                        lineage,
                        &self.state,
                        selected.and_then(|(_, node)| *node),
                    );
                }
                let mut local = self.state.clone();
                local.overrides.clear();
                if let Some((_, Some(node))) = selected {
                    local.replace(0, Some(nodes::expression(&self.captured, index, *node)?));
                }
                conditioning::condition_sources(
                    std::slice::from_ref(source),
                    &local,
                    &self.captured.base,
                )
                .map(|mut sources| sources.remove(0))
            })
            .collect()
    }
}
fn validate_handle(
    captured: &Arc<CapturedProgram>,
    handle: &PositionSourceNode,
) -> Result<(), TransitionError> {
    ensure(
        Arc::ptr_eq(captured, &handle.captured),
        "Position source node belongs to another captured program",
    )?;
    ensure(
        captured
            .active
            .get(handle.source_index)
            .is_some_and(|active| active.contains(&handle.node)),
        "Position source node is not an active original subtree",
    )?;
    Ok(())
}
#[cfg(test)]
mod forest_tests;
#[cfg(test)]
mod origin_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod batch_identity_tests;

#[cfg(test)]
mod local_conditioning_tests;

#[cfg(test)]
mod scope_boundary_tests;
