//! Coupled retained branches whose base representation or complete/component footprint changes.
//! Materialized endpoint values do not grant ownership: callers arbitrate the separate role
//! footprint and preserve explicit Color orthogonals independently from whole-color defaults.
use super::expression_family::retained_reachability;
use super::{
    CompiledDynamicValueAddress, DynamicFamilyRepresentation, DynamicSampleExpression,
    DynamicSemanticColorBasis, DynamicValue, DynamicValueAddress, FamilyExpressionOperation,
    WholeFamilyExpressionFrameResolver, expression_family::resolve_operation,
};
use crate::{
    DynamicNativeModelResolver, DynamicTransitionReason, RetainedExpressionNode,
    RetainedExpressionTape, RetainedNodeId,
};
use light_core::{AttributeValue, NativeColorIdentity, programming::*};
use std::sync::Arc;
mod angle_pair;
mod position;
pub(crate) use angle_pair::{
    CapturedPositionCurrent, PositionAngleAxis, PositionAnglePairEndpoint,
};
pub use position::{
    PositionEvaluationContinuation, PositionEvaluationProgress, PositionMaterializationOperation,
    PositionMaterializationRequest,
};
pub(crate) use position::{PositionForestLineage, PositionForestNode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoupledExpressionRole {
    Base,
    ColorOrthogonal(ColorComponent),
}

pub enum CoupledExpressionFootprint<'a> {
    Inactive,
    /// Route an exact endpoint through its ordinary mask, even though evaluating a
    /// component internally would produce a complete family for transition math.
    Exact {
        address: &'a Arc<CompiledDynamicValueAddress>,
        value: &'a DynamicValue,
    },
    ExactCohort {
        components: &'a [CoupledComponentEndpoint],
    },
    ExactSources {
        sources: &'a [CoupledCohortEndpoint],
    },
    Coupled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoupledLeafRole {
    Authored,
    Current,
}

#[derive(Clone)]
pub struct CoupledComponentEndpoint {
    pub lane_id: uuid::Uuid,
    pub address: Arc<CompiledDynamicValueAddress>,
    pub value: DynamicValue,
    pub role: CoupledLeafRole,
    pub occurrence: Option<super::DynamicSourceOccurrenceId>,
    pub dependency_occurrence: Option<crate::DynamicSourceDependency>,
}

#[derive(Clone)]
pub enum CoupledCohortEndpoint {
    Materialized(CoupledComponentEndpoint),
    WholeExpression {
        lane_id: uuid::Uuid,
        expression: Arc<super::CompiledProgrammingFamilyExpression>,
    },
}
impl CoupledCohortEndpoint {
    pub fn lane_id(&self) -> uuid::Uuid {
        match self {
            Self::Materialized(source) => source.lane_id,
            Self::WholeExpression { lane_id, .. } => *lane_id,
        }
    }
}

#[derive(Clone)]
pub enum CoupledBaseEndpoint {
    Value {
        address: Arc<CompiledDynamicValueAddress>,
        value: DynamicValue,
    },
    Components(Arc<[CoupledComponentEndpoint]>),
    Sources(Arc<[CoupledCohortEndpoint]>),
}
impl CoupledBaseEndpoint {
    pub fn cohort(&self) -> Option<&[CoupledComponentEndpoint]> {
        match self {
            Self::Components(components) => Some(components),
            _ => None,
        }
    }
    pub fn cohort_sources(&self) -> Option<&[CoupledCohortEndpoint]> {
        match self {
            Self::Sources(sources) => Some(sources),
            _ => None,
        }
    }
    pub fn materialized(&self) -> Option<(&Arc<CompiledDynamicValueAddress>, &DynamicValue)> {
        match self {
            Self::Value { address, value } => Some((address, value)),
            _ => None,
        }
    }
}

/// Bound by the compositor to this source's strictly lower ranked prefix in one frame.
pub trait CoupledExpressionContext {
    /// Select the endpoint's compatible cohort BEFORE applying its component edit.
    /// None requests the eligible lower base, without explicit Color orthogonal lanes.
    fn materialize_base(
        &self,
        endpoint: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
    ) -> Result<AttributeValue, TransitionError>;

    /// All original component edits share one reference-compatible lower cohort. Implementors
    /// must preserve untouched coordinates and must not resolve each edit over separate bases.
    fn materialize_cohort(
        &self,
        _components: &[CoupledComponentEndpoint],
    ) -> Result<AttributeValue, TransitionError> {
        Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners,
        ))
    }

    /// A complete Target lane and its authored offsets retain their original source order.
    /// Deferred whole sources must use the same eligible prefix as their materialized siblings.
    fn materialize_cohort_sources(
        &self,
        _sources: &[CoupledCohortEndpoint],
    ) -> Result<AttributeValue, TransitionError> {
        Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners,
        ))
    }

    /// Apply older explicit writes to this component over THIS branch's implicit default.
    /// Do not extract from the already interpolated base or include this source's fragments.
    fn orthogonal_underlay(
        &self,
        component: ColorComponent,
        branch_base: &AttributeValue,
    ) -> Result<f32, TransitionError>;
}

type NativeModels = Vec<(
    NativeColorIdentity,
    Arc<dyn NativeColorEditModel + Send + Sync>,
)>;

enum Node {
    Underlay,
    Leaf {
        address: Arc<CompiledDynamicValueAddress>,
        value: DynamicValue,
        role: CoupledLeafRole,
        occurrence: Option<super::DynamicSourceOccurrenceId>,
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    },
    Cohort(CoupledBaseEndpoint),
    AnglePair(Arc<PositionAnglePairEndpoint>),
    Transition {
        from: usize,
        to: usize,
        progress: f32,
        reason: DynamicTransitionReason,
    },
    Scale {
        base: CoupledBaseEndpoint,
        value: usize,
        factor: f32,
        baseline_occurrence: Option<super::DynamicSourceOccurrenceId>,
    },
}

fn metadata(nodes: &[Node]) -> (Vec<Vec<CoupledExpressionRole>>, Vec<bool>) {
    let mut roles: Vec<Vec<CoupledExpressionRole>> = Vec::with_capacity(nodes.len());
    let mut needs = Vec::with_capacity(nodes.len());
    for node in nodes {
        let (current, underlay) = match node {
            Node::Underlay => (vec![], true),
            Node::Cohort(_) | Node::AnglePair(_) => (vec![CoupledExpressionRole::Base], false),
            Node::Leaf { address, .. } => {
                let role = role_of(address.address());
                (vec![role], role != CoupledExpressionRole::Base)
            }
            Node::Scale { value, .. } => (vec![CoupledExpressionRole::Base], needs[*value]),
            Node::Transition { from, to, .. } => {
                let mut combined = roles[*from].clone();
                for role in &roles[*to] {
                    if !combined.contains(role) {
                        combined.push(*role);
                    }
                }
                (combined, needs[*from] || needs[*to])
            }
        };
        roles.push(current);
        needs.push(underlay);
    }
    (roles, needs)
}

fn collect_base_endpoints(
    nodes: &[Node],
    root: usize,
) -> (Vec<CoupledBaseEndpoint>, Vec<Vec<usize>>) {
    let mut endpoints: Vec<CoupledBaseEndpoint> = vec![];
    let mut origins = Vec::<Vec<usize>>::new();
    let mut visited = vec![false; nodes.len()];
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if std::mem::replace(&mut visited[id], true) {
            continue;
        }
        let endpoint = match &nodes[id] {
            Node::Leaf { address, value, .. }
                if role_of(address.address()) == CoupledExpressionRole::Base =>
            {
                Some((address, value))
            }
            Node::Cohort(endpoint) => {
                endpoints.push(endpoint.clone());
                origins.push(vec![id]);
                None
            }
            Node::Transition { from, to, .. } => {
                pending.push(*to);
                pending.push(*from);
                None
            }
            Node::Scale { base, value, .. } => {
                pending.push(*value);
                base.materialized()
            }
            _ => None,
        };
        if let Some((address, value)) = endpoint {
            if let Some(existing) = endpoints.iter().position(|existing| {
                existing
                    .materialized()
                    .is_some_and(|(known_address, known_value)| {
                        known_address.address() == address.address() && known_value == value
                    })
            }) {
                origins[existing].push(id);
            } else {
                origins.push(vec![id]);
                endpoints.push(CoupledBaseEndpoint::Value {
                    address: address.clone(),
                    value: value.clone(),
                });
            }
        }
    }
    (endpoints, origins)
}

/// Origins belong to the retained compiler input, never a shared evaluation cut.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PositionCompiledNodeLocation {
    Tape(RetainedNodeId),
    Forest(usize),
    WholeTape(usize, RetainedNodeId),
}

pub struct CompiledCoupledExpression {
    retained: CoupledRetainedSources,
    position_forest_lineage: Option<Arc<PositionForestLineage>>,
    position_node_origins: Vec<Option<PositionCompiledNodeLocation>>,
    owner: ProgrammingOwner,
    nodes: Vec<Node>,
    root: usize,
    node_roles: Vec<Vec<CoupledExpressionRole>>,
    base_underlay: bool,
    roles: Vec<CoupledExpressionRole>,
    orthogonals: Vec<(ColorComponent, Arc<CompiledDynamicValueAddress>)>,
    base_endpoints: Vec<CoupledBaseEndpoint>,
    base_endpoint_nodes: Vec<Vec<usize>>,
    models: NativeModels,
    leaf_sources: Vec<Arc<[CoupledComponentEndpoint]>>,
}

pub enum CoupledRetainedSources {
    Expression(Arc<DynamicSampleExpression>),
    PositionForest(Arc<[crate::DynamicRuntimeSample]>),
}

pub enum CoupledEvaluationStep<'a> {
    /// Materialized by this evaluation; Current leaves retain their original source evidence.
    AnglePair {
        sources: &'a [CoupledComponentEndpoint],
    },
    Underlay,
    Leaf {
        address: &'a CompiledDynamicValueAddress,
        value: &'a DynamicValue,
        sources: &'a [CoupledComponentEndpoint],
        role: CoupledLeafRole,
        occurrence: Option<super::DynamicSourceOccurrenceId>,
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    },
    Cohort {
        components: &'a [CoupledComponentEndpoint],
    },
    SourceCohort {
        sources: &'a [CoupledCohortEndpoint],
    },
    Transition {
        from: usize,
        to: usize,
        progress: f32,
    },
    Scale {
        base: &'a CoupledBaseEndpoint,
        value: usize,
        factor: f32,
        sources: &'a [CoupledComponentEndpoint],
        baseline_occurrence: Option<super::DynamicSourceOccurrenceId>,
    },
}

/// Topological observations from the same value evaluation. IDs are local to this immutable
/// compiled graph; Current leaves describe dependencies, not authored component writes.
pub trait CoupledEvaluationObserver {
    fn evaluated(
        &mut self,
        node: usize,
        step: CoupledEvaluationStep<'_>,
        value: &AttributeValue,
    ) -> Result<(), TransitionError>;
}

impl CompiledCoupledExpression {
    pub fn new(
        expression: Arc<DynamicSampleExpression>,
        native_models: Option<&dyn DynamicNativeModelResolver>,
    ) -> Result<Self, TransitionError> {
        let tape = RetainedExpressionTape::from_roots(std::slice::from_ref(&expression))?;
        let tape_root = tape.roots()[0];
        let mut owner = None;
        preflight(&tape, tape_root, &mut owner)?;
        let owner = owner.ok_or(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners,
        ))?;
        let mut models = Vec::new();
        let (nodes, root, _, origins) = compile(&tape, tape_root, native_models, &mut models)?;
        let (node_roles, needs) = metadata(&nodes);
        let roles = node_roles[root].clone();
        let base_underlay = needs[root];
        let mut orthogonals = Vec::new();
        for role in &roles {
            if let CoupledExpressionRole::ColorOrthogonal(component) = *role {
                orthogonals.push((
                    component,
                    Arc::new(CompiledDynamicValueAddress::new(
                        DynamicValueAddress {
                            representation: DynamicFamilyRepresentation::SemanticColor {
                                basis: DynamicSemanticColorBasis::Retain,
                            },
                            component: Some(ProgrammingComponent::Color(component)),
                        },
                        None,
                    )?),
                ));
            }
        }
        let (base_endpoints, base_endpoint_nodes) = collect_base_endpoints(&nodes, root);
        let leaf_sources = vec![Arc::from([]); nodes.len()];
        Ok(Self {
            retained: CoupledRetainedSources::Expression(expression),
            position_forest_lineage: None,
            position_node_origins: origins
                .into_iter()
                .map(|node| node.map(PositionCompiledNodeLocation::Tape))
                .collect(),
            owner,
            nodes,
            root,
            node_roles,
            base_underlay,
            roles,
            orthogonals,
            base_endpoints,
            base_endpoint_nodes,
            models,
            leaf_sources,
        })
    }

    /// Emission origin of an actual Position graph node. Aliases keep the child origin.
    pub(crate) fn position_node_origin(&self, node: usize) -> Option<PositionCompiledNodeLocation> {
        if self.owner != ProgrammingOwner::Position {
            return None;
        }
        self.position_node_origins.get(node).copied().flatten()
    }

    pub fn expression(&self) -> Option<&Arc<DynamicSampleExpression>> {
        match &self.retained {
            CoupledRetainedSources::Expression(expression) => Some(expression),
            CoupledRetainedSources::PositionForest(_) => None,
        }
    }
    pub fn retained_sources(&self) -> &CoupledRetainedSources {
        &self.retained
    }
    pub fn trace_root_node(&self) -> usize {
        self.root
    }
    /// Retained reason for a transition in this compiled graph. Node IDs are graph-local;
    /// a Required reason does not establish a common evaluation cut across owners.
    pub fn transition_reason(&self, node: usize) -> Option<DynamicTransitionReason> {
        match self.nodes.get(node)? {
            Node::Transition { reason, .. } => Some(*reason),
            _ => None,
        }
    }
    pub fn owner(&self) -> ProgrammingOwner {
        self.owner
    }
    pub fn roles(&self) -> &[CoupledExpressionRole] {
        &self.roles
    }
    pub(crate) fn base_endpoint_nodes(&self, index: usize) -> Option<&[usize]> {
        self.base_endpoint_nodes.get(index).map(Vec::as_slice)
    }
    pub fn base_endpoints(&self) -> &[CoupledBaseEndpoint] {
        &self.base_endpoints
    }
    pub fn needs_base_underlay(&self) -> bool {
        self.roles.contains(&CoupledExpressionRole::Base) && self.base_underlay
    }

    pub fn footprint(&self, role: CoupledExpressionRole) -> CoupledExpressionFootprint<'_> {
        if !self.roles.contains(&role) {
            return CoupledExpressionFootprint::Inactive;
        }
        match &self.nodes[self.root] {
            Node::Leaf { address, value, .. } => {
                CoupledExpressionFootprint::Exact { address, value }
            }
            Node::Cohort(CoupledBaseEndpoint::Components(components)) => {
                CoupledExpressionFootprint::ExactCohort { components }
            }
            Node::Cohort(CoupledBaseEndpoint::Sources(sources)) => {
                CoupledExpressionFootprint::ExactSources { sources }
            }
            _ => CoupledExpressionFootprint::Coupled,
        }
    }

    /// Exact forest Angle endpoints retain their original authored axes and live Current
    /// dependencies even when composition routes the value through the ordinary whole mask.
    pub fn exact_leaf_sources(&self) -> &[CoupledComponentEndpoint] {
        if matches!(self.nodes[self.root], Node::Leaf { .. }) {
            &self.leaf_sources[self.root]
        } else {
            &[]
        }
    }

    /// Non-forest exact leaves use the caller's lane rank. Keep their captured metadata when
    /// the compositor takes the ordinary Known fast path without evaluating an observer.
    pub(crate) fn exact_leaf_provenance(
        &self,
    ) -> Option<(
        CoupledLeafRole,
        Option<super::DynamicSourceOccurrenceId>,
        Option<&crate::DynamicSourceDependency>,
    )> {
        match &self.nodes[self.root] {
            Node::Leaf {
                role,
                occurrence,
                dependency_occurrence,
                ..
            } => Some((*role, *occurrence, dependency_occurrence.as_ref())),
            _ => None,
        }
    }

    pub fn resolve_original_native_model(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, TransitionError> {
        self.models
            .iter()
            .find(|(identity, _)| identity == source)
            .map(|(_, model)| model.clone())
            .ok_or(TransitionError::Requires(
                TransitionRequirement::NativeColorModel,
            ))
    }

    pub fn evaluate_base(
        &self,
        context: &dyn CoupledExpressionContext,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.evaluate_base_observed(context, frame, None)
    }

    pub fn evaluate_base_observed(
        &self,
        context: &dyn CoupledExpressionContext,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        observer: Option<&mut dyn CoupledEvaluationObserver>,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        if !self.roles.contains(&CoupledExpressionRole::Base) {
            return Ok(None);
        }
        let value = self.base(
            self.root,
            context,
            frame,
            &mut vec![None; self.nodes.len()],
            observer,
        )?;
        self.validate_result(&value)?;
        Ok(Some(value))
    }

    pub fn evaluate_orthogonal(
        &self,
        component: ColorComponent,
        context: &dyn CoupledExpressionContext,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<Option<f32>, TransitionError> {
        let Some((_, address)) = self
            .orthogonals
            .iter()
            .find(|(candidate, _)| *candidate == component)
        else {
            return Ok(None);
        };
        self.orthogonal(component, address, context, frame)
            .map(Some)
    }

    pub fn transition(
        &self,
        from: &AttributeValue,
        to: &AttributeValue,
        progress: f32,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<AttributeValue, TransitionError> {
        self.operation(
            from,
            to,
            FamilyExpressionOperation::Transition { progress },
            frame,
        )
    }

    fn operation(
        &self,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<AttributeValue, TransitionError> {
        validate_family(self.owner, from)?;
        validate_family(self.owner, to)?;
        let model = match (direct_source(from), direct_source(to)) {
            (Some(a), Some(b)) if a == b => self
                .models
                .iter()
                .find(|(source, _)| source == a)
                .map(|(_, model)| model.clone()),
            _ => None,
        };
        let compiled = CompiledProgrammingTransition::new(from.clone(), to.clone(), model)?;
        let result = match operation {
            FamilyExpressionOperation::Transition { progress } => compiled.sample(progress),
            FamilyExpressionOperation::Scale { factor } => compiled.scale(factor),
        };
        let value =
            resolve_operation(self.owner, result, from, to, operation, &self.models, frame)?;
        self.validate_result(&value)?;
        Ok(value)
    }

    fn validate_result(&self, value: &AttributeValue) -> Result<(), TransitionError> {
        validate_family(self.owner, value)?;
        if let Some(source) = direct_source(value) {
            self.resolve_original_native_model(source)?;
        }
        Ok(())
    }

    fn base(
        &self,
        root: usize,
        context: &dyn CoupledExpressionContext,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        values: &mut [Option<AttributeValue>],
        mut observer: Option<&mut dyn CoupledEvaluationObserver>,
    ) -> Result<AttributeValue, TransitionError> {
        let mut pending = vec![(root, false)];
        while let Some((id, exit)) = pending.pop() {
            if values[id].is_some() {
                continue;
            }
            if !exit {
                pending.push((id, true));
                match &self.nodes[id] {
                    Node::Transition { from, to, .. } => {
                        pending.push((*to, false));
                        pending.push((*from, false));
                    }
                    Node::Scale { value, .. } => pending.push((*value, false)),
                    _ => {}
                }
                continue;
            }
            let (result, angle_pair_sources) =
                self.coupled_node_result(id, values, context, frame)?;
            if let Some(observer) = observer.as_deref_mut() {
                self.observe_coupled_node(id, &result, angle_pair_sources.as_ref(), observer)?;
            }
            values[id] = Some(result);
        }
        Ok(values[root].as_ref().expect("evaluated graph root").clone())
    }

    fn orthogonal(
        &self,
        component: ColorComponent,
        address: &CompiledDynamicValueAddress,
        context: &dyn CoupledExpressionContext,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<f32, TransitionError> {
        let role = CoupledExpressionRole::ColorOrthogonal(component);
        let mut values = vec![None; self.nodes.len()];
        let mut bases = vec![None; self.nodes.len()];
        let mut pending = vec![(self.root, false)];
        while let Some((id, exit)) = pending.pop() {
            if values[id].is_some() {
                continue;
            }
            if !self.node_roles[id].contains(&role) {
                let branch = self.base(id, context, frame, &mut bases, None)?;
                let value = context.orthogonal_underlay(component, &branch)?;
                address.validate_value(&DynamicValue::Scalar(value))?;
                values[id] = Some(value);
                continue;
            }
            match &self.nodes[id] {
                Node::Leaf {
                    value: DynamicValue::Scalar(value),
                    ..
                } => values[id] = Some(*value),
                Node::Transition {
                    from, to, progress, ..
                } => {
                    if !exit {
                        pending.push((id, true));
                        pending.push((*to, false));
                        pending.push((*from, false));
                        continue;
                    }
                    let DynamicValue::Scalar(value) = address
                        .transition(
                            DynamicValue::Scalar(values[*from].expect("evaluated outgoing role")),
                            DynamicValue::Scalar(values[*to].expect("evaluated incoming role")),
                        )?
                        .sample(*progress)?
                    else {
                        unreachable!("compiled orthogonal scalar")
                    };
                    values[id] = Some(value);
                }
                _ => unreachable!("explicit Color orthogonal role"),
            }
        }
        Ok(values[self.root].expect("evaluated orthogonal root"))
    }
}

fn role_of(address: &DynamicValueAddress) -> CoupledExpressionRole {
    match address.component {
        Some(ProgrammingComponent::Color(component))
            if ProgrammingComponent::Color(component).descriptor().role
                == ComponentRole::ColorOrthogonal =>
        {
            CoupledExpressionRole::ColorOrthogonal(component)
        }
        _ => CoupledExpressionRole::Base,
    }
}

fn preflight(
    tape: &RetainedExpressionTape,
    root: RetainedNodeId,
    owner: &mut Option<ProgrammingOwner>,
) -> Result<(), TransitionError> {
    let reachable = retained_reachability(tape, root, false);
    for (index, node) in tape.nodes.iter().enumerate() {
        if !reachable[index] {
            continue;
        }
        match node {
            RetainedExpressionNode::Programming { address, .. }
            | RetainedExpressionNode::Scale { address, .. } => {
                if owner.is_some_and(|owner| owner != address.owner())
                    || (address.representation == DynamicFamilyRepresentation::Angles
                        && address.component.is_some())
                {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::CompatibleOwners,
                    ));
                }
                *owner = Some(address.owner());
            }
            RetainedExpressionNode::Transition { .. } => {}
            _ => {
                return Err(TransitionError::Requires(
                    TransitionRequirement::CompatibleOwners,
                ));
            }
        }
    }
    Ok(())
}

fn compile(
    tape: &RetainedExpressionTape,
    root: RetainedNodeId,
    resolver: Option<&dyn DynamicNativeModelResolver>,
    models: &mut NativeModels,
) -> Result<
    (
        Vec<Node>,
        usize,
        Vec<Option<usize>>,
        Vec<Option<RetainedNodeId>>,
    ),
    TransitionError,
> {
    let active = retained_reachability(tape, root, true);
    let mut nodes = vec![Node::Underlay];
    let mut origins = vec![None];
    let mut ids = vec![None; tape.nodes.len()];
    for (index, source) in tape.nodes.iter().enumerate() {
        if !active[index] {
            continue;
        }
        let child = |id: RetainedNodeId| ids[id.0 as usize].expect("compiled earlier child");
        let node = match source {
            RetainedExpressionNode::Programming {
                address,
                value,
                occurrence,
                dependency_occurrence,
            } => {
                let mut leaf = compile_leaf(address, value, resolver, models)?;
                if let Node::Leaf {
                    occurrence: slot,
                    dependency_occurrence: dependency_slot,
                    ..
                } = &mut leaf
                {
                    *slot = *occurrence;
                    *dependency_slot = dependency_occurrence.clone();
                }
                leaf
            }
            RetainedExpressionNode::Transition {
                from,
                to,
                progress,
                reason,
            } => {
                if *progress == 0.0 {
                    ids[index] = Some(from.map_or(0, child));
                    continue;
                }
                if *progress == 1.0 {
                    ids[index] = Some(to.map_or(0, child));
                    continue;
                }
                let from = from.map_or(0, child);
                let to = to.map_or(0, child);
                if from == 0 && to == 0 {
                    ids[index] = Some(0);
                    continue;
                }
                Node::Transition {
                    from,
                    to,
                    progress: *progress,
                    reason: *reason,
                }
            }
            RetainedExpressionNode::Scale {
                address,
                base: DynamicValue::Family(base),
                value,
                factor,
                baseline_occurrence,
                ..
            } => {
                if *factor == 1.0 {
                    ids[index] = Some(child(*value));
                    continue;
                }
                let actual = DynamicValueAddress::whole_family(address.owner(), base)?;
                let mut baseline = compile_leaf(
                    &actual,
                    &DynamicValue::Family(base.clone()),
                    resolver,
                    models,
                )?;
                if *factor == 0.0 {
                    if let Node::Leaf {
                        role,
                        occurrence,
                        dependency_occurrence,
                        ..
                    } = &mut baseline
                    {
                        *role = CoupledLeafRole::Current;
                        *occurrence = *baseline_occurrence;
                        *dependency_occurrence = Some(crate::DynamicSourceDependency::identity(
                            *baseline_occurrence,
                        ));
                    }
                    baseline
                } else {
                    let Node::Leaf {
                        address,
                        value: base,
                        ..
                    } = baseline
                    else {
                        unreachable!("whole baseline leaf")
                    };
                    Node::Scale {
                        base: CoupledBaseEndpoint::Value {
                            address,
                            value: base,
                        },
                        value: child(*value),
                        factor: *factor,
                        baseline_occurrence: *baseline_occurrence,
                    }
                }
            }
            _ => unreachable!("validated coupled expression"),
        };
        ids[index] = Some(nodes.len());
        nodes.push(node);
        origins.push(Some(RetainedNodeId(index as u32)));
    }
    let root = ids[root.0 as usize].expect("compiled root");
    Ok((nodes, root, ids, origins))
}

fn compile_leaf(
    address: &DynamicValueAddress,
    value: &DynamicValue,
    resolver: Option<&dyn DynamicNativeModelResolver>,
    models: &mut NativeModels,
) -> Result<Node, TransitionError> {
    let model = if let DynamicFamilyRepresentation::DirectColor { source } = &address.representation
    {
        if !models.iter().any(|(identity, _)| identity == source) {
            let model = resolver
                .ok_or(TransitionError::Requires(
                    TransitionRequirement::NativeColorModel,
                ))?
                .resolve(source)?;
            if model.source() != source {
                return Err(IntentError(
                    "coupled native model differs from pinned original source".into(),
                )
                .into());
            }
            models.push((source.clone(), model));
        }
        models
            .iter()
            .find(|(identity, _)| identity == source)
            .map(|(_, model)| model.clone())
    } else {
        None
    };
    let address = Arc::new(CompiledDynamicValueAddress::new(address.clone(), model)?);
    address.validate_value(value)?;
    if let DynamicValue::Family(value) = value {
        validate_direct_callback(value, models)?;
    }
    Ok(Node::Leaf {
        address,
        value: value.clone(),
        role: CoupledLeafRole::Authored,
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn direct_source(value: &AttributeValue) -> Option<&NativeColorIdentity> {
    if let AttributeValue::ColorProgram(program) = value
        && let ColorProgram::Direct { recipe, .. } = program.as_ref()
    {
        Some(&recipe.source)
    } else {
        None
    }
}

fn validate_direct_callback(
    value: &AttributeValue,
    models: &NativeModels,
) -> Result<(), TransitionError> {
    if let AttributeValue::ColorProgram(program) = value
        && let ColorProgram::Direct { recipe, .. } = program.as_ref()
    {
        let model = models
            .iter()
            .find(|(source, _)| source == &recipe.source)
            .map(|(_, model)| model)
            .ok_or(TransitionError::Requires(
                TransitionRequirement::NativeColorModel,
            ))?;
        let prediction = model.predict(recipe)?;
        if prediction.model_revision != recipe.source.model_revision {
            return Err(IntentError(
                "coupled native prediction differs from pinned source revision".into(),
            )
            .into());
        }
        prediction.validate()?;
    }
    Ok(())
}

fn validate_family(owner: ProgrammingOwner, value: &AttributeValue) -> Result<(), TransitionError> {
    DynamicValueAddress::whole_family(owner, value)?;
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod transition_identity_tests;

#[cfg(test)]
#[path = "expression_coupled/position_origin_tests.rs"]
mod position_origin_tests;
