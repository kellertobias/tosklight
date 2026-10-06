//! Compile original Position forest nodes, in forest order, into one immutable coupled graph
//! while recording the cold lineage that maps each original node onto its compiled root.
use super::*;

/// Graph under construction. `mapped` holds each compiled forest node's graph root, so a
/// Transition can only reference earlier original nodes (already checked by the shape pass).
pub(super) struct PositionForestCompilation {
    pub(super) nodes: Vec<Node>,
    pub(super) position_node_origins: Vec<Option<PositionCompiledNodeLocation>>,
    pub(super) leaf_sources: Vec<Arc<[CoupledComponentEndpoint]>>,
    pub(super) mapped: Vec<usize>,
    pub(super) wholes: Vec<PositionForestWholeLineage>,
    pub(super) cohort_members: Vec<PositionForestCohortMemberLineage>,
}

impl PositionForestCompilation {
    pub(super) fn new(forest_len: usize) -> Self {
        // Sized for one graph node per forest node (TL-639 round 7); a Whole branch reserves its
        // imported nodes.
        let mut nodes = Vec::with_capacity(forest_len + 1);
        nodes.push(Node::Underlay);
        let mut position_node_origins = Vec::with_capacity(forest_len + 1);
        position_node_origins.push(None);
        let mut leaf_sources: Vec<Arc<[CoupledComponentEndpoint]>> =
            Vec::with_capacity(forest_len + 1);
        leaf_sources.push(Arc::from([]));
        let mapped = Vec::with_capacity(forest_len);
        let wholes = Vec::new();
        let cohort_members = Vec::new();
        Self {
            nodes,
            position_node_origins,
            leaf_sources,
            mapped,
            wholes,
            cohort_members,
        }
    }

    /// Compile one original forest node and return its compiled graph root.
    pub(super) fn compile_node(
        &mut self,
        forest_node: usize,
        branch: &PositionForestNode,
    ) -> Result<usize, TransitionError> {
        let root = match branch {
            PositionForestNode::AnglePair(pair) => {
                pair.validate()?;
                let root = self.nodes.len();
                self.nodes.push(Node::AnglePair(pair.clone()));
                self.position_node_origins
                    .push(Some(PositionCompiledNodeLocation::Forest(forest_node)));
                self.leaf_sources.push(Arc::from([]));
                root
            }
            PositionForestNode::Whole {
                expression,
                lane_id,
                sources,
            } => self.whole(forest_node, expression, lane_id, sources)?,
            PositionForestNode::Cohort(components) => {
                validate_cohort(components)?;
                self.cohort_members
                    .extend(components.iter().enumerate().map(|(member_index, member)| {
                        PositionForestCohortMemberLineage {
                            forest_node,
                            member_index,
                            lane_id: member.lane_id,
                            whole_tape: None,
                        }
                    }));
                let root = self.nodes.len();
                self.nodes
                    .push(Node::Cohort(CoupledBaseEndpoint::Components(
                        components.clone(),
                    )));
                self.position_node_origins
                    .push(Some(PositionCompiledNodeLocation::Forest(forest_node)));
                self.leaf_sources.push(components.clone());
                root
            }
            PositionForestNode::SourceCohort(sources) => {
                self.source_cohort(forest_node, sources)?
            }
            PositionForestNode::Transition {
                from,
                to,
                progress,
                reason,
            } => {
                let from = from.map_or(0, |id| self.mapped[id]);
                let to = to.map_or(0, |id| self.mapped[id]);
                if *progress == 0.0 {
                    from
                } else if *progress == 1.0 {
                    to
                } else {
                    let root = self.nodes.len();
                    self.nodes.push(Node::Transition {
                        from,
                        to,
                        progress: *progress,
                        reason: *reason,
                    });
                    self.position_node_origins
                        .push(Some(PositionCompiledNodeLocation::Forest(forest_node)));
                    self.leaf_sources.push(Arc::from([]));
                    root
                }
            }
        };
        Ok(root)
    }

    /// Import a Whole branch's retained tape behind the current graph, remapping its node
    /// references and attributing each authored leaf to the branch's sources.
    fn whole(
        &mut self,
        forest_node: usize,
        expression: &Arc<DynamicSampleExpression>,
        lane_id: &uuid::Uuid,
        sources: &Arc<[CoupledComponentEndpoint]>,
    ) -> Result<usize, TransitionError> {
        let tape = Arc::new(RetainedExpressionTape::from_roots(std::slice::from_ref(
            expression,
        ))?);
        let root = tape.roots[0];
        let mut owner = None;
        preflight(&tape, root, &mut owner)?;
        if owner != Some(ProgrammingOwner::Position) {
            return Err(TransitionError::Requires(
                TransitionRequirement::CompatibleOwners,
            ));
        }
        let (imported, imported_root, original_map, imported_origins) =
            compile(&tape, root, None, &mut Vec::new())?;
        let offset = self.nodes.len() - 1;
        let remap = |id: usize| if id == 0 { 0 } else { id + offset };
        // The branch's own node is already counted; its other imported nodes are not.
        let extra = imported.len().saturating_sub(2);
        self.nodes.reserve(extra);
        self.position_node_origins.reserve(extra);
        self.leaf_sources.reserve(extra);
        self.wholes.push(PositionForestWholeLineage {
            forest_node,
            lane_id: *lane_id,
            tape: Arc::clone(&tape),
            tape_to_compiled: original_map
                .into_iter()
                .map(|id| id.map(remap))
                .collect::<Vec<_>>()
                .into(),
        });
        for (mut node, origin) in imported.into_iter().zip(imported_origins).skip(1) {
            match &mut node {
                Node::Transition { from, to, .. } => {
                    *from = remap(*from);
                    *to = remap(*to);
                }
                Node::Scale { value, .. } => *value = remap(*value),
                _ => {}
            }
            let provenance: Arc<[CoupledComponentEndpoint]> = match &node {
                Node::Leaf {
                    address,
                    value,
                    role,
                    occurrence,
                    dependency_occurrence,
                } => {
                    if sources.is_empty() {
                        Arc::from([CoupledComponentEndpoint {
                            lane_id: *lane_id,
                            address: address.clone(),
                            value: value.clone(),
                            role: *role,
                            occurrence: *occurrence,
                            dependency_occurrence: dependency_occurrence.clone(),
                        }])
                    } else {
                        sources.clone()
                    }
                }
                // A Size baseline is a calculation dependency, never an authored
                // whole-family replacement. Its ID travels on the Scale node.
                _ => Arc::from([]),
            };
            self.nodes.push(node);
            self.position_node_origins.push(
                origin.map(|node| PositionCompiledNodeLocation::WholeTape(forest_node, node)),
            );
            self.leaf_sources.push(provenance);
        }
        Ok(remap(imported_root))
    }

    /// Validate every SourceCohort member's owner and record its lineage, then add the cohort.
    fn source_cohort(
        &mut self,
        forest_node: usize,
        sources: &Arc<[CoupledCohortEndpoint]>,
    ) -> Result<usize, TransitionError> {
        if sources.is_empty() {
            return Err(IntentError("Position source cohort is empty".into()).into());
        }
        for (member_index, source) in sources.iter().enumerate() {
            let whole_tape = match source {
                CoupledCohortEndpoint::Materialized(source) => {
                    if source.address.address().owner() != ProgrammingOwner::Position {
                        return Err(TransitionError::Requires(
                            TransitionRequirement::CompatibleOwners,
                        ));
                    }
                    source.address.validate_source_value(&source.value)?;
                    None
                }
                CoupledCohortEndpoint::WholeExpression { expression, .. } => {
                    if expression.owner() != ProgrammingOwner::Position {
                        return Err(TransitionError::Requires(
                            TransitionRequirement::CompatibleOwners,
                        ));
                    }
                    Some(Arc::new(RetainedExpressionTape::from_roots(&[Arc::new(
                        expression.expression().clone(),
                    )])?))
                }
            };
            self.cohort_members.push(PositionForestCohortMemberLineage {
                forest_node,
                member_index,
                lane_id: source.lane_id(),
                whole_tape,
            });
        }
        let root = self.nodes.len();
        self.nodes
            .push(Node::Cohort(CoupledBaseEndpoint::Sources(sources.clone())));
        self.position_node_origins
            .push(Some(PositionCompiledNodeLocation::Forest(forest_node)));
        self.leaf_sources.push(Arc::from([]));
        Ok(root)
    }
}
