//! Versioned storage of pinned numeric Angle arithmetic; Current is never solved on save.
use super::*;
use crate::{AngleNumericNode, AngleNumericProgram, DynamicSourceDependency};

fn occurrence(value: u128) -> DynamicSourceOccurrenceId {
    DynamicSourceOccurrenceId::new(Uuid::from_u128(value)).unwrap()
}

fn numeric() -> Arc<DynamicSampleExpression> {
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    };
    Arc::new(DynamicSampleExpression::AngleNumeric {
        program: Arc::new(AngleNumericProgram {
            address: address.clone(),
            occurrence: Some(occurrence(901)),
            nodes: vec![
                AngleNumericNode::Current,
                AngleNumericNode::Materialized {
                    value: DynamicValue::Scalar(90.),
                    dependency_occurrence: Some(DynamicSourceDependency::compatible(
                        Some(occurrence(902)),
                        &address,
                    )),
                },
                AngleNumericNode::Transition {
                    from: 0,
                    to: 1,
                    progress: 0.375,
                },
                AngleNumericNode::ScaleFrom {
                    pivot: 0,
                    value: 2,
                    factor: 1.5,
                },
            ],
            root: 3,
            operations: Default::default(),
        }),
    })
}

#[test]
fn numeric_v3_history_roundtrip_retains_current_arithmetic_identity_and_sharing() {
    let leaf = numeric();
    leaf.validate().unwrap();
    let resume = Arc::new(DynamicSampleExpression::Transition {
        from: Some(leaf.clone()),
        to: Some(leaf.clone()),
        progress: 0.25,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(903),
        },
    });
    let mut tape = RetainedExpressionTape::from_roots(&[resume.clone(), leaf.clone()]).unwrap();
    assert_eq!(tape.version, 3);
    assert_eq!(tape.nodes.len(), 2);
    let dead = tape
        .append_resume(Some(tape.roots[0]), None, 0.5, Uuid::from_u128(904))
        .unwrap();
    assert_ne!(dead, tape.roots[0]);
    assert_eq!(tape.compact_reachable().unwrap(), 1);
    let checkpoint = serde_json::to_vec(&tape).unwrap();
    let restored: RetainedExpressionTape = serde_json::from_slice(&checkpoint).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, tape);
    let RetainedExpressionNode::AngleNumeric { program } = &restored.nodes[0] else {
        panic!("numeric history must remain a numeric leaf");
    };
    assert!(matches!(program.nodes[0], AngleNumericNode::Current));
    assert!(matches!(
        program.nodes[2],
        AngleNumericNode::Transition {
            progress: 0.375,
            ..
        }
    ));
    assert!(matches!(
        program.nodes[3],
        AngleNumericNode::ScaleFrom { factor: 1.5, .. }
    ));
    let retained = DynamicSampleExpression::Retained {
        root: restored.roots[0],
        tape: Arc::new(restored),
    };
    assert_eq!(retained, *resume);
    assert!(retained.contains_angles());
    assert!(retained.has_angle_numeric());
    assert_eq!(
        retained.required_programming_contract(),
        PROGRAMMING_CONTRACT_VERSION
    );
    assert!(!retained.visit_legacy_contributions(|_, _, _| panic!("numeric Angle is not legacy")));
    let mut ids = Vec::new();
    retained
        .visit_source_occurrences(&mut |id| ids.push(id))
        .unwrap();
    assert_eq!(ids, vec![occurrence(901), occurrence(902)]);
    let mut values = Vec::new();
    retained
        .visit_programming_values(&mut |address, value| {
            values.push((address.clone(), value.clone()));
            Ok(())
        })
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].1, DynamicValue::Scalar(90.));
    assert_eq!(values[0].0.component, Some(ProgrammingComponent::Pan));
    let shallow = retained.shallow().unwrap();
    let DynamicSampleExpression::Transition {
        from: Some(from),
        to: Some(to),
        ..
    } = shallow
    else {
        panic!("resume remains a shallow history node");
    };
    assert_eq!(from, to);
    assert_eq!(*from, *leaf);
}

#[test]
fn numeric_fresh_binding_keeps_held_occurrence_and_materialized_dependencies() {
    let held = numeric();
    let mut fresh = held.as_ref().clone();
    fresh.bind_fresh_authored_occurrence(Some(occurrence(905)));
    let mut held_ids = Vec::new();
    let mut fresh_ids = Vec::new();
    held.visit_source_occurrences(&mut |id| held_ids.push(id))
        .unwrap();
    fresh
        .visit_source_occurrences(&mut |id| fresh_ids.push(id))
        .unwrap();
    assert_eq!(held_ids, vec![occurrence(901), occurrence(902)]);
    assert_eq!(fresh_ids, vec![occurrence(905), occurrence(902)]);
    assert_ne!(fresh, *held);
    assert!(fresh.has_source_occurrences());
    let DynamicSampleExpression::AngleNumeric { program } = &mut fresh else {
        panic!()
    };
    Arc::make_mut(program).occurrence = None;
    assert!(
        fresh.has_source_occurrences(),
        "dependency evidence survives absent authorship"
    );
}

#[test]
fn numeric_nodes_require_v3_and_invalid_program_graphs_reject_on_checkpoint_validation() {
    let tape = RetainedExpressionTape::from_roots(&[numeric()]).unwrap();
    for version in [0, 1, 2, 4, u16::MAX] {
        let mut invalid = tape.clone();
        invalid.version = version;
        let restored: RetainedExpressionTape =
            serde_json::from_slice(&serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert!(
            restored.validate().is_err(),
            "numeric tape accepted version {version}"
        );
    }
    let mut invalid = tape.clone();
    let RetainedExpressionNode::AngleNumeric { program } = &mut invalid.nodes[0] else {
        panic!()
    };
    Arc::make_mut(program).root = 99;
    let restored: RetainedExpressionTape =
        serde_json::from_slice(&serde_json::to_vec(&invalid).unwrap()).unwrap();
    assert!(restored.validate().is_err());
    let mut invalid = tape;
    let RetainedExpressionNode::AngleNumeric { program } = &mut invalid.nodes[0] else {
        panic!()
    };
    let program = Arc::make_mut(program);
    program.nodes[2] = AngleNumericNode::Transition {
        from: 2,
        to: 1,
        progress: 0.5,
    };
    assert!(
        invalid.validate().is_err(),
        "self-referencing numeric operation cannot restore"
    );
}

#[test]
fn valid_v1_and_v2_checkpoints_restore_without_rewriting_original_sources() {
    // Representative old writer shape, including version-2 source/dependency evidence.
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Tilt),
    };
    for version in [1, 2] {
        let authored = (version == 2).then(|| occurrence(906));
        let dependency = (version == 2)
            .then(|| DynamicSourceDependency::compatible(Some(occurrence(907)), &address));
        let checkpoint = RetainedExpressionTape {
            version,
            nodes: vec![
                RetainedExpressionNode::AngleCurrent {
                    address: address.clone(),
                },
                RetainedExpressionNode::Programming {
                    address: address.clone(),
                    value: DynamicValue::Scalar(-25.),
                    occurrence: authored,
                    dependency_occurrence: dependency,
                },
                RetainedExpressionNode::Transition {
                    from: Some(RetainedNodeId(0)),
                    to: Some(RetainedNodeId(1)),
                    progress: 0.5,
                    reason: DynamicTransitionReason::Resume {
                        occurrence_id: Uuid::from_u128(908),
                    },
                },
            ],
            roots: vec![RetainedNodeId(2)],
            ..RetainedExpressionTape::empty()
        };
        let bytes = serde_json::to_vec(&checkpoint).unwrap();
        let restored: RetainedExpressionTape = serde_json::from_slice(&bytes).unwrap();
        restored.validate().unwrap();
        assert_eq!(
            restored, checkpoint,
            "version {version} keeps its original nodes"
        );
        assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
        let original = Arc::new(DynamicSampleExpression::Retained {
            root: restored.roots[0],
            tape: Arc::new(restored),
        });
        let imported = RetainedExpressionTape::from_roots(&[original.clone()]).unwrap();
        assert_eq!(imported.version, 3, "new writers use version 3");
        let imported = DynamicSampleExpression::Retained {
            root: imported.roots[0],
            tape: Arc::new(imported),
        };
        assert_eq!(
            imported, *original,
            "migration must not solve Current or rewrite endpoints"
        );
        let mut ids = Vec::new();
        imported
            .visit_source_occurrences(&mut |id| ids.push(id))
            .unwrap();
        assert_eq!(
            ids,
            if version == 2 {
                vec![occurrence(906), occurrence(907)]
            } else {
                vec![]
            }
        );
    }
}

/// TL-639 round 5: distinct live leaf roots import without the walk's maps, into exactly the
/// tape (and source keys) the walk builds; anything else takes the walk.
#[test]
fn distinct_leaf_roots_import_exactly_as_the_walk_does() {
    let pan = Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        }),
        value: DynamicValue::Scalar(12.5),
        occurrence: Some(occurrence(7)),
        dependency_occurrence: None,
    });
    let current = Arc::new(DynamicSampleExpression::AngleCurrent {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Tilt),
        }),
    });
    let wrapped = Arc::new(DynamicSampleExpression::Operation {
        origin: None,
        value: numeric(),
    });
    for roots in [
        vec![pan.clone()],
        vec![numeric()],
        vec![pan.clone(), current.clone()],
        vec![numeric(), current.clone(), wrapped.clone()],
    ] {
        let mut fast_keys = Vec::new();
        let fast = RetainedExpressionTape::import_leaf_roots(&roots, Some(&mut fast_keys))
            .expect("distinct leaf roots take the direct import")
            .unwrap();
        let mut walk_keys = Vec::new();
        let walk = RetainedExpressionTape::import_roots_walk(&roots, Some(&mut walk_keys)).unwrap();
        assert_eq!(fast, walk);
        assert_eq!(fast_keys, walk_keys);
    }
    // A repeated root, a root with children and a retained root all take the walk.
    let held = Arc::new(DynamicSampleExpression::Retained {
        tape: Arc::new(RetainedExpressionTape::from_roots(std::slice::from_ref(&current)).unwrap()),
        root: RetainedNodeId(0),
    });
    let transition = Arc::new(DynamicSampleExpression::Transition {
        from: Some(pan.clone()),
        to: None,
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(5),
        },
    });
    for roots in [vec![pan.clone(), pan.clone()], vec![transition], vec![held]] {
        assert!(RetainedExpressionTape::import_leaf_roots(&roots, None).is_none());
    }
}
