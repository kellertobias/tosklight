//! Versioned, flat retained expressions for runtime checkpoints. IDs refer only to
//! earlier nodes, so interruption history remains live without recursive depth or tree copying.
//! This is storage and traversal infrastructure; source model verification belongs to the
//! cold compiler and no node contains fitted fixture output or solved world coordinates.
use super::expression::{ExpressionNode, ExpressionNodeRef};
use super::{
    DynamicFamilyRepresentation, DynamicSampleExpression, DynamicSourceOccurrenceId,
    DynamicTransitionReason, DynamicValue, DynamicValueAddress,
};
use light_core::{AttributeKey, attribute_descriptor, programming::*};
type InputKey = (usize, Option<RetainedNodeId>);
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

pub const RETAINED_EXPRESSION_TAPE_VERSION: u16 = 3;
/// Tapes carrying a producer-operation object table. Tapes without operation provenance keep
/// writing [`RETAINED_EXPRESSION_TAPE_VERSION`]; older tapes remain explicitly uncorrelated.
pub const RETAINED_OPERATION_TAPE_VERSION: u16 = 4;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RetainedNodeId(pub u32);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RetainedExpressionNode {
    LegacyScalar {
        attribute: AttributeKey,
        value: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        occurrence: Option<DynamicSourceOccurrenceId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    },
    Programming {
        address: DynamicValueAddress,
        value: DynamicValue,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        occurrence: Option<DynamicSourceOccurrenceId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    },
    AngleCurrent {
        address: DynamicValueAddress,
    },
    AngleNumeric {
        program: Arc<crate::AngleNumericProgram>,
    },
    Scale {
        address: DynamicValueAddress,
        base: DynamicValue,
        value: RetainedNodeId,
        factor: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        baseline_occurrence: Option<DynamicSourceOccurrenceId>,
    },
    Transition {
        from: Option<RetainedNodeId>,
        to: Option<RetainedNodeId>,
        progress: f32,
        reason: DynamicTransitionReason,
    },
}

impl RetainedExpressionNode {
    pub fn children(&self) -> impl Iterator<Item = RetainedNodeId> + '_ {
        let mut children = [None, None];
        match self {
            Self::Scale { value, .. } => children[0] = Some(*value),
            Self::Transition { from, to, .. } => {
                children[0] = *from;
                children[1] = *to;
            }
            _ => {}
        }
        children.into_iter().flatten()
    }

    fn from_view(
        node: ExpressionNode<'_>,
        ids: &HashMap<InputKey, RetainedNodeId>,
    ) -> Result<Self, IntentError> {
        let child = |value: ExpressionNodeRef<'_>| {
            ids.get(&value.key()).copied().ok_or_else(|| {
                IntentError("retained child was not imported before its parent".into())
            })
        };
        Ok(match node {
            ExpressionNode::Legacy(attribute, value, occurrence, dependency_occurrence) => {
                Self::LegacyScalar {
                    attribute: attribute.clone(),
                    value,
                    occurrence,
                    dependency_occurrence: dependency_occurrence.cloned(),
                }
            }
            ExpressionNode::Programming(address, value, occurrence, dependency_occurrence) => {
                Self::Programming {
                    address: address.clone(),
                    value: value.clone(),
                    occurrence,
                    dependency_occurrence: dependency_occurrence.cloned(),
                }
            }
            ExpressionNode::Current(address) => Self::AngleCurrent {
                address: address.clone(),
            },
            ExpressionNode::Numeric(program) => Self::AngleNumeric {
                program: Arc::new(if program.operations.is_empty() {
                    program.clone()
                } else {
                    // The tape's object table is the only retained copy of these origins.
                    let mut program = program.clone();
                    program.operations = Default::default();
                    program
                }),
            },
            ExpressionNode::Scale {
                address,
                base,
                value,
                factor,
                baseline_occurrence,
            } => Self::Scale {
                address: address.clone(),
                base: base.clone(),
                value: child(value)?,
                factor,
                baseline_occurrence,
            },
            ExpressionNode::Transition {
                from,
                to,
                progress,
                reason,
            } => Self::Transition {
                from: from.map(child).transpose()?,
                to: to.map(child).transpose()?,
                progress,
                reason,
            },
        })
    }
}

/// Checkpoint shape. `roots` is ordered by the caller's held owner/lane list; identical roots
/// and shared descendants retain one NodeId. Unknown future versions are rejected explicitly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedExpressionTape {
    pub version: u16,
    pub nodes: Vec<RetainedExpressionNode>,
    pub roots: Vec<RetainedNodeId>,
    /// Producer emission object table. Each genuine witness is serialized once and restored
    /// as one `Arc` shared by every reference; equal entries remain independent objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    emissions: Vec<Arc<crate::DynamicEmissionWitness>>,
    /// Explicit node/site references, ordered by node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    operations: Vec<RetainedOperationReference>,
}

/// One attributed producer operation of one tape node.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetainedOperationReference {
    node: RetainedNodeId,
    emission: u32,
    target: light_core::FixtureId,
    lane_id: Uuid,
    site: crate::DynamicOperationSite,
    /// Locator inside a numeric Angle program node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    program_node: Option<u32>,
}

/// The single instance/controller/target/lane that owns the operations below a node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RetainedOperationOwner {
    Unattributed,
    Exact {
        instance_id: Uuid,
        controller_id: Uuid,
        target: light_core::FixtureId,
        lane_id: Uuid,
    },
    Mixed,
}

impl RetainedOperationOwner {
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unattributed, other) | (other, Self::Unattributed) => other,
            (a, b) if a == b => a,
            _ => Self::Mixed,
        }
    }
}

/// Interns witnesses by original object identity while a tape gains references.
#[derive(Default)]
struct EmissionInterner(HashMap<usize, u32>);

impl EmissionInterner {
    fn reference(
        &mut self,
        tape: &mut RetainedExpressionTape,
        node: RetainedNodeId,
        origin: &crate::DynamicOperationOrigin,
        program_node: Option<u32>,
    ) -> Result<(), IntentError> {
        let emission = origin.emission();
        let index = match self.0.entry(Arc::as_ptr(emission) as usize) {
            std::collections::hash_map::Entry::Occupied(entry) => *entry.get(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let index = u32::try_from(tape.emissions.len())
                    .map_err(|_| IntentError("retained tape has too many emissions".into()))?;
                tape.emissions.push(Arc::clone(emission));
                *entry.insert(index)
            }
        };
        tape.operations.push(RetainedOperationReference {
            node,
            emission: index,
            target: origin.target(),
            lane_id: origin.lane_id(),
            site: origin.site(),
            program_node,
        });
        tape.version = tape.version.max(RETAINED_OPERATION_TAPE_VERSION);
        Ok(())
    }

    fn seeded(tape: &RetainedExpressionTape) -> Self {
        Self(
            tape.emissions
                .iter()
                .enumerate()
                .map(|(index, emission)| (Arc::as_ptr(emission) as usize, index as u32))
                .collect(),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
enum AddressSummary {
    Empty,
    Exact(DynamicValueAddress),
    Mixed,
}

fn combine(a: AddressSummary, b: AddressSummary) -> AddressSummary {
    match (a, b) {
        (AddressSummary::Empty, other) | (other, AddressSummary::Empty) => other,
        (AddressSummary::Exact(a), AddressSummary::Exact(b)) if a == b => AddressSummary::Exact(a),
        _ => AddressSummary::Mixed,
    }
}

impl RetainedExpressionTape {
    pub fn empty() -> Self {
        Self {
            version: RETAINED_EXPRESSION_TAPE_VERSION,
            nodes: vec![],
            roots: vec![],
            emissions: vec![],
            operations: vec![],
        }
    }

    /// Import a historical Arc expression forest without recursive validation or traversal.
    /// Pointer memoization keeps old shared Arc descendants shared in the checkpoint tape.
    pub fn from_roots(roots: &[Arc<DynamicSampleExpression>]) -> Result<Self, IntentError> {
        Self::import_roots(roots, None)
    }

    /// Source identities are emitted during the same import. These private keys describe
    /// the input tree/tape only; they never establish a frame or shared mechanical cut.
    pub(crate) fn from_roots_with_source_keys(
        roots: &[Arc<DynamicSampleExpression>],
    ) -> Result<(Self, Vec<(usize, Option<RetainedNodeId>)>), IntentError> {
        let mut keys = Vec::new();
        let tape = Self::import_roots(roots, Some(&mut keys))?;
        Ok((tape, keys))
    }

    fn import_roots(
        roots: &[Arc<DynamicSampleExpression>],
        mut source_keys: Option<&mut Vec<InputKey>>,
    ) -> Result<Self, IntentError> {
        let mut tape = Self::empty();
        let mut ids = HashMap::<InputKey, RetainedNodeId>::new();
        let mut active = HashSet::new();
        let mut verified_tapes = HashSet::new();
        let mut interner = EmissionInterner::default();
        let mut pending = roots
            .iter()
            .rev()
            .map(|root| (ExpressionNodeRef::new(root), false))
            .collect::<Vec<_>>();
        while let Some((node, exit)) = pending.pop() {
            let key = node.key();
            if exit {
                active.remove(&key);
                let id =
                    RetainedNodeId(u32::try_from(tape.nodes.len()).map_err(|_| {
                        IntentError("retained expression has too many nodes".into())
                    })?);
                let view = node.node()?;
                let numeric = match &view {
                    ExpressionNode::Numeric(program) if !program.operations.is_empty() => {
                        Some(program.operations.clone())
                    }
                    _ => None,
                };
                tape.nodes
                    .push(RetainedExpressionNode::from_view(view, &ids)?);
                // Origins move by original witness identity; synthetic nodes receive none.
                if let Some(origin) = node.operation_origin() {
                    interner.reference(&mut tape, id, origin, None)?;
                }
                for (program_node, origin) in numeric.iter().flat_map(|origins| origins.iter()) {
                    interner.reference(&mut tape, id, origin, Some(*program_node))?;
                }
                if let ExpressionNodeRef::Tape(source, source_id) = node {
                    for reference in source.operation_references(source_id) {
                        let origin = source.reference_origin(reference)?;
                        interner.reference(&mut tape, id, &origin, reference.program_node)?;
                    }
                }
                ids.insert(key, id);
                if let Some(keys) = source_keys.as_deref_mut() {
                    keys.push(key);
                }
                continue;
            }
            if ids.contains_key(&key) {
                continue;
            }
            if !active.insert(key) {
                return Err(IntentError("retained expression contains a cycle".into()));
            }
            if let ExpressionNodeRef::Tape(source, _) = node
                && verified_tapes.insert(source as *const _ as usize)
            {
                source.validate()?;
            }
            pending.push((node, true));
            match node.node()? {
                ExpressionNode::Scale { value, .. } => pending.push((value, false)),
                ExpressionNode::Transition { from, to, .. } => {
                    if let Some(to) = to {
                        pending.push((to, false));
                    }
                    if let Some(from) = from {
                        pending.push((from, false));
                    }
                }
                _ => {}
            }
        }
        tape.roots = roots
            .iter()
            .map(|root| ids[&ExpressionNodeRef::new(root).key()])
            .collect();
        tape.validate()?;
        Ok(tape)
    }

    pub fn node(&self, id: RetainedNodeId) -> Option<&RetainedExpressionNode> {
        self.nodes.get(id.0 as usize)
    }

    pub fn roots(&self) -> &[RetainedNodeId] {
        &self.roots
    }

    pub fn push_root(&mut self, root: RetainedNodeId) -> Result<(), IntentError> {
        self.require_existing(root)?;
        self.roots.push(root);
        Ok(())
    }

    pub fn replace_root(&mut self, index: usize, root: RetainedNodeId) -> Result<(), IntentError> {
        self.require_existing(root)?;
        let slot = self
            .roots
            .get_mut(index)
            .ok_or_else(|| IntentError("retained root index is absent".into()))?;
        *slot = root;
        Ok(())
    }

    /// An interruption adds one reference node. The old subtree is not cloned or resolved.
    pub fn append_resume(
        &mut self,
        from: Option<RetainedNodeId>,
        to: Option<RetainedNodeId>,
        progress: f32,
        occurrence_id: Uuid,
    ) -> Result<RetainedNodeId, IntentError> {
        if from.is_none() && to.is_none() {
            return Err(IntentError(
                "retained resume needs at least one endpoint".into(),
            ));
        }
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) || occurrence_id.is_nil() {
            return Err(IntentError(
                "retained resume progress or occurrence is invalid".into(),
            ));
        }
        for child in from.into_iter().chain(to) {
            self.require_existing(child)?;
        }
        let id = RetainedNodeId(
            u32::try_from(self.nodes.len())
                .map_err(|_| IntentError("retained expression has too many nodes".into()))?,
        );
        let node = RetainedExpressionNode::Transition {
            from,
            to,
            progress,
            reason: DynamicTransitionReason::Resume { occurrence_id },
        };
        self.nodes.push(node);
        Ok(id)
    }

    fn require_existing(&self, id: RetainedNodeId) -> Result<(), IntentError> {
        if self.node(id).is_none() {
            return Err(IntentError(
                "retained expression references an absent node".into(),
            ));
        }
        Ok(())
    }

    /// Validate every node, including currently unreachable ones. Native function bounds and
    /// complete Direct ownership are intentionally verified later with the pinned source model.
    pub fn validate(&self) -> Result<(), IntentError> {
        if !(1..=RETAINED_OPERATION_TAPE_VERSION).contains(&self.version) {
            return Err(IntentError(
                "unsupported retained expression tape version".into(),
            ));
        }
        if self.nodes.len() > u32::MAX as usize {
            return Err(IntentError("retained expression has too many nodes".into()));
        }
        let mut summaries = Vec::<AddressSummary>::with_capacity(self.nodes.len());
        for (index, node) in self.nodes.iter().enumerate() {
            if self.version < 3 && matches!(node, RetainedExpressionNode::AngleNumeric { .. }) {
                return Err(IntentError(
                    "numeric Angle expressions require retained tape version 3".into(),
                ));
            }
            if self.version == 1
                && match node {
                    RetainedExpressionNode::LegacyScalar {
                        occurrence,
                        dependency_occurrence,
                        ..
                    } => occurrence.is_some() || dependency_occurrence.is_some(),
                    RetainedExpressionNode::Programming {
                        occurrence,
                        dependency_occurrence,
                        ..
                    } => occurrence.is_some() || dependency_occurrence.is_some(),
                    RetainedExpressionNode::Scale {
                        baseline_occurrence,
                        ..
                    } => baseline_occurrence.is_some(),
                    _ => false,
                }
            {
                return Err(IntentError(
                    "version 1 retained tape cannot carry source occurrences".into(),
                ));
            }
            for child in node.children() {
                if child.0 as usize >= index {
                    return Err(IntentError("retained child must precede its parent".into()));
                }
            }
            let summary = match node {
                RetainedExpressionNode::LegacyScalar {
                    attribute,
                    value,
                    dependency_occurrence,
                    ..
                } => {
                    if !attribute_descriptor(attribute).supports_dynamics() || !value.is_finite() {
                        return Err(IntentError("retained scalar is invalid".into()));
                    }
                    if dependency_occurrence.as_ref().is_some_and(|dependency| {
                        matches!(dependency.transfer, crate::DynamicSourceTransfer::Mapped(_))
                    }) {
                        return Err(IntentError(
                            "legacy scalar dependency cannot carry family field transfers".into(),
                        ));
                    }
                    AddressSummary::Mixed
                }
                RetainedExpressionNode::Programming {
                    address,
                    value,
                    dependency_occurrence,
                    ..
                } => {
                    address.validate_value_shape(value)?;
                    if let Some(dependency) = dependency_occurrence {
                        dependency.validate(address.owner())?;
                    }
                    AddressSummary::Exact(address.clone())
                }
                RetainedExpressionNode::AngleCurrent { address } => {
                    address.validate()?;
                    if address.representation != DynamicFamilyRepresentation::Angles
                        || !matches!(
                            address.component,
                            None | Some(ProgrammingComponent::Pan | ProgrammingComponent::Tilt)
                        )
                    {
                        return Err(IntentError(
                            "retained Angle Current needs whole Angles or Pan/Tilt".into(),
                        ));
                    }
                    AddressSummary::Exact(address.clone())
                }
                RetainedExpressionNode::AngleNumeric { program } => {
                    program.validate()?;
                    if !program.operations.is_empty() {
                        return Err(IntentError(
                            "retained numeric Angle origins belong to the tape object table".into(),
                        ));
                    }
                    AddressSummary::Exact(program.address.clone())
                }
                RetainedExpressionNode::Scale {
                    address,
                    base,
                    value,
                    factor,
                    ..
                } => {
                    if address.component.is_some() || !factor.is_finite() || *factor < 0.0 {
                        return Err(IntentError("retained whole-family Size is invalid".into()));
                    }
                    address.validate()?;
                    let DynamicValue::Family(base) = base else {
                        return Err(IntentError(
                            "retained Size requires a whole-family base".into(),
                        ));
                    };
                    DynamicValueAddress::whole_family(address.owner(), base)?;
                    if summaries[value.0 as usize] != AddressSummary::Exact(address.clone()) {
                        return Err(IntentError(
                            "retained Size has a different family address".into(),
                        ));
                    }
                    AddressSummary::Exact(address.clone())
                }
                RetainedExpressionNode::Transition {
                    from,
                    to,
                    progress,
                    reason,
                } => {
                    if (from.is_none() && to.is_none())
                        || !progress.is_finite()
                        || !(0.0..=1.0).contains(progress)
                    {
                        return Err(IntentError("retained transition is invalid".into()));
                    }
                    if let DynamicTransitionReason::Resume { occurrence_id } = reason
                        && occurrence_id.is_nil()
                    {
                        return Err(IntentError(
                            "retained resume needs a stable occurrence".into(),
                        ));
                    }
                    let a =
                        from.map_or(AddressSummary::Empty, |id| summaries[id.0 as usize].clone());
                    let b = to.map_or(AddressSummary::Empty, |id| summaries[id.0 as usize].clone());
                    combine(a, b)
                }
            };
            summaries.push(summary);
        }
        for root in &self.roots {
            self.require_existing(*root)?;
        }
        self.validate_operations()
    }

    /// Object-table validation: one referenced entry per witness, ordered node references,
    /// each naming a matching original site and a validated member target.
    fn validate_operations(&self) -> Result<(), IntentError> {
        if self.emissions.is_empty() && self.operations.is_empty() {
            if self.version == RETAINED_OPERATION_TAPE_VERSION {
                return Err(IntentError(
                    "retained tape version 4 requires an operation object table".into(),
                ));
            }
            return Ok(());
        }
        if self.version < RETAINED_OPERATION_TAPE_VERSION {
            return Err(IntentError(
                "retained operation provenance requires tape version 4".into(),
            ));
        }
        let mut referenced = vec![false; self.emissions.len()];
        for emission in &self.emissions {
            emission.validate()?;
        }
        let mut previous: Option<&RetainedOperationReference> = None;
        for reference in &self.operations {
            if let Some(previous) = previous
                && (previous.node > reference.node
                    || (previous.node == reference.node && previous.site == reference.site))
            {
                return Err(IntentError(
                    "retained operation references are unordered or duplicated".into(),
                ));
            }
            previous = Some(reference);
            let node = self.node(reference.node).ok_or_else(|| {
                IntentError("retained operation references an absent node".into())
            })?;
            if !site_matches(node, reference.site, reference.program_node) {
                return Err(IntentError(
                    "retained operation site does not match its node".into(),
                ));
            }
            *referenced
                .get_mut(reference.emission as usize)
                .ok_or_else(|| {
                    IntentError("retained operation names an absent emission".into())
                })? = true;
            self.reference_origin(reference)?.validate()?;
        }
        if referenced.contains(&false) {
            return Err(IntentError(
                "retained emission table contains an unreferenced entry".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn operation_references(
        &self,
        node: RetainedNodeId,
    ) -> &[RetainedOperationReference] {
        let start = self
            .operations
            .partition_point(|reference| reference.node < node);
        let length = self.operations[start..].partition_point(|reference| reference.node == node);
        &self.operations[start..start + length]
    }

    pub(crate) fn reference_origin(
        &self,
        reference: &RetainedOperationReference,
    ) -> Result<crate::DynamicOperationOrigin, IntentError> {
        let emission = self
            .emissions
            .get(reference.emission as usize)
            .ok_or_else(|| IntentError("retained operation names an absent emission".into()))?;
        Ok(crate::DynamicOperationOrigin::restore(
            emission,
            reference.target,
            reference.lane_id,
            reference.site,
        ))
    }

    /// Guarded read-only handles of one node. Each reference is checked against its node and
    /// its witness's pinned membership before a handle is returned.
    pub fn operation_handles(
        &self,
        node: RetainedNodeId,
    ) -> Result<Vec<crate::DynamicOperationHandle>, IntentError> {
        let retained = self
            .node(node)
            .ok_or_else(|| IntentError("retained expression references an absent node".into()))?;
        self.operation_references(node)
            .iter()
            .map(|reference| {
                if !site_matches(retained, reference.site, reference.program_node) {
                    return Err(IntentError(
                        "retained operation site does not match its node".into(),
                    ));
                }
                let origin = self.reference_origin(reference)?;
                origin.validate()?;
                Ok(crate::DynamicOperationHandle::new(
                    origin,
                    reference.program_node,
                ))
            })
            .collect()
    }

    /// The distinct emission objects referenced by this tape.
    pub fn operation_emission_count(&self) -> usize {
        self.emissions.len()
    }

    /// Explicitly carry the original operations of `source` nodes onto equivalent nodes of
    /// this tape (owner splitting). Nodes whose kind no longer matches the original site are
    /// synthetic and remain unattributed; no new emission is ever created.
    pub(crate) fn inherit_operations(
        &mut self,
        source: &RetainedExpressionTape,
        mapped: &[Option<RetainedNodeId>],
    ) -> Result<(), IntentError> {
        if source.operations.is_empty() {
            return Ok(());
        }
        let mut interner = EmissionInterner::seeded(self);
        for reference in &source.operations {
            let Some(Some(node)) = mapped.get(reference.node.0 as usize).copied() else {
                continue;
            };
            let Some(target) = self.node(node) else {
                continue;
            };
            if !site_matches(target, reference.site, reference.program_node) {
                continue;
            }
            let origin = source.reference_origin(reference)?;
            interner.reference(self, node, &origin, reference.program_node)?;
        }
        self.operations.sort_by_key(|reference| reference.node);
        Ok(())
    }

    /// Per-node owner of every reachable operation reference, for restore-time row checks.
    pub(crate) fn operation_owners(&self) -> Result<Vec<RetainedOperationOwner>, IntentError> {
        let mut owners = Vec::<RetainedOperationOwner>::with_capacity(self.nodes.len());
        for (index, node) in self.nodes.iter().enumerate() {
            let mut owner = RetainedOperationOwner::Unattributed;
            for reference in self.operation_references(RetainedNodeId(index as u32)) {
                let emission =
                    self.emissions
                        .get(reference.emission as usize)
                        .ok_or_else(|| {
                            IntentError("retained operation names an absent emission".into())
                        })?;
                owner = owner.merge(RetainedOperationOwner::Exact {
                    instance_id: emission.instance_id(),
                    controller_id: emission.controller().id,
                    target: reference.target,
                    lane_id: reference.lane_id,
                });
            }
            for child in node.children() {
                owner = owner.merge(owners[child.0 as usize]);
            }
            owners.push(owner);
        }
        Ok(owners)
    }

    /// Programmer identity normalization: rewrite each shared witness once. All references
    /// name table entries, so every affected root in both held maps is rebound together.
    pub(crate) fn rekey_emission_controllers(&mut self, remap: &HashMap<Uuid, Uuid>) -> usize {
        let mut count = 0;
        for emission in &mut self.emissions {
            if let Some(&new_id) = remap.get(&emission.controller().id) {
                Arc::make_mut(emission).rekey_controller(new_id);
                count += 1;
            }
        }
        count
    }

    /// Replace each table entry by one historical object, still shared by all its references.
    pub(crate) fn restore_operation_emissions(&mut self) {
        for emission in &mut self.emissions {
            *emission = Arc::new(emission.restored_copy());
        }
    }

    pub(crate) fn operation_emissions(&self) -> &[Arc<crate::DynamicEmissionWitness>] {
        &self.emissions
    }

    /// Visit each reachable node once in topological order, with no recursion or path repeats.
    pub fn visit_reachable(
        &self,
        roots: &[RetainedNodeId],
        mut visit: impl FnMut(RetainedNodeId, &RetainedExpressionNode) -> Result<(), IntentError>,
    ) -> Result<(), IntentError> {
        let reachable = self.reachable(roots)?;
        for (index, marked) in reachable.into_iter().enumerate() {
            if marked {
                visit(RetainedNodeId(index as u32), &self.nodes[index])?;
            }
        }
        Ok(())
    }

    fn reachable(&self, roots: &[RetainedNodeId]) -> Result<Vec<bool>, IntentError> {
        let mut marks = vec![false; self.nodes.len()];
        let mut stack = roots.to_vec();
        while let Some(id) = stack.pop() {
            let Some(node) = self.node(id) else {
                return Err(IntentError(
                    "retained expression references an absent node".into(),
                ));
            };
            if std::mem::replace(&mut marks[id.0 as usize], true) {
                continue;
            }
            stack.extend(node.children());
        }
        Ok(marks)
    }

    /// Remove obsolete branches after roots change, preserving root order and shared IDs.
    /// Returns the number of removed nodes; all retained values/occurrences remain exact.
    pub fn compact_reachable(&mut self) -> Result<usize, IntentError> {
        self.validate()?;
        let reachable = self.reachable(&self.roots)?;
        let removed = reachable.iter().filter(|marked| !**marked).count();
        if removed == 0 {
            return Ok(0);
        }
        let old_nodes = std::mem::take(&mut self.nodes);
        let mut remap = vec![None; old_nodes.len()];
        for (index, mut node) in old_nodes.into_iter().enumerate() {
            if !reachable[index] {
                continue;
            }
            let map = |id: RetainedNodeId| {
                remap[id.0 as usize].expect("reachable child precedes its parent")
            };
            match &mut node {
                RetainedExpressionNode::Scale { value, .. } => *value = map(*value),
                RetainedExpressionNode::Transition { from, to, .. } => {
                    *from = from.map(map);
                    *to = to.map(map);
                }
                _ => {}
            }
            let id = RetainedNodeId(self.nodes.len() as u32);
            remap[index] = Some(id);
            self.nodes.push(node);
        }
        for root in &mut self.roots {
            *root = remap[root.0 as usize].expect("root remains reachable");
        }
        if !self.operations.is_empty() {
            // Released branches drop their references; an emission still referenced by a
            // surviving member keeps its single shared object and original membership.
            let emissions = std::mem::take(&mut self.emissions);
            let mut emission_remap = vec![None; emissions.len()];
            let operations = std::mem::take(&mut self.operations);
            for mut reference in operations {
                let Some(node) = remap[reference.node.0 as usize] else {
                    continue;
                };
                reference.node = node;
                let slot = &mut emission_remap[reference.emission as usize];
                reference.emission = *slot.get_or_insert_with(|| {
                    self.emissions
                        .push(Arc::clone(&emissions[reference.emission as usize]));
                    (self.emissions.len() - 1) as u32
                });
                self.operations.push(reference);
            }
            if self.operations.is_empty() {
                self.version = RETAINED_EXPRESSION_TAPE_VERSION;
            }
        }
        Ok(removed)
    }
}

fn site_matches(
    node: &RetainedExpressionNode,
    site: crate::DynamicOperationSite,
    program_node: Option<u32>,
) -> bool {
    use crate::{DynamicControllerSizeRole as Role, DynamicOperationSite as Site};
    match (node, site, program_node) {
        (
            RetainedExpressionNode::Transition {
                reason: DynamicTransitionReason::Required { .. },
                ..
            },
            Site::KeyframeTransition { .. },
            None,
        )
        | (
            RetainedExpressionNode::Scale { .. },
            Site::ControllerSize {
                role: Role::FamilyScale,
            },
            None,
        ) => true,
        (RetainedExpressionNode::AngleNumeric { program }, site, Some(index)) => {
            matches!(
                (program.nodes.get(index as usize), site),
                (
                    Some(crate::AngleNumericNode::Transition { .. }),
                    Site::KeyframeTransition { .. }
                ) | (
                    Some(crate::AngleNumericNode::ScaleFrom { .. }),
                    Site::ControllerSize {
                        role: Role::AngleNumericScaleFrom
                    }
                )
            )
        }
        _ => false,
    }
}

#[cfg(test)]
mod numeric_tests;
#[cfg(test)]
mod tests;
