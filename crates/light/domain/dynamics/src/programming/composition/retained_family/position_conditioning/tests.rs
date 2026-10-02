use super::*;
use crate::{CompiledCoupledExpression, DynamicTransitionReason};

fn angles(pan: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, 20.)))
}
fn rank(instance: u128, controller: u128, lane: u128) -> FamilySampleRank {
    FamilySampleRank {
        priority: 10,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 9,
        stable_order: lane,
        identity: FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(instance),
            controller_id: Uuid::from_u128(controller),
            lane_id: Uuid::from_u128(lane),
        },
    }
}
fn leaf(pan: f32) -> Arc<DynamicSampleExpression> {
    let value = angles(pan);
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn resume(
    from: Option<Arc<DynamicSampleExpression>>,
    to: Option<Arc<DynamicSampleExpression>>,
    occurrence: u128,
    progress: f32,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from,
        to,
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(occurrence),
        },
    })
}
fn source(root: Arc<DynamicSampleExpression>, rank: FamilySampleRank) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(root, ProgrammingOwner::Position, None, None)
                .unwrap(),
        ),
        rank,
        activation_mix: 0.7,
    }
}
fn scope(instance: u128, controller: u128, occurrence: u128) -> PositionResumeScope {
    PositionResumeScope {
        instance_id: Uuid::from_u128(instance),
        controller_id: Uuid::from_u128(controller),
        occurrence_id: Uuid::from_u128(occurrence),
    }
}
fn expression(source: &FamilyCompositionSample) -> Arc<DynamicSampleExpression> {
    match source {
        FamilyCompositionSample::WholeExpression { expression, .. } => {
            Arc::new(expression.expression().clone())
        }
        FamilyCompositionSample::CoupledExpression { expression, .. } => {
            expression.expression().unwrap().clone()
        }
        _ => panic!("retained test source"),
    }
}
fn exact_value(source: &FamilyCompositionSample) -> AttributeValue {
    let DynamicSampleExpression::Programming {
        value: DynamicValue::Family(value),
        ..
    } = expression(source).shallow().unwrap()
    else {
        panic!("conditioned original leaf")
    };
    value
}

#[test]
fn choices_use_actual_instance_controller_and_occurrence_without_changing_other_sources() {
    let root = resume(Some(leaf(10.)), Some(leaf(90.)), 40, 0.25);
    let required = Arc::new(DynamicSampleExpression::Transition {
        from: Some(leaf(30.)),
        to: Some(leaf(70.)),
        progress: 0.25,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::LiveJointAngles,
        },
    });
    let sources = [
        source(root.clone(), rank(1, 2, 3)),
        source(root.clone(), rank(1, 9, 4)),
        source(root, rank(99, 2, 5)),
        source(required, rank(1, 2, 6)),
    ];
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &sources).unwrap();
    assert_eq!(registry.source_count(), 4);
    let mut branch = registry.branch();
    branch
        .choose_resume(scope(1, 2, 40), PositionResumeEndpoint::Incoming)
        .unwrap();
    assert!(
        branch
            .choose_resume(scope(1, 2, 40), PositionResumeEndpoint::Outgoing)
            .is_err()
    );
    assert!(
        branch
            .choose_resume(scope(1, 2, 999), PositionResumeEndpoint::Incoming)
            .is_err()
    );
    let conditioned = branch.conditioned_sources().unwrap();
    assert_eq!(exact_value(conditioned[0].as_ref().unwrap()), angles(90.));
    assert_eq!(conditioned[0].as_ref().unwrap().rank(), sources[0].rank());
    let FamilyCompositionSample::WholeExpression { activation_mix, .. } =
        conditioned[0].as_ref().unwrap()
    else {
        panic!()
    };
    assert_eq!(*activation_mix, 0.7);
    for index in 1..4 {
        let FamilyCompositionSample::WholeExpression {
            expression: old, ..
        } = &sources[index]
        else {
            panic!()
        };
        let FamilyCompositionSample::WholeExpression {
            expression: new, ..
        } = conditioned[index].as_ref().unwrap()
        else {
            panic!()
        };
        assert!(Arc::ptr_eq(old, new));
    }
}

#[test]
fn original_source_slots_and_nested_handles_survive_release_and_replacement() {
    let root = resume(
        Some(leaf(10.)),
        Some(resume(Some(leaf(30.)), Some(leaf(90.)), 41, 0.5)),
        40,
        0.5,
    );
    let sources = [
        source(root, rank(1, 2, 3)),
        source(leaf(180.), rank(1, 9, 4)),
    ];
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &sources).unwrap();
    let root = registry.source_root(0).unwrap().unwrap();
    let PositionSourceNodeView::Transition {
        to: Some(inner), ..
    } = registry.node_view(&root).unwrap()
    else {
        panic!()
    };
    let PositionSourceNodeView::Transition {
        to: Some(incoming), ..
    } = registry.node_view(&inner).unwrap()
    else {
        panic!()
    };
    assert_eq!(incoming.source_index(), 0);
    assert_eq!(incoming.capture_id(), registry.capture_id());
    assert_eq!(
        registry.resume_scope(&inner).unwrap(),
        Some(scope(1, 2, 41))
    );
    let mut branch = registry.branch();
    branch
        .choose_resume(scope(1, 2, 40), PositionResumeEndpoint::Incoming)
        .unwrap();
    branch
        .choose_resume(scope(1, 2, 41), PositionResumeEndpoint::Incoming)
        .unwrap();
    branch.replace_source(0, Some(&inner)).unwrap();
    branch.replace_source(0, Some(&incoming)).unwrap();
    branch.replace_source(1, None).unwrap();
    let conditioned = branch.conditioned_sources().unwrap();
    assert_eq!(conditioned.len(), 2);
    assert!(conditioned[1].is_none());
    assert_eq!(exact_value(conditioned[0].as_ref().unwrap()), angles(90.));
    assert_eq!(conditioned[0].as_ref().unwrap().rank(), sources[0].rank());
}

#[test]
fn foreign_program_same_capture_and_other_source_handles_cannot_authorize_overrides() {
    let capture = Uuid::new_v4();
    let sources = [
        source(leaf(10.), rank(1, 2, 3)),
        source(leaf(90.), rank(1, 2, 4)),
    ];
    let original = CapturedPositionProgram::new(capture, &angles(0.), &sources).unwrap();
    let foreign = CapturedPositionProgram::new(capture, &angles(0.), &sources).unwrap();
    let mut branch = original.branch();
    assert!(
        branch
            .replace_source(0, foreign.source_root(0).unwrap().as_ref())
            .is_err()
    );
    assert!(
        branch
            .replace_source(0, original.source_root(1).unwrap().as_ref())
            .is_err()
    );
    assert!(branch.replace_source(9, None).is_err());
    assert!(
        original
            .node_view(&foreign.source_root(0).unwrap().unwrap())
            .is_err()
    );
    let cloned = original.clone();
    branch
        .replace_source(0, cloned.source_root(0).unwrap().as_ref())
        .unwrap();
    assert_eq!(
        exact_value(branch.conditioned_sources().unwrap()[0].as_ref().unwrap()),
        angles(10.)
    );
}

#[test]
fn selected_resume_ancestry_cannot_be_replaced_by_its_opposite_original_child() {
    let sources = [source(
        resume(Some(leaf(10.)), Some(leaf(90.)), 40, 0.5),
        rank(1, 2, 3),
    )];
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &sources).unwrap();
    let root = registry.source_root(0).unwrap().unwrap();
    let PositionSourceNodeView::Transition {
        from: Some(outgoing),
        ..
    } = registry.node_view(&root).unwrap()
    else {
        panic!()
    };
    let mut branch = registry.branch();
    branch
        .choose_resume(scope(1, 2, 40), PositionResumeEndpoint::Incoming)
        .unwrap();
    assert!(branch.replace_source(0, Some(&outgoing)).is_err());
    assert_eq!(
        exact_value(branch.conditioned_sources().unwrap()[0].as_ref().unwrap()),
        angles(90.)
    );
    let mut nested = registry.branch();
    nested.replace_source(0, Some(&outgoing)).unwrap();
    assert!(
        nested
            .choose_resume(scope(1, 2, 40), PositionResumeEndpoint::Incoming)
            .is_err()
    );
    assert_eq!(
        exact_value(nested.conditioned_sources().unwrap()[0].as_ref().unwrap()),
        angles(10.)
    );
}

#[test]
fn exact_endpoints_and_zero_size_do_not_expose_unused_history_for_revival() {
    let hidden = resume(Some(leaf(10.)), Some(leaf(20.)), 41, 0.5);
    let root = resume(Some(hidden.clone()), Some(leaf(90.)), 40, 1.);
    let scale = Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &angles(30.)).unwrap(),
        ),
        base: DynamicValue::Family(angles(30.)),
        value: hidden,
        factor: 0.,
        baseline_occurrence: None,
    });
    let sources = [source(root, rank(1, 2, 3)), source(scale, rank(1, 2, 4))];
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &sources).unwrap();
    let PositionSourceNodeView::Transition { from, to, .. } = registry
        .node_view(&registry.source_root(0).unwrap().unwrap())
        .unwrap()
    else {
        panic!()
    };
    assert!(from.is_none());
    assert!(to.is_some());
    let PositionSourceNodeView::Scale { value, .. } = registry
        .node_view(&registry.source_root(1).unwrap().unwrap())
        .unwrap()
    else {
        panic!()
    };
    assert!(value.is_none());
    assert!(registry.resume_nodes(scope(1, 2, 41)).unwrap().is_empty());
    assert!(
        registry
            .branch()
            .choose_resume(scope(1, 2, 40), PositionResumeEndpoint::Outgoing)
            .is_err()
    );
}

#[test]
fn owner_and_original_resume_progress_are_validated_before_any_choices_erase_nodes() {
    let focus = AttributeValue::Normalized(0.5);
    let root = Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Focus, &focus).unwrap(),
        ),
        value: DynamicValue::Family(focus),
        occurrence: None,
        dependency_occurrence: None,
    });
    let foreign = FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(root, ProgrammingOwner::Focus, None, None)
                .unwrap(),
        ),
        rank: rank(1, 2, 3),
        activation_mix: 1.,
    };
    assert!(CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &[foreign]).is_err());
    assert!(CapturedPositionProgram::new(Uuid::nil(), &angles(0.), &[]).is_err());
    let inconsistent = [
        source(
            resume(Some(leaf(10.)), Some(leaf(90.)), 40, 0.25),
            rank(1, 2, 3),
        ),
        source(
            resume(Some(leaf(10.)), Some(leaf(90.)), 40, 0.75),
            rank(1, 2, 4),
        ),
    ];
    assert!(CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &inconsistent).is_err());
}

#[test]
fn compiled_forest_preserves_original_identity_and_supports_scoped_selection_and_release() {
    use crate::programming::expression_coupled::PositionForestNode;
    let forest = [
        PositionForestNode::Whole {
            expression: leaf(10.),
            lane_id: Uuid::from_u128(3),
            sources: Arc::from([]),
        },
        PositionForestNode::Whole {
            expression: leaf(90.),
            lane_id: Uuid::from_u128(4),
            sources: Arc::from([]),
        },
        PositionForestNode::Transition {
            from: Some(0),
            to: Some(1),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(40),
            },
        },
    ];
    let compiled = Arc::new(
        CompiledCoupledExpression::from_position_forest(Arc::from([]), &forest, 2).unwrap(),
    );
    let sources = [FamilyCompositionSample::CoupledExpression {
        expression: compiled.clone(),
        rank: rank(1, 2, 3),
        activation_mix: 1.,
    }];
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &sources).unwrap();
    let root = registry.source_root(0).unwrap().unwrap();
    let PositionSourceNodeView::Transition {
        from: Some(outgoing),
        to: Some(incoming),
        ..
    } = registry.node_view(&root).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        registry
            .node_source_rank(&outgoing)
            .unwrap()
            .dynamic_identity()
            .unwrap()
            .lane_id,
        Uuid::from_u128(3)
    );
    assert_eq!(
        registry
            .node_source_rank(&incoming)
            .unwrap()
            .dynamic_identity()
            .unwrap()
            .lane_id,
        Uuid::from_u128(4)
    );
    let mut branch = registry.branch();
    let conditioned = branch.conditioned_sources().unwrap();
    let FamilyCompositionSample::CoupledExpression { expression, .. } =
        conditioned[0].as_ref().unwrap()
    else {
        panic!()
    };
    assert!(Arc::ptr_eq(expression, &compiled));
    branch
        .choose_resume(scope(1, 2, 40), PositionResumeEndpoint::Incoming)
        .unwrap();
    let selected = branch.conditioned_sources().unwrap();
    let FamilyCompositionSample::CoupledExpression { expression, .. } =
        selected[0].as_ref().unwrap()
    else {
        panic!()
    };
    let lineage = expression.position_forest_lineage().unwrap();
    let PositionForestNode::Whole {
        expression: selected,
        lane_id,
        ..
    } = &lineage.nodes[lineage.root]
    else {
        panic!()
    };
    assert_eq!(*lane_id, Uuid::from_u128(4));
    let PositionForestNode::Whole {
        expression: original,
        ..
    } = &forest[1]
    else {
        panic!()
    };
    assert!(Arc::ptr_eq(selected, original));
    branch.replace_source(0, None).unwrap();
    assert!(branch.conditioned_sources().unwrap()[0].is_none());
}

#[test]
fn override_subtree_cannot_witness_a_resume_that_only_exists_elsewhere_in_original_source() {
    let outgoing = resume(Some(leaf(10.)), Some(leaf(20.)), 41, 0.5);
    let incoming = resume(Some(leaf(80.)), Some(leaf(90.)), 42, 0.5);
    let sources = [source(
        resume(Some(outgoing), Some(incoming), 40, 0.5),
        rank(1, 2, 3),
    )];
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &sources).unwrap();
    let root = registry.source_root(0).unwrap().unwrap();
    let PositionSourceNodeView::Transition {
        from: Some(outgoing),
        ..
    } = registry.node_view(&root).unwrap()
    else {
        panic!()
    };
    let mut branch = registry.branch();
    branch.replace_source(0, Some(&outgoing)).unwrap();
    assert!(
        branch
            .choose_resume(scope(1, 2, 42), PositionResumeEndpoint::Incoming)
            .is_err()
    );
    assert!(
        branch
            .choose_resume(scope(1, 2, 40), PositionResumeEndpoint::Outgoing)
            .is_err()
    );
    // Rejection leaves this branch intact; its actual nested Resume remains selectable.
    branch
        .choose_resume(scope(1, 2, 41), PositionResumeEndpoint::Incoming)
        .unwrap();
    assert_eq!(
        exact_value(branch.conditioned_sources().unwrap()[0].as_ref().unwrap()),
        angles(20.)
    );
}
