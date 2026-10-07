//! Opaque batch keys and branch-aware scope membership retain original registry authority.
use super::*;
use crate::CoupledCohortEndpoint;
use std::collections::hash_map::DefaultHasher;

fn angles(pan: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, 20.)))
}
fn rank(lane: u128) -> FamilySampleRank {
    FamilySampleRank {
        priority: 10,
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
            occurrence_id: Uuid::from_u128(id),
        },
    })
}
fn compiled(root: Arc<DynamicSampleExpression>) -> Arc<CompiledProgrammingFamilyExpression> {
    Arc::new(
        CompiledProgrammingFamilyExpression::new(root, ProgrammingOwner::Position, None, None)
            .unwrap(),
    )
}
fn source(root: Arc<DynamicSampleExpression>, lane: u128) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: compiled(root),
        rank: rank(lane),
        activation_mix: 1.,
    }
}
fn hash(node: &PositionSourceNode) -> u64 {
    let mut state = DefaultHasher::new();
    node.hash(&mut state);
    state.finish()
}

#[test]
fn opaque_node_keys_and_branch_validation_require_the_exact_original_registry() {
    let capture = Uuid::new_v4();
    let samples = [source(leaf(10.), 3), source(leaf(10.), 4)];
    let registry = CapturedPositionProgram::new(capture, &angles(0.), &samples).unwrap();
    let foreign = CapturedPositionProgram::new(capture, &angles(0.), &samples).unwrap();
    let root = registry.source_root(0).unwrap().unwrap();
    let cloned = registry.clone().source_root(0).unwrap().unwrap();
    let other_slot = registry.source_root(1).unwrap().unwrap();
    let foreign_node = foreign.source_root(0).unwrap().unwrap();
    assert!(root == cloned);
    assert_eq!(hash(&root), hash(&cloned));
    let mut keys = HashSet::new();
    keys.insert(root);
    keys.insert(cloned);
    keys.insert(other_slot);
    keys.insert(foreign_node);
    assert_eq!(
        keys.len(),
        3,
        "equal values and capture UUIDs cannot alias original node keys"
    );
    registry
        .validate_branch(&registry.clone().branch())
        .unwrap();
    assert!(registry.validate_branch(&foreign.branch()).is_err());
    assert!(foreign.validate_branch(&registry.branch()).is_err());
}

#[test]
fn membership_distinguishes_absence_inactive_ancestry_and_released_source_slots() {
    let nested = resume(Some(leaf(30.)), Some(leaf(90.)), 41);
    let samples = [
        source(resume(Some(leaf(10.)), Some(nested), 40), 3),
        source(resume(Some(leaf(20.)), Some(leaf(80.)), 42), 4),
    ];
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &samples).unwrap();
    let mut branch = registry.branch();
    assert!(matches!(
        branch.resume_scope_membership(scope(999)).unwrap(),
        PositionResumeScopeMembership::Absent
    ));
    assert!(
        matches!(branch.resume_scope_membership(scope(41)).unwrap(), PositionResumeScopeMembership::Active(nodes) if nodes.len() == 1)
    );
    branch
        .choose_resume(scope(40), PositionResumeEndpoint::Outgoing)
        .unwrap();
    assert!(matches!(
        branch.resume_scope_membership(scope(41)).unwrap(),
        PositionResumeScopeMembership::Inactive
    ));
    assert!(
        branch
            .choose_resume(scope(41), PositionResumeEndpoint::Incoming)
            .is_err()
    );
    branch.replace_source(1, None).unwrap();
    assert!(matches!(
        branch.resume_scope_membership(scope(42)).unwrap(),
        PositionResumeScopeMembership::Inactive
    ));
    let conditioned = branch.conditioned_sources().unwrap();
    assert_eq!(conditioned.len(), 2);
    assert!(conditioned[0].is_some());
    assert!(conditioned[1].is_none());
    assert_eq!(
        registry.resume_nodes(scope(42)).unwrap()[0].source_index(),
        1
    );
}

#[test]
fn selected_subtree_membership_cannot_witness_an_occurrence_elsewhere_in_the_slot() {
    let samples = [source(
        resume(
            Some(leaf(10.)),
            Some(resume(Some(leaf(30.)), Some(leaf(90.)), 41)),
            40,
        ),
        3,
    )];
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &samples).unwrap();
    let PositionSourceNodeView::Transition {
        from: Some(outgoing),
        ..
    } = registry
        .node_view(&registry.source_root(0).unwrap().unwrap())
        .unwrap()
    else {
        panic!()
    };
    let mut branch = registry.branch();
    branch.replace_source(0, Some(&outgoing)).unwrap();
    assert!(matches!(
        branch.resume_scope_membership(scope(40)).unwrap(),
        PositionResumeScopeMembership::Inactive
    ));
    assert!(matches!(
        branch.resume_scope_membership(scope(41)).unwrap(),
        PositionResumeScopeMembership::Inactive
    ));
}

#[test]
fn nested_member_release_keeps_surviving_member_identity_and_active_scope() {
    let members: Arc<[CoupledCohortEndpoint]> = vec![
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(101),
            expression: compiled(resume(
                Some(resume(Some(leaf(10.)), Some(leaf(20.)), 42)),
                None,
                40,
            )),
        },
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(102),
            expression: compiled(resume(Some(leaf(30.)), Some(leaf(90.)), 41)),
        },
    ]
    .into();
    let sample = FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::from_position_forest(
                Arc::from([]),
                &[PositionForestNode::SourceCohort(members)],
                0,
            )
            .unwrap(),
        ),
        rank: rank(999),
        activation_mix: 1.,
    };
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(0.), &[sample]).unwrap();
    let surviving = registry.resume_nodes(scope(41)).unwrap().remove(0);
    assert_eq!(
        registry
            .node_source_rank(&surviving)
            .unwrap()
            .dynamic_identity()
            .unwrap()
            .lane_id,
        Uuid::from_u128(102)
    );
    let mut branch = registry.branch();
    branch
        .choose_resume(scope(40), PositionResumeEndpoint::Incoming)
        .unwrap();
    assert!(matches!(
        branch.resume_scope_membership(scope(42)).unwrap(),
        PositionResumeScopeMembership::Inactive
    ));
    let PositionResumeScopeMembership::Active(nodes) =
        branch.resume_scope_membership(scope(41)).unwrap()
    else {
        panic!("surviving member scope")
    };
    assert!(nodes == vec![surviving]);
    let sources = branch.conditioned_sources().unwrap();
    let FamilyCompositionSample::CoupledExpression { expression, .. } =
        sources[0].as_ref().unwrap()
    else {
        panic!()
    };
    let lineage = expression.position_forest_lineage().unwrap();
    let PositionForestNode::SourceCohort(members) = &lineage.nodes[lineage.root] else {
        panic!()
    };
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].lane_id(), Uuid::from_u128(102));
}

#[test]
fn nil_scope_identities_are_rejected_without_changing_the_branch() {
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0.),
        &[source(resume(Some(leaf(10.)), Some(leaf(90.)), 40), 3)],
    )
    .unwrap();
    let branch = registry.branch();
    for invalid in [
        PositionResumeScope {
            instance_id: Uuid::nil(),
            ..scope(40)
        },
        PositionResumeScope {
            controller_id: Uuid::nil(),
            ..scope(40)
        },
        PositionResumeScope {
            occurrence_id: Uuid::nil(),
            ..scope(40)
        },
    ] {
        assert!(branch.resume_scope_membership(invalid).is_err());
    }
    assert!(matches!(
        branch.resume_scope_membership(scope(40)).unwrap(),
        PositionResumeScopeMembership::Active(_)
    ));
}
