//! Controller Position forests are runtime-only graphs. A Target cohort remains a list of
//! authored component leaves until the compositor supplies its eligible reference-matching base.
use super::*;
use crate::programming::expression_family::{
    GraphOperationKind, GraphOperationOperand, GraphOperationPlanCallback, GraphOperationProgress,
    GraphOperationReachedCallback, GraphOperationSelection, GraphReachedAction,
};

#[derive(Clone)]
pub(crate) enum PositionForestNode {
    AnglePair(Arc<PositionAnglePairEndpoint>),
    Whole {
        expression: Arc<DynamicSampleExpression>,
        lane_id: uuid::Uuid,
        sources: Arc<[CoupledComponentEndpoint]>,
    },
    Cohort(Arc<[CoupledComponentEndpoint]>),
    SourceCohort(Arc<[CoupledCohortEndpoint]>),
    Transition {
        from: Option<usize>,
        to: Option<usize>,
        progress: f32,
        reason: crate::DynamicTransitionReason,
    },
}

/// Immutable cold source lineage. Forest IDs and imported tape IDs are original-source
/// indices; compiled IDs are only an explicit routing map and establish no shared cut.
pub(crate) struct PositionForestLineage {
    pub nodes: Arc<[PositionForestNode]>,
    pub root: usize,
    pub compiled_nodes: Arc<[usize]>,
    pub wholes: Arc<[PositionForestWholeLineage]>,
    pub cohort_members: Arc<[PositionForestCohortMemberLineage]>,
}
pub(crate) struct PositionForestWholeLineage {
    pub forest_node: usize,
    pub lane_id: uuid::Uuid,
    pub tape: Arc<RetainedExpressionTape>,
    /// Inactive original nodes are None. Exact aliases may map several original nodes to
    /// the same graph node; zero is the graph's eligible Underlay, not a new source identity.
    pub tape_to_compiled: Arc<[Option<usize>]>,
}
pub(crate) struct PositionForestCohortMemberLineage {
    pub forest_node: usize,
    pub member_index: usize,
    pub lane_id: uuid::Uuid,
    /// Original Whole source subtree for a nested SourceCohort member. Its compiled family
    /// has its own graph IDs; those must never be inferred from the enclosing graph map.
    pub whole_tape: Option<Arc<RetainedExpressionTape>>,
}

impl CompiledCoupledExpression {
    pub(crate) fn from_position_forest(
        retained: Arc<[crate::DynamicRuntimeSample]>,
        forest: &[PositionForestNode],
        forest_root: usize,
    ) -> Result<Self, TransitionError> {
        validate_forest_shape(forest, forest_root)?;
        let mut nodes = vec![Node::Underlay];
        let mut position_node_origins = vec![None];
        let mut leaf_sources: Vec<Arc<[CoupledComponentEndpoint]>> = vec![Arc::from([])];
        let mut mapped = Vec::with_capacity(forest.len());
        let mut wholes = Vec::new();
        let mut cohort_members = Vec::new();
        for (forest_node, branch) in forest.iter().enumerate() {
            let root = match branch {
                PositionForestNode::AnglePair(pair) => {
                    pair.validate()?;
                    let root = nodes.len();
                    nodes.push(Node::AnglePair(pair.clone()));
                    position_node_origins
                        .push(Some(PositionCompiledNodeLocation::Forest(forest_node)));
                    leaf_sources.push(Arc::from([]));
                    root
                }
                PositionForestNode::Whole {
                    expression,
                    lane_id,
                    sources,
                } => {
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
                    let offset = nodes.len() - 1;
                    let remap = |id: usize| if id == 0 { 0 } else { id + offset };
                    wholes.push(PositionForestWholeLineage {
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
                        nodes.push(node);
                        position_node_origins.push(origin.map(|node| {
                            PositionCompiledNodeLocation::WholeTape(forest_node, node)
                        }));
                        leaf_sources.push(provenance);
                    }
                    remap(imported_root)
                }
                PositionForestNode::Cohort(components) => {
                    validate_cohort(components)?;
                    cohort_members.extend(components.iter().enumerate().map(
                        |(member_index, member)| PositionForestCohortMemberLineage {
                            forest_node,
                            member_index,
                            lane_id: member.lane_id,
                            whole_tape: None,
                        },
                    ));
                    let root = nodes.len();
                    nodes.push(Node::Cohort(CoupledBaseEndpoint::Components(
                        components.clone(),
                    )));
                    position_node_origins
                        .push(Some(PositionCompiledNodeLocation::Forest(forest_node)));
                    leaf_sources.push(components.clone());
                    root
                }
                PositionForestNode::SourceCohort(sources) => {
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
                        cohort_members.push(PositionForestCohortMemberLineage {
                            forest_node,
                            member_index,
                            lane_id: source.lane_id(),
                            whole_tape,
                        });
                    }
                    let root = nodes.len();
                    nodes.push(Node::Cohort(CoupledBaseEndpoint::Sources(sources.clone())));
                    position_node_origins
                        .push(Some(PositionCompiledNodeLocation::Forest(forest_node)));
                    leaf_sources.push(Arc::from([]));
                    root
                }
                PositionForestNode::Transition {
                    from,
                    to,
                    progress,
                    reason,
                } => {
                    let from = from.map_or(0, |id| mapped[id]);
                    let to = to.map_or(0, |id| mapped[id]);
                    if *progress == 0.0 {
                        from
                    } else if *progress == 1.0 {
                        to
                    } else {
                        let root = nodes.len();
                        nodes.push(Node::Transition {
                            from,
                            to,
                            progress: *progress,
                            reason: *reason,
                        });
                        position_node_origins
                            .push(Some(PositionCompiledNodeLocation::Forest(forest_node)));
                        leaf_sources.push(Arc::from([]));
                        root
                    }
                }
            };
            mapped.push(root);
        }
        let root = *mapped
            .get(forest_root)
            .ok_or_else(|| IntentError("Position forest has no root".into()))?;
        let (node_roles, needs) = metadata(&nodes);
        let roles = node_roles[root].clone();
        let base_underlay = needs[root];
        let (base_endpoints, base_endpoint_nodes) = collect_base_endpoints(&nodes, root);
        Ok(Self {
            retained: CoupledRetainedSources::PositionForest(retained),
            position_forest_lineage: Some(Arc::new(PositionForestLineage {
                nodes: forest.to_vec().into(),
                root: forest_root,
                compiled_nodes: mapped.into(),
                wholes: wholes.into(),
                cohort_members: cohort_members.into(),
            })),
            position_node_origins,
            owner: ProgrammingOwner::Position,
            nodes,
            root,
            node_roles,
            base_underlay,
            roles,
            orthogonals: Vec::new(),
            base_endpoints,
            base_endpoint_nodes,
            models: Vec::new(),
            leaf_sources,
        })
    }

    pub(crate) fn position_forest_lineage(&self) -> Option<&Arc<PositionForestLineage>> {
        self.position_forest_lineage.as_ref()
    }

    /// Producer-operation provenance of every original lane retained by a Position forest,
    /// in retained order. Forest rebundling never re-stamps an operation: numeric Angle axes
    /// and Whole branches resolve their genuine origins through these retained source lanes.
    /// `None` for an ordinary (non-forest) coupled expression.
    pub fn position_operation_provenance(
        &self,
    ) -> Option<Result<Vec<(uuid::Uuid, crate::DynamicOperationProvenance)>, IntentError>> {
        let CoupledRetainedSources::PositionForest(samples) = &self.retained else {
            return None;
        };
        Some(
            samples
                .iter()
                .map(|sample| {
                    sample
                        .expression
                        .operation_provenance()
                        .map(|provenance| (sample.lane_id, provenance))
                })
                .collect(),
        )
    }
}

fn validate_forest_shape(
    forest: &[PositionForestNode],
    root: usize,
) -> Result<(), TransitionError> {
    if root >= forest.len() {
        return Err(IntentError("Position forest has no original root".into()).into());
    }
    for (index, node) in forest.iter().enumerate() {
        if let PositionForestNode::Transition {
            from,
            to,
            progress,
            reason,
        } = node
        {
            if from.is_none() && to.is_none() {
                return Err(IntentError(
                    "Position forest transition has no original endpoint".into(),
                )
                .into());
            }
            if from.iter().chain(to).any(|child| *child >= index) {
                return Err(IntentError(
                    "Position forest children must reference earlier original nodes".into(),
                )
                .into());
            }
            if !progress.is_finite() || !(0.0..=1.0).contains(progress) {
                return Err(IntentError(
                    "Position forest transition progress must be between zero and one".into(),
                )
                .into());
            }
            if matches!(reason, DynamicTransitionReason::Resume { occurrence_id } if occurrence_id.is_nil())
            {
                return Err(IntentError(
                    "Position forest Resume requires a stable original occurrence".into(),
                )
                .into());
            }
        }
    }
    Ok(())
}

fn validate_cohort(components: &[CoupledComponentEndpoint]) -> Result<(), TransitionError> {
    let Some(first) = components.first() else {
        return Err(IntentError("Target cohort is empty".into()).into());
    };
    let representation = &first.address.address().representation;
    if !matches!(
        representation,
        DynamicFamilyRepresentation::Target { reference: Some(_) }
    ) {
        return Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners,
        ));
    }
    for component in components {
        let address = component.address.address();
        if &address.representation != representation
            || !matches!(
                address.component,
                None | Some(
                    ProgrammingComponent::TargetX
                        | ProgrammingComponent::TargetY
                        | ProgrammingComponent::TargetZ
                )
            )
            || component.role != CoupledLeafRole::Authored
        {
            return Err(IntentError(
                "Target cohort requires authored values in one reference frame".into(),
            )
            .into());
        }
        component.address.validate_source_value(&component.value)?;
    }
    Ok(())
}

/// The single blocked operation in one immutable compiled Position graph. Node numbers belong
/// only to this graph; they do not correlate evaluation cuts across fixtures or controllers.
#[derive(Clone)]
pub struct PositionMaterializationRequest {
    pub node: usize,
    pub requirement: TransitionRequirement,
    pub operation: PositionMaterializationOperation,
}

/// Original operands at the evaluation boundary. An AngleCurrent response is an adopted complete
/// Angle pair; all other responses are the complete result of this node's requested operation.
#[derive(Clone)]
pub enum PositionMaterializationOperation {
    Underlay,
    Endpoint(CoupledBaseEndpoint),
    Cohort(CoupledBaseEndpoint),
    AngleCurrent {
        original: AttributeValue,
        original_occurrence: Option<super::super::DynamicSourceOccurrenceId>,
    },
    Transition {
        from_node: usize,
        to_node: usize,
        from: AttributeValue,
        to: AttributeValue,
        progress: f32,
        reason: DynamicTransitionReason,
    },
    Scale {
        base: CoupledBaseEndpoint,
        value_node: usize,
        value: AttributeValue,
        factor: f32,
        baseline_occurrence: Option<super::super::DynamicSourceOccurrenceId>,
    },
}

pub enum PositionEvaluationProgress {
    Complete(AttributeValue),
    NeedsMaterialization(PositionMaterializationRequest),
}

pub(super) type CoupledNodeResult = (AttributeValue, Option<[CoupledComponentEndpoint; 2]>);

/// Runtime-only evaluation state for an already sampled immutable Position graph. Completed
/// children and the iterative pending stack survive a yield; no clock, Random envelope or lane
/// sampling is restarted. The caller must retain the same captured context/frame and trace arena
/// while servicing requests. This does not retain the outer composition BaseTask stack or nested
/// SourceCohort scratch; that separate integration is required before cohort orchestration uses it.
/// No persisted data or cross-owner evaluation-cut identity is introduced here.
struct GraphOperandReplay {
    node: usize,
    operand: GraphOperationOperand,
    needed: Vec<bool>,
    depth: usize,
}
pub struct PositionEvaluationContinuation {
    expression: Arc<CompiledCoupledExpression>,
    values: Vec<Option<AttributeValue>>,
    pending: Vec<(usize, bool)>,
    materialization: Option<PositionMaterializationRequest>,
    response: Option<(usize, CoupledNodeResult)>,
    failed: bool,
    graph_probe_configured: bool,
    graph_operand: Option<GraphOperandReplay>,
    graph_operand_ready: Option<AttributeValue>,
    graph_reached: Option<Vec<bool>>,
    graph_root_reachable: Option<Vec<bool>>,
}

impl CompiledCoupledExpression {
    pub fn begin_position_evaluation(
        self: &Arc<Self>,
    ) -> Result<PositionEvaluationContinuation, TransitionError> {
        if self.owner != ProgrammingOwner::Position
            || !self.roles.contains(&CoupledExpressionRole::Base)
        {
            return Err(
                IntentError("resumable evaluation requires a Position base graph".into()).into(),
            );
        }
        Ok(PositionEvaluationContinuation {
            expression: self.clone(),
            values: vec![None; self.nodes.len()],
            pending: vec![(self.root, false)],
            materialization: None,
            response: None,
            failed: false,
            graph_probe_configured: false,
            graph_operand: None,
            graph_operand_ready: None,
            graph_reached: None,
            graph_root_reachable: None,
        })
    }

    pub(super) fn coupled_node_result(
        &self,
        id: usize,
        values: &[Option<AttributeValue>],
        context: &dyn CoupledExpressionContext,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<CoupledNodeResult, TransitionError> {
        let mut angle_pair_sources = None;
        let result = match &self.nodes[id] {
            Node::Leaf {
                address,
                value: DynamicValue::Family(value),
                ..
            } => {
                debug_assert!(address.address().component.is_none());
                value.clone()
            }
            Node::Leaf { address, value, .. }
                if role_of(address.address()) == CoupledExpressionRole::Base =>
            {
                let result = context.materialize_base(Some((address, value)))?;
                validate_family(self.owner, &result)?;
                if !address.address().matches_authored_source(&result) {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::CompatibleOwners,
                    ));
                }
                validate_direct_callback(&result, &self.models)?;
                result
            }
            Node::AnglePair(pair) => {
                let (value, sources) = pair.materialize(frame)?;
                angle_pair_sources = Some(sources);
                value
            }
            Node::Cohort(endpoint) => {
                let result = match endpoint {
                    CoupledBaseEndpoint::Components(components) => {
                        context.materialize_cohort(components)?
                    }
                    CoupledBaseEndpoint::Sources(sources) => {
                        context.materialize_cohort_sources(sources)?
                    }
                    CoupledBaseEndpoint::Value { .. } => unreachable!("cohort endpoint"),
                };
                validate_family(self.owner, &result)?;
                for component in endpoint.cohort().into_iter().flatten() {
                    if !component.address.address().matches_authored_source(&result) {
                        return Err(TransitionError::Requires(
                            TransitionRequirement::CompatibleOwners,
                        ));
                    }
                }
                result
            }
            Node::Underlay | Node::Leaf { .. } => {
                let result = context.materialize_base(None)?;
                validate_family(self.owner, &result)?;
                if direct_source(&result)
                    .is_some_and(|source| self.models.iter().any(|(known, _)| known == source))
                {
                    validate_direct_callback(&result, &self.models)?;
                }
                result
            }
            Node::Transition {
                from, to, progress, ..
            } => self.transition(
                values[*from].as_ref().expect("evaluated outgoing child"),
                values[*to].as_ref().expect("evaluated incoming child"),
                *progress,
                frame,
            )?,
            Node::Scale {
                base,
                value,
                factor,
                ..
            } => {
                let (_, DynamicValue::Family(base)) = base.materialized().expect("Size baseline")
                else {
                    unreachable!("whole Size baseline")
                };
                self.operation(
                    base,
                    values[*value].as_ref().expect("evaluated Size child"),
                    FamilyExpressionOperation::Scale { factor: *factor },
                    frame,
                )?
            }
        };

        Ok((result, angle_pair_sources))
    }

    pub(super) fn observe_coupled_node(
        &self,
        id: usize,
        result: &AttributeValue,
        angle_pair_sources: Option<&[CoupledComponentEndpoint; 2]>,
        observer: &mut dyn CoupledEvaluationObserver,
    ) -> Result<(), TransitionError> {
        let step = match &self.nodes[id] {
            Node::Underlay => CoupledEvaluationStep::Underlay,
            Node::AnglePair(_) => CoupledEvaluationStep::AnglePair {
                sources: angle_pair_sources.expect("evaluated Angle pair"),
            },
            Node::Leaf {
                address,
                value,
                role,
                occurrence,
                dependency_occurrence,
            } => CoupledEvaluationStep::Leaf {
                address,
                value,
                sources: &self.leaf_sources[id],
                role: *role,
                occurrence: *occurrence,
                dependency_occurrence: dependency_occurrence.clone(),
            },
            Node::Cohort(CoupledBaseEndpoint::Components(components)) => {
                CoupledEvaluationStep::Cohort { components }
            }
            Node::Cohort(CoupledBaseEndpoint::Sources(sources)) => {
                CoupledEvaluationStep::SourceCohort { sources }
            }
            Node::Cohort(CoupledBaseEndpoint::Value { .. }) => {
                unreachable!("cohort endpoint")
            }
            Node::Transition {
                from, to, progress, ..
            } => CoupledEvaluationStep::Transition {
                from: *from,
                to: *to,
                progress: *progress,
            },
            Node::Scale {
                base,
                value,
                factor,
                baseline_occurrence,
            } => CoupledEvaluationStep::Scale {
                base,
                value: *value,
                factor: *factor,
                sources: &self.leaf_sources[id],
                baseline_occurrence: *baseline_occurrence,
            },
        };
        observer.evaluated(id, step, result)?;

        Ok(())
    }

    fn position_materialization_request(
        &self,
        id: usize,
        requirement: TransitionRequirement,
        values: &[Option<AttributeValue>],
    ) -> Result<PositionMaterializationRequest, TransitionError> {
        let operation = match &self.nodes[id] {
            Node::Underlay => PositionMaterializationOperation::Underlay,
            Node::Leaf { address, value, .. } => {
                PositionMaterializationOperation::Endpoint(CoupledBaseEndpoint::Value {
                    address: address.clone(),
                    value: value.clone(),
                })
            }
            Node::Cohort(endpoint) => PositionMaterializationOperation::Cohort(endpoint.clone()),
            Node::AnglePair(pair) => {
                let original = pair
                    .axes
                    .iter()
                    .find_map(|axis| match axis {
                        PositionAngleAxis::Current { original, .. }
                        | PositionAngleAxis::Numeric { original, .. } => Some(original),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        IntentError("blocked Angle pair has no captured Current".into())
                    })?;
                PositionMaterializationOperation::AngleCurrent {
                    original: original.value.clone(),
                    original_occurrence: original.occurrence,
                }
            }
            Node::Transition {
                from,
                to,
                progress,
                reason,
            } => PositionMaterializationOperation::Transition {
                from_node: *from,
                to_node: *to,
                from: values[*from]
                    .as_ref()
                    .expect("evaluated outgoing child")
                    .clone(),
                to: values[*to]
                    .as_ref()
                    .expect("evaluated incoming child")
                    .clone(),
                progress: *progress,
                reason: *reason,
            },
            Node::Scale {
                base,
                value,
                factor,
                baseline_occurrence,
            } => PositionMaterializationOperation::Scale {
                base: base.clone(),
                value_node: *value,
                value: values[*value]
                    .as_ref()
                    .expect("evaluated Size child")
                    .clone(),
                factor: *factor,
                baseline_occurrence: *baseline_occurrence,
            },
        };
        Ok(PositionMaterializationRequest {
            node: id,
            requirement,
            operation,
        })
    }

    fn validate_position_response(
        &self,
        id: usize,
        response: AttributeValue,
    ) -> Result<CoupledNodeResult, TransitionError> {
        self.validate_result(&response)?;
        match &self.nodes[id] {
            Node::AnglePair(pair) => {
                struct Adopted(AttributeValue);
                impl WholeFamilyExpressionFrameResolver for Adopted {
                    fn adopt_position_angles(
                        &self,
                        _: &AttributeValue,
                    ) -> Result<AttributeValue, TransitionError> {
                        Ok(self.0.clone())
                    }
                    fn resolve(
                        &self,
                        _: TransitionRequirement,
                        _: &AttributeValue,
                        _: &AttributeValue,
                        _: FamilyExpressionOperation,
                    ) -> Result<AttributeValue, TransitionError> {
                        Err(IntentError(
                            "Angle adoption response cannot initiate another frame operation"
                                .into(),
                        )
                        .into())
                    }
                }
                // Reuse the original pair's numeric and provenance calculation after one adopted
                // Current response. This performs no native fitting or clock/Random sampling.
                let (value, sources) = pair.materialize(&Adopted(response))?;
                self.validate_result(&value)?;
                Ok((value, Some(sources)))
            }
            Node::Leaf { address, .. }
                if role_of(address.address()) == CoupledExpressionRole::Base =>
            {
                if !address.address().matches_authored_source(&response) {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::CompatibleOwners,
                    ));
                }
                Ok((response, None))
            }
            Node::Cohort(endpoint) => {
                for component in endpoint.cohort().into_iter().flatten() {
                    if !component
                        .address
                        .address()
                        .matches_authored_source(&response)
                    {
                        return Err(TransitionError::Requires(
                            TransitionRequirement::CompatibleOwners,
                        ));
                    }
                }
                Ok((response, None))
            }
            _ => Ok((response, None)),
        }
    }
}

impl PositionEvaluationContinuation {
    pub fn pending_materialization(&self) -> Option<&PositionMaterializationRequest> {
        self.materialization.as_ref()
    }

    /// A failed response leaves the exact pending request and completed cache intact. The node
    /// guard refers to this opaque continuation's graph, never a global materialization identity.
    pub fn resume_materialization(
        &mut self,
        node: usize,
        value: AttributeValue,
    ) -> Result<(), TransitionError> {
        if self.failed {
            return Err(IntentError("failed Position evaluation must be discarded".into()).into());
        }
        let request = self.materialization.as_ref().ok_or_else(|| {
            IntentError("Position evaluation has no pending materialization".into())
        })?;
        if request.node != node {
            return Err(IntentError(
                "Position materialization response names another graph node".into(),
            )
            .into());
        }
        let response = self.expression.validate_position_response(node, value)?;
        self.response = Some((node, response));
        self.materialization = None;
        Ok(())
    }

    pub(crate) fn configure_graph_operand(
        &mut self,
        selector: &mut dyn FnMut(
            usize,
            GraphOperationKind,
        ) -> Result<Option<GraphOperationOperand>, TransitionError>,
    ) -> Result<(), TransitionError> {
        let mut plan = |node, kind| {
            Ok(selector(node, kind)?
                .into_iter()
                .map(|operand| GraphOperationSelection { operand, depth: 0 })
                .collect())
        };
        self.configure_graph_plan(&mut plan)
    }
    pub(crate) fn graph_selected_depth(&self) -> Option<usize> {
        self.graph_operand.as_ref().map(|replay| replay.depth)
    }
    pub(crate) fn configure_graph_plan(
        &mut self,
        selector: &mut GraphOperationPlanCallback<'_>,
    ) -> Result<(), TransitionError> {
        if self.failed {
            return Err(IntentError("failed Position evaluation must be discarded".into()).into());
        }
        if self.graph_probe_configured {
            return Ok(());
        }
        self.failed = true;
        let result = (|| {
            // Conditioned forests can retain compiled nodes from a removed branch. Only
            // the actual selected root's dependency closure can name a reached operation.
            let mut reachable = vec![false; self.expression.nodes.len()];
            let mut pending = vec![self.expression.root];
            while let Some(node) = pending.pop() {
                if reachable[node] {
                    continue;
                }
                reachable[node] = true;
                match &self.expression.nodes[node] {
                    Node::Transition { from, to, .. } => {
                        pending.push(*from);
                        pending.push(*to);
                    }
                    Node::Scale { value, .. } => pending.push(*value),
                    _ => {}
                }
            }
            self.graph_root_reachable = Some(reachable.clone());
            let mut candidates = Vec::new();
            for (node, value) in self.expression.nodes.iter().enumerate() {
                if !reachable[node] {
                    continue;
                }
                let kind = match value {
                    Node::Transition {
                        reason: DynamicTransitionReason::Required { .. },
                        ..
                    } => GraphOperationKind::Required,
                    Node::Transition {
                        reason: DynamicTransitionReason::Resume { .. },
                        ..
                    } => GraphOperationKind::Resume,
                    Node::Scale { .. } => GraphOperationKind::Size,
                    _ => continue,
                };
                for selection in selector(node, kind)? {
                    if !selection.operand.supports(kind) {
                        return Err(IntentError(
                            "Position graph operand has another operation role".into(),
                        )
                        .into());
                    }
                    candidates.push((node, selection));
                }
            }
            candidates.sort_by_key(|(_, selection)| selection.depth);
            let mut selected = None;
            let mut current_depth = None;
            let mut depth_reachable = reachable.clone();
            for (node, selection) in candidates {
                if current_depth != Some(selection.depth) {
                    depth_reachable.clone_from(&reachable);
                }
                if !depth_reachable[node] {
                    continue;
                }
                if current_depth == Some(selection.depth) {
                    return Err(
                        IntentError("Position graph operand location is ambiguous".into()).into(),
                    );
                }
                current_depth = Some(selection.depth);
                let operand = selection.operand;
                let root = match (&self.expression.nodes[node], operand) {
                    (
                        Node::Transition { from, .. },
                        GraphOperationOperand::RequiredOutgoing
                        | GraphOperationOperand::ResumeOutgoing,
                    ) => Some(*from),
                    (
                        Node::Transition { to, .. },
                        GraphOperationOperand::RequiredIncoming
                        | GraphOperationOperand::ResumeIncoming,
                    ) => Some(*to),
                    (Node::Scale { value, .. }, GraphOperationOperand::SizeValue) => Some(*value),
                    (Node::Scale { .. }, GraphOperationOperand::SizeBaseline) => None,
                    _ => unreachable!("validated graph role"),
                };
                let mut needed = vec![false; self.expression.nodes.len()];
                let mut pending = root.into_iter().collect::<Vec<_>>();
                while let Some(child) = pending.pop() {
                    if needed[child] {
                        continue;
                    }
                    needed[child] = true;
                    match &self.expression.nodes[child] {
                        Node::Transition { from, to, .. } => {
                            pending.push(*from);
                            pending.push(*to);
                        }
                        Node::Scale { value, .. } => pending.push(*value),
                        _ => {}
                    }
                }
                reachable.clone_from(&needed);
                selected = Some((
                    root,
                    GraphOperandReplay {
                        node,
                        operand,
                        needed,
                        depth: selection.depth,
                    },
                ));
            }
            if let Some((root, replay)) = selected {
                self.pending = root.into_iter().map(|child| (child, false)).collect();
                self.graph_operand = Some(replay);
            }
            self.graph_probe_configured = true;
            Ok(())
        })();
        if result.is_ok() {
            self.failed = false;
        }
        result
    }
    pub(crate) fn graph_operand_selected(&self) -> bool {
        self.graph_operand.is_some()
    }
    pub(crate) fn graph_operand_endpoint_needed(&self, index: usize) -> bool {
        self.graph_operand.as_ref().map_or_else(
            || {
                self.graph_root_reachable
                    .as_ref()
                    .map_or(true, |reachable| {
                        self.expression
                            .base_endpoint_nodes(index)
                            .is_some_and(|nodes| nodes.iter().any(|node| reachable[*node]))
                    })
            },
            |replay| {
                self.expression
                    .base_endpoint_nodes(index)
                    .is_some_and(|nodes| nodes.iter().any(|node| replay.needed[*node]))
            },
        )
    }
    pub(crate) fn graph_operand_needs_underlay(&self) -> bool {
        self.graph_operand.as_ref().map_or(self.expression.needs_base_underlay(), |replay| {
            self.expression.nodes.iter().enumerate().any(|(id, node)| replay.needed[id] && (matches!(node, Node::Underlay) || matches!(node, Node::Leaf { address, .. } if role_of(address.address()) != CoupledExpressionRole::Base)))
        })
    }
    pub(crate) fn advance_graph_operand_with_reached(
        &mut self,
        context: &dyn CoupledExpressionContext,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        observer: Option<&mut dyn CoupledEvaluationObserver>,
        reached: Option<&mut GraphOperationReachedCallback<'_>>,
    ) -> Result<GraphOperationProgress<PositionEvaluationProgress>, TransitionError> {
        if self.failed {
            return Err(IntentError("failed Position evaluation must be discarded".into()).into());
        }
        self.failed = true;
        let result =
            self.advance_impl(context, frame, observer, reached)
                .map(|progress| match &self.graph_operand_ready {
                    Some(value) => GraphOperationProgress::OperandReady(value.clone()),
                    None => GraphOperationProgress::Ordinary(progress),
                });
        if result.is_ok() {
            self.failed = false;
        }
        result
    }

    pub fn advance(
        &mut self,
        context: &dyn CoupledExpressionContext,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        observer: Option<&mut dyn CoupledEvaluationObserver>,
    ) -> Result<PositionEvaluationProgress, TransitionError> {
        self.advance_with_reached(context, frame, observer, None)
    }
    pub(crate) fn advance_with_reached(
        &mut self,
        context: &dyn CoupledExpressionContext,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        observer: Option<&mut dyn CoupledEvaluationObserver>,
        reached: Option<&mut GraphOperationReachedCallback<'_>>,
    ) -> Result<PositionEvaluationProgress, TransitionError> {
        if self.failed {
            return Err(IntentError("failed Position evaluation must be discarded".into()).into());
        }
        if self.graph_probe_configured {
            return Err(IntentError(
                "Position graph operand cannot use ordinary evaluation".into(),
            )
            .into());
        }
        // A callback unwind leaves this evaluation terminal even if its owner catches it.
        self.failed = true;
        let result = self.advance_impl(context, frame, observer, reached);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }

    fn advance_impl(
        &mut self,
        context: &dyn CoupledExpressionContext,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        mut observer: Option<&mut dyn CoupledEvaluationObserver>,
        mut reached: Option<&mut GraphOperationReachedCallback<'_>>,
    ) -> Result<PositionEvaluationProgress, TransitionError> {
        if let Some(request) = &self.materialization {
            return Ok(PositionEvaluationProgress::NeedsMaterialization(
                request.clone(),
            ));
        }
        while let Some((id, exit)) = self.pending.pop() {
            if self.values[id].is_some() {
                continue;
            }
            if !exit {
                self.pending.push((id, true));
                match &self.expression.nodes[id] {
                    Node::Transition { from, to, .. } => {
                        self.pending.push((*to, false));
                        self.pending.push((*from, false));
                    }
                    Node::Scale { value, .. } => self.pending.push((*value, false)),
                    _ => {}
                }
                continue;
            }
            // This exit task is reached from the actual root after its selected dependencies.
            // Detached forest nodes and an operand's excluded site/suffix never visit here.
            let mut action = GraphReachedAction::Continue;
            if !self.response.as_ref().is_some_and(|(node, _)| *node == id)
                && let Some(callback) = reached.as_deref_mut()
            {
                let seen = self
                    .graph_reached
                    .get_or_insert_with(|| vec![false; self.expression.nodes.len()]);
                let kind = match &self.expression.nodes[id] {
                    Node::Transition {
                        reason: DynamicTransitionReason::Required { .. },
                        ..
                    } => Some(GraphOperationKind::Required),
                    Node::Scale { .. } => Some(GraphOperationKind::Size),
                    _ => None,
                };
                if let Some(kind) = kind.filter(|_| !seen[id]) {
                    seen[id] = true;
                    action = callback(id, kind)?;
                }
            }
            let evaluated = if self.response.as_ref().is_some_and(|(node, _)| *node == id) {
                Ok(self.response.take().expect("pending response").1)
            } else {
                self.expression
                    .coupled_node_result(id, &self.values, context, frame)
            };
            let (value, sources) = match action.apply(evaluated) {
                Ok(result) => result,
                Err(TransitionError::Requires(requirement)) => {
                    self.pending.push((id, true));
                    let request = match self.expression.position_materialization_request(
                        id,
                        requirement,
                        &self.values,
                    ) {
                        Ok(request) => request,
                        Err(error) => {
                            self.failed = true;
                            return Err(error);
                        }
                    };
                    self.materialization = Some(request.clone());
                    return Ok(PositionEvaluationProgress::NeedsMaterialization(request));
                }
                Err(error) => {
                    self.failed = true;
                    return Err(error);
                }
            };
            if let Err(error) = self.expression.validate_result(&value) {
                self.failed = true;
                return Err(error);
            }
            if let Some(observer) = observer.as_deref_mut() {
                if let Err(error) =
                    self.expression
                        .observe_coupled_node(id, &value, sources.as_ref(), observer)
                {
                    self.failed = true;
                    return Err(error);
                }
            }
            self.values[id] = Some(value);
        }
        if let Some(replay) = &self.graph_operand {
            let value = match (&self.expression.nodes[replay.node], replay.operand) {
                (
                    Node::Transition { from, .. },
                    GraphOperationOperand::RequiredOutgoing | GraphOperationOperand::ResumeOutgoing,
                ) => self.values[*from]
                    .as_ref()
                    .expect("replayed outgoing child")
                    .clone(),
                (
                    Node::Transition { to, .. },
                    GraphOperationOperand::RequiredIncoming | GraphOperationOperand::ResumeIncoming,
                ) => self.values[*to]
                    .as_ref()
                    .expect("replayed incoming child")
                    .clone(),
                (Node::Scale { base, .. }, GraphOperationOperand::SizeBaseline) => {
                    let (_, DynamicValue::Family(base)) =
                        base.materialized().expect("original Size baseline")
                    else {
                        unreachable!("whole Size baseline")
                    };
                    base.clone()
                }
                (Node::Scale { value, .. }, GraphOperationOperand::SizeValue) => self.values
                    [*value]
                    .as_ref()
                    .expect("replayed Size child")
                    .clone(),
                _ => unreachable!("validated graph role"),
            };
            self.graph_operand_ready = Some(value.clone());
            return Ok(PositionEvaluationProgress::Complete(value));
        }
        Ok(PositionEvaluationProgress::Complete(
            self.values[self.expression.root]
                .as_ref()
                .expect("evaluated Position graph root")
                .clone(),
        ))
    }
}

#[cfg(test)]
#[path = "position_resumption_tests.rs"]
mod resumption_tests;

#[cfg(test)]
#[path = "position_lineage_tests.rs"]
mod lineage_tests;
