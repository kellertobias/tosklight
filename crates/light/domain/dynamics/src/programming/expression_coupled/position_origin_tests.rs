use super::*;
use crate::{CompiledProgrammingFamilyExpression, FamilyEvaluationProgress};
use uuid::Uuid;

fn angles(pan: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, 20.)))
}
fn target() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [4., 2., 3.],
    )))
}
fn leaf(value: AttributeValue) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn transition(
    from: Option<Arc<DynamicSampleExpression>>,
    to: Option<Arc<DynamicSampleExpression>>,
    progress: f32,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from,
        to,
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(40),
        },
    })
}
fn whole(expression: Arc<DynamicSampleExpression>, lane: u128) -> PositionForestNode {
    PositionForestNode::Whole {
        expression,
        lane_id: Uuid::from_u128(lane),
        sources: Arc::from([]),
    }
}
struct Endpoints;
impl CoupledExpressionContext for Endpoints {
    fn orthogonal_underlay(
        &self,
        _: ColorComponent,
        _: &AttributeValue,
    ) -> Result<f32, TransitionError> {
        panic!("Position has no Color orthogonal role")
    }
    fn materialize_base(
        &self,
        endpoint: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
    ) -> Result<AttributeValue, TransitionError> {
        match endpoint {
            Some((_, DynamicValue::Family(value))) => Ok(value.clone()),
            _ => Err(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            )),
        }
    }
}
struct Unavailable;
impl WholeFamilyExpressionFrameResolver for Unavailable {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        Err(TransitionError::Requires(requirement))
    }
}

#[test]
fn ordinary_pending_operation_routes_to_its_emitted_original_tape_node() {
    let root = transition(Some(leaf(target())), Some(leaf(angles(90.))), 0.5);
    let tape = RetainedExpressionTape::from_roots(std::slice::from_ref(&root)).unwrap();
    let compiled = Arc::new(CompiledCoupledExpression::new(root, None).unwrap());
    let mut evaluation = compiled.begin_position_evaluation().unwrap();
    let PositionEvaluationProgress::NeedsMaterialization(request) =
        evaluation.advance(&Endpoints, &Unavailable, None).unwrap()
    else {
        panic!("mixed retained Position operation must suspend")
    };
    assert_eq!(
        compiled.position_node_origin(request.node),
        Some(PositionCompiledNodeLocation::Tape(tape.roots[0]))
    );
    assert_eq!(compiled.position_node_origin(0), None);
    assert_eq!(compiled.position_node_origin(usize::MAX), None);
}

#[test]
fn ordinary_exact_alias_and_zero_size_do_not_claim_inactive_history() {
    let exact = transition(Some(leaf(angles(10.))), Some(leaf(angles(90.))), 1.);
    let tape = RetainedExpressionTape::from_roots(std::slice::from_ref(&exact)).unwrap();
    let RetainedExpressionNode::Transition {
        to: Some(selected),
        from: Some(unused),
        ..
    } = &tape.nodes[tape.roots[0].0 as usize]
    else {
        panic!()
    };
    let compiled = CompiledCoupledExpression::new(exact, None).unwrap();
    assert_eq!(
        compiled.position_node_origin(compiled.trace_root_node()),
        Some(PositionCompiledNodeLocation::Tape(*selected))
    );
    assert!(
        !compiled
            .position_node_origins
            .contains(&Some(PositionCompiledNodeLocation::Tape(*unused)))
    );
    assert!(
        !compiled
            .position_node_origins
            .contains(&Some(PositionCompiledNodeLocation::Tape(tape.roots[0])))
    );

    let baseline = angles(30.);
    let zero = Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &baseline).unwrap(),
        ),
        base: DynamicValue::Family(baseline),
        value: leaf(angles(180.)),
        factor: 0.,
        baseline_occurrence: None,
    });
    let tape = RetainedExpressionTape::from_roots(std::slice::from_ref(&zero)).unwrap();
    let compiled = CompiledCoupledExpression::new(zero, None).unwrap();
    assert_eq!(
        compiled.position_node_origin(compiled.trace_root_node()),
        Some(PositionCompiledNodeLocation::Tape(tape.roots[0]))
    );
    assert_eq!(
        compiled.nodes.len(),
        2,
        "unused Size child must not acquire an origin"
    );
}

#[test]
fn imported_whole_and_actual_forest_transition_keep_distinct_emission_paths() {
    let nested = transition(Some(leaf(target())), Some(leaf(angles(30.))), 0.5);
    let forest = [
        whole(nested, 10),
        whole(leaf(angles(90.)), 20),
        PositionForestNode::Transition {
            from: Some(0),
            to: Some(1),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(41),
            },
        },
    ];
    let compiled = Arc::new(
        CompiledCoupledExpression::from_position_forest(Arc::from([]), &forest, 2).unwrap(),
    );
    let lineage = compiled.position_forest_lineage().unwrap();
    let original_whole = &lineage.wholes[0];
    assert_eq!(
        compiled.position_node_origin(lineage.compiled_nodes[0]),
        Some(PositionCompiledNodeLocation::WholeTape(
            0,
            original_whole.tape.roots[0]
        ))
    );
    assert_eq!(
        compiled.position_node_origin(compiled.trace_root_node()),
        Some(PositionCompiledNodeLocation::Forest(2))
    );
    let mut evaluation = compiled.begin_position_evaluation().unwrap();
    let PositionEvaluationProgress::NeedsMaterialization(request) =
        evaluation.advance(&Endpoints, &Unavailable, None).unwrap()
    else {
        panic!("imported operation suspends first")
    };
    assert_eq!(
        compiled.position_node_origin(request.node),
        Some(PositionCompiledNodeLocation::WholeTape(
            0,
            original_whole.tape.roots[0]
        ))
    );
}

#[test]
fn forest_exact_alias_does_not_overwrite_imported_child_or_assign_underlay_origin() {
    let forest = [
        whole(
            transition(Some(leaf(angles(10.))), Some(leaf(angles(30.))), 1.),
            10,
        ),
        PositionForestNode::Transition {
            from: Some(0),
            to: None,
            progress: 0.,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(40),
            },
        },
    ];
    let compiled =
        CompiledCoupledExpression::from_position_forest(Arc::from([]), &forest, 1).unwrap();
    let lineage = compiled.position_forest_lineage().unwrap();
    let tape = &lineage.wholes[0].tape;
    let RetainedExpressionNode::Transition {
        to: Some(selected),
        from: Some(unused),
        ..
    } = &tape.nodes[tape.roots[0].0 as usize]
    else {
        panic!()
    };
    assert_eq!(
        compiled.position_node_origin(compiled.trace_root_node()),
        Some(PositionCompiledNodeLocation::WholeTape(0, *selected))
    );
    for inactive_or_alias in [*unused, tape.roots[0]] {
        assert!(!compiled.position_node_origins.contains(&Some(
            PositionCompiledNodeLocation::WholeTape(0, inactive_or_alias)
        )));
    }
    assert!(
        !compiled
            .position_node_origins
            .contains(&Some(PositionCompiledNodeLocation::Forest(1)))
    );
    let release = [
        forest[0].clone(),
        PositionForestNode::Transition {
            from: Some(0),
            to: None,
            progress: 1.,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(40),
            },
        },
    ];
    let released =
        CompiledCoupledExpression::from_position_forest(Arc::from([]), &release, 1).unwrap();
    assert_eq!(released.trace_root_node(), 0);
    assert_eq!(
        released.position_node_origin(released.trace_root_node()),
        None
    );
}

#[test]
fn nested_sourcecohort_member_uses_its_own_whole_graph_not_outer_forest_ids() {
    let nested = transition(Some(leaf(target())), Some(leaf(angles(30.))), 0.5);
    let original_tape = RetainedExpressionTape::from_roots(std::slice::from_ref(&nested)).unwrap();
    let inner = Arc::new(
        CompiledProgrammingFamilyExpression::new(nested, ProgrammingOwner::Position, None, None)
            .unwrap(),
    );
    let forest = [PositionForestNode::SourceCohort(Arc::from([
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(10),
            expression: inner.clone(),
        },
    ]))];
    let outer = CompiledCoupledExpression::from_position_forest(Arc::from([]), &forest, 0).unwrap();
    assert_eq!(
        outer.position_node_origin(outer.trace_root_node()),
        Some(PositionCompiledNodeLocation::Forest(0))
    );
    let mut evaluation = inner.begin_evaluation(None).unwrap();
    let FamilyEvaluationProgress::NeedsMaterialization(request) =
        evaluation.advance(&Unavailable, None).unwrap()
    else {
        panic!("nested whole graph operation suspends")
    };
    assert_eq!(
        inner.retained_node_origin(request.node),
        Some(original_tape.roots[0])
    );
    assert_eq!(
        outer.position_forest_lineage().unwrap().cohort_members[0].member_index,
        0
    );
    assert_eq!(
        outer.position_forest_lineage().unwrap().cohort_members[0].lane_id,
        Uuid::from_u128(10)
    );
}

#[test]
fn actual_anglepair_and_target_cohort_nodes_have_original_forest_emission_origins() {
    let axis = |lane, component, value| CoupledComponentEndpoint {
        lane_id: Uuid::from_u128(lane),
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
        value: DynamicValue::Scalar(value),
        role: CoupledLeafRole::Authored,
        occurrence: None,
        dependency_occurrence: None,
    };
    let pair = PositionAnglePairEndpoint {
        axes: [
            PositionAngleAxis::Materialized(axis(10, ProgrammingComponent::Pan, 90.)),
            PositionAngleAxis::Materialized(axis(11, ProgrammingComponent::Tilt, 20.)),
        ],
    };
    let target_x = CoupledComponentEndpoint {
        lane_id: Uuid::from_u128(12),
        address: Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Target {
                        reference: Some(TargetReference::Origin),
                    },
                    component: Some(ProgrammingComponent::TargetX),
                },
                None,
            )
            .unwrap(),
        ),
        value: DynamicValue::Scalar(4.),
        role: CoupledLeafRole::Authored,
        occurrence: None,
        dependency_occurrence: None,
    };
    let forest = [
        PositionForestNode::AnglePair(Arc::new(pair)),
        PositionForestNode::Cohort(Arc::from([target_x])),
        PositionForestNode::Transition {
            from: Some(0),
            to: Some(1),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(40),
            },
        },
    ];
    let compiled =
        CompiledCoupledExpression::from_position_forest(Arc::from([]), &forest, 2).unwrap();
    let lineage = compiled.position_forest_lineage().unwrap();
    for (original, emitted) in lineage.compiled_nodes.iter().enumerate() {
        assert_eq!(
            compiled.position_node_origin(*emitted),
            Some(PositionCompiledNodeLocation::Forest(original))
        );
    }
}
