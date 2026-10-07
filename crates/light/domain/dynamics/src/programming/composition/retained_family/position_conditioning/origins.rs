//! Explicit conditioned-local to original-captured locations. Keys are routing metadata,
//! never inferred from equal operands, ranks, progress, or compiled node aliases.
use super::super::base_evaluation::{BaseMaterializationOperation, BaseMaterializationRequest};
use super::*;

#[derive(Clone, Default)]
pub(super) struct PositionSourceOriginMap {
    locations: HashMap<NodeLocation, NodeLocation>,
}
impl PositionSourceOriginMap {
    pub fn insert(
        &mut self,
        local: NodeLocation,
        original: NodeLocation,
    ) -> Result<(), TransitionError> {
        if let Some(previous) = self.locations.insert(local, original) {
            ensure(
                previous == original,
                "one conditioned Position location has conflicting original origins",
            )?;
        }
        Ok(())
    }
    pub fn get(&self, local: NodeLocation) -> Option<NodeLocation> {
        self.locations.get(&local).copied()
    }
    pub fn identity(captured: &CapturedProgram, index: usize) -> Self {
        Self {
            locations: captured.active[index]
                .iter()
                .copied()
                .map(|node| (node, node))
                .collect(),
        }
    }
}

pub(super) struct BoundPositionSource {
    pub original_index: usize,
    pub sample: FamilyCompositionSample,
    pub origins: PositionSourceOriginMap,
}

fn tape_location(
    location: NodeLocation,
    node: RetainedNodeId,
) -> Result<NodeLocation, TransitionError> {
    Ok(match location {
        NodeLocation::Tape(_) => NodeLocation::Tape(node),
        NodeLocation::WholeTape(forest, _) => NodeLocation::WholeTape(forest, node),
        NodeLocation::MemberTape(forest, member, _) => {
            NodeLocation::MemberTape(forest, member, node)
        }
        _ => {
            return Err(
                IntentError("Position origin is not an original tape location".into()).into(),
            );
        }
    })
}

impl PositionProgramBranch {
    pub(super) fn conditioned_with_origins(
        &self,
    ) -> Result<Vec<BoundPositionSource>, TransitionError> {
        let mut output = Vec::new();
        let models = conditioning::OriginalModels(&self.captured.sources);
        for (index, source) in self.captured.sources.iter().enumerate() {
            let selected = self.selected_nodes.iter().find(|(key, _)| *key == index);
            if matches!(selected, Some((_, None))) {
                continue;
            }
            if self.state.decisions.is_empty()
                && selected.is_none()
                && self
                    .local_endpoints
                    .iter()
                    .all(|choice| choice.source_index != index)
                && self
                    .member_roots
                    .iter()
                    .all(|choice| choice.source_index != index)
            {
                output.push(BoundPositionSource {
                    original_index: index,
                    sample: source.clone(),
                    origins: PositionSourceOriginMap::identity(&self.captured, index),
                });
                continue;
            }
            if let Some(lineage) = &self.captured.forests[index] {
                if let Some((sample, origins)) = forest::condition_with_origins(
                    source,
                    lineage,
                    &self.state,
                    selected.and_then(|(_, node)| *node),
                    index,
                    &self.local_endpoints,
                    &self.member_roots,
                )? {
                    output.push(BoundPositionSource {
                        original_index: index,
                        sample,
                        origins,
                    });
                }
                continue;
            }
            let location = selected
                .and_then(|(_, node)| *node)
                .or_else(|| nodes::root(&self.captured, index));
            let Some(location) = location else {
                output.push(BoundPositionSource {
                    original_index: index,
                    sample: source.clone(),
                    origins: PositionSourceOriginMap::default(),
                });
                continue;
            };
            let (tape, root) = nodes::tape(&self.captured, index, location)?;
            let local = tape_local_endpoints(&self.local_endpoints, index, location);
            let conditioned = conditioning::condition_tape_node_with_local(
                tape,
                root,
                source.rank(),
                &self.state,
                &local,
            )?;
            let Some(expression) = conditioned.expression else {
                continue;
            };
            let sample = if !conditioned.changed && selected.is_none() {
                source.clone()
            } else {
                conditioning::reconstruct(source, expression, &models)?
            };
            let mut origins = PositionSourceOriginMap::default();
            for (local, original) in conditioned.origins.into_iter().enumerate() {
                if let Some(original) = original {
                    origins.insert(
                        NodeLocation::Tape(RetainedNodeId(local as u32)),
                        tape_location(location, original)?,
                    )?;
                }
            }
            output.push(BoundPositionSource {
                original_index: index,
                sample,
                origins,
            });
        }
        Ok(output)
    }
}

/// Owns the captured program, conditioned source views and explicit preparation identities.
/// No borrowed frame/context or producer clock can escape into this binding.
pub(crate) struct PositionOriginBinding {
    captured: Arc<CapturedProgram>,
    sources: Vec<BoundPositionSource>,
}
impl PositionProgramBranch {
    pub fn begin_composition(
        &self,
        context: &FamilyCompositionContext<'_>,
        scratch: RetainedFamilyCompositionScratch,
        tracing: bool,
    ) -> Result<PositionCompositionContinuation, TransitionError> {
        let sources = self.conditioned_with_origins()?;
        let mut scratch = scratch;
        let base_trace = prepare_family_inputs_with_origins(
            ProgrammingOwner::Position,
            &self.captured.base,
            sources
                .iter()
                .map(|source| (source.sample.clone(), vec![source.original_index])),
            context,
            &mut scratch,
            tracing,
        )?;
        let mut evaluation = PositionCompositionContinuation::prepared(
            self.captured.capture_id,
            &self.captured.base,
            base_trace,
            scratch,
        )?;
        evaluation.bind_origins(PositionOriginBinding {
            captured: self.captured.clone(),
            sources,
        });
        Ok(evaluation)
    }
}
impl PositionOriginBinding {
    fn source(&self, index: usize) -> Result<&BoundPositionSource, TransitionError> {
        self.sources
            .iter()
            .find(|source| source.original_index == index)
            .ok_or_else(|| {
                IntentError("Position request refers to a released original source".into()).into()
            })
    }
    fn handle(
        &self,
        source: &BoundPositionSource,
        local: NodeLocation,
    ) -> Result<Option<PositionSourceNode>, TransitionError> {
        let Some(original) = source.origins.get(local) else {
            return Ok(None);
        };
        let handle = PositionSourceNode {
            captured: self.captured.clone(),
            source_index: source.original_index,
            node: original,
        };
        validate_handle(&self.captured, &handle)?;
        Ok(Some(handle))
    }
    fn coupled_location(
        expression: &CompiledCoupledExpression,
        node: usize,
    ) -> Option<NodeLocation> {
        use crate::programming::expression_coupled::PositionCompiledNodeLocation as L;
        expression
            .position_node_origin(node)
            .map(|location| match location {
                L::Tape(node) => NodeLocation::Tape(node),
                L::Forest(forest) => NodeLocation::Forest(forest),
                L::WholeTape(forest, node) => NodeLocation::WholeTape(forest, node),
            })
    }
    fn prepared_whole_location(
        &self,
        origin: PreparedSourceOrigin,
        node: RetainedNodeId,
    ) -> Result<Option<NodeLocation>, TransitionError> {
        let source = self.source(origin.original_index)?;
        if let Some(member) = origin.member {
            let FamilyCompositionSample::CoupledExpression { expression, .. } = &source.sample
            else {
                return Err(IntentError(
                    "expanded Position member has no original coupled source".into(),
                )
                .into());
            };
            let Some(NodeLocation::Forest(forest)) =
                Self::coupled_location(expression, expression.trace_root_node())
            else {
                return Err(IntentError(
                    "expanded Position member has no explicit forest root origin".into(),
                )
                .into());
            };
            Ok(Some(NodeLocation::MemberTape(forest, member, node)))
        } else {
            Ok(Some(NodeLocation::Tape(node)))
        }
    }
    fn base_node(
        &self,
        prepared: &FamilyCompositionSample,
        origins: &[PreparedSourceOrigin],
        request: &BaseMaterializationRequest,
    ) -> Result<Option<PositionSourceNode>, TransitionError> {
        // Aggregate inputs, synthetic completion and component envelopes do not identify one
        // original retained operation. The complete source indices remain available separately.
        if origins.len() != 1 {
            return Ok(None);
        }
        let source = self.source(origins[0].original_index)?;
        match &request.operation {
            BaseMaterializationOperation::Coupled(inner) => {
                let FamilyCompositionSample::CoupledExpression { expression, .. } = prepared else {
                    return Err(IntentError(
                        "Position graph request source is not a coupled expression".into(),
                    )
                    .into());
                };
                if let Some(location) = Self::coupled_location(expression, inner.node) {
                    self.handle(source, location)
                } else {
                    Ok(None)
                }
            }
            BaseMaterializationOperation::Whole(inner) => {
                let FamilyCompositionSample::WholeExpression { expression, .. } = prepared else {
                    return Err(IntentError(
                        "Position whole request source is not a whole expression".into(),
                    )
                    .into());
                };
                let Some(node) = expression.retained_node_origin(inner.node) else {
                    return Ok(None);
                };
                match self.prepared_whole_location(origins[0], node)? {
                    Some(location) => self.handle(source, location),
                    None => Ok(None),
                }
            }
            BaseMaterializationOperation::SourceCohort {
                endpoint_index,
                request: child,
                ..
            } => {
                let FamilyCompositionSample::CoupledExpression { expression, .. } = prepared else {
                    return Err(IntentError(
                        "Position source cohort request source is not a coupled expression".into(),
                    )
                    .into());
                };
                let endpoint = expression
                    .base_endpoints()
                    .get(*endpoint_index)
                    .ok_or_else(|| {
                        IntentError("Position request endpoint origin is absent".into())
                    })?;
                let graph_nodes =
                    expression
                        .base_endpoint_nodes(*endpoint_index)
                        .ok_or_else(|| {
                            IntentError("Position request endpoint node origins are absent".into())
                        })?;
                // Value-deduplicated endpoints retain all contributors. Ambiguity cannot
                // authorize a single original history node.
                if graph_nodes.len() != 1 || child.source_origins.len() != 1 {
                    return Ok(None);
                }
                let Some(NodeLocation::Forest(forest)) =
                    Self::coupled_location(expression, graph_nodes[0])
                else {
                    return Err(IntentError(
                        "Position source cohort has no explicit original forest operation".into(),
                    )
                    .into());
                };
                let member = child.source_origins[0];
                ensure(
                    member.member.is_none(),
                    "nested Position cohort preparation changed its original member path",
                )?;
                let members = endpoint.cohort_sources().ok_or_else(|| {
                    IntentError("Position source cohort endpoint is not a member list".into())
                })?;
                let member_index = member.original_index;
                let member = members.get(member_index).ok_or_else(|| {
                    IntentError("Position original cohort member is absent".into())
                })?;
                match (&child.operation,member) {
                    (BaseMaterializationOperation::Whole(inner),crate::CoupledCohortEndpoint::WholeExpression { expression,.. })=> {
                        let Some(node)=expression.retained_node_origin(inner.node) else { return Ok(None) };
                        self.handle(source,NodeLocation::MemberTape(forest,member_index,node))
                    },
                    (BaseMaterializationOperation::Completion(_)|BaseMaterializationOperation::Segment(_),_)=>Ok(None),
                    _=>Err(IntentError("Position nested request has no supported explicit original operation route".into()).into()),
                }
            }
            BaseMaterializationOperation::Completion(_)
            | BaseMaterializationOperation::Segment(_) => Ok(None),
        }
    }
    pub(crate) fn request_origin(
        &self,
        operation: &PositionCompositionOperation,
        scratch: &RetainedFamilyCompositionScratch,
    ) -> Result<PositionCompositionOrigin, TransitionError> {
        let (origins, node) = match operation {
            PositionCompositionOperation::Base {
                source_index,
                request,
                ..
            } => {
                let origins = &request.source_origins;
                let node = if matches!(request.operation, BaseMaterializationOperation::Segment(_))
                {
                    None
                } else {
                    let prepared = scratch.sources.get(*source_index).ok_or_else(|| {
                        IntentError("Position prepared request source is absent".into())
                    })?;
                    self.base_node(prepared, origins, request)?
                };
                (origins.clone(), node)
            }
            PositionCompositionOperation::MaskAdoption { source_index, .. }
            | PositionCompositionOperation::MaskTransition { source_index, .. } => (
                scratch
                    .source_origins
                    .get(*source_index)
                    .cloned()
                    .ok_or_else(|| IntentError("Position mask source origins are absent".into()))?,
                None,
            ),
        };
        let mut original_indices = Vec::new();
        for origin in origins {
            self.source(origin.original_index)?;
            if !original_indices.contains(&origin.original_index) {
                original_indices.push(origin.original_index)
            }
        }
        ensure(
            !original_indices.is_empty(),
            "bound Position request has no original contributing source",
        )?;
        Ok(PositionCompositionOrigin {
            captured: self.captured.clone(),
            original_indices,
            node,
        })
    }
}

mod stage;
pub use stage::{PositionStageKind, PositionStageLocator, PositionStageOperand};

mod graph;
pub use graph::{
    PositionGraphOperationKind, PositionGraphOperationLocator, PositionGraphOperationOperand,
};

mod resume;
pub use resume::PositionResumeOperandLocator;
pub(in crate::programming::composition) use resume::{ResumeGoal, ResumeStop};
