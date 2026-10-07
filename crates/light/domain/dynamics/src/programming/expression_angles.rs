//! Reconstruct controller-wide Position branches before owner arbitration. Per-lane resume
//! samples must not be flattened into values: old and new memberships are complete sources.
//! Pairing operates on flat node IDs so interrupted histories neither recurse nor expand trees.
use super::{
    DynamicFamilyRepresentation, DynamicSampleExpression as Expression, DynamicTransitionReason,
    DynamicValue, DynamicValueAddress, DynamicValueSourceResolver, RetainedExpressionNode as Node,
    RetainedExpressionTape, RetainedNodeId, address::ensure,
};
use crate::DynamicRuntimeSample;
use light_core::{AttributeValue, FixtureId, programming::*};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::sync::Arc;
use uuid::Uuid;
mod forest;
pub use forest::{
    PositionComponentForestBundle, bundle_position_component_forest, set_plan_verification,
};

/// Replace the input source set with these samples; do not keep its original Position lanes.
/// Non-Position fragments retain their own addresses, component masks and resume influence.
pub struct PositionSampleBundle {
    pub position: Option<DynamicRuntimeSample>,
    pub remainder: Vec<DynamicRuntimeSample>,
}

/// All samples must belong to one instance/controller/target at one timestamp. Current comes
/// from the immutable pre-Dynamic frame, independently from the eligible composition underlay.
/// Call again each output tick, including while the controller's phase is paused. The returned
/// complete expression goes through family evaluation and competes with other controllers once.
/// Target component forests require the component compositor; this function refuses to promote
/// them into complete masks. Whole Target expressions can cross an Angle branch intact.
pub fn bundle_position_sample_expressions(
    samples: &[DynamicRuntimeSample],
    sources: &dyn DynamicValueSourceResolver,
) -> Result<PositionSampleBundle, TransitionError> {
    let Some(first) = samples.first() else {
        return Ok(PositionSampleBundle {
            position: None,
            remainder: vec![],
        });
    };
    // This legacy single-expression bundle numerically collapses the two axis leaves.
    // Provenance-bearing samples must use the component forest, which retains both.
    if samples.iter().any(|sample| {
        sample.expression.has_source_occurrences() || sample.expression.has_angle_numeric()
    }) {
        return Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners,
        ));
    }
    let mut lanes = HashSet::with_capacity_and_hasher(samples.len(), Default::default());
    for sample in samples {
        ensure(
            sample.instance_id == first.instance_id
                && sample.controller_id == first.controller_id
                && sample.target == first.target
                && sample.priority == first.priority
                && sample.activated_at_millis == first.activated_at_millis
                && sample.activation_mix == first.activation_mix,
            "Position branches require one controller, target, rank and activation influence",
        )?;
        ensure(
            lanes.insert(sample.lane_id),
            "duplicate Dynamic source lane",
        )?;
    }
    let expressions = samples
        .iter()
        .map(|sample| Arc::new(sample.expression.clone()))
        .collect::<Vec<_>>();
    let mut tape = RetainedExpressionTape::from_roots(&expressions)?;
    let roots = tape.roots.clone();
    let metadata = position_metadata(&tape);
    let position_root = bundle(&roots, &mut tape, &metadata, first.target, sources)?;
    let non_position = non_position_roots(&mut tape, &metadata)?;
    let remainder_indices = roots
        .iter()
        .enumerate()
        .filter_map(|(index, root)| non_position[root.0 as usize].map(|root| (index, root)))
        .collect::<Vec<_>>();

    tape.roots.clear();
    tape.roots.extend(position_root);
    tape.roots
        .extend(remainder_indices.iter().map(|(_, root)| *root));
    tape.compact_reachable()?;
    let tape = Arc::new(tape);
    let position = position_root.map(|_| {
        let index = roots
            .iter()
            .enumerate()
            .filter(|(_, root)| metadata[root.0 as usize].contains)
            .max_by_key(|(index, _)| samples[*index].lane_id)
            .expect("Position expression has a source")
            .0;
        let mut result = samples[index].clone();
        result.expression = Expression::Retained {
            tape: tape.clone(),
            root: tape.roots[0],
        };
        // The winning retained lane may now address a different owner. Re-intern the final owner.
        result.address = None;
        result
    });
    let remainder = remainder_indices
        .iter()
        .enumerate()
        .map(|(offset, (index, _))| {
            let mut result = samples[*index].clone();
            result.expression = Expression::Retained {
                tape: tape.clone(),
                root: tape.roots[offset + usize::from(position_root.is_some())],
            };
            if metadata[roots[*index].0 as usize].contains {
                result.address = None;
            }
            result
        })
        .collect();
    Ok(PositionSampleBundle {
        position,
        remainder,
    })
}

#[derive(Clone, Copy)]
struct PositionMetadata {
    contains: bool,
    complete: bool,
}

fn position_metadata(tape: &RetainedExpressionTape) -> Vec<PositionMetadata> {
    let mut result: Vec<PositionMetadata> = Vec::with_capacity(tape.nodes.len());
    for node in &tape.nodes {
        let metadata = match node {
            Node::AngleCurrent { .. } | Node::AngleNumeric { .. } => PositionMetadata {
                contains: true,
                complete: false,
            },
            Node::Programming { address, .. } | Node::Scale { address, .. } => {
                let contains = address.owner() == ProgrammingOwner::Position;
                PositionMetadata {
                    contains,
                    complete: contains && address.component.is_none(),
                }
            }
            Node::Transition { from, to, .. } => PositionMetadata {
                contains: from
                    .iter()
                    .chain(to)
                    .any(|id| result[id.0 as usize].contains),
                complete: from
                    .iter()
                    .chain(to)
                    .all(|id| result[id.0 as usize].complete),
            },
            Node::LegacyScalar { .. } => PositionMetadata {
                contains: false,
                complete: false,
            },
        };
        result.push(metadata);
    }
    result
}

fn append(tape: &mut RetainedExpressionTape, node: Node) -> Result<RetainedNodeId, IntentError> {
    let id = RetainedNodeId(
        u32::try_from(tape.nodes.len())
            .map_err(|_| IntentError("retained Position expression has too many nodes".into()))?,
    );
    tape.nodes.push(node);
    Ok(id)
}

fn non_position_roots(
    tape: &mut RetainedExpressionTape,
    metadata: &[PositionMetadata],
) -> Result<Vec<Option<RetainedNodeId>>, IntentError> {
    let mut result = Vec::with_capacity(metadata.len());
    for (index, metadata) in metadata.iter().enumerate() {
        let root = if !metadata.contains {
            Some(RetainedNodeId(index as u32))
        } else if let Node::Transition {
            from,
            to,
            progress,
            reason,
        } = tape.nodes[index].clone()
        {
            let from = from.and_then(|id| result[id.0 as usize]);
            let to = to.and_then(|id| result[id.0 as usize]);
            if from.is_some() || to.is_some() {
                Some(append(
                    tape,
                    Node::Transition {
                        from,
                        to,
                        progress,
                        reason,
                    },
                )?)
            } else {
                None
            }
        } else {
            None
        };
        result.push(root);
    }
    Ok(result)
}

enum PairTask {
    Visit(Vec<RetainedNodeId>),
    Alias {
        roots: Vec<RetainedNodeId>,
        child: Vec<RetainedNodeId>,
    },
    Join {
        roots: Vec<RetainedNodeId>,
        from: Vec<RetainedNodeId>,
        to: Vec<RetainedNodeId>,
        progress: f32,
        occurrence: Uuid,
    },
}

fn bundle(
    roots: &[RetainedNodeId],
    tape: &mut RetainedExpressionTape,
    metadata: &[PositionMetadata],
    target: FixtureId,
    sources: &dyn DynamicValueSourceResolver,
) -> Result<Option<RetainedNodeId>, TransitionError> {
    let mut results = HashMap::<Vec<RetainedNodeId>, Option<RetainedNodeId>>::default();
    let mut stack = vec![PairTask::Visit(roots.to_vec())];
    // The caller supplies one coherent frame. Query each Current component only if a visible
    // branch needs it, then share that frame value across every interrupted occurrence.
    let mut current = [None, None, None];
    while let Some(task) = stack.pop() {
        match task {
            PairTask::Alias { roots, child } => {
                results.insert(roots, results[&child]);
            }
            PairTask::Join {
                roots,
                from,
                to,
                progress,
                occurrence,
            } => {
                let from = results[&from];
                let to = results[&to];
                let value = if from.is_some() || to.is_some() {
                    Some(tape.append_resume(from, to, progress, occurrence)?)
                } else {
                    None
                };
                results.insert(roots, value);
            }
            PairTask::Visit(roots) => {
                if results.contains_key(&roots) {
                    continue;
                }
                // Required keyframe transitions remain local; only shared Resume occurrences
                // describe old/new controller membership and can be zipped across lanes.
                let resume = roots
                    .iter()
                    .filter(|id| metadata[id.0 as usize].contains)
                    .find_map(|id| match &tape.nodes[id.0 as usize] {
                        Node::Transition {
                            reason: DynamicTransitionReason::Resume { occurrence_id },
                            progress,
                            ..
                        } => Some((*occurrence_id, *progress)),
                        _ => None,
                    });
                let Some((occurrence, progress)) = resume else {
                    let value = pair_branch(&roots, tape, metadata, target, sources, &mut current)?;
                    results.insert(roots, value);
                    continue;
                };
                let mut from = Vec::with_capacity(roots.len());
                let mut to = Vec::with_capacity(roots.len());
                for root in &roots {
                    if let Node::Transition {
                        from: old,
                        to: new,
                        progress: other_progress,
                        reason: DynamicTransitionReason::Resume { occurrence_id },
                    } = &tape.nodes[root.0 as usize]
                        && *occurrence_id == occurrence
                    {
                        ensure(
                            *other_progress == progress,
                            "one resume occurrence has different progress",
                        )?;
                        from.extend(old);
                        to.extend(new);
                    } else {
                        ensure(
                            !metadata[root.0 as usize].contains
                                || !matches!(
                                    &tape.nodes[root.0 as usize],
                                    Node::Transition {
                                        reason: DynamicTransitionReason::Resume { .. },
                                        ..
                                    }
                                ),
                            "Position resume sources have different outer occurrences",
                        )?;
                        from.push(*root);
                        to.push(*root);
                    }
                }
                if progress == 0.0 || progress == 1.0 {
                    let child = if progress == 0.0 { from } else { to };
                    stack.push(PairTask::Alias {
                        roots,
                        child: child.clone(),
                    });
                    stack.push(PairTask::Visit(child));
                } else {
                    stack.push(PairTask::Join {
                        roots,
                        from: from.clone(),
                        to: to.clone(),
                        progress,
                        occurrence,
                    });
                    stack.push(PairTask::Visit(to));
                    stack.push(PairTask::Visit(from));
                }
            }
        }
    }
    Ok(results[roots])
}

fn pair_branch(
    roots: &[RetainedNodeId],
    tape: &mut RetainedExpressionTape,
    metadata: &[PositionMetadata],
    target: FixtureId,
    sources: &dyn DynamicValueSourceResolver,
    current: &mut [Option<Option<DynamicValue>>; 3],
) -> Result<Option<RetainedNodeId>, TransitionError> {
    let mut axes = [None, None];
    let mut seen = [false, false];
    let mut whole = None;
    let mut whole_seen = false;
    let mut used_current = false;
    for root in roots.iter().filter(|id| metadata[id.0 as usize].contains) {
        match tape.nodes[root.0 as usize].clone() {
            Node::AngleCurrent { address } => {
                let dependency = sources.current_dependency(target, &address);
                if sources
                    .current_family_occurrence(target, &address)
                    .is_some()
                    || dependency.occurrence.is_some()
                {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::CompatibleOwners,
                    ));
                }
                address.validate()?;
                if address.component.is_none() {
                    ensure(
                        address.representation == DynamicFamilyRepresentation::Angles,
                        "live Angle Current has a different representation",
                    )?;
                    ensure(
                        !std::mem::replace(&mut whole_seen, true),
                        "multiple complete Position sources in one branch",
                    )?;
                    if let Some(value) =
                        current[2].get_or_insert_with(|| sources.current(target, &address))
                    {
                        address.validate_value_shape(value)?;
                        whole = Some(append(
                            tape,
                            Node::Programming {
                                address,
                                value: value.clone(),
                                occurrence: None,
                                dependency_occurrence: Some(dependency),
                            },
                        )?);
                    }
                    continue;
                }
                let axis = axis(&address)?;
                ensure(
                    !std::mem::replace(&mut seen[axis], true),
                    "duplicate axis in one Angle branch",
                )?;
                if let Some(value) =
                    current[axis].get_or_insert_with(|| sources.current(target, &address))
                {
                    address.validate_value_shape(value)?;
                    let DynamicValue::Scalar(value) = value else {
                        unreachable!("validated Angle Current")
                    };
                    axes[axis] = Some(*value);
                    used_current = true;
                }
            }
            Node::Programming { address, value, .. } if address.component.is_some() => {
                let axis = axis(&address)?;
                address.validate_value_shape(&value)?;
                ensure(
                    !std::mem::replace(&mut seen[axis], true),
                    "duplicate axis in one Angle branch",
                )?;
                let DynamicValue::Scalar(value) = value else {
                    unreachable!("validated Angle component")
                };
                axes[axis] = Some(value);
            }
            _ => {
                ensure(
                    metadata[root.0 as usize].complete,
                    "Position branch still contains component expressions",
                )?;
                ensure(
                    !std::mem::replace(&mut whole_seen, true),
                    "multiple complete Position sources in one branch",
                )?;
                whole = Some(*root);
            }
        }
    }
    ensure(
        !whole_seen || seen == [false, false],
        "Angle and whole Position sources coexist in one branch",
    )?;
    if whole.is_some() {
        return Ok(whole);
    }
    Ok(match axes {
        [Some(pan), Some(tilt)] => Some(append(
            tape,
            Node::Programming {
                address: DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Angles,
                    component: None,
                },
                value: DynamicValue::Family(AttributeValue::Position(Arc::new(
                    PositionIntent::angles(pan, tilt),
                ))),
                occurrence: None,
                // This compatibility bundle collapses axis footprints. Its value remains
                // usable, but it cannot claim a transfer for that collapsed Current input.
                dependency_occurrence: used_current
                    .then(|| crate::DynamicSourceDependency::unknown(None)),
            },
        )?),
        // Missing Current or an authored axis suppresses the pair. Never borrow another effect.
        _ => None,
    })
}

fn axis(address: &DynamicValueAddress) -> Result<usize, TransitionError> {
    if address.representation != DynamicFamilyRepresentation::Angles {
        return Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners,
        ));
    }
    match address.component {
        Some(ProgrammingComponent::Pan) => Ok(0),
        Some(ProgrammingComponent::Tilt) => Ok(1),
        _ => Err(IntentError("Angle Current requires a Pan or Tilt component".into()).into()),
    }
}

#[cfg(test)]
mod tests;
