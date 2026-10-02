use super::*;
use crate::DynamicValueAddress;
use uuid::Uuid;

fn angles(pan: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, 20.)))
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
fn compile(
    root: Arc<DynamicSampleExpression>,
) -> (RetainedExpressionTape, CompiledProgrammingFamilyExpression) {
    let tape = RetainedExpressionTape::from_roots(std::slice::from_ref(&root)).unwrap();
    let compiled =
        CompiledProgrammingFamilyExpression::new(root, ProgrammingOwner::Position, None, None)
            .unwrap();
    (tape, compiled)
}

#[test]
fn actual_whole_transition_has_the_original_untruncated_tape_origin() {
    let (tape, compiled) = compile(transition(
        Some(leaf(angles(10.))),
        Some(leaf(angles(90.))),
        0.5,
    ));
    assert_eq!(
        compiled.retained_node_origin(compiled.graph.root),
        Some(tape.roots[0])
    );
    assert_eq!(compiled.retained_node_origin(0), None);
    assert_eq!(compiled.retained_node_origin(usize::MAX), None);
    assert_eq!(
        compiled.graph.retained_node_origins.len(),
        compiled.graph.nodes.len()
    );
}

#[test]
fn exact_and_size_one_aliases_keep_the_selected_child_origin() {
    let selected = leaf(angles(90.));
    let exact = transition(Some(leaf(angles(10.))), Some(selected), 1.);
    let baseline = angles(30.);
    let root = Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &baseline).unwrap(),
        ),
        base: DynamicValue::Family(baseline),
        value: exact,
        factor: 1.,
        baseline_occurrence: None,
    });
    let (tape, compiled) = compile(root);
    let RetainedExpressionNode::Scale { value: exact, .. } = &tape.nodes[tape.roots[0].0 as usize]
    else {
        panic!()
    };
    let RetainedExpressionNode::Transition {
        from: Some(unused),
        to: Some(selected),
        ..
    } = &tape.nodes[exact.0 as usize]
    else {
        panic!()
    };
    assert_eq!(
        compiled.retained_node_origin(compiled.graph.root),
        Some(*selected)
    );
    for inactive_or_alias in [*unused, *exact, tape.roots[0]] {
        assert!(
            !compiled
                .graph
                .retained_node_origins
                .contains(&Some(inactive_or_alias))
        );
    }
}

#[test]
fn zero_size_baseline_origin_is_scale_and_does_not_revive_its_child() {
    let baseline = angles(30.);
    let root = Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &baseline).unwrap(),
        ),
        base: DynamicValue::Family(baseline),
        value: leaf(angles(180.)),
        factor: 0.,
        baseline_occurrence: None,
    });
    let (tape, compiled) = compile(root);
    let RetainedExpressionNode::Scale { value: unused, .. } = &tape.nodes[tape.roots[0].0 as usize]
    else {
        panic!()
    };
    assert_eq!(
        compiled.retained_node_origin(compiled.graph.root),
        Some(tape.roots[0])
    );
    assert!(
        !compiled
            .graph
            .retained_node_origins
            .contains(&Some(*unused))
    );
    assert_eq!(compiled.graph.nodes.len(), 2);
}

#[test]
fn exact_release_alias_to_underlay_has_no_original_retained_node() {
    let (tape, compiled) = compile(transition(Some(leaf(angles(10.))), None, 1.));
    assert_eq!(compiled.graph.root, 0);
    assert_eq!(compiled.retained_node_origin(compiled.graph.root), None);
    assert!(
        !compiled
            .graph
            .retained_node_origins
            .contains(&Some(tape.roots[0]))
    );
    assert_eq!(compiled.graph.nodes.len(), 1);
}
