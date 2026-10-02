use super::*;
use light_core::{NativeColorIdentity, NativeColorValue, PhysicalDataQuality};
use std::sync::Arc;

fn current() -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::AngleCurrent {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Tilt),
        }),
    })
}

fn whole_current() -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::AngleCurrent {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: None,
        }),
    })
}

#[test]
fn mixed_live_and_retained_roots_share_imported_history_without_expansion() {
    let source = Arc::new(
        RetainedExpressionTape::from_roots(&[current(), target(Uuid::from_u128(91)), direct()])
            .unwrap(),
    );
    let held = Arc::new(DynamicSampleExpression::Retained {
        tape: source.clone(),
        root: source.roots[0],
    });
    let live = Arc::new(DynamicSampleExpression::Transition {
        from: Some(held.clone()),
        to: Some(held.clone()),
        progress: 0.25,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(8),
        },
    });
    let imported =
        RetainedExpressionTape::from_roots(&[live.clone(), held.clone(), live.clone()]).unwrap();
    assert_eq!(
        imported.nodes.len(),
        2,
        "unused original roots are not copied"
    );
    assert_eq!(imported.roots[0], imported.roots[2]);
    let node = imported.node(imported.roots[0]).unwrap();
    assert!(
        matches!(node, RetainedExpressionNode::Transition { from: Some(a), to: Some(b), .. }
        if *a == imported.roots[1] && a == b)
    );
    let restored = DynamicSampleExpression::Retained {
        root: imported.roots[0],
        tape: Arc::new(imported),
    };
    assert_eq!(restored, *live);
    restored.validate().unwrap();
    assert!(restored.contains_angles());
    assert_eq!(restored.required_programming_contract(), 1);
}

#[test]
fn shared_legacy_dag_accumulates_weights_once_without_recursive_path_expansion() {
    let leaf = Arc::new(DynamicSampleExpression::LegacyScalar {
        attribute: light_core::AttributeKey::intensity(),
        value: 0.4,
        occurrence: None,
        dependency_occurrence: None,
    });
    let mut tape = RetainedExpressionTape::from_roots(&[leaf]).unwrap();
    let mut root = tape.roots[0];
    for index in 0..160 {
        root = tape
            .append_resume(Some(root), Some(root), 0.5, Uuid::from_u128(1000 + index))
            .unwrap();
    }
    root = tape
        .append_resume(Some(root), None, 0.25, Uuid::from_u128(999))
        .unwrap();
    tape.roots = vec![root];
    let expression = DynamicSampleExpression::Retained {
        tape: Arc::new(tape),
        root,
    };
    let mut contributions = Vec::new();
    assert!(
        expression.visit_legacy_contributions(|key, value, weight| contributions.push((
            key.clone(),
            value,
            weight
        )))
    );
    assert_eq!(
        contributions,
        vec![(light_core::AttributeKey::intensity(), 0.4, 0.75)]
    );
    assert_eq!(expression.required_programming_contract(), 0);
}

#[test]
fn whole_angle_current_and_size_import_with_the_same_contract_as_expression() {
    let whole = whole_current();
    whole.validate().unwrap();
    let base = light_core::AttributeValue::Position(Arc::new(PositionIntent::angles(30.0, -10.0)));
    let size = Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &base).unwrap(),
        ),
        base: DynamicValue::Family(base),
        value: whole.clone(),
        factor: 1.5,
        baseline_occurrence: None,
    });
    size.validate().unwrap();
    let tape = RetainedExpressionTape::from_roots(&[whole.clone(), size]).unwrap();
    assert_eq!(tape.nodes.len(), 2);
    assert!(matches!(tape.node(tape.roots[0]),
        Some(RetainedExpressionNode::AngleCurrent { address }) if address.component.is_none()));
    tape.validate().unwrap();
    let restored: RetainedExpressionTape =
        serde_json::from_slice(&serde_json::to_vec(&tape).unwrap()).unwrap();
    assert_eq!(restored, tape);
}

fn target(point_id: Uuid) -> Arc<DynamicSampleExpression> {
    let value = light_core::AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point { point_id },
        [1.0, -2.0, 3.0],
    )));
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn direct() -> Arc<DynamicSampleExpression> {
    let source = NativeColorIdentity {
        profile_id: Uuid::from_u128(1),
        profile_revision: 4,
        profile_digest: "original-profile".into(),
        mode_id: Uuid::from_u128(2),
        head_id: Uuid::from_u128(3),
        path_id: Uuid::from_u128(4),
        model_revision: 7,
        native_layout_signature: "native-u32".into(),
    };
    let value = light_core::AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: source.clone(),
            channels: vec![NativeColorValue {
                channel_id: Uuid::from_u128(5),
                function_id: Uuid::from_u128(6),
                raw: u32::MAX - 1,
            }],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: source.model_revision,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec![],
        },
    }));
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Color, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}

#[test]
fn repeated_live_histories_are_flat_shared_and_roundtrip_without_freezing_sources() {
    let point_id = Uuid::from_u128(91);
    let angle = current();
    let target = target(point_id);
    let direct = direct();
    let mut tape =
        RetainedExpressionTape::from_roots(&[angle.clone(), angle, target, direct]).unwrap();
    assert_eq!(tape.nodes.len(), 3);
    assert_eq!(tape.roots[0], tape.roots[1]);
    let original = tape.roots.clone();
    for iteration in 0..120_u128 {
        for (root_index, original_index) in [(0, 1), (2, 2), (3, 3)] {
            let root = tape
                .append_resume(
                    Some(tape.roots[root_index]),
                    Some(original[original_index]),
                    0.375,
                    Uuid::from_u128(10_000 + iteration * 10 + root_index as u128),
                )
                .unwrap();
            tape.replace_root(root_index, root).unwrap();
        }
    }
    assert_eq!(tape.nodes.len(), 363);
    tape.validate().unwrap();
    let serialized = serde_json::to_vec(&tape).unwrap();
    let restored: RetainedExpressionTape = serde_json::from_slice(&serialized).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, tape);
    assert_eq!(restored.roots[1], original[1]);
    let mut seen = [false; 3];
    restored
        .visit_reachable(&restored.roots, |_, node| {
            match node {
                RetainedExpressionNode::AngleCurrent { address } => {
                    assert_eq!(address.component, Some(ProgrammingComponent::Tilt));
                    seen[0] = true;
                }
                RetainedExpressionNode::Programming {
                    value: DynamicValue::Family(light_core::AttributeValue::Position(position)),
                    ..
                } if matches!(position.as_ref(), PositionIntent::Target { .. }) => {
                    assert_eq!(position.referenced_point(), Some(point_id));
                    seen[1] = true;
                }
                RetainedExpressionNode::Programming {
                    value: DynamicValue::Family(light_core::AttributeValue::ColorProgram(color)),
                    ..
                } => {
                    let ColorProgram::Direct { recipe, .. } = color.as_ref() else {
                        panic!()
                    };
                    assert_eq!(recipe.channels[0].raw, u32::MAX - 1);
                    assert_eq!(recipe.source.profile_revision, 4);
                    seen[2] = true;
                }
                _ => {}
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(seen, [true; 3]);
}

#[test]
fn historical_arc_chain_beyond_old_depth_limit_imports_iteratively() {
    let leaf = current();
    let mut expression = leaf.clone();
    for iteration in 0..130_u128 {
        expression = Arc::new(DynamicSampleExpression::Transition {
            from: Some(expression),
            to: Some(leaf.clone()),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(100 + iteration),
            },
        });
    }
    // Both import and cold expression validation now traverse iteratively.
    expression.validate().unwrap();
    let tape = RetainedExpressionTape::from_roots(&[expression]).unwrap();
    assert_eq!(tape.nodes.len(), 131);
    tape.validate().unwrap();
    assert!(
        serde_json::to_string(&tape)
            .unwrap()
            .contains("angle_current")
    );
}

#[test]
fn compaction_keeps_order_and_sharing_without_rewriting_live_leaves() {
    let angle = current();
    let point = target(Uuid::from_u128(99));
    let mut tape = RetainedExpressionTape::from_roots(&[angle.clone(), angle, point]).unwrap();
    let old_roots = tape.roots.clone();
    let dead = tape
        .append_resume(
            Some(old_roots[0]),
            Some(old_roots[2]),
            0.5,
            Uuid::from_u128(777),
        )
        .unwrap();
    tape.replace_root(0, dead).unwrap();
    assert_eq!(tape.compact_reachable().unwrap(), 0);
    tape.replace_root(0, old_roots[0]).unwrap();
    assert_eq!(tape.compact_reachable().unwrap(), 1);
    assert_eq!(tape.nodes.len(), 2);
    assert_eq!(tape.roots[0], tape.roots[1]);
    assert_ne!(tape.roots[0], tape.roots[2]);
}

#[test]
fn malformed_references_versions_and_local_values_are_rejected() {
    let mut tape = RetainedExpressionTape::from_roots(&[current()]).unwrap();
    tape.version += 1;
    assert!(tape.validate().is_err());
    tape.version = RETAINED_EXPRESSION_TAPE_VERSION;
    tape.roots[0] = RetainedNodeId(99);
    assert!(tape.validate().is_err());
    tape.roots[0] = RetainedNodeId(0);
    tape.nodes.push(RetainedExpressionNode::Transition {
        from: Some(RetainedNodeId(1)),
        to: None,
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(1),
        },
    });
    assert!(tape.validate().is_err());
    if let RetainedExpressionNode::Transition { from, .. } = &mut tape.nodes[1] {
        *from = Some(RetainedNodeId(0));
    }
    assert!(tape.validate().is_ok());
    if let RetainedExpressionNode::Transition { reason, .. } = &mut tape.nodes[1] {
        *reason = DynamicTransitionReason::Resume {
            occurrence_id: Uuid::nil(),
        };
    }
    assert!(tape.validate().is_err());

    let invalid = RetainedExpressionTape {
        version: RETAINED_EXPRESSION_TAPE_VERSION,
        nodes: vec![RetainedExpressionNode::Programming {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Focus,
                component: Some(ProgrammingComponent::Focus),
            },
            value: DynamicValue::Scalar(2.0),
            occurrence: None,
            dependency_occurrence: None,
        }],
        roots: vec![RetainedNodeId(0)],
        ..RetainedExpressionTape::empty()
    };
    assert!(invalid.validate().is_err());
    assert!(
        RetainedExpressionTape::empty()
            .append_resume(None, None, 0.5, Uuid::from_u128(1))
            .is_err()
    );
}

#[test]
fn import_preserves_mixed_owner_resume_and_rejects_invalid_size_address() {
    let mixed = Arc::new(DynamicSampleExpression::Transition {
        from: Some(target(Uuid::from_u128(9))),
        to: Some(direct()),
        progress: 0.25,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(8),
        },
    });
    let tape = RetainedExpressionTape::from_roots(&[mixed]).unwrap();
    assert_eq!(tape.nodes.len(), 3);
    assert!(matches!(
        tape.node(tape.roots[0]),
        Some(RetainedExpressionNode::Transition { .. })
    ));

    let bad = Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: None,
        }),
        base: DynamicValue::Family(light_core::AttributeValue::Position(Arc::new(
            PositionIntent::angles(0.0, 0.0),
        ))),
        value: direct(),
        factor: 2.0,
        baseline_occurrence: None,
    });
    assert!(RetainedExpressionTape::from_roots(&[bad]).is_err());
}

#[test]
fn source_occurrences_survive_flat_history_compaction_and_v1_unknown_restore() {
    let first = DynamicSourceOccurrenceId::new(Uuid::from_u128(701)).unwrap();
    let second = DynamicSourceOccurrenceId::new(Uuid::from_u128(702)).unwrap();
    let static_base = DynamicSourceOccurrenceId::new(Uuid::from_u128(703)).unwrap();
    let value = light_core::AttributeValue::Position(Arc::new(PositionIntent::angles(12.0, 34.0)));
    let address =
        Arc::new(DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap());
    let leaf = |occurrence| {
        Arc::new(DynamicSampleExpression::Programming {
            address: address.clone(),
            value: DynamicValue::Family(value.clone()),
            occurrence: Some(occurrence),
            dependency_occurrence: None,
        })
    };
    let from = leaf(first);
    let to = leaf(second);
    assert_ne!(
        from, to,
        "equal values from distinct assignments must not share identity"
    );
    let size = Arc::new(DynamicSampleExpression::Scale {
        address,
        base: DynamicValue::Family(value),
        value: to,
        factor: 0.5,
        baseline_occurrence: Some(static_base),
    });
    let old = Arc::new(DynamicSampleExpression::Transition {
        from: Some(from),
        to: Some(size),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(704),
        },
    });
    let mut tape = RetainedExpressionTape::from_roots(&[old]).unwrap();
    tape.roots.push(tape.roots[0]);
    let unreachable = tape
        .append_resume(Some(tape.roots[0]), None, 0.2, Uuid::from_u128(705))
        .unwrap();
    assert_ne!(unreachable, tape.roots[0]);
    assert_eq!(tape.compact_reachable().unwrap(), 1);
    let restored: RetainedExpressionTape =
        serde_json::from_slice(&serde_json::to_vec(&tape).unwrap()).unwrap();
    let expression = DynamicSampleExpression::Retained {
        tape: Arc::new(restored),
        root: tape.roots[0],
    };
    let mut ids = Vec::new();
    expression
        .visit_source_occurrences(&mut |id| ids.push(id))
        .unwrap();
    ids.sort_unstable();
    assert_eq!(ids, vec![first, second, static_base]);

    let mut legacy = tape.clone();
    legacy.version = 1;
    for node in &mut legacy.nodes {
        match node {
            RetainedExpressionNode::Programming {
                occurrence,
                dependency_occurrence,
                ..
            } => {
                *occurrence = None;
                *dependency_occurrence = None;
            }
            RetainedExpressionNode::Scale {
                baseline_occurrence,
                ..
            } => *baseline_occurrence = None,
            _ => {}
        }
    }
    legacy.validate().unwrap();
    let migrated =
        RetainedExpressionTape::from_roots(&[Arc::new(DynamicSampleExpression::Retained {
            root: legacy.roots[0],
            tape: Arc::new(legacy),
        })])
        .unwrap();
    assert_eq!(migrated.version, RETAINED_EXPRESSION_TAPE_VERSION);
    let unknown = DynamicSampleExpression::Retained {
        root: migrated.roots[0],
        tape: Arc::new(migrated),
    };
    let mut ids = Vec::new();
    unknown
        .visit_source_occurrences(&mut |id| ids.push(id))
        .unwrap();
    assert!(ids.is_empty());
}

#[test]
fn occurrence_id_rejects_nil_even_during_json_restore() {
    assert!(DynamicSourceOccurrenceId::new(Uuid::nil()).is_err());
    assert!(
        serde_json::from_str::<DynamicSourceOccurrenceId>(
            "\"00000000-0000-0000-0000-000000000000\""
        )
        .is_err()
    );
}
