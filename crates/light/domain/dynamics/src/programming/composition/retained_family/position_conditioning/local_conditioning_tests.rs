//! Local Required/Size cuts preserve original ancestry and ordered mechanical source members.
use super::*;
use crate::programming::expression_coupled::{
    CapturedPositionCurrent, PositionAngleAxis, PositionAnglePairEndpoint,
};
use crate::{
    CoupledCohortEndpoint, DynamicSourceDependency, DynamicSourceOccurrenceId,
    DynamicSourceTransfer, WholeFamilyExpressionFrameResolver,
};
use std::cell::Cell;

fn angles(pan: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, 20.)))
}
fn target(x: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [x, 2., 3.],
    )))
}
fn rank(lane: u128) -> FamilySampleRank {
    FamilySampleRank {
        priority: 3,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: lane,
        identity: FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(2),
            lane_id: Uuid::from_u128(lane),
        },
    }
}
fn occurrence(id: u128) -> DynamicSourceOccurrenceId {
    DynamicSourceOccurrenceId::new(Uuid::from_u128(id)).unwrap()
}
fn scope(id: u128) -> PositionResumeScope {
    PositionResumeScope {
        instance_id: Uuid::from_u128(1),
        controller_id: Uuid::from_u128(2),
        occurrence_id: Uuid::from_u128(id),
    }
}
fn leaf(value: AttributeValue) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: Some(occurrence(600)),
        dependency_occurrence: None,
    })
}
fn required(
    from: Option<Arc<DynamicSampleExpression>>,
    to: Option<Arc<DynamicSampleExpression>>,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from,
        to,
        progress: 0.4,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::LiveJointAngles,
        },
    })
}
fn resume(
    from: Option<Arc<DynamicSampleExpression>>,
    to: Option<Arc<DynamicSampleExpression>>,
    id: u128,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from,
        to,
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(id),
        },
    })
}
fn scale(
    base: AttributeValue,
    value: Arc<DynamicSampleExpression>,
    factor: f32,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &base).unwrap(),
        ),
        base: DynamicValue::Family(base),
        value,
        factor,
        baseline_occurrence: Some(occurrence(700)),
    })
}
fn compiled(expression: Arc<DynamicSampleExpression>) -> Arc<CompiledProgrammingFamilyExpression> {
    Arc::new(
        CompiledProgrammingFamilyExpression::new(
            expression,
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    )
}
fn whole(expression: Arc<DynamicSampleExpression>, lane: u128) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: compiled(expression),
        rank: rank(lane),
        activation_mix: 1.,
    }
}
fn forest(nodes: Vec<PositionForestNode>, root: usize) -> FamilyCompositionSample {
    FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::from_position_forest(Arc::from([]), &nodes, root).unwrap(),
        ),
        rank: rank(90),
        activation_mix: 1.,
    }
}
fn original_nodes(registry: &CapturedPositionProgram, source: usize) -> Vec<PositionSourceNode> {
    let mut pending = registry
        .source_root(source)
        .unwrap()
        .into_iter()
        .collect::<Vec<_>>();
    let mut found = Vec::new();
    let mut visited = HashSet::new();
    while let Some(node) = pending.pop() {
        if !visited.insert(node.clone()) {
            continue;
        }
        match registry.node_view(&node).unwrap() {
            PositionSourceNodeView::Transition { from, to, .. } => {
                pending.extend(from);
                pending.extend(to);
            }
            PositionSourceNodeView::Scale { value, .. } => pending.extend(value),
            PositionSourceNodeView::Whole { root, .. } => pending.push(root),
            PositionSourceNodeView::SourceCohort { members } => pending.extend(members),
            _ => {}
        }
        found.push(node);
    }
    found
}
fn operation(registry: &CapturedPositionProgram, source: usize, size: bool) -> PositionSourceNode {
    original_nodes(registry, source)
        .into_iter()
        .find(|node| match registry.node_view(node).unwrap() {
            PositionSourceNodeView::Scale { .. } => size,
            PositionSourceNodeView::Transition {
                reason: DynamicTransitionReason::Required { .. },
                ..
            } => !size,
            _ => false,
        })
        .unwrap()
}
fn expression(source: &FamilyCompositionSample) -> DynamicSampleExpression {
    let FamilyCompositionSample::WholeExpression { expression, .. } = source else {
        panic!()
    };
    expression.expression().shallow().unwrap()
}
fn same_family(a: &AttributeValue, b: &AttributeValue) -> bool {
    let (AttributeValue::Position(a), AttributeValue::Position(b)) = (a, b) else {
        panic!()
    };
    Arc::ptr_eq(a, b)
}

#[test]
fn required_selection_keeps_size_resume_ancestors_and_other_original_slots() {
    let baseline = angles(5.);
    let source = whole(
        resume(
            Some(scale(
                baseline.clone(),
                required(Some(leaf(angles(10.))), Some(leaf(angles(90.)))),
                1.5,
            )),
            Some(leaf(angles(180.))),
            40,
        ),
        3,
    );
    let untouched = whole(leaf(angles(250.)), 4);
    let registry =
        CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &[source, untouched.clone()])
            .unwrap();
    let node = operation(&registry, 0, false);
    let mut branch = registry.branch();
    branch
        .select_local_endpoint(&node, PositionLocalEndpoint::RequiredIncoming)
        .unwrap();
    assert!(matches!(
        branch.resume_scope_membership(scope(40)).unwrap(),
        PositionResumeScopeMembership::Active(_)
    ));
    let sources = branch.conditioned_sources().unwrap();
    let DynamicSampleExpression::Transition {
        from: Some(from),
        reason,
        ..
    } = expression(sources[0].as_ref().unwrap())
    else {
        panic!("Resume ancestor")
    };
    assert!(
        matches!(reason, DynamicTransitionReason::Resume { occurrence_id } if occurrence_id == scope(40).occurrence_id)
    );
    let DynamicSampleExpression::Scale {
        base: DynamicValue::Family(base),
        factor,
        value,
        baseline_occurrence,
        ..
    } = from.shallow().unwrap()
    else {
        panic!("Size ancestor")
    };
    assert!(same_family(&base, &baseline));
    assert_eq!(factor, 1.5);
    assert_eq!(baseline_occurrence, Some(occurrence(700)));
    assert!(
        matches!(value.shallow().unwrap(), DynamicSampleExpression::Programming { value: DynamicValue::Family(value), .. } if value == angles(90.))
    );
    let (
        FamilyCompositionSample::WholeExpression { expression: a, .. },
        FamilyCompositionSample::WholeExpression { expression: b, .. },
    ) = (&untouched, sources[1].as_ref().unwrap())
    else {
        panic!()
    };
    assert!(Arc::ptr_eq(a, b));
    branch
        .choose_resume(scope(40), PositionResumeEndpoint::Incoming)
        .unwrap();
    assert!(
        branch
            .select_local_endpoint(&node, PositionLocalEndpoint::RequiredIncoming)
            .is_err(),
        "inactive old subtree cannot authorize another cut"
    );
}

#[test]
fn size_baseline_uses_actual_captured_operand_and_occurrence_without_fake_node_authority() {
    let baseline = target(7.);
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0.),
        &[whole(scale(baseline.clone(), leaf(target(90.)), 1.5), 3)],
    )
    .unwrap();
    let node = operation(&registry, 0, true);
    let mut baseline_branch = registry.branch();
    baseline_branch
        .select_local_endpoint(&node, PositionLocalEndpoint::ScaleBaseline)
        .unwrap();
    let bound = baseline_branch
        .conditioned_with_origins()
        .unwrap()
        .remove(0);
    let DynamicSampleExpression::Programming {
        value: DynamicValue::Family(value),
        occurrence: authored,
        dependency_occurrence: Some(dependency),
        ..
    } = expression(&bound.sample)
    else {
        panic!()
    };
    assert!(same_family(&value, &baseline));
    assert_eq!(authored, None);
    assert_eq!(
        dependency,
        DynamicSourceDependency::identity(Some(occurrence(700)))
    );
    assert!(matches!(
        dependency.transfer,
        DynamicSourceTransfer::Identity
    ));
    let FamilyCompositionSample::WholeExpression { expression, .. } = &bound.sample else {
        panic!()
    };
    let tape =
        RetainedExpressionTape::from_roots(&[Arc::new(expression.expression().clone())]).unwrap();
    assert!(
        bound
            .origins
            .get(NodeLocation::Tape(tape.roots[0]))
            .is_none(),
        "captured baseline is not a retained authored operation node"
    );
    let mut value_branch = registry.branch();
    value_branch
        .select_local_endpoint(&node, PositionLocalEndpoint::ScaleValue)
        .unwrap();
    assert!(
        matches!(expression_of(&value_branch), DynamicSampleExpression::Programming { value: DynamicValue::Family(value), .. } if value == target(90.))
    );
}
fn expression_of(branch: &PositionProgramBranch) -> DynamicSampleExpression {
    expression(branch.conditioned_sources().unwrap()[0].as_ref().unwrap())
}

#[test]
fn member_local_release_preserves_sibling_arc_actual_lane_and_captured_current_pair() {
    let sibling = compiled(leaf(target(99.)));
    let members: Arc<[CoupledCohortEndpoint]> = vec![
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(20),
            expression: compiled(required(Some(leaf(target(4.))), None)),
        },
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(21),
            expression: sibling.clone(),
        },
    ]
    .into();
    let original_current = Arc::new(CapturedPositionCurrent {
        value: target(1.),
        occurrence: Some(occurrence(800)),
    });
    let pair = Arc::new(PositionAnglePairEndpoint {
        axes: [ProgrammingComponent::Pan, ProgrammingComponent::Tilt].map(|component| {
            PositionAngleAxis::Current {
                lane_id: Uuid::from_u128(if component == ProgrammingComponent::Pan {
                    30
                } else {
                    31
                }),
                address: Arc::new(
                    CompiledDynamicValueAddress::new(
                        DynamicValueAddress {
                            representation: DynamicFamilyRepresentation::Angles,
                            component: Some(component),
                        },
                        None,
                    )
                    .unwrap(),
                ),
                original: original_current.clone(),
            }
        }),
    });
    let source = forest(
        vec![
            PositionForestNode::SourceCohort(members),
            PositionForestNode::AnglePair(pair.clone()),
            PositionForestNode::Transition {
                from: Some(0),
                to: Some(1),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: scope(40).occurrence_id,
                },
            },
        ],
        2,
    );
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &target(1.), &[source]).unwrap();
    let node = operation(&registry, 0, false);
    assert_eq!(
        registry
            .node_source_rank(&node)
            .unwrap()
            .dynamic_identity()
            .unwrap()
            .lane_id,
        Uuid::from_u128(20)
    );
    let mut branch = registry.branch();
    branch
        .select_local_endpoint(&node, PositionLocalEndpoint::RequiredIncoming)
        .unwrap();
    let sources = branch.conditioned_sources().unwrap();
    let FamilyCompositionSample::CoupledExpression { expression, .. } =
        sources[0].as_ref().unwrap()
    else {
        panic!()
    };
    let lineage = expression.position_forest_lineage().unwrap();
    let members = lineage
        .nodes
        .iter()
        .find_map(|node| {
            if let PositionForestNode::SourceCohort(members) = node {
                Some(members)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(members.len(), 1);
    let CoupledCohortEndpoint::WholeExpression {
        lane_id,
        expression,
    } = &members[0]
    else {
        panic!()
    };
    assert_eq!(*lane_id, Uuid::from_u128(21));
    assert!(Arc::ptr_eq(expression, &sibling));
    let preserved = lineage
        .nodes
        .iter()
        .find_map(|node| {
            if let PositionForestNode::AnglePair(pair) = node {
                Some(pair)
            } else {
                None
            }
        })
        .unwrap();
    assert!(Arc::ptr_eq(preserved, &pair));
    let PositionAngleAxis::Current { original, .. } = &preserved.axes[0] else {
        panic!()
    };
    assert!(Arc::ptr_eq(original, &original_current));
}

#[test]
fn forest_required_selection_replaces_in_place_and_retains_original_root_origin() {
    let first = PositionForestNode::Whole {
        expression: leaf(angles(10.)),
        lane_id: Uuid::from_u128(10),
        sources: Arc::from([]),
    };
    let second = PositionForestNode::Whole {
        expression: leaf(angles(90.)),
        lane_id: Uuid::from_u128(11),
        sources: Arc::from([]),
    };
    let third = PositionForestNode::Whole {
        expression: leaf(angles(180.)),
        lane_id: Uuid::from_u128(12),
        sources: Arc::from([]),
    };
    let source = forest(
        vec![
            first,
            second,
            PositionForestNode::Transition {
                from: Some(0),
                to: Some(1),
                progress: 0.4,
                reason: DynamicTransitionReason::Required {
                    requirement: TransitionRequirement::LiveJointAngles,
                },
            },
            third,
            PositionForestNode::Transition {
                from: Some(2),
                to: Some(3),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: scope(40).occurrence_id,
                },
            },
        ],
        4,
    );
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &[source]).unwrap();
    let mut branch = registry.branch();
    branch
        .select_local_endpoint(
            &operation(&registry, 0, false),
            PositionLocalEndpoint::RequiredIncoming,
        )
        .unwrap();
    let bound = branch.conditioned_with_origins().unwrap().remove(0);
    let FamilyCompositionSample::CoupledExpression { expression, .. } = bound.sample else {
        panic!()
    };
    let lineage = expression.position_forest_lineage().unwrap();
    assert_eq!(
        bound.origins.get(NodeLocation::Forest(lineage.root)),
        Some(NodeLocation::Forest(4))
    );
    let PositionForestNode::Transition {
        from: Some(from),
        reason,
        ..
    } = &lineage.nodes[lineage.root]
    else {
        panic!()
    };
    assert!(matches!(reason, DynamicTransitionReason::Resume { .. }));
    let PositionForestNode::Whole { expression, .. } = &lineage.nodes[*from] else {
        panic!()
    };
    assert!(
        matches!(expression.shallow().unwrap(), DynamicSampleExpression::Programming { value: DynamicValue::Family(value), .. } if value == angles(90.))
    );
}
struct Blocked {
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for Blocked {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        Err(TransitionError::Requires(requirement))
    }
}
#[test]
fn remaining_required_request_keeps_original_node_after_local_size_selection_and_source_hole() {
    let root = required(
        Some(scale(target(5.), leaf(target(4.)), 1.5)),
        Some(leaf(angles(90.))),
    );
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0.),
        &[whole(leaf(angles(10.)), 1), whole(root, 3)],
    )
    .unwrap();
    let original_root = registry.source_root(1).unwrap().unwrap();
    let mut branch = registry.branch();
    branch.replace_source(0, None).unwrap();
    branch
        .select_local_endpoint(
            &operation(&registry, 1, true),
            PositionLocalEndpoint::ScaleValue,
        )
        .unwrap();
    let context = FamilyCompositionContext::default();
    let mut evaluation = branch
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let frame = Blocked {
        calls: Cell::new(0),
    };
    let PositionCompositionProgress::NeedsMaterialization(request) = evaluation
        .advance(registry.capture_id(), &context, &frame)
        .unwrap()
    else {
        panic!()
    };
    let origin = request.origin().unwrap();
    assert_eq!(origin.original_source_indices(), &[1]);
    assert!(registry.source_node_for_origin(origin).unwrap().unwrap() == original_root);
    assert!(matches!(
        registry.node_view(&original_root).unwrap(),
        PositionSourceNodeView::Transition {
            reason: DynamicTransitionReason::Required { .. },
            ..
        }
    ));
    let repeated = evaluation
        .advance(registry.capture_id(), &context, &frame)
        .unwrap();
    assert!(
        matches!(repeated, PositionCompositionProgress::NeedsMaterialization(repeated) if repeated.request_id == request.request_id)
    );
    assert_eq!(frame.calls.get(), 1);
    evaluation.into_scratch();
}

#[test]
fn wrong_registry_kind_conflict_and_inactive_histories_reject_without_mutation() {
    let samples = [whole(
        required(Some(leaf(angles(10.))), Some(leaf(angles(90.)))),
        3,
    )];
    let capture = Uuid::new_v4();
    let registry = CapturedPositionProgram::new(capture, &angles(0.), &samples).unwrap();
    let foreign = CapturedPositionProgram::new(capture, &angles(0.), &samples).unwrap();
    let root = registry.source_root(0).unwrap().unwrap();
    let mut branch = registry.branch();
    assert!(
        branch
            .select_local_endpoint(
                &foreign.source_root(0).unwrap().unwrap(),
                PositionLocalEndpoint::RequiredIncoming
            )
            .is_err()
    );
    assert!(
        branch
            .select_local_endpoint(&root, PositionLocalEndpoint::ScaleValue)
            .is_err()
    );
    branch
        .select_local_endpoint(&root, PositionLocalEndpoint::RequiredIncoming)
        .unwrap();
    assert!(
        branch
            .select_local_endpoint(&root, PositionLocalEndpoint::RequiredOutgoing)
            .is_err()
    );
    assert!(
        matches!(expression_of(&branch), DynamicSampleExpression::Programming { value: DynamicValue::Family(value), .. } if value == angles(90.))
    );
    let zero = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0.),
        &[whole(
            scale(
                angles(5.),
                resume(Some(leaf(angles(10.))), Some(leaf(angles(90.))), 40),
                0.,
            ),
            3,
        )],
    )
    .unwrap();
    let mut branch = zero.branch();
    assert!(
        branch
            .select_local_endpoint(
                &zero.source_root(0).unwrap().unwrap(),
                PositionLocalEndpoint::ScaleValue
            )
            .is_err()
    );
    assert!(matches!(
        branch.resume_scope_membership(scope(40)).unwrap(),
        PositionResumeScopeMembership::Absent
    ));
    let scoped = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0.),
        &[whole(
            resume(
                Some(required(Some(leaf(angles(10.))), Some(leaf(angles(90.))))),
                Some(leaf(angles(180.))),
                40,
            ),
            3,
        )],
    )
    .unwrap();
    let inactive = operation(&scoped, 0, false);
    let mut branch = scoped.branch();
    branch
        .choose_resume(scope(40), PositionResumeEndpoint::Incoming)
        .unwrap();
    assert!(
        branch
            .select_local_endpoint(&inactive, PositionLocalEndpoint::RequiredIncoming)
            .is_err()
    );
}
