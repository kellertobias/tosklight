//! Original forest/tape/member seeds travel through each transformation at emission time.
use super::origins::PositionSourceOriginMap;
use super::*;
use crate::CoupledCohortEndpoint;

pub(super) struct TapeSeed {
    tape: Arc<RetainedExpressionTape>,
    root: RetainedNodeId,
    prefix: NodeLocation,
}
pub(super) struct Seed {
    pub origin: NodeLocation,
    whole: Option<TapeSeed>,
    members: Vec<(NodeLocation, Option<TapeSeed>)>,
}
fn fail() -> TransitionError {
    IntentError("original Position forest origin is absent".into()).into()
}
fn member_seed(
    lineage: &PositionForestLineage,
    forest: usize,
    member: usize,
) -> Result<(NodeLocation, Option<TapeSeed>), TransitionError> {
    let metadata = lineage
        .cohort_members
        .iter()
        .find(|item| item.forest_node == forest && item.member_index == member)
        .ok_or_else(fail)?;
    let tape = metadata.whole_tape.as_ref().map(|tape| TapeSeed {
        root: tape.roots[0],
        tape: tape.clone(),
        prefix: NodeLocation::MemberTape(forest, member, tape.roots[0]),
    });
    Ok((NodeLocation::Member(forest, member), tape))
}
fn forest_seed(lineage: &PositionForestLineage, forest: usize) -> Result<Seed, TransitionError> {
    let node = lineage.nodes.get(forest).ok_or_else(fail)?;
    let whole = if matches!(node, PositionForestNode::Whole { .. }) {
        let metadata = lineage
            .wholes
            .iter()
            .find(|whole| whole.forest_node == forest)
            .ok_or_else(fail)?;
        Some(TapeSeed {
            tape: metadata.tape.clone(),
            root: metadata.tape.roots[0],
            prefix: NodeLocation::WholeTape(forest, metadata.tape.roots[0]),
        })
    } else {
        None
    };
    let members = match node {
        PositionForestNode::SourceCohort(members) => (0..members.len())
            .map(|member| member_seed(lineage, forest, member))
            .collect::<Result<Vec<_>, _>>()?,
        PositionForestNode::Cohort(members) => (0..members.len())
            .map(|member| (NodeLocation::Member(forest, member), None))
            .collect(),
        _ => Vec::new(),
    };
    Ok(Seed {
        origin: NodeLocation::Forest(forest),
        whole,
        members,
    })
}
pub(super) fn seeds(
    lineage: &PositionForestLineage,
    selected: Option<NodeLocation>,
) -> Result<Vec<Seed>, TransitionError> {
    match selected {
        None | Some(NodeLocation::Forest(_)) => (0..lineage.nodes.len())
            .map(|forest| forest_seed(lineage, forest))
            .collect(),
        Some(location @ NodeLocation::WholeTape(forest, root)) => {
            let mut seed = forest_seed(lineage, forest)?;
            let tape = seed.whole.as_mut().ok_or_else(fail)?;
            tape.root = root;
            seed.origin = location;
            Ok(vec![seed])
        }
        Some(location @ NodeLocation::Member(forest, member))
        | Some(location @ NodeLocation::MemberTape(forest, member, _)) => {
            let materialized = matches!(&lineage.nodes[forest], PositionForestNode::Cohort(_))
                || matches!(&lineage.nodes[forest],PositionForestNode::SourceCohort(members) if matches!(members[member],CoupledCohortEndpoint::Materialized(_)));
            if materialized {
                Ok(vec![Seed {
                    origin: location,
                    whole: None,
                    members: vec![(NodeLocation::Member(forest, member), None)],
                }])
            } else {
                let (_, mut tape) = member_seed(lineage, forest, member)?;
                if let NodeLocation::MemberTape(_, _, root) = location {
                    tape.as_mut().ok_or_else(fail)?.root = root;
                }
                Ok(vec![Seed {
                    origin: location,
                    whole: tape,
                    members: Vec::new(),
                }])
            }
        }
        _ => Err(fail()),
    }
}
fn original_tape_location(
    prefix: NodeLocation,
    node: RetainedNodeId,
) -> Result<NodeLocation, TransitionError> {
    Ok(match prefix {
        NodeLocation::WholeTape(forest, _) => NodeLocation::WholeTape(forest, node),
        NodeLocation::MemberTape(forest, member, _) => {
            NodeLocation::MemberTape(forest, member, node)
        }
        _ => return Err(fail()),
    })
}
fn record_tape(
    seed: &TapeSeed,
    local: impl Fn(RetainedNodeId) -> NodeLocation,
    output: &[Option<RetainedNodeId>],
    origins: &mut PositionSourceOriginMap,
) -> Result<(), TransitionError> {
    for (index, original) in output.iter().enumerate() {
        if let Some(original) = original {
            origins.insert(
                local(RetainedNodeId(index as u32)),
                original_tape_location(seed.prefix, *original)?,
            )?;
        }
    }
    Ok(())
}
fn lane_rank(rank: FamilySampleRank, lane: Uuid) -> Result<FamilySampleRank, TransitionError> {
    Ok(if rank.dynamic_identity().is_some() {
        rank.with_dynamic_lane(lane)?
    } else {
        rank
    })
}
pub(super) fn condition_branch(
    node: &PositionForestNode,
    seed: &Seed,
    forest: usize,
    rank: FamilySampleRank,
    state: &conditioning::BranchState,
    origins: &mut PositionSourceOriginMap,
    source_index: usize,
    local: &[LocalNodeSelection],
    member_roots: &[MemberRootSelection],
) -> Result<Option<PositionForestNode>, TransitionError> {
    Ok(match node {
        PositionForestNode::Whole {
            expression,
            lane_id,
            sources,
        } => {
            let tape = seed.whole.as_ref().ok_or_else(fail)?;
            let selections = tape_local_endpoints(local, source_index, tape.prefix);
            let result = conditioning::condition_tape_node_with_local(
                &tape.tape,
                tape.root,
                lane_rank(rank, *lane_id)?,
                state,
                &selections,
            )?;
            if result.expression.is_none() {
                None
            } else {
                record_tape(
                    tape,
                    |node| NodeLocation::WholeTape(forest, node),
                    &result.origins,
                    origins,
                )?;
                Some(PositionForestNode::Whole {
                    expression: if result.changed {
                        result.expression.unwrap()
                    } else {
                        expression.clone()
                    },
                    lane_id: *lane_id,
                    sources: sources.clone(),
                })
            }
        }
        PositionForestNode::SourceCohort(members) => {
            let mut updated = Vec::new();
            let mut changed = false;
            ensure(
                members.len() == seed.members.len(),
                "Position cohort origin membership differs from original source",
            )?;
            for (index, member) in members.iter().enumerate() {
                let (original, tape) = &seed.members[index];
                let next = updated.len();
                match member {
                    CoupledCohortEndpoint::Materialized(_) => {
                        origins.insert(NodeLocation::Member(forest, next), *original)?;
                        updated.push(member.clone());
                    }
                    CoupledCohortEndpoint::WholeExpression { lane_id, .. } => {
                        let tape = tape.as_ref().ok_or_else(fail)?;
                        let selections = tape_local_endpoints(local, source_index, tape.prefix);
                        let selected_root = if let NodeLocation::Member(
                            original_forest,
                            original_member,
                        ) = original
                        {
                            member_roots
                                .iter()
                                .find(|selection| {
                                    selection.source_index == source_index
                                        && selection.forest == *original_forest
                                        && selection.member == *original_member
                                })
                                .map(|selection| selection.root)
                        } else {
                            None
                        };
                        let root = selected_root.unwrap_or(tape.root);
                        let result = conditioning::condition_tape_node_with_local(
                            &tape.tape,
                            root,
                            lane_rank(rank, *lane_id)?,
                            state,
                            &selections,
                        )?;
                        if let Some(next_expression) = result.expression {
                            origins.insert(NodeLocation::Member(forest, next), *original)?;
                            record_tape(
                                tape,
                                |node| NodeLocation::MemberTape(forest, next, node),
                                &result.origins,
                                origins,
                            )?;
                            if result.changed || root != tape.root {
                                changed = true;
                                updated.push(CoupledCohortEndpoint::WholeExpression {
                                    lane_id: *lane_id,
                                    expression: Arc::new(CompiledProgrammingFamilyExpression::new(
                                        next_expression,
                                        ProgrammingOwner::Position,
                                        None,
                                        None,
                                    )?),
                                });
                            } else {
                                updated.push(member.clone())
                            }
                        } else {
                            changed = true
                        }
                    }
                }
            }
            if updated.is_empty() {
                None
            } else {
                Some(PositionForestNode::SourceCohort(if changed {
                    updated.into()
                } else {
                    members.clone()
                }))
            }
        }
        PositionForestNode::Cohort(members) => {
            ensure(
                members.len() == seed.members.len(),
                "Position cohort origin membership differs from original source",
            )?;
            for (index, (original, _)) in seed.members.iter().enumerate() {
                origins.insert(NodeLocation::Member(forest, index), *original)?;
            }
            Some(node.clone())
        }
        _ => Some(node.clone()),
    })
}
