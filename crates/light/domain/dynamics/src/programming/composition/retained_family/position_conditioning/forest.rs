//! Condition original immutable forests, preserving every unchanged captured endpoint Arc.
use super::*;
use crate::CoupledCohortEndpoint;
fn selected(
    state: &conditioning::BranchState,
    rank: FamilySampleRank,
    progress: f32,
    reason: DynamicTransitionReason,
) -> Option<bool> {
    let choice = if let (Some(identity), DynamicTransitionReason::Resume { occurrence_id }) =
        (rank.dynamic_identity(), reason)
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
    };
    choice.or(if progress == 0.0 {
        Some(false)
    } else if progress == 1.0 {
        Some(true)
    } else {
        None
    })
}
fn override_node(
    lineage: &PositionForestLineage,
    node: NodeLocation,
) -> Result<PositionForestNode, TransitionError> {
    match node {
        NodeLocation::WholeTape(forest, root) => {
            let PositionForestNode::Whole {
                lane_id, sources, ..
            } = &lineage.nodes[forest]
            else {
                unreachable!("validated Whole")
            };
            let whole = lineage
                .wholes
                .iter()
                .find(|whole| whole.forest_node == forest)
                .expect("original whole tape");
            Ok(PositionForestNode::Whole {
                lane_id: *lane_id,
                sources: sources.clone(),
                expression: Arc::new(DynamicSampleExpression::Retained {
                    tape: whole.tape.clone(),
                    root,
                }),
            })
        }
        NodeLocation::Member(forest, member) | NodeLocation::MemberTape(forest, member, _) => {
            let endpoint = match &lineage.nodes[forest] {
                PositionForestNode::Cohort(members) => {
                    return Ok(PositionForestNode::Cohort(Arc::from([
                        members[member].clone()
                    ])));
                }
                PositionForestNode::SourceCohort(members) => &members[member],
                _ => unreachable!("validated original member"),
            };
            match endpoint {
                CoupledCohortEndpoint::Materialized(endpoint) => {
                    Ok(PositionForestNode::Cohort(Arc::from([endpoint.clone()])))
                }
                CoupledCohortEndpoint::WholeExpression {
                    lane_id,
                    expression,
                } => {
                    let expression = if let NodeLocation::MemberTape(_, _, root) = node {
                        let tape = lineage
                            .cohort_members
                            .iter()
                            .find(|member| {
                                member.forest_node == forest
                                    && member.member_index == member_index(node)
                            })
                            .and_then(|member| member.whole_tape.as_ref())
                            .expect("original member Whole tape");
                        Arc::new(DynamicSampleExpression::Retained {
                            tape: tape.clone(),
                            root,
                        })
                    } else {
                        Arc::new(expression.expression().clone())
                    };
                    Ok(PositionForestNode::Whole {
                        lane_id: *lane_id,
                        expression,
                        sources: Arc::from([]),
                    })
                }
            }
        }
        _ => Err(
            IntentError("Position forest override is not an original forest location".into())
                .into(),
        ),
    }
}
fn member_index(node: NodeLocation) -> usize {
    match node {
        NodeLocation::Member(_, member) | NodeLocation::MemberTape(_, member, _) => member,
        _ => unreachable!("member node"),
    }
}
pub(super) fn condition(
    source: &FamilyCompositionSample,
    lineage: &Arc<PositionForestLineage>,
    state: &conditioning::BranchState,
    selected_node: Option<NodeLocation>,
) -> Result<Option<FamilyCompositionSample>, TransitionError> {
    condition_with_origins(source, lineage, state, selected_node, 0, &[], &[])
        .map(|result| result.map(|(sample, _)| sample))
}

pub(super) fn condition_with_origins(
    source: &FamilyCompositionSample,
    lineage: &Arc<PositionForestLineage>,
    state: &conditioning::BranchState,
    selected_node: Option<NodeLocation>,
    source_index: usize,
    local: &[LocalNodeSelection],
    member_roots: &[MemberRootSelection],
) -> Result<
    Option<(
        FamilyCompositionSample,
        super::origins::PositionSourceOriginMap,
    )>,
    TransitionError,
> {
    let FamilyCompositionSample::CoupledExpression {
        expression,
        rank,
        activation_mix,
    } = source
    else {
        unreachable!("captured forest source")
    };
    let (original, root) = match selected_node {
        None => (lineage.nodes.clone(), lineage.root),
        Some(NodeLocation::Forest(root)) => (lineage.nodes.clone(), root),
        Some(node) => (Arc::from([override_node(lineage, node)?]), 0),
    };
    let seeds = super::forest_origins::seeds(lineage, selected_node)?;
    let mut origins = super::origins::PositionSourceOriginMap::default();
    let mut mapped = Vec::<Option<usize>>::with_capacity(original.len());
    let mut forest = Vec::new();
    let mut changed = selected_node.is_some();
    for (index, node) in original.iter().enumerate() {
        match node {
            PositionForestNode::Transition {
                from,
                to,
                progress,
                reason,
            } => {
                let a = from.and_then(|id| mapped[id]);
                let b = to.and_then(|id| mapped[id]);
                let local_endpoint = local
                    .iter()
                    .find(|choice| {
                        choice.source_index == source_index && choice.node == seeds[index].origin
                    })
                    .map(|choice| choice.endpoint);
                let endpoint = match local_endpoint {
                    Some(PositionLocalEndpoint::RequiredOutgoing) => Some(false),
                    Some(PositionLocalEndpoint::RequiredIncoming) => Some(true),
                    _ => selected(state, *rank, *progress, *reason),
                };
                if let Some(incoming) = endpoint {
                    mapped.push(if incoming { b } else { a });
                    changed = true;
                } else if a.is_none() && b.is_none() {
                    mapped.push(None);
                    changed = true
                } else {
                    let next = forest.len();
                    origins.insert(NodeLocation::Forest(next), seeds[index].origin)?;
                    forest.push(PositionForestNode::Transition {
                        from: a,
                        to: b,
                        progress: *progress,
                        reason: *reason,
                    });
                    mapped.push(Some(next));
                }
            }
            _ => {
                let next = forest.len();
                let conditioned = super::forest_origins::condition_branch(
                    node,
                    &seeds[index],
                    next,
                    *rank,
                    state,
                    &mut origins,
                    source_index,
                    local,
                    member_roots,
                )?;
                if let Some(node) = conditioned {
                    origins.insert(NodeLocation::Forest(next), seeds[index].origin)?;
                    forest.push(node);
                    mapped.push(Some(next));
                } else {
                    mapped.push(None);
                    changed = true
                }
            }
        }
        // Recompilation is cold; structural equality below preserves exact original compiled
        // identity whenever choices do not affect this source, including nested member views.
        if mapped[index] != Some(index) {
            changed = true
        }
    }
    let Some(root) = mapped[root] else {
        return Ok(None);
    };
    if !changed && same_forest(&original, &forest) {
        return Ok(Some((source.clone(), origins)));
    }
    let crate::CoupledRetainedSources::PositionForest(retained) = expression.retained_sources()
    else {
        unreachable!("forest original samples")
    };
    let conditioned = Arc::new(CompiledCoupledExpression::from_position_forest(
        retained.clone(),
        &forest,
        root,
    )?);
    Ok(Some((
        FamilyCompositionSample::CoupledExpression {
            expression: conditioned,
            rank: *rank,
            activation_mix: *activation_mix,
        },
        origins,
    )))
}
fn same_forest(original: &[PositionForestNode], next: &[PositionForestNode]) -> bool {
    original.len() == next.len()
        && original.iter().zip(next).all(|(a, b)| match (a, b) {
            (PositionForestNode::AnglePair(a), PositionForestNode::AnglePair(b)) => {
                Arc::ptr_eq(a, b)
            }
            (
                PositionForestNode::Whole {
                    expression: a,
                    lane_id: la,
                    sources: sa,
                },
                PositionForestNode::Whole {
                    expression: b,
                    lane_id: lb,
                    sources: sb,
                },
            ) => la == lb && Arc::ptr_eq(a, b) && Arc::ptr_eq(sa, sb),
            (PositionForestNode::Cohort(a), PositionForestNode::Cohort(b)) => Arc::ptr_eq(a, b),
            (PositionForestNode::SourceCohort(a), PositionForestNode::SourceCohort(b)) => {
                Arc::ptr_eq(a, b)
            }
            (
                PositionForestNode::Transition {
                    from: fa,
                    to: ta,
                    progress: pa,
                    reason: ra,
                },
                PositionForestNode::Transition {
                    from: fb,
                    to: tb,
                    progress: pb,
                    reason: rb,
                },
            ) => fa == fb && ta == tb && pa == pb && ra == rb,
            _ => false,
        })
}
