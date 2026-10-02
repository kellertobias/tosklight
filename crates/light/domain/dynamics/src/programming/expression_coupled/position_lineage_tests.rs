use super::*;
use crate::{AngleNumericNode, AngleNumericProgram};
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
fn whole(value: AttributeValue) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn branch(expression: Arc<DynamicSampleExpression>, lane: u128) -> PositionForestNode {
    PositionForestNode::Whole {
        expression,
        lane_id: Uuid::from_u128(lane),
        sources: Arc::from([]),
    }
}
fn reason(occurrence: u128) -> DynamicTransitionReason {
    DynamicTransitionReason::Resume {
        occurrence_id: Uuid::from_u128(occurrence),
    }
}

#[test]
fn original_forest_and_imported_whole_tape_map_to_the_actual_single_compiler_graph() {
    let required = DynamicTransitionReason::Required {
        requirement: TransitionRequirement::LiveJointAngles,
    };
    let original = Arc::new(DynamicSampleExpression::Transition {
        from: Some(whole(angles(10.))),
        to: Some(whole(angles(30.))),
        progress: 0.5,
        reason: required,
    });
    let forest = [
        branch(original.clone(), 10),
        branch(whole(angles(90.)), 11),
        PositionForestNode::Transition {
            from: Some(0),
            to: Some(1),
            progress: 0.25,
            reason: reason(40),
        },
    ];
    let retained: Arc<[crate::DynamicRuntimeSample]> = Arc::from([]);
    let compiled =
        CompiledCoupledExpression::from_position_forest(retained.clone(), &forest, 2).unwrap();
    let lineage = compiled.position_forest_lineage().unwrap();
    assert_eq!(lineage.root, 2);
    assert_eq!(lineage.nodes.len(), forest.len());
    let PositionForestNode::Whole { expression, .. } = &lineage.nodes[0] else {
        panic!()
    };
    assert!(Arc::ptr_eq(expression, &original));
    assert_eq!(lineage.compiled_nodes[2], compiled.trace_root_node());
    assert_eq!(
        compiled.transition_reason(lineage.compiled_nodes[2]),
        Some(reason(40))
    );
    assert_eq!(
        compiled.transition_reason(lineage.compiled_nodes[0]),
        Some(required)
    );
    assert_ne!(
        lineage.compiled_nodes[2], 2,
        "graph IDs are not original forest indices"
    );
    let imported = lineage
        .wholes
        .iter()
        .find(|member| member.forest_node == 0)
        .unwrap();
    assert_eq!(imported.lane_id, Uuid::from_u128(10));
    assert_eq!(imported.tape_to_compiled.len(), imported.tape.nodes.len());
    assert_eq!(
        imported.tape_to_compiled[imported.tape.roots[0].0 as usize],
        Some(lineage.compiled_nodes[0])
    );
    for (index, node) in imported.tape.nodes.iter().enumerate() {
        if let RetainedExpressionNode::Transition { reason, .. } = node {
            assert_eq!(
                compiled.transition_reason(imported.tape_to_compiled[index].unwrap()),
                Some(*reason)
            );
        }
    }
    let CoupledRetainedSources::PositionForest(actual) = compiled.retained_sources() else {
        panic!()
    };
    assert!(Arc::ptr_eq(actual, &retained));
}

#[test]
fn imported_exact_endpoint_maps_aliases_and_leaves_unused_original_nodes_unmapped() {
    let original = Arc::new(DynamicSampleExpression::Transition {
        from: Some(whole(angles(10.))),
        to: Some(whole(angles(90.))),
        progress: 1.,
        reason: reason(40),
    });
    let compiled =
        CompiledCoupledExpression::from_position_forest(Arc::from([]), &[branch(original, 10)], 0)
            .unwrap();
    let lineage = compiled.position_forest_lineage().unwrap();
    let imported = &lineage.wholes[0];
    let RetainedExpressionNode::Transition {
        from: Some(from),
        to: Some(to),
        ..
    } = &imported.tape.nodes[imported.tape.roots[0].0 as usize]
    else {
        panic!()
    };
    assert_eq!(imported.tape_to_compiled[from.0 as usize], None);
    assert_eq!(
        imported.tape_to_compiled[to.0 as usize],
        Some(compiled.trace_root_node())
    );
    assert_eq!(
        imported.tape_to_compiled[imported.tape.roots[0].0 as usize],
        Some(compiled.trace_root_node())
    );
    let CoupledExpressionFootprint::Exact {
        value: DynamicValue::Family(value),
        ..
    } = compiled.footprint(CoupledExpressionRole::Base)
    else {
        panic!()
    };
    assert_eq!(value, &angles(90.));
}

#[test]
fn source_cohort_lineage_preserves_original_member_order_lane_and_whole_subtree() {
    let reference = TargetReference::Origin;
    let component = CoupledComponentEndpoint {
        lane_id: Uuid::from_u128(17),
        address: Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Target {
                        reference: Some(reference),
                    },
                    component: Some(ProgrammingComponent::TargetX),
                },
                None,
            )
            .unwrap(),
        ),
        value: DynamicValue::Scalar(9.),
        role: CoupledLeafRole::Authored,
        occurrence: None,
        dependency_occurrence: None,
    };
    let expression = Arc::new(
        crate::CompiledProgrammingFamilyExpression::new(
            whole(target()),
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    );
    let members: Arc<[CoupledCohortEndpoint]> = Arc::from([
        CoupledCohortEndpoint::Materialized(component),
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(9),
            expression: expression.clone(),
        },
    ]);
    let compiled = CompiledCoupledExpression::from_position_forest(
        Arc::from([]),
        &[PositionForestNode::SourceCohort(members.clone())],
        0,
    )
    .unwrap();
    let lineage = compiled.position_forest_lineage().unwrap();
    let PositionForestNode::SourceCohort(actual) = &lineage.nodes[0] else {
        panic!()
    };
    assert!(Arc::ptr_eq(actual, &members));
    let CoupledCohortEndpoint::WholeExpression {
        expression: actual, ..
    } = &actual[1]
    else {
        panic!()
    };
    assert!(Arc::ptr_eq(actual, &expression));
    assert_eq!(lineage.cohort_members.len(), 2);
    for (index, lane) in [17, 9].into_iter().enumerate() {
        let member = &lineage.cohort_members[index];
        assert_eq!(
            (member.forest_node, member.member_index, member.lane_id),
            (0, index, Uuid::from_u128(lane))
        );
    }
    assert!(lineage.cohort_members[0].whole_tape.is_none());
    let tape = lineage.cohort_members[1].whole_tape.as_ref().unwrap();
    let RetainedExpressionNode::Programming {
        value: DynamicValue::Family(value),
        ..
    } = &tape.nodes[tape.roots[0].0 as usize]
    else {
        panic!()
    };
    assert_eq!(value, &target());
}

#[test]
fn numeric_and_current_pair_lineage_retains_shared_original_capture_and_numeric_program_arcs() {
    let original = Arc::new(CapturedPositionCurrent {
        value: target(),
        occurrence: None,
    });
    let address = |component| {
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Angles,
                    component: Some(component),
                },
                None,
            )
            .unwrap(),
        )
    };
    let numeric = Arc::new(AngleNumericProgram {
        address: address(ProgrammingComponent::Pan).address().clone(),
        occurrence: None,
        nodes: vec![AngleNumericNode::Current],
        root: 0,
        operations: Default::default(),
    });
    let pair = Arc::new(PositionAnglePairEndpoint {
        axes: [
            PositionAngleAxis::Numeric {
                lane_id: Uuid::from_u128(10),
                address: address(ProgrammingComponent::Pan),
                program: numeric.clone(),
                original: original.clone(),
            },
            PositionAngleAxis::Current {
                lane_id: Uuid::from_u128(11),
                address: address(ProgrammingComponent::Tilt),
                original: original.clone(),
            },
        ],
    });
    let compiled = CompiledCoupledExpression::from_position_forest(
        Arc::from([]),
        &[PositionForestNode::AnglePair(pair.clone())],
        0,
    )
    .unwrap();
    let lineage = compiled.position_forest_lineage().unwrap();
    let PositionForestNode::AnglePair(actual) = &lineage.nodes[0] else {
        panic!()
    };
    assert!(Arc::ptr_eq(actual, &pair));
    let PositionAngleAxis::Numeric {
        program,
        original: pan,
        lane_id,
        ..
    } = &actual.axes[0]
    else {
        panic!()
    };
    assert!(Arc::ptr_eq(program, &numeric));
    assert!(Arc::ptr_eq(pan, &original));
    assert_eq!(*lane_id, Uuid::from_u128(10));
    let PositionAngleAxis::Current {
        original: tilt,
        lane_id,
        ..
    } = &actual.axes[1]
    else {
        panic!()
    };
    assert!(Arc::ptr_eq(tilt, &original));
    assert!(Arc::ptr_eq(pan, tilt));
    assert_eq!(*lane_id, Uuid::from_u128(11));
}

#[test]
fn exact_forest_release_preserves_original_endpoint_and_aliases_the_eligible_underlay() {
    let original = whole(angles(90.));
    let forest = [
        branch(original.clone(), 10),
        PositionForestNode::Transition {
            from: Some(0),
            to: None,
            progress: 1.,
            reason: reason(40),
        },
    ];
    let compiled =
        CompiledCoupledExpression::from_position_forest(Arc::from([]), &forest, 1).unwrap();
    let lineage = compiled.position_forest_lineage().unwrap();
    assert_eq!(lineage.root, 1);
    assert_eq!(lineage.compiled_nodes[1], 0);
    assert_eq!(compiled.trace_root_node(), 0);
    assert!(matches!(
        compiled.footprint(CoupledExpressionRole::Base),
        CoupledExpressionFootprint::Inactive
    ));
    let PositionForestNode::Whole { expression, .. } = &lineage.nodes[0] else {
        panic!()
    };
    assert!(Arc::ptr_eq(expression, &original));
}

#[test]
fn malformed_forest_references_progress_and_occurrence_are_errors_before_any_indexing() {
    let make = |from, to, progress, reason| {
        vec![
            branch(whole(angles(10.)), 10),
            PositionForestNode::Transition {
                from,
                to,
                progress,
                reason,
            },
        ]
    };
    let invalid = [
        make(Some(1), None, 0.5, reason(40)),
        make(Some(999), Some(0), 0.5, reason(40)),
        make(Some(0), Some(2), 0.5, reason(40)),
        make(None, None, 0.5, reason(40)),
        make(Some(0), None, f32::NAN, reason(40)),
        make(Some(0), None, -0.1, reason(40)),
        make(Some(0), None, 1.1, reason(40)),
        make(Some(0), None, 0.5, reason(0)),
    ];
    for forest in invalid {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            CompiledCoupledExpression::from_position_forest(Arc::from([]), &forest, 1)
        }));
        assert!(
            result.is_ok(),
            "ordinary malformed forest input must not unwind"
        );
        assert!(result.unwrap().is_err());
    }
    assert!(CompiledCoupledExpression::from_position_forest(Arc::from([]), &[], 0).is_err());
    assert!(
        CompiledCoupledExpression::from_position_forest(
            Arc::from([]),
            &[branch(whole(angles(10.)), 10)],
            9
        )
        .is_err()
    );
    let ordinary = CompiledCoupledExpression::new(whole(angles(10.)), None).unwrap();
    assert!(ordinary.position_forest_lineage().is_none());
}
