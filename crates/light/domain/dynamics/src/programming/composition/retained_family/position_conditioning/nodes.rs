//! Original forest/member/tape locations. These paths never derive from evaluator node IDs.
use super::*;
use crate::CoupledCohortEndpoint;
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum NodeLocation {
    Tape(RetainedNodeId),
    Forest(usize),
    WholeTape(usize, RetainedNodeId),
    Member(usize, usize),
    MemberTape(usize, usize, RetainedNodeId),
}
pub(super) enum NodeDescription {
    Leaf,
    Transition {
        progress: f32,
        reason: DynamicTransitionReason,
        from: Option<NodeLocation>,
        to: Option<NodeLocation>,
    },
    Scale {
        factor: f32,
        value: Option<NodeLocation>,
    },
    Whole {
        lane_id: Uuid,
        root: NodeLocation,
    },
    SourceCohort {
        members: Vec<NodeLocation>,
    },
}
fn missing() -> TransitionError {
    IntentError("original Position subtree is absent".into()).into()
}
pub(super) fn root(captured: &CapturedProgram, index: usize) -> Option<NodeLocation> {
    if let Some(forest) = &captured.forests[index] {
        Some(NodeLocation::Forest(forest.root))
    } else {
        captured.tapes[index]
            .as_ref()
            .map(|tape| NodeLocation::Tape(tape.roots[0]))
    }
}
fn lineage(
    captured: &CapturedProgram,
    index: usize,
) -> Result<&PositionForestLineage, TransitionError> {
    captured
        .forests
        .get(index)
        .and_then(Option::as_deref)
        .ok_or_else(missing)
}
fn member(
    captured: &CapturedProgram,
    index: usize,
    forest: usize,
    member: usize,
) -> Result<&CoupledCohortEndpoint, TransitionError> {
    match lineage(captured, index)?.nodes.get(forest) {
        Some(PositionForestNode::SourceCohort(members)) => members.get(member).ok_or_else(missing),
        _ => Err(missing()),
    }
}
pub(super) fn tape(
    captured: &CapturedProgram,
    index: usize,
    node: NodeLocation,
) -> Result<(&Arc<RetainedExpressionTape>, RetainedNodeId), TransitionError> {
    match node {
        NodeLocation::Tape(root) => Ok((captured.tapes[index].as_ref().ok_or_else(missing)?, root)),
        NodeLocation::WholeTape(forest, root) => Ok((
            &lineage(captured, index)?
                .wholes
                .iter()
                .find(|whole| whole.forest_node == forest)
                .ok_or_else(missing)?
                .tape,
            root,
        )),
        NodeLocation::MemberTape(forest, member, root) => Ok((
            lineage(captured, index)?
                .cohort_members
                .iter()
                .find(|item| item.forest_node == forest && item.member_index == member)
                .and_then(|item| item.whole_tape.as_ref())
                .ok_or_else(missing)?,
            root,
        )),
        _ => Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners,
        )),
    }
}
pub(super) fn rank(
    captured: &CapturedProgram,
    index: usize,
    node: NodeLocation,
) -> Result<FamilySampleRank, TransitionError> {
    let rank = captured.sources[index].rank();
    let lane = match node {
        NodeLocation::Forest(forest) => match &lineage(captured, index)?.nodes[forest] {
            PositionForestNode::Whole { lane_id, .. } => Some(*lane_id),
            _ => None,
        },
        NodeLocation::WholeTape(forest, _) => match &lineage(captured, index)?.nodes[forest] {
            PositionForestNode::Whole { lane_id, .. } => Some(*lane_id),
            _ => return Err(missing()),
        },
        NodeLocation::Member(forest, member) | NodeLocation::MemberTape(forest, member, _) => {
            Some(match &lineage(captured, index)?.nodes[forest] {
                PositionForestNode::SourceCohort(members) => {
                    members.get(member).ok_or_else(missing)?.lane_id()
                }
                PositionForestNode::Cohort(members) => {
                    members.get(member).ok_or_else(missing)?.lane_id
                }
                _ => return Err(missing()),
            })
        }
        _ => None,
    };
    Ok(
        if let Some(lane) = lane.filter(|_| rank.dynamic_identity().is_some()) {
            rank.with_dynamic_lane(lane)?
        } else {
            rank
        },
    )
}
pub(super) fn describe(
    captured: &CapturedProgram,
    index: usize,
    node: NodeLocation,
) -> Result<NodeDescription, TransitionError> {
    match node {
        NodeLocation::Forest(forest) => match lineage(captured, index)?
            .nodes
            .get(forest)
            .ok_or_else(missing)?
        {
            PositionForestNode::Transition {
                from,
                to,
                progress,
                reason,
            } => Ok(NodeDescription::Transition {
                from: from.map(NodeLocation::Forest),
                to: to.map(NodeLocation::Forest),
                progress: *progress,
                reason: *reason,
            }),
            PositionForestNode::Whole { lane_id, .. } => {
                let whole = lineage(captured, index)?
                    .wholes
                    .iter()
                    .find(|whole| whole.forest_node == forest)
                    .ok_or_else(missing)?;
                Ok(NodeDescription::Whole {
                    lane_id: *lane_id,
                    root: NodeLocation::WholeTape(forest, whole.tape.roots[0]),
                })
            }
            PositionForestNode::SourceCohort(members) => Ok(NodeDescription::SourceCohort {
                members: (0..members.len())
                    .map(|member| NodeLocation::Member(forest, member))
                    .collect(),
            }),
            PositionForestNode::Cohort(members) => Ok(NodeDescription::SourceCohort {
                members: (0..members.len())
                    .map(|member| NodeLocation::Member(forest, member))
                    .collect(),
            }),
            PositionForestNode::AnglePair(_) => Ok(NodeDescription::Leaf),
        },
        NodeLocation::Member(forest, offset) => {
            if matches!(
                lineage(captured, index)?.nodes[forest],
                PositionForestNode::Cohort(_)
            ) {
                return Ok(NodeDescription::Leaf);
            }
            match member(captured, index, forest, offset)? {
                CoupledCohortEndpoint::Materialized(_) => Ok(NodeDescription::Leaf),
                CoupledCohortEndpoint::WholeExpression { lane_id, .. } => {
                    let tape = lineage(captured, index)?
                        .cohort_members
                        .iter()
                        .find(|member| {
                            member.forest_node == forest && member.member_index == offset
                        })
                        .and_then(|member| member.whole_tape.as_ref())
                        .ok_or_else(missing)?;
                    Ok(NodeDescription::Whole {
                        lane_id: *lane_id,
                        root: NodeLocation::MemberTape(forest, offset, tape.roots[0]),
                    })
                }
            }
        }
        _ => {
            let (tape, root) = tape(captured, index, node)?;
            let next = |root| match node {
                NodeLocation::Tape(_) => NodeLocation::Tape(root),
                NodeLocation::WholeTape(forest, _) => NodeLocation::WholeTape(forest, root),
                NodeLocation::MemberTape(forest, member, _) => {
                    NodeLocation::MemberTape(forest, member, root)
                }
                _ => unreachable!("tape location"),
            };
            Ok(match tape.node(root).ok_or_else(missing)? {
                RetainedExpressionNode::Transition {
                    from,
                    to,
                    progress,
                    reason,
                } => NodeDescription::Transition {
                    from: from.map(next),
                    to: to.map(next),
                    progress: *progress,
                    reason: *reason,
                },
                RetainedExpressionNode::Scale { value, factor, .. } => NodeDescription::Scale {
                    value: Some(next(*value)),
                    factor: *factor,
                },
                _ => NodeDescription::Leaf,
            })
        }
    }
}
pub(super) fn expression(
    captured: &CapturedProgram,
    index: usize,
    node: NodeLocation,
) -> Result<Arc<DynamicSampleExpression>, TransitionError> {
    let (tape, root) = tape(captured, index, node)?;
    Ok(Arc::new(DynamicSampleExpression::Retained {
        tape: tape.clone(),
        root,
    }))
}
pub(super) fn active_nodes(
    captured: &CapturedProgram,
    index: usize,
    root: Option<NodeLocation>,
    state: &conditioning::BranchState,
) -> Result<HashSet<NodeLocation>, TransitionError> {
    active_nodes_with_local(captured, index, root, state, &[])
}
pub(super) fn active_nodes_with_local(
    captured: &CapturedProgram,
    index: usize,
    root: Option<NodeLocation>,
    state: &conditioning::BranchState,
    local: &[LocalNodeSelection],
) -> Result<HashSet<NodeLocation>, TransitionError> {
    active_nodes_with_boundary(captured, index, root, state, local, &[])
}
pub(super) fn active_nodes_with_boundary(
    captured: &CapturedProgram,
    index: usize,
    root: Option<NodeLocation>,
    state: &conditioning::BranchState,
    local: &[LocalNodeSelection],
    member_roots: &[MemberRootSelection],
) -> Result<HashSet<NodeLocation>, TransitionError> {
    let mut active = HashSet::new();
    let mut pending = root.into_iter().collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        if !active.insert(node) {
            continue;
        }
        match describe(captured, index, node)? {
            NodeDescription::Transition {
                from,
                to,
                progress,
                reason,
            } => {
                let local_endpoint = local
                    .iter()
                    .find(|selection| selection.source_index == index && selection.node == node)
                    .map(|selection| selection.endpoint);
                let selected = match local_endpoint {
                    Some(PositionLocalEndpoint::RequiredOutgoing) => Some(false),
                    Some(PositionLocalEndpoint::RequiredIncoming) => Some(true),
                    _ => {
                        if let (Some(identity), DynamicTransitionReason::Resume { occurrence_id }) =
                            (rank(captured, index, node)?.dynamic_identity(), reason)
                        {
                            state
                                .decisions
                                .iter()
                                .find(|decision| {
                                    decision.instance_id == identity.instance_id
                                        && decision.controller_id == identity.controller_id
                                        && decision.occurrence_id == occurrence_id
                                })
                                .map(|decision| decision.incoming)
                        } else {
                            None
                        }
                    }
                };
                if progress < 1.0 && selected != Some(true) {
                    pending.extend(from)
                }
                if progress > 0.0 && selected != Some(false) {
                    pending.extend(to)
                }
            }
            NodeDescription::Scale { value, factor } if factor != 0.0 => {
                let baseline = local.iter().any(|selection| {
                    selection.source_index == index
                        && selection.node == node
                        && selection.endpoint == PositionLocalEndpoint::ScaleBaseline
                });
                if !baseline {
                    pending.extend(value);
                }
            }
            NodeDescription::Whole { root, .. } => {
                let selected = if let NodeLocation::Member(forest, member) = node {
                    member_roots
                        .iter()
                        .find(|selection| {
                            selection.source_index == index
                                && selection.forest == forest
                                && selection.member == member
                        })
                        .map(|selection| NodeLocation::MemberTape(forest, member, selection.root))
                } else {
                    None
                };
                pending.push(selected.unwrap_or(root));
            }
            NodeDescription::SourceCohort { members } => pending.extend(members),
            _ => {}
        }
    }
    Ok(active)
}
