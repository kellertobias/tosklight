//! Zip controller membership before materializing Target endpoints. Target offsets remain
//! cohort edits; only a complete controller-local Angle pair becomes a whole authored value.
use super::*;
use crate::programming::expression_coupled::{
    CapturedPositionCurrent, PositionAngleAxis, PositionAnglePairEndpoint,
};
use crate::{
    CompiledCoupledExpression, CompiledDynamicValueAddress, CompiledProgrammingFamilyExpression,
    CoupledCohortEndpoint, CoupledComponentEndpoint, CoupledLeafRole, DynamicSourceOccurrenceId,
    FamilyCompositionSample, FamilySampleRank, programming::expression_coupled::PositionForestNode,
};

pub struct PositionComponentForestBundle {
    pub position: Option<FamilyCompositionSample>,
    pub remainder: Vec<DynamicRuntimeSample>,
}

type Roots = Vec<(Uuid, RetainedNodeId)>;
enum Task {
    Visit(Roots),
    Alias {
        roots: Roots,
        child: Roots,
    },
    Join {
        roots: Roots,
        from: Roots,
        to: Roots,
        progress: f32,
        reason: DynamicTransitionReason,
    },
}

/// All inputs belong to one instance/controller/target. Interior Angle/Target transitions
/// preserve one correlated branch history, and Target leaves are resolved only by composition.
pub fn bundle_position_component_forest(
    samples: &[DynamicRuntimeSample],
    sources: &dyn DynamicValueSourceResolver,
) -> Result<PositionComponentForestBundle, TransitionError> {
    let Some(first) = samples.first() else {
        return Ok(PositionComponentForestBundle {
            position: None,
            remainder: Vec::new(),
        });
    };
    let mut lanes = HashSet::new();
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
    let tape = Arc::new(RetainedExpressionTape::from_roots(&expressions)?);
    let metadata = position_metadata(&tape);
    let roots = tape
        .roots
        .iter()
        .zip(samples)
        .map(|(root, sample)| (sample.lane_id, *root))
        .collect::<Roots>();
    let mut forest = Vec::new();
    let mut results = HashMap::<Roots, Option<usize>>::new();
    let mut tasks = vec![Task::Visit(roots.clone())];
    let mut current = [None, None, None];
    let mut captured_current = None;
    while let Some(task) = tasks.pop() {
        match task {
            Task::Alias { roots, child } => {
                results.insert(roots, results[&child]);
            }
            Task::Join {
                roots,
                from,
                to,
                progress,
                reason,
            } => {
                let (from, to) = (results[&from], results[&to]);
                let root = if from.is_some() || to.is_some() {
                    let id = forest.len();
                    forest.push(PositionForestNode::Transition {
                        from,
                        to,
                        progress,
                        reason,
                    });
                    Some(id)
                } else {
                    None
                };
                results.insert(roots, root);
            }
            Task::Visit(roots) => {
                if results.contains_key(&roots) {
                    continue;
                }
                let resume = roots
                    .iter()
                    .filter(|(_, id)| metadata[id.0 as usize].contains)
                    .find_map(|(_, id)| {
                        if let Node::Transition {
                            progress,
                            reason: DynamicTransitionReason::Resume { occurrence_id },
                            ..
                        } = &tape.nodes[id.0 as usize]
                        {
                            Some((*occurrence_id, *progress))
                        } else {
                            None
                        }
                    });
                let Some((occurrence, progress)) = resume else {
                    let branch = branch(
                        &roots,
                        &tape,
                        &metadata,
                        first.target,
                        sources,
                        &mut current,
                        &mut captured_current,
                    )?;
                    let root = branch.map(|branch| {
                        let id = forest.len();
                        forest.push(branch);
                        id
                    });
                    results.insert(roots, root);
                    continue;
                };
                let mut from = Vec::new();
                let mut to = Vec::new();
                for &(lane, root) in &roots {
                    if let Node::Transition {
                        from: old,
                        to: new,
                        progress: other,
                        reason: DynamicTransitionReason::Resume { occurrence_id },
                    } = &tape.nodes[root.0 as usize]
                        && *occurrence_id == occurrence
                    {
                        ensure(
                            *other == progress,
                            "one resume occurrence has different progress",
                        )?;
                        from.extend(old.map(|id| (lane, id)));
                        to.extend(new.map(|id| (lane, id)));
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
                        from.push((lane, root));
                        to.push((lane, root));
                    }
                }
                if progress == 0.0 || progress == 1.0 {
                    let child = if progress == 0.0 { from } else { to };
                    tasks.push(Task::Alias {
                        roots,
                        child: child.clone(),
                    });
                    tasks.push(Task::Visit(child));
                } else {
                    tasks.push(Task::Join {
                        roots,
                        from: from.clone(),
                        to: to.clone(),
                        progress,
                        reason: DynamicTransitionReason::Resume {
                            occurrence_id: occurrence,
                        },
                    });
                    tasks.push(Task::Visit(to));
                    tasks.push(Task::Visit(from));
                }
            }
        }
    }
    let position = if let Some(root) = results[&roots] {
        let lane_id = roots
            .iter()
            .filter(|(_, id)| metadata[id.0 as usize].contains)
            .map(|(lane, _)| *lane)
            .max()
            .expect("Position source");
        Some(FamilyCompositionSample::CoupledExpression {
            expression: Arc::new(CompiledCoupledExpression::from_position_forest(
                Arc::from(samples),
                &forest,
                root,
            )?),
            rank: FamilySampleRank {
                priority: first.priority,
                changed_at_millis: first.activated_at_millis,
                changed_at_submillis_nanos: 0,
                stable_order: first.controller_id.as_u128(),
                identity: crate::FamilySampleIdentity::Dynamic {
                    instance_id: first.instance_id,
                    controller_id: first.controller_id,
                    lane_id,
                },
            },
            activation_mix: first.activation_mix,
        })
    } else {
        None
    };
    let mut remainder_tape = tape.as_ref().clone();
    let non_position = non_position_roots(&mut remainder_tape, &metadata)?;
    let remaining = roots
        .iter()
        .enumerate()
        .filter_map(|(index, (_, root))| non_position[root.0 as usize].map(|root| (index, root)))
        .collect::<Vec<_>>();
    remainder_tape.roots = remaining.iter().map(|(_, root)| *root).collect();
    remainder_tape.compact_reachable()?;
    let remainder_tape = Arc::new(remainder_tape);
    let remainder = remaining
        .iter()
        .enumerate()
        .map(|(offset, (index, _))| {
            let mut sample = samples[*index].clone();
            sample.expression = Expression::Retained {
                tape: remainder_tape.clone(),
                root: remainder_tape.roots[offset],
            };
            sample.address = None;
            sample
        })
        .collect();
    Ok(PositionComponentForestBundle {
        position,
        remainder,
    })
}

fn endpoint(
    lane_id: Uuid,
    address: DynamicValueAddress,
    value: DynamicValue,
    role: CoupledLeafRole,
    occurrence: Option<DynamicSourceOccurrenceId>,
    dependency_occurrence: Option<crate::DynamicSourceDependency>,
) -> Result<CoupledComponentEndpoint, TransitionError> {
    let address = Arc::new(CompiledDynamicValueAddress::new(address, None)?);
    address.validate_value(&value)?;
    if let Some(dependency) = &dependency_occurrence {
        dependency.validate(address.address().owner())?;
    }
    Ok(CoupledComponentEndpoint {
        lane_id,
        address,
        value,
        role,
        occurrence,
        dependency_occurrence,
    })
}

fn branch(
    roots: &Roots,
    tape: &Arc<RetainedExpressionTape>,
    metadata: &[PositionMetadata],
    target: FixtureId,
    sources: &dyn DynamicValueSourceResolver,
    current: &mut [Option<Option<DynamicValue>>; 3],
    captured_current: &mut Option<Option<Arc<CapturedPositionCurrent>>>,
) -> Result<Option<PositionForestNode>, TransitionError> {
    let mut axes = [None, None];
    let mut pair_axes = [None, None];
    let mut seen = [false, false];
    let mut axis_sources = Vec::new();
    let mut targets = Vec::new();
    let mut whole = Vec::new();
    let mut whole_seen = 0usize;
    let mut only_target_wholes = true;
    for &(lane_id, root) in roots
        .iter()
        .filter(|(_, id)| metadata[id.0 as usize].contains)
    {
        match &tape.nodes[root.0 as usize] {
            Node::AngleNumeric { program } => {
                let address = &program.address;
                let axis = axis(address)?;
                ensure(
                    !std::mem::replace(&mut seen[axis], true),
                    "duplicate axis in one Angle branch",
                )?;
                let original = capture_current(target, address, sources, captured_current)?.ok_or(
                    TransitionError::Requires(TransitionRequirement::LiveJointAngles),
                )?;
                pair_axes[axis] = Some(PositionAngleAxis::Numeric {
                    lane_id,
                    address: Arc::new(CompiledDynamicValueAddress::new(address.clone(), None)?),
                    program: program.clone(),
                    original,
                });
            }
            Node::AngleCurrent { address } => {
                let original = capture_current(target, address, sources, captured_current)?;
                let target_current = original.as_ref().is_some_and(|original| {
                    matches!(&original.value, AttributeValue::Position(position)
                        if matches!(position.as_ref(), PositionIntent::Target { .. }))
                });
                if address.component.is_none() {
                    whole_seen += 1;
                    only_target_wholes = false;
                    if target_current {
                        let original = original.unwrap();
                        let axes = [ProgrammingComponent::Pan, ProgrammingComponent::Tilt].map(
                            |component| {
                                let mut address = address.clone();
                                address.component = Some(component);
                                Ok(PositionAngleAxis::Current {
                                    lane_id,
                                    address: Arc::new(CompiledDynamicValueAddress::new(
                                        address, None,
                                    )?),
                                    original: original.clone(),
                                })
                            },
                        );
                        let [pan, tilt]: [Result<PositionAngleAxis, IntentError>; 2] = axes;
                        whole.push(PositionForestNode::AnglePair(Arc::new(
                            PositionAnglePairEndpoint {
                                axes: [pan?, tilt?],
                            },
                        )));
                    } else {
                        let value = if let Some(original) = &original {
                            Some(DynamicValue::Family(original.value.clone()))
                        } else {
                            // Scalar-only callers keep their pre-existing compatible path.
                            current[2]
                                .get_or_insert_with(|| sources.current(target, address))
                                .clone()
                        };
                        if let Some(value) = value {
                            let dependency = original.as_ref().map_or_else(
                                || sources.current_dependency(target, address),
                                |original| {
                                    crate::DynamicSourceDependency::compatible(
                                        original.occurrence,
                                        address,
                                    )
                                },
                            );
                            let source = endpoint(
                                lane_id,
                                address.clone(),
                                value.clone(),
                                CoupledLeafRole::Current,
                                None,
                                Some(dependency),
                            )?;
                            whole.push(PositionForestNode::Whole {
                                expression: Arc::new(Expression::Programming {
                                    address: Arc::new(address.clone()),
                                    value,
                                    occurrence: None,
                                    dependency_occurrence: source.dependency_occurrence.clone(),
                                }),
                                lane_id,
                                sources: Arc::from([source]),
                            });
                        }
                    }
                } else {
                    let axis = axis(address)?;
                    ensure(
                        !std::mem::replace(&mut seen[axis], true),
                        "duplicate axis in one Angle branch",
                    )?;
                    if target_current {
                        pair_axes[axis] = Some(PositionAngleAxis::Current {
                            lane_id,
                            address: Arc::new(CompiledDynamicValueAddress::new(
                                address.clone(),
                                None,
                            )?),
                            original: original.unwrap(),
                        });
                    } else {
                        let value = if let Some(original) = &original {
                            crate::extract_compatible_dynamic_value(
                                &original.value,
                                address,
                                &FamilyEditContext::default(),
                            )?
                        } else {
                            current[axis]
                                .get_or_insert_with(|| sources.current(target, address))
                                .clone()
                        };
                        if let Some(value) = value {
                            let dependency = original.as_ref().map_or_else(
                                || sources.current_dependency(target, address),
                                |original| {
                                    crate::DynamicSourceDependency::compatible(
                                        original.occurrence,
                                        address,
                                    )
                                },
                            );
                            let source = endpoint(
                                lane_id,
                                address.clone(),
                                value,
                                CoupledLeafRole::Current,
                                None,
                                Some(dependency),
                            )?;
                            let DynamicValue::Scalar(value) = &source.value else {
                                unreachable!("verified axis")
                            };
                            axes[axis] = Some(*value);
                            pair_axes[axis] = Some(PositionAngleAxis::Materialized(source.clone()));
                            axis_sources.push(source);
                        }
                    }
                }
            }
            Node::Programming {
                address,
                value,
                occurrence,
                dependency_occurrence,
            } if address.component.is_some()
                || matches!(
                    address.representation,
                    DynamicFamilyRepresentation::Target { reference: Some(_) }
                ) =>
            {
                let source = endpoint(
                    lane_id,
                    address.clone(),
                    value.clone(),
                    CoupledLeafRole::Authored,
                    *occurrence,
                    dependency_occurrence.clone(),
                )?;
                if matches!(
                    address.representation,
                    DynamicFamilyRepresentation::Target { .. }
                ) {
                    targets.push(source);
                } else {
                    let axis = axis(address)?;
                    ensure(
                        !std::mem::replace(&mut seen[axis], true),
                        "duplicate axis in one Angle branch",
                    )?;
                    let DynamicValue::Scalar(value) = value else {
                        unreachable!("verified axis")
                    };
                    axes[axis] = Some(*value);
                    pair_axes[axis] = Some(PositionAngleAxis::Materialized(source.clone()));
                    axis_sources.push(source);
                }
            }
            _ => {
                ensure(
                    metadata[root.0 as usize].complete,
                    "Position branch contains an unresolved component expression",
                )?;
                whole_seen += 1;
                tape.visit_reachable(&[root], |_, node| {
                    match node {
                        Node::Programming { address, .. } | Node::Scale { address, .. } => {
                            only_target_wholes &= matches!(
                                address.representation,
                                DynamicFamilyRepresentation::Target { .. }
                            );
                        }
                        Node::AngleCurrent { .. }
                        | Node::AngleNumeric { .. }
                        | Node::LegacyScalar { .. } => only_target_wholes = false,
                        Node::Transition { .. } => {}
                    }
                    Ok(())
                })?;
                whole.push(PositionForestNode::Whole {
                    expression: Arc::new(Expression::Retained {
                        tape: tape.clone(),
                        root,
                    }),
                    lane_id,
                    sources: Arc::from([]),
                });
            }
        }
    }
    ensure(
        whole_seen == 0 || seen == [false, false],
        "Angle components and whole Position cannot coexist in one branch",
    )?;
    ensure(
        targets.is_empty() || seen == [false, false],
        "Angle and Target components cannot coexist in one branch",
    )?;
    if whole_seen != 0 {
        if whole_seen == 1 && targets.is_empty() {
            return Ok(whole.pop());
        }
        ensure(
            only_target_wholes,
            "complete Angles cannot coexist with other Position sources",
        )?;
        let mut cohort = targets
            .into_iter()
            .map(CoupledCohortEndpoint::Materialized)
            .collect::<Vec<_>>();
        for source in whole {
            let PositionForestNode::Whole {
                expression,
                lane_id,
                ..
            } = source
            else {
                unreachable!("whole source")
            };
            cohort.push(CoupledCohortEndpoint::WholeExpression {
                lane_id,
                expression: Arc::new(CompiledProgrammingFamilyExpression::new(
                    expression,
                    ProgrammingOwner::Position,
                    None,
                    None,
                )?),
            });
        }
        cohort.sort_unstable_by_key(CoupledCohortEndpoint::lane_id);
        return Ok(Some(PositionForestNode::SourceCohort(cohort.into())));
    }
    if !targets.is_empty() {
        targets.sort_unstable_by_key(|source| source.lane_id);
        return Ok(Some(PositionForestNode::Cohort(targets.into())));
    }
    if pair_axes.iter().any(|axis| {
        matches!(
            axis,
            Some(PositionAngleAxis::Current { .. } | PositionAngleAxis::Numeric { .. })
        )
    }) {
        return Ok(match pair_axes {
            [Some(pan), Some(tilt)] => Some(PositionForestNode::AnglePair(Arc::new(
                PositionAnglePairEndpoint { axes: [pan, tilt] },
            ))),
            _ => None,
        });
    }
    Ok(if let [Some(pan), Some(tilt)] = axes {
        let lane_id = axis_sources
            .iter()
            .map(|source| source.lane_id)
            .max()
            .expect("complete pair");
        Some(PositionForestNode::Whole {
            expression: Arc::new(Expression::Programming {
                address: Arc::new(DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Angles,
                    component: None,
                }),
                value: DynamicValue::Family(AttributeValue::Position(Arc::new(
                    PositionIntent::angles(pan, tilt),
                ))),
                occurrence: None,
                dependency_occurrence: None,
            }),
            lane_id,
            sources: axis_sources.into(),
        })
    } else {
        None
    })
}

/// Read an explicitly supplied original Position Current, never a Size baseline or an adopted
/// scalar. Legacy scalar-only sources do not opt in and keep their original single-read path.
fn capture_current(
    target: FixtureId,
    address: &DynamicValueAddress,
    sources: &dyn DynamicValueSourceResolver,
    cache: &mut Option<Option<Arc<CapturedPositionCurrent>>>,
) -> Result<Option<Arc<CapturedPositionCurrent>>, TransitionError> {
    if let Some(current) = cache {
        return Ok(current.clone());
    }
    let current = sources
        .try_position_current_family(target, address)?
        .map(|value| {
            value.validate_programming_address(&ProgrammingOwner::Position.key())?;
            ensure(
                value.spread_control_points() == 0 && matches!(&value, AttributeValue::Position(_)),
                "Angle Current needs a complete captured Position family",
            )?;
            Ok::<_, IntentError>(Arc::new(CapturedPositionCurrent {
                value,
                occurrence: sources.current_family_occurrence(target, address),
            }))
        })
        .transpose()?;
    *cache = Some(current.clone());
    Ok(current)
}
