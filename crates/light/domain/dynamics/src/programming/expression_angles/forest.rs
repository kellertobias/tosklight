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
    ensure_one_position_controller(samples, first)?;
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
    let (forest, results) = build_forest(&roots, &tape, &metadata, first.target, sources)?;
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
    let remainder = remainder_samples(samples, &roots, &tape, &metadata)?;
    Ok(PositionComponentForestBundle {
        position,
        remainder,
    })
}

fn ensure_one_position_controller(
    samples: &[DynamicRuntimeSample],
    first: &DynamicRuntimeSample,
) -> Result<(), TransitionError> {
    let mut lanes = HashSet::default();
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
    Ok(())
}

type ForestResults = HashMap<Roots, Option<usize>>;

/// Walk correlated root sets depth-first, splitting shared Resume occurrences into one
/// transition history and resolving every other set as a single branch.
fn build_forest(
    roots: &Roots,
    tape: &Arc<RetainedExpressionTape>,
    metadata: &[PositionMetadata],
    target: FixtureId,
    sources: &dyn DynamicValueSourceResolver,
) -> Result<(Vec<PositionForestNode>, ForestResults), TransitionError> {
    let mut forest = Vec::new();
    let mut results = ForestResults::default();
    let mut tasks = vec![Task::Visit(roots.clone())];
    let mut current = [None, None, None];
    let mut captured_current = None;
    let mut reads = CurrentReads {
        target,
        sources,
        current: &mut current,
        captured_current: &mut captured_current,
    };
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
                    let branch = branch(&roots, tape, metadata, &mut reads)?;
                    let root = branch.map(|branch| {
                        let id = forest.len();
                        forest.push(branch);
                        id
                    });
                    results.insert(roots, root);
                    continue;
                };
                let (from, to) = split_resume(&roots, tape, metadata, occurrence, progress)?;
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
    Ok((forest, results))
}

/// Split each lane at the shared Resume occurrence; lanes without it continue unchanged on
/// both sides of the transition.
fn split_resume(
    roots: &Roots,
    tape: &RetainedExpressionTape,
    metadata: &[PositionMetadata],
    occurrence: Uuid,
    progress: f32,
) -> Result<(Roots, Roots), TransitionError> {
    let mut from = Vec::new();
    let mut to = Vec::new();
    for &(lane, root) in roots {
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
    Ok((from, to))
}

/// Keep every non-Position part of the original lanes as retained runtime samples.
fn remainder_samples(
    samples: &[DynamicRuntimeSample],
    roots: &Roots,
    tape: &Arc<RetainedExpressionTape>,
    metadata: &[PositionMetadata],
) -> Result<Vec<DynamicRuntimeSample>, TransitionError> {
    // TL-639 round 2: a Position-only forest (every caller's ordinary case) has no remainder;
    // decide that by reading the tape instead of copying and compacting it first.
    if roots
        .iter()
        .all(|(_, root)| !has_non_position_part(tape, metadata, *root))
    {
        return Ok(Vec::new());
    }
    let mut remainder_tape = tape.as_ref().clone();
    let non_position = non_position_roots(&mut remainder_tape, metadata)?;
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
    Ok(remainder)
}

/// Whether `non_position_roots` would keep a remainder root for `root`: the node is not
/// Position, or a transition with such a side.
fn has_non_position_part(
    tape: &RetainedExpressionTape,
    metadata: &[PositionMetadata],
    root: RetainedNodeId,
) -> bool {
    let index = root.0 as usize;
    if !metadata[index].contains {
        return true;
    }
    match &tape.nodes[index] {
        Node::Transition { from, to, .. } => from
            .iter()
            .chain(to)
            .any(|child| has_non_position_part(tape, metadata, *child)),
        _ => false,
    }
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

/// Fixture-scoped Current reads shared by every branch of one forest, so each original Current
/// is read at most once per bundle.
struct CurrentReads<'a> {
    target: FixtureId,
    sources: &'a dyn DynamicValueSourceResolver,
    current: &'a mut [Option<Option<DynamicValue>>; 3],
    captured_current: &'a mut Option<Option<Arc<CapturedPositionCurrent>>>,
}

fn branch(
    roots: &Roots,
    tape: &Arc<RetainedExpressionTape>,
    metadata: &[PositionMetadata],
    reads: &mut CurrentReads<'_>,
) -> Result<Option<PositionForestNode>, TransitionError> {
    let mut leaves = BranchLeaves::new();
    for &(lane_id, root) in roots
        .iter()
        .filter(|(_, id)| metadata[id.0 as usize].contains)
    {
        match &tape.nodes[root.0 as usize] {
            Node::AngleNumeric { program } => leaves.angle_numeric(lane_id, program, reads)?,
            Node::AngleCurrent { address } => leaves.angle_current(lane_id, address, reads)?,
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
                leaves.authored(lane_id, address, value, occurrence, dependency_occurrence)?
            }
            _ => leaves.opaque_whole(lane_id, root, tape, metadata)?,
        }
    }
    leaves.finish()
}

/// The leaves of one correlated branch, accumulated in lane order.
struct BranchLeaves {
    axes: [Option<f32>; 2],
    pair_axes: [Option<PositionAngleAxis>; 2],
    seen: [bool; 2],
    axis_sources: Vec<CoupledComponentEndpoint>,
    targets: Vec<CoupledComponentEndpoint>,
    whole: Vec<PositionForestNode>,
    whole_seen: usize,
    only_target_wholes: bool,
}

impl BranchLeaves {
    fn new() -> Self {
        Self {
            axes: [None, None],
            pair_axes: [None, None],
            seen: [false, false],
            axis_sources: Vec::new(),
            targets: Vec::new(),
            whole: Vec::new(),
            whole_seen: 0usize,
            only_target_wholes: true,
        }
    }

    fn claim_axis(&mut self, address: &DynamicValueAddress) -> Result<usize, TransitionError> {
        let axis = axis(address)?;
        ensure(
            !std::mem::replace(&mut self.seen[axis], true),
            "duplicate axis in one Angle branch",
        )?;
        Ok(axis)
    }

    fn angle_numeric(
        &mut self,
        lane_id: Uuid,
        program: &Arc<crate::AngleNumericProgram>,
        reads: &mut CurrentReads<'_>,
    ) -> Result<(), TransitionError> {
        let address = &program.address;
        let axis = self.claim_axis(address)?;
        let original =
            capture_current(reads.target, address, reads.sources, reads.captured_current)?.ok_or(
                TransitionError::Requires(TransitionRequirement::LiveJointAngles),
            )?;
        self.pair_axes[axis] = Some(PositionAngleAxis::Numeric {
            lane_id,
            address: Arc::new(CompiledDynamicValueAddress::new(address.clone(), None)?),
            program: program.clone(),
            original,
        });
        Ok(())
    }

    fn angle_current(
        &mut self,
        lane_id: Uuid,
        address: &DynamicValueAddress,
        reads: &mut CurrentReads<'_>,
    ) -> Result<(), TransitionError> {
        let original =
            capture_current(reads.target, address, reads.sources, reads.captured_current)?;
        let target_current = original.as_ref().is_some_and(|original| {
            matches!(&original.value, AttributeValue::Position(position)
                if matches!(position.as_ref(), PositionIntent::Target { .. }))
        });
        if address.component.is_none() {
            self.whole_current(lane_id, address, original, target_current, reads)
        } else {
            self.axis_current(lane_id, address, original, target_current, reads)
        }
    }

    fn whole_current(
        &mut self,
        lane_id: Uuid,
        address: &DynamicValueAddress,
        original: Option<Arc<CapturedPositionCurrent>>,
        target_current: bool,
        reads: &mut CurrentReads<'_>,
    ) -> Result<(), TransitionError> {
        let (target, sources) = (reads.target, reads.sources);
        self.whole_seen += 1;
        self.only_target_wholes = false;
        if target_current {
            let original = original.unwrap();
            let axes = [ProgrammingComponent::Pan, ProgrammingComponent::Tilt].map(|component| {
                let mut address = address.clone();
                address.component = Some(component);
                Ok(PositionAngleAxis::Current {
                    lane_id,
                    address: Arc::new(CompiledDynamicValueAddress::new(address, None)?),
                    original: original.clone(),
                })
            });
            let [pan, tilt]: [Result<PositionAngleAxis, IntentError>; 2] = axes;
            self.whole.push(PositionForestNode::AnglePair(Arc::new(
                PositionAnglePairEndpoint {
                    axes: [pan?, tilt?],
                },
            )));
        } else {
            let value = if let Some(original) = &original {
                Some(DynamicValue::Family(original.value.clone()))
            } else {
                // Scalar-only callers keep their pre-existing compatible path.
                reads.current[2]
                    .get_or_insert_with(|| sources.current(target, address))
                    .clone()
            };
            if let Some(value) = value {
                let source = current_endpoint(lane_id, address, value.clone(), &original, reads)?;
                self.whole.push(PositionForestNode::Whole {
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
        Ok(())
    }

    fn axis_current(
        &mut self,
        lane_id: Uuid,
        address: &DynamicValueAddress,
        original: Option<Arc<CapturedPositionCurrent>>,
        target_current: bool,
        reads: &mut CurrentReads<'_>,
    ) -> Result<(), TransitionError> {
        let (target, sources) = (reads.target, reads.sources);
        let axis = self.claim_axis(address)?;
        if target_current {
            self.pair_axes[axis] = Some(PositionAngleAxis::Current {
                lane_id,
                address: Arc::new(CompiledDynamicValueAddress::new(address.clone(), None)?),
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
                reads.current[axis]
                    .get_or_insert_with(|| sources.current(target, address))
                    .clone()
            };
            if let Some(value) = value {
                let source = current_endpoint(lane_id, address, value, &original, reads)?;
                let DynamicValue::Scalar(value) = &source.value else {
                    unreachable!("verified axis")
                };
                self.axes[axis] = Some(*value);
                self.pair_axes[axis] = Some(PositionAngleAxis::Materialized(source.clone()));
                self.axis_sources.push(source);
            }
        }
        Ok(())
    }

    fn authored(
        &mut self,
        lane_id: Uuid,
        address: &DynamicValueAddress,
        value: &DynamicValue,
        occurrence: &Option<DynamicSourceOccurrenceId>,
        dependency_occurrence: &Option<crate::DynamicSourceDependency>,
    ) -> Result<(), TransitionError> {
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
            self.targets.push(source);
        } else {
            let axis = self.claim_axis(address)?;
            let DynamicValue::Scalar(value) = value else {
                unreachable!("verified axis")
            };
            self.axes[axis] = Some(*value);
            self.pair_axes[axis] = Some(PositionAngleAxis::Materialized(source.clone()));
            self.axis_sources.push(source);
        }
        Ok(())
    }

    fn opaque_whole(
        &mut self,
        lane_id: Uuid,
        root: RetainedNodeId,
        tape: &Arc<RetainedExpressionTape>,
        metadata: &[PositionMetadata],
    ) -> Result<(), TransitionError> {
        ensure(
            metadata[root.0 as usize].complete,
            "Position branch contains an unresolved component expression",
        )?;
        self.whole_seen += 1;
        let only_target_wholes = &mut self.only_target_wholes;
        tape.visit_reachable(&[root], |_, node| {
            match node {
                Node::Programming { address, .. } | Node::Scale { address, .. } => {
                    *only_target_wholes &= matches!(
                        address.representation,
                        DynamicFamilyRepresentation::Target { .. }
                    );
                }
                Node::AngleCurrent { .. }
                | Node::AngleNumeric { .. }
                | Node::LegacyScalar { .. } => *only_target_wholes = false,
                Node::Transition { .. } => {}
            }
            Ok(())
        })?;
        self.whole.push(PositionForestNode::Whole {
            expression: Arc::new(Expression::Retained {
                tape: tape.clone(),
                root,
            }),
            lane_id,
            sources: Arc::from([]),
        });
        Ok(())
    }

    fn finish(self) -> Result<Option<PositionForestNode>, TransitionError> {
        let Self {
            axes,
            pair_axes,
            seen,
            axis_sources,
            mut targets,
            mut whole,
            whole_seen,
            only_target_wholes,
        } = self;
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
}

/// A Current leaf endpoint carrying its explicit original occurrence, or the resolver's
/// compatible dependency for scalar-only callers.
fn current_endpoint(
    lane_id: Uuid,
    address: &DynamicValueAddress,
    value: DynamicValue,
    original: &Option<Arc<CapturedPositionCurrent>>,
    reads: &CurrentReads<'_>,
) -> Result<CoupledComponentEndpoint, TransitionError> {
    let dependency = original.as_ref().map_or_else(
        || reads.sources.current_dependency(reads.target, address),
        |original| crate::DynamicSourceDependency::compatible(original.occurrence, address),
    );
    endpoint(
        lane_id,
        address.clone(),
        value,
        CoupledLeafRole::Current,
        None,
        Some(dependency),
    )
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
