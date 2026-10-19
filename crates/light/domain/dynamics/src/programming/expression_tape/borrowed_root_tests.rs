//! Borrowed and owning entry points preserve the complete retained graph contract.
use super::*;

fn leaf(value: f32) -> DynamicSampleExpression {
    DynamicSampleExpression::LegacyScalar {
        attribute: AttributeKey::intensity(),
        value,
        occurrence: Some(DynamicSourceOccurrenceId::new(Uuid::from_u128(71)).unwrap()),
        dependency_occurrence: None,
    }
}
fn compare(roots: &[Arc<DynamicSampleExpression>]) -> Result<RetainedExpressionTape, String> {
    let borrowed = roots.iter().map(Arc::as_ref).collect::<Vec<_>>();
    let old = RetainedExpressionTape::from_roots(roots).map_err(|e| e.to_string());
    let new = RetainedExpressionTape::from_borrowed_roots(&borrowed).map_err(|e| e.to_string());
    assert_eq!(new, old);
    if let (Ok(new), Ok(old)) = (&new, &old) {
        assert_eq!(
            serde_json::to_value(new).unwrap(),
            serde_json::to_value(old).unwrap()
        );
        for (a, b) in new
            .operation_emissions()
            .iter()
            .zip(old.operation_emissions())
        {
            assert!(Arc::ptr_eq(a, b));
        }
    }
    new
}
#[test]
fn borrowed_roots_keep_distinct_equal_and_repeated_root_identity_at_leaf_boundary() {
    for count in [0, 1, 8, 9, 64] {
        let roots = (0..count).map(|_| Arc::new(leaf(0.5))).collect::<Vec<_>>();
        let tape = compare(&roots).unwrap();
        assert_eq!(tape.nodes.len(), count);
        assert_eq!(
            tape.roots.iter().map(|id| id.0).collect::<Vec<_>>(),
            (0..count as u32).collect::<Vec<_>>()
        );
    }
    let root = Arc::new(leaf(0.5));
    let tape = compare(&[root.clone(), root]).unwrap();
    assert_eq!(tape.nodes.len(), 1);
    assert_eq!(tape.roots[0], tape.roots[1]);
}
#[test]
fn borrowed_roots_keep_shared_descendants_retained_tapes_and_transparent_wrappers() {
    let shared = Arc::new(leaf(0.4));
    let branch = Arc::new(DynamicSampleExpression::Transition {
        from: Some(shared.clone()),
        to: Some(shared.clone()),
        progress: 0.3,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(72),
        },
    });
    let source = Arc::new(compare(&[branch.clone(), shared.clone()]).unwrap());
    let held = Arc::new(DynamicSampleExpression::Retained {
        tape: source.clone(),
        root: source.roots[0],
    });
    let wrapper = Arc::new(DynamicSampleExpression::Operation {
        origin: None,
        value: held.clone(),
    });
    let tape = compare(&[wrapper, held.clone(), branch, held, shared]).unwrap();
    assert_eq!(tape.roots[1], tape.roots[3]);
    tape.validate().unwrap();
}
#[test]
fn borrowed_roots_keep_first_validation_errors_and_recover_after_failure() {
    assert!(compare(&[Arc::new(leaf(f32::NAN)), Arc::new(leaf(f32::INFINITY))]).is_err());
    let mut tape = RetainedExpressionTape::empty();
    tape.nodes.push(RetainedExpressionNode::Transition {
        from: Some(RetainedNodeId(0)),
        to: None,
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(73),
        },
    });
    tape.roots.push(RetainedNodeId(0));
    assert!(
        compare(&[Arc::new(DynamicSampleExpression::Retained {
            tape: Arc::new(tape),
            root: RetainedNodeId(0)
        })])
        .is_err()
    );
    let mut tape = RetainedExpressionTape::empty();
    tape.roots.push(RetainedNodeId(999));
    assert!(
        compare(&[Arc::new(DynamicSampleExpression::Retained {
            tape: Arc::new(tape),
            root: RetainedNodeId(999)
        })])
        .is_err()
    );
    compare(&[Arc::new(leaf(0.7))]).unwrap();
}
