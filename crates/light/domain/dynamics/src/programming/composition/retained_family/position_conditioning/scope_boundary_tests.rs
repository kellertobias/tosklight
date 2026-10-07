//! Resume children end at their captured cut, preserving atomic original cohort siblings.
use super::*;
use crate::CoupledCohortEndpoint;

fn angles(pan: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, 20.)))
}
fn rank() -> FamilySampleRank {
    FamilySampleRank {
        priority: 3,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: 3,
        identity: FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(2),
            lane_id: Uuid::from_u128(3),
        },
    }
}
fn scope(id: u128) -> PositionResumeScope {
    PositionResumeScope {
        instance_id: Uuid::from_u128(1),
        controller_id: Uuid::from_u128(2),
        occurrence_id: Uuid::from_u128(id),
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
    id: u128,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from,
        to,
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: scope(id).occurrence_id,
        },
    })
}
fn compiled(root: Arc<DynamicSampleExpression>) -> Arc<CompiledProgrammingFamilyExpression> {
    Arc::new(
        CompiledProgrammingFamilyExpression::new(root, ProgrammingOwner::Position, None, None)
            .unwrap(),
    )
}
fn whole(root: Arc<DynamicSampleExpression>) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: compiled(root),
        rank: rank(),
        activation_mix: 1.,
    }
}
fn forest(nodes: Vec<PositionForestNode>, root: usize) -> FamilyCompositionSample {
    FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::from_position_forest(Arc::from([]), &nodes, root).unwrap(),
        ),
        rank: rank(),
        activation_mix: 1.,
    }
}
fn registry(source: FamilyCompositionSample) -> CapturedPositionProgram {
    CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &[source]).unwrap()
}
fn result(branch: &PositionProgramBranch) -> FamilyCompositionSample {
    branch.conditioned_sources().unwrap().remove(0).unwrap()
}
fn whole_value(source: &FamilyCompositionSample) -> AttributeValue {
    let FamilyCompositionSample::WholeExpression { expression, .. } = source else {
        panic!("whole operand")
    };
    let DynamicSampleExpression::Programming {
        value: DynamicValue::Family(value),
        ..
    } = expression.expression().shallow().unwrap()
    else {
        panic!("original endpoint only")
    };
    value
}
fn lineage(source: &FamilyCompositionSample) -> Arc<PositionForestLineage> {
    let FamilyCompositionSample::CoupledExpression { expression, .. } = source else {
        panic!("forest operand")
    };
    expression.position_forest_lineage().unwrap().clone()
}
fn member_value(member: &CoupledCohortEndpoint) -> AttributeValue {
    let CoupledCohortEndpoint::WholeExpression { expression, .. } = member else {
        panic!()
    };
    let DynamicSampleExpression::Programming {
        value: DynamicValue::Family(value),
        ..
    } = expression.expression().shallow().unwrap()
    else {
        panic!("member endpoint only")
    };
    value
}

#[test]
fn nested_tape_cut_strips_resume_and_size_suffix_without_mutating_parent() {
    let inner = resume(Some(leaf(10.)), Some(leaf(90.)), 40);
    let scaled = Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &angles(5.)).unwrap(),
        ),
        base: DynamicValue::Family(angles(5.)),
        value: inner,
        factor: 1.5,
        baseline_occurrence: None,
    });
    let registry = registry(whole(resume(Some(scaled), Some(leaf(180.)), 50)));
    let parent = registry.branch();
    let child = parent
        .at_resume_operand(scope(40), PositionResumeEndpoint::Incoming)
        .unwrap()
        .unwrap();
    assert_eq!(whole_value(&result(&child)), angles(90.));
    assert!(child.operand_boundary_nodes().unwrap() == registry.resume_nodes(scope(40)).unwrap());
    assert!(parent.operand_boundary_nodes().unwrap().is_empty());
    assert!(matches!(
        parent.resume_scope_membership(scope(50)).unwrap(),
        PositionResumeScopeMembership::Active(_)
    ));
    assert!(matches!(
        child.resume_scope_membership(scope(50)).unwrap(),
        PositionResumeScopeMembership::Inactive
    ));
    assert!(
        child
            .at_resume_operand(scope(50), PositionResumeEndpoint::Incoming)
            .is_err()
    );
}

#[test]
fn imported_whole_cut_retains_original_lane_and_strips_outer_forest() {
    let source = forest(
        vec![
            PositionForestNode::Whole {
                expression: resume(Some(leaf(10.)), Some(leaf(90.)), 40),
                lane_id: Uuid::from_u128(20),
                sources: Arc::from([]),
            },
            PositionForestNode::Whole {
                expression: leaf(180.),
                lane_id: Uuid::from_u128(21),
                sources: Arc::from([]),
            },
            PositionForestNode::Transition {
                from: Some(0),
                to: Some(1),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: scope(50).occurrence_id,
                },
            },
        ],
        2,
    );
    let registry = registry(source);
    let child = registry
        .branch()
        .at_resume_operand(scope(40), PositionResumeEndpoint::Outgoing)
        .unwrap()
        .unwrap();
    let bound = child.conditioned_with_origins().unwrap().remove(0);
    let output = lineage(&bound.sample);
    let PositionForestNode::Whole {
        expression,
        lane_id,
        ..
    } = &output.nodes[output.root]
    else {
        panic!("no forest ancestor")
    };
    assert_eq!(*lane_id, Uuid::from_u128(20));
    assert!(
        matches!(expression.shallow().unwrap(), DynamicSampleExpression::Programming { value: DynamicValue::Family(value), .. } if value == angles(10.))
    );
    let root = &output.wholes[0].tape;
    assert!(matches!(
        bound
            .origins
            .get(NodeLocation::WholeTape(output.root, root.roots[0])),
        Some(NodeLocation::WholeTape(0, _))
    ));
}

#[test]
fn forest_scope_cut_strips_outer_resume_and_keeps_exact_child_arc() {
    let first = leaf(10.);
    let second = leaf(90.);
    let source = forest(
        vec![
            PositionForestNode::Whole {
                expression: first.clone(),
                lane_id: Uuid::from_u128(20),
                sources: Arc::from([]),
            },
            PositionForestNode::Whole {
                expression: second.clone(),
                lane_id: Uuid::from_u128(21),
                sources: Arc::from([]),
            },
            PositionForestNode::Transition {
                from: Some(0),
                to: Some(1),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: scope(40).occurrence_id,
                },
            },
            PositionForestNode::Transition {
                from: Some(2),
                to: Some(0),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: scope(50).occurrence_id,
                },
            },
        ],
        3,
    );
    let registry = registry(source);
    let child = registry
        .branch()
        .at_resume_operand(scope(40), PositionResumeEndpoint::Incoming)
        .unwrap()
        .unwrap();
    let bound = child.conditioned_with_origins().unwrap().remove(0);
    let output = lineage(&bound.sample);
    let PositionForestNode::Whole { expression, .. } = &output.nodes[output.root] else {
        panic!()
    };
    assert!(Arc::ptr_eq(expression, &second));
    assert_eq!(
        bound.origins.get(NodeLocation::Forest(output.root)),
        Some(NodeLocation::Forest(1))
    );
}

#[test]
fn member_cut_preserves_atomic_cohort_siblings_arcs_lanes_and_original_map() {
    let sibling = compiled(leaf(250.));
    let nested = resume(
        Some(resume(Some(leaf(10.)), Some(leaf(90.)), 40)),
        Some(leaf(180.)),
        50,
    );
    let members: Arc<[CoupledCohortEndpoint]> = vec![
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(20),
            expression: compiled(nested),
        },
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(21),
            expression: sibling.clone(),
        },
    ]
    .into();
    let registry = registry(forest(
        vec![
            PositionForestNode::SourceCohort(members),
            PositionForestNode::Whole {
                expression: leaf(360.),
                lane_id: Uuid::from_u128(22),
                sources: Arc::from([]),
            },
            PositionForestNode::Transition {
                from: Some(0),
                to: Some(1),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: scope(60).occurrence_id,
                },
            },
        ],
        2,
    ));
    let parent = registry.branch();
    let child = parent
        .at_resume_operand(scope(40), PositionResumeEndpoint::Incoming)
        .unwrap()
        .unwrap();
    let bound = child.conditioned_with_origins().unwrap().remove(0);
    let output = lineage(&bound.sample);
    let PositionForestNode::SourceCohort(members) = &output.nodes[output.root] else {
        panic!("atomic sibling cohort")
    };
    assert_eq!(members.len(), 2);
    assert_eq!(member_value(&members[0]), angles(90.));
    let CoupledCohortEndpoint::WholeExpression {
        expression,
        lane_id,
    } = &members[1]
    else {
        panic!()
    };
    assert!(Arc::ptr_eq(expression, &sibling));
    assert_eq!(*lane_id, Uuid::from_u128(21));
    let member = &output.cohort_members[0];
    assert!(matches!(
        bound.origins.get(NodeLocation::MemberTape(
            output.root,
            0,
            member.whole_tape.as_ref().unwrap().roots[0]
        )),
        Some(NodeLocation::MemberTape(0, 0, _))
    ));
    let boundaries = child.operand_boundary_nodes().unwrap();
    assert_eq!(boundaries.len(), 2);
    assert!(boundaries.contains(&registry.resume_nodes(scope(40)).unwrap()[0]));
    assert!(matches!(
        child.resume_scope_membership(scope(50)).unwrap(),
        PositionResumeScopeMembership::Inactive
    ));
    assert!(matches!(
        child.resume_scope_membership(scope(60)).unwrap(),
        PositionResumeScopeMembership::Inactive
    ));
    assert!(parent.member_roots.is_empty());
}

#[test]
fn same_scope_multiple_member_cuts_preserve_order_and_release_holes() {
    let members: Arc<[CoupledCohortEndpoint]> = vec![
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(20),
            expression: compiled(resume(Some(leaf(10.)), None, 40)),
        },
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(21),
            expression: compiled(resume(Some(leaf(20.)), Some(leaf(90.)), 40)),
        },
    ]
    .into();
    let registry = registry(forest(vec![PositionForestNode::SourceCohort(members)], 0));
    let child = registry
        .branch()
        .at_resume_operand(scope(40), PositionResumeEndpoint::Incoming)
        .unwrap()
        .unwrap();
    assert_eq!(child.member_roots.len(), 2);
    let bound = child.conditioned_with_origins().unwrap().remove(0);
    let output = lineage(&bound.sample);
    let PositionForestNode::SourceCohort(members) = &output.nodes[output.root] else {
        panic!()
    };
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].lane_id(), Uuid::from_u128(21));
    assert_eq!(member_value(&members[0]), angles(90.));
    assert_eq!(
        bound.origins.get(NodeLocation::Member(output.root, 0)),
        Some(NodeLocation::Member(0, 1))
    );
}

#[test]
fn absent_and_ambiguous_scope_do_not_mutate_original_branch() {
    let source = whole(resume(
        Some(resume(Some(leaf(10.)), Some(leaf(20.)), 40)),
        Some(leaf(90.)),
        40,
    ));
    let registry = registry(source);
    let parent = registry.branch();
    assert!(
        parent
            .at_resume_operand(scope(99), PositionResumeEndpoint::Incoming)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        parent.at_resume_operand(scope(40), PositionResumeEndpoint::Incoming),
        Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners
        ))
    ));
    assert!(parent.operand_boundary_nodes().unwrap().is_empty());
}

#[test]
fn inherited_member_cuts_keep_independent_sibling_boundary_and_strip_outer_suffix() {
    let members: Arc<[CoupledCohortEndpoint]> = vec![
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(20),
            expression: compiled(resume(
                Some(resume(Some(leaf(10.)), Some(leaf(90.)), 40)),
                Some(leaf(180.)),
                50,
            )),
        },
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(21),
            expression: compiled(resume(Some(leaf(200.)), Some(leaf(250.)), 60)),
        },
    ]
    .into();
    let registry = registry(forest(vec![PositionForestNode::SourceCohort(members)], 0));
    let parent = registry.branch();
    let sibling_cut = parent
        .at_resume_operand(scope(60), PositionResumeEndpoint::Incoming)
        .unwrap()
        .unwrap();
    let outer = sibling_cut
        .at_resume_operand(scope(50), PositionResumeEndpoint::Outgoing)
        .unwrap()
        .unwrap();
    let inner = outer
        .at_resume_operand(scope(40), PositionResumeEndpoint::Incoming)
        .unwrap()
        .unwrap();
    assert_eq!(inner.member_roots.len(), 2);
    assert!(matches!(
        inner.resume_scope_membership(scope(50)).unwrap(),
        PositionResumeScopeMembership::Inactive
    ));
    assert!(
        inner
            .at_resume_operand(scope(50), PositionResumeEndpoint::Outgoing)
            .is_err()
    );
    let boundaries = inner.operand_boundary_nodes().unwrap();
    assert_eq!(boundaries.len(), 3);
    assert!(boundaries.contains(&registry.resume_nodes(scope(40)).unwrap()[0]));
    assert!(boundaries.contains(&registry.resume_nodes(scope(60)).unwrap()[0]));
    assert!(!boundaries.contains(&registry.resume_nodes(scope(50)).unwrap()[0]));
    assert!(inner.operand_boundary_nodes().unwrap() != outer.operand_boundary_nodes().unwrap());
    let bound = inner.conditioned_with_origins().unwrap().remove(0);
    let output = lineage(&bound.sample);
    let PositionForestNode::SourceCohort(members) = &output.nodes[output.root] else {
        panic!("atomic cohort boundary")
    };
    assert_eq!(members.len(), 2);
    assert_eq!(member_value(&members[0]), angles(90.));
    assert_eq!(member_value(&members[1]), angles(250.));
    assert_eq!(members[0].lane_id(), Uuid::from_u128(20));
    assert_eq!(members[1].lane_id(), Uuid::from_u128(21));
    for member in output.cohort_members.iter() {
        let root = member.whole_tape.as_ref().unwrap().roots[0];
        assert!(
            matches!(bound.origins.get(NodeLocation::MemberTape(output.root, member.member_index, root)), Some(NodeLocation::MemberTape(0, original, _)) if original == member.member_index)
        );
    }
    assert!(parent.member_roots.is_empty());
    assert!(parent.operand_boundary_nodes().unwrap().is_empty());
    assert!(matches!(
        parent.resume_scope_membership(scope(50)).unwrap(),
        PositionResumeScopeMembership::Active(_)
    ));
    let original = lineage(&result(&parent));
    let PositionForestNode::SourceCohort(original_members) = &original.nodes[original.root] else {
        panic!()
    };
    let CoupledCohortEndpoint::WholeExpression { expression, .. } = &original_members[0] else {
        panic!()
    };
    assert!(
        matches!(expression.expression().shallow().unwrap(), DynamicSampleExpression::Transition { reason: DynamicTransitionReason::Resume { occurrence_id }, .. } if occurrence_id == scope(50).occurrence_id)
    );
}

#[test]
fn multiple_original_source_slots_require_an_explicit_prefix_recipe() {
    let a = whole(resume(Some(leaf(10.)), Some(leaf(90.)), 40));
    let b = whole(leaf(250.));
    let registry =
        CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &[a.clone(), b.clone()]).unwrap();
    let branch = registry.branch();
    assert!(matches!(
        branch.at_resume_operand(scope(40), PositionResumeEndpoint::Incoming),
        Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners
        ))
    ));
    assert!(branch.operand_boundary_nodes().unwrap().is_empty());
    let originals = branch.conditioned_sources().unwrap();
    for (original, result) in [&a, &b].into_iter().zip(originals.iter()) {
        let (
            FamilyCompositionSample::WholeExpression { expression: a, .. },
            Some(FamilyCompositionSample::WholeExpression { expression: b, .. }),
        ) = (original, result)
        else {
            panic!()
        };
        assert!(Arc::ptr_eq(a, b));
    }
}
