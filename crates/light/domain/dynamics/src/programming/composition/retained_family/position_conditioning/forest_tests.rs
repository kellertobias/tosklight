//! Condition the already bundled original forest; never resample numeric Current or rebuild lanes.
use super::*;
use crate::programming::expression_coupled::{PositionAngleAxis, PositionForestNode};
use crate::{
    AngleNumericNode, AngleNumericProgram, DynamicPresetSourceBinding, DynamicRuntimeSample,
    DynamicSourceOccurrenceId, DynamicValueSourceResolver, bundle_position_component_forest,
};
use light_core::FixtureId;
use std::cell::{Cell, RefCell};
fn occurrence(id: u128) -> DynamicSourceOccurrenceId {
    DynamicSourceOccurrenceId::new(Uuid::from_u128(id)).unwrap()
}
fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target(xyz: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        xyz,
    )))
}
fn address(component: Option<ProgrammingComponent>) -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component,
    }
}
struct OriginalCurrent {
    value: AttributeValue,
    reads: Cell<usize>,
}
impl DynamicValueSourceResolver for OriginalCurrent {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        panic!("forest must retain original family Current")
    }
    fn try_position_current_family(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.reads.set(self.reads.get() + 1);
        Ok(Some(self.value.clone()))
    }
    fn current_family_occurrence(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Option<DynamicSourceOccurrenceId> {
        Some(occurrence(500))
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}
struct Destination {
    original: AttributeValue,
    adopted: AttributeValue,
    calls: RefCell<Vec<AttributeValue>>,
}
impl WholeFamilyExpressionFrameResolver for Destination {
    fn adopt_position_angles(
        &self,
        value: &AttributeValue,
    ) -> Result<AttributeValue, TransitionError> {
        let (AttributeValue::Position(expected), AttributeValue::Position(actual)) =
            (&self.original, value)
        else {
            panic!("original Position")
        };
        assert!(
            Arc::ptr_eq(expected, actual),
            "conditioning retains captured Current Arc rather than a sampled scalar"
        );
        self.calls.borrow_mut().push(value.clone());
        Ok(self.adopted.clone())
    }
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
fn sample(lane: u128, expression: DynamicSampleExpression) -> DynamicRuntimeSample {
    DynamicRuntimeSample {
        instance_id: Uuid::from_u128(1),
        controller_id: Uuid::from_u128(2),
        target: FixtureId(Uuid::from_u128(3)),
        lane_id: Uuid::from_u128(lane),
        expression,
        priority: 3,
        activated_at_millis: 100,
        activation_mix: 1.,
        address: None,
    }
}
fn resume(
    from: Option<DynamicSampleExpression>,
    to: Option<DynamicSampleExpression>,
    id: u128,
) -> DynamicSampleExpression {
    DynamicSampleExpression::Transition {
        from: from.map(Arc::new),
        to: to.map(Arc::new),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(id),
        },
    }
}
fn numeric(amplitude: f32, id: u128) -> Arc<AngleNumericProgram> {
    Arc::new(AngleNumericProgram {
        address: address(Some(ProgrammingComponent::Pan)),
        occurrence: Some(occurrence(id)),
        nodes: vec![
            AngleNumericNode::Current,
            AngleNumericNode::Around {
                middle: 0,
                amplitude: DynamicValue::Scalar(amplitude),
                amount: 0.5,
            },
        ],
        root: 1,
        operations: Default::default(),
    })
}
fn nested_numeric_samples(
    a: Arc<AngleNumericProgram>,
    b: Arc<AngleNumericProgram>,
) -> Vec<DynamicRuntimeSample> {
    let current = || DynamicSampleExpression::AngleCurrent {
        address: Arc::new(address(Some(ProgrammingComponent::Tilt))),
    };
    vec![
        sample(
            10,
            resume(
                Some(DynamicSampleExpression::AngleNumeric { program: a }),
                Some(resume(
                    Some(DynamicSampleExpression::AngleNumeric { program: b }),
                    None,
                    41,
                )),
                40,
            ),
        ),
        sample(
            11,
            resume(Some(current()), Some(resume(Some(current()), None, 41)), 40),
        ),
    ]
}
fn scope(id: u128) -> PositionResumeScope {
    PositionResumeScope {
        instance_id: Uuid::from_u128(1),
        controller_id: Uuid::from_u128(2),
        occurrence_id: Uuid::from_u128(id),
    }
}
fn coupled(source: &FamilyCompositionSample) -> &Arc<CompiledCoupledExpression> {
    let FamilyCompositionSample::CoupledExpression { expression, .. } = source else {
        panic!("original compiled forest")
    };
    expression
}
fn conditioned(branch: &PositionProgramBranch) -> FamilyCompositionSample {
    branch.conditioned_sources().unwrap()[0]
        .as_ref()
        .unwrap()
        .clone()
}
fn evaluate(
    source: FamilyCompositionSample,
    base: &AttributeValue,
    frame: &Destination,
) -> (AttributeValue, RetainedFamilyCompositionScratch) {
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let value = compose_retained_dynamic_family_traced(
        ProgrammingOwner::Position,
        base,
        &[source],
        &FamilyCompositionContext::default(),
        frame,
        &mut scratch,
    )
    .unwrap();
    (value, scratch)
}
#[test]
fn original_numeric_pan_and_captured_static_tilt_survive_nested_forest_conditioning() {
    let a = numeric(20., 100);
    let b = numeric(40., 101);
    let originals = OriginalCurrent {
        value: target([1., 2., 3.]),
        reads: Cell::new(0),
    };
    let samples = nested_numeric_samples(Arc::clone(&a), Arc::clone(&b));
    let original = bundle_position_component_forest(&samples, &originals)
        .unwrap()
        .position
        .unwrap();
    assert_eq!(originals.reads.get(), 1);
    let lineage = coupled(&original).position_forest_lineage().unwrap();
    let pairs = lineage
        .nodes
        .iter()
        .filter_map(|node| match node {
            PositionForestNode::AnglePair(pair) => Some(pair),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(pairs.len(), 2);
    let mut captured = None;
    for (pair, expected) in pairs.iter().zip([&a, &b]) {
        let PositionAngleAxis::Numeric {
            lane_id,
            program,
            original,
            ..
        } = &pair.axes[0]
        else {
            panic!("numeric Pan retained")
        };
        assert_eq!(*lane_id, Uuid::from_u128(10));
        assert_eq!(
            program.as_ref(),
            expected.as_ref(),
            "bundling retains the original numeric program content"
        );
        if let Some(captured) = &captured {
            assert!(Arc::ptr_eq(captured, original));
        } else {
            captured = Some(Arc::clone(original));
        }
        let PositionAngleAxis::Current {
            lane_id,
            original: tilt,
            ..
        } = &pair.axes[1]
        else {
            panic!("static Tilt Current retained")
        };
        assert_eq!(*lane_id, Uuid::from_u128(11));
        assert!(Arc::ptr_eq(original, tilt));
        assert_eq!(tilt.occurrence, Some(occurrence(500)));
    }
    let registry =
        CapturedPositionProgram::new(Uuid::from_u128(900), &angles(0., 0.), &[original.clone()])
            .unwrap();
    let untouched = conditioned(&registry.branch());
    assert!(
        Arc::ptr_eq(coupled(&untouched), coupled(&original)),
        "unconditioned forest retains original compiled source identity"
    );
    let root = registry
        .source_root(0)
        .unwrap()
        .expect("opaque forest root");
    let PositionSourceNodeView::Transition {
        progress,
        reason,
        to: Some(inner),
        ..
    } = registry.node_view(&root).unwrap()
    else {
        panic!("outer Resume")
    };
    assert_eq!(progress, 0.5);
    assert_eq!(
        reason,
        DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(40)
        }
    );
    let PositionSourceNodeView::Transition {
        progress, reason, ..
    } = registry.node_view(&inner).unwrap()
    else {
        panic!("nested Resume")
    };
    assert_eq!(progress, 0.5);
    assert_eq!(
        reason,
        DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(41)
        }
    );
    let mut branch = registry.branch();
    branch
        .choose_resume(scope(40), PositionResumeEndpoint::Incoming)
        .unwrap();
    branch
        .choose_resume(scope(41), PositionResumeEndpoint::Outgoing)
        .unwrap();
    let selected = conditioned(&branch);
    assert_eq!(selected.rank(), original.rank());
    let selected_lineage = coupled(&selected).position_forest_lineage().unwrap();
    let PositionForestNode::AnglePair(selected_pair) =
        &selected_lineage.nodes[selected_lineage.root]
    else {
        panic!("selected original pair")
    };
    assert!(Arc::ptr_eq(selected_pair, pairs[1]));
    let PositionAngleAxis::Numeric {
        program: selected_numeric,
        ..
    } = &selected_pair.axes[0]
    else {
        panic!()
    };
    let PositionAngleAxis::Numeric {
        program: original_numeric,
        ..
    } = &pairs[1].axes[0]
    else {
        panic!()
    };
    assert!(
        Arc::ptr_eq(selected_numeric, original_numeric),
        "conditioning retains the original captured numeric Arc"
    );
    for (pan, tilt) in [(10., 30.), (50., 70.)] {
        let frame = Destination {
            original: originals.value.clone(),
            adopted: angles(pan, tilt),
            calls: RefCell::new(Vec::new()),
        };
        assert_eq!(
            evaluate(selected.clone(), &angles(0., 0.), &frame).0,
            angles(pan + 20., tilt)
        );
        assert_eq!(frame.calls.borrow().len(), 1);
    }
    assert_eq!(
        originals.reads.get(),
        1,
        "conditioning/evaluation cannot rebundle or recapture Current"
    );
    let mut outgoing = registry.branch();
    outgoing
        .choose_resume(scope(40), PositionResumeEndpoint::Outgoing)
        .unwrap();
    assert!(
        outgoing
            .choose_resume(scope(41), PositionResumeEndpoint::Incoming)
            .is_err(),
        "equal progress cannot identify another occurrence or unreachable branch"
    );
    let mut release = registry.branch();
    release
        .choose_resume(scope(40), PositionResumeEndpoint::Incoming)
        .unwrap();
    release
        .choose_resume(scope(41), PositionResumeEndpoint::Incoming)
        .unwrap();
    assert!(
        release.conditioned_sources().unwrap()[0].is_none(),
        "released forest retains its original source slot as a hole"
    );
}
fn target_address(component: Option<ProgrammingComponent>) -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Target {
            reference: Some(TargetReference::Origin),
        },
        component,
    }
}
fn target_leaf(xyz: [f32; 3], id: u128) -> DynamicSampleExpression {
    DynamicSampleExpression::Programming {
        address: Arc::new(target_address(None)),
        value: DynamicValue::Family(target(xyz)),
        occurrence: Some(occurrence(id)),
        dependency_occurrence: None,
    }
}
fn target_scale(xyz: [f32; 3], id: u128) -> DynamicSampleExpression {
    DynamicSampleExpression::Scale {
        address: Arc::new(target_address(None)),
        base: DynamicValue::Family(target([1., 2., 3.])),
        value: Arc::new(target_leaf(xyz, id)),
        factor: 1.5,
        baseline_occurrence: Some(occurrence(600)),
    }
}
fn target_x(value: f32, id: u128) -> DynamicSampleExpression {
    DynamicSampleExpression::Programming {
        address: Arc::new(target_address(Some(ProgrammingComponent::TargetX))),
        value: DynamicValue::Scalar(value),
        occurrence: Some(occurrence(id)),
        dependency_occurrence: None,
    }
}
#[test]
fn original_sourcecohort_member_arcs_lane_ranks_and_narrow_release_footprints_survive_conditioning()
{
    let originals = OriginalCurrent {
        value: target([1., 2., 3.]),
        reads: Cell::new(0),
    };
    let samples = [
        sample(
            20,
            resume(
                Some(target_scale([2., 4., 6.], 210)),
                Some(target_scale([3., 6., 9.], 211)),
                50,
            ),
        ),
        sample(
            21,
            resume(Some(target_x(99., 212)), Some(target_x(199., 213)), 50),
        ),
    ];
    let original = bundle_position_component_forest(&samples, &originals)
        .unwrap()
        .position
        .unwrap();
    let lineage = coupled(&original).position_forest_lineage().unwrap();
    let original_cohorts = lineage
        .nodes
        .iter()
        .filter_map(|node| match node {
            PositionForestNode::SourceCohort(members) => Some(members),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(original_cohorts.len(), 2);
    for members in &original_cohorts {
        assert_eq!(
            members
                .iter()
                .map(|member| member.lane_id())
                .collect::<Vec<_>>(),
            vec![Uuid::from_u128(20), Uuid::from_u128(21)]
        );
    }
    let base = target([1., 2., 3.]);
    let registry =
        CapturedPositionProgram::new(Uuid::from_u128(901), &base, &[original.clone()]).unwrap();
    let root = registry.source_root(0).unwrap().unwrap();
    let PositionSourceNodeView::Transition {
        from: Some(outgoing),
        ..
    } = registry.node_view(&root).unwrap()
    else {
        panic!("original forest Resume")
    };
    let PositionSourceNodeView::SourceCohort { members } = registry.node_view(&outgoing).unwrap()
    else {
        panic!("original ordered source cohort")
    };
    assert_eq!(members.len(), 2);
    for (member, lane) in members.iter().zip([20, 21]) {
        let rank = registry.node_source_rank(member).unwrap();
        assert_eq!(
            rank.dynamic_identity().unwrap().lane_id,
            Uuid::from_u128(lane)
        );
        assert_eq!(rank.priority, 3);
        assert_eq!(rank.changed_at_millis, 100);
        assert_eq!(
            rank.dynamic_identity().unwrap().instance_id,
            Uuid::from_u128(1)
        );
        assert_eq!(
            rank.dynamic_identity().unwrap().controller_id,
            Uuid::from_u128(2)
        );
    }
    let mut branch = registry.branch();
    branch
        .choose_resume(scope(50), PositionResumeEndpoint::Outgoing)
        .unwrap();
    let selected = conditioned(&branch);
    let selected_lineage = coupled(&selected).position_forest_lineage().unwrap();
    let PositionForestNode::SourceCohort(selected_members) =
        &selected_lineage.nodes[selected_lineage.root]
    else {
        panic!("selected original source cohort")
    };
    assert!(
        Arc::ptr_eq(selected_members, original_cohorts[0]),
        "conditioning preserves member order, source handles and immutable compiled whole expressions"
    );
    let frame = Destination {
        original: originals.value.clone(),
        adopted: angles(0., 0.),
        calls: RefCell::new(Vec::new()),
    };
    let (value, scratch) = evaluate(selected, &base, &frame);
    assert_eq!(value, target([99., 5., 7.5]));
    let trace = scratch.family_trace();
    let trace_root = trace.root().unwrap();
    let x = trace
        .sources_for_component(trace_root, ProgrammingComponent::TargetX)
        .unwrap();
    assert!(
        x.iter()
            .any(
                |source| source.rank.dynamic_identity().unwrap().lane_id == Uuid::from_u128(21)
                    && source.occurrence == Some(occurrence(212))
            )
    );
    let y = trace
        .sources_for_component(trace_root, ProgrammingComponent::TargetY)
        .unwrap();
    assert!(
        y.iter()
            .any(
                |source| source.rank.dynamic_identity().unwrap().lane_id == Uuid::from_u128(20)
                    && source.occurrence == Some(occurrence(210))
            )
    );
    let mut narrow = registry.branch();
    narrow.replace_source(0, Some(&members[1])).unwrap();
    let (value, scratch) = evaluate(conditioned(&narrow), &base, &frame);
    assert_eq!(
        value,
        target([99., 2., 3.]),
        "selecting an original TargetX member cannot retain the released whole Target mask"
    );
    let trace = scratch.family_trace();
    let x = trace
        .sources_for_component(trace.root().unwrap(), ProgrammingComponent::TargetX)
        .unwrap();
    assert!(
        x.iter()
            .any(|source| source.occurrence == Some(occurrence(212)))
    );
    assert!(
        trace
            .sources_for_component(trace.root().unwrap(), ProgrammingComponent::TargetY)
            .unwrap()
            .is_empty(),
        "released whole source cannot keep authored TargetY ownership"
    );
    assert_eq!(
        originals.reads.get(),
        0,
        "conditioning pure Target source cohorts does not query unrelated Current"
    );
}

#[test]
fn nested_sourcecohort_member_tape_resume_keeps_size_and_original_lane_evidence() {
    let originals = OriginalCurrent {
        value: target([1., 2., 3.]),
        reads: Cell::new(0),
    };
    // Resume sits inside Size, so it remains an original member tape rather than an outer
    // zipped forest transition. The second member deliberately has the representative lane.
    let scaled = DynamicSampleExpression::Scale {
        address: Arc::new(target_address(None)),
        base: DynamicValue::Family(target([1., 2., 3.])),
        value: Arc::new(resume(
            Some(target_leaf([2., 4., 6.], 310)),
            Some(target_leaf([4., 8., 12.], 311)),
            61,
        )),
        factor: 1.5,
        baseline_occurrence: Some(occurrence(600)),
    };
    let samples = [sample(20, scaled), sample(21, target_x(99., 312))];
    let original = bundle_position_component_forest(&samples, &originals)
        .unwrap()
        .position
        .unwrap();
    let base = target([1., 2., 3.]);
    let registry =
        CapturedPositionProgram::new(Uuid::from_u128(902), &base, &[original.clone()]).unwrap();
    assert_eq!(
        registry
            .source_rank(0)
            .unwrap()
            .dynamic_identity()
            .unwrap()
            .lane_id,
        Uuid::from_u128(21)
    );
    let root = registry.source_root(0).unwrap().unwrap();
    let PositionSourceNodeView::SourceCohort { members } = registry.node_view(&root).unwrap()
    else {
        panic!("original source cohort")
    };
    let PositionSourceNodeView::Whole {
        lane_id,
        root: member_tape,
    } = registry.node_view(&members[0]).unwrap()
    else {
        panic!("whole member tape")
    };
    assert_eq!(lane_id, Uuid::from_u128(20));
    let PositionSourceNodeView::Scale {
        factor,
        value: Some(nested),
    } = registry.node_view(&member_tape).unwrap()
    else {
        panic!("original Size envelope")
    };
    assert_eq!(factor, 1.5);
    let PositionSourceNodeView::Transition {
        progress, reason, ..
    } = registry.node_view(&nested).unwrap()
    else {
        panic!("nested original Resume")
    };
    assert_eq!(progress, 0.5);
    assert_eq!(
        reason,
        DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(61)
        }
    );
    assert_eq!(
        registry
            .node_source_rank(&nested)
            .unwrap()
            .dynamic_identity()
            .unwrap()
            .lane_id,
        Uuid::from_u128(20),
        "nested original tape rank cannot borrow the representative lane"
    );
    assert_eq!(registry.resume_scope(&nested).unwrap(), Some(scope(61)));
    let frame = Destination {
        original: originals.value.clone(),
        adopted: angles(0., 0.),
        calls: RefCell::new(Vec::new()),
    };
    let mut incoming = registry.branch();
    incoming
        .choose_resume(scope(61), PositionResumeEndpoint::Incoming)
        .unwrap();
    let (value, scratch) = evaluate(conditioned(&incoming), &base, &frame);
    assert_eq!(
        value,
        target([99., 11., 16.5]),
        "Resume conditioning retains the complete original Size envelope and higher TargetX member"
    );
    let trace = scratch.family_trace();
    let y = trace
        .sources_for_component(trace.root().unwrap(), ProgrammingComponent::TargetY)
        .unwrap();
    assert!(
        y.iter()
            .any(|source| source.occurrence == Some(occurrence(311))
                && source.rank.dynamic_identity().unwrap().lane_id == Uuid::from_u128(20))
    );
    assert!(
        !y.iter()
            .any(|source| source.occurrence == Some(occurrence(310))),
        "outgoing authored occurrence is released"
    );
    assert!(
        y.iter()
            .any(|source| source.occurrence == Some(occurrence(600))
                && source.role == FamilyTraceRole::CalculationDependency),
        "original Size pivot remains a dependency"
    );
    let x = trace
        .sources_for_component(trace.root().unwrap(), ProgrammingComponent::TargetX)
        .unwrap();
    assert!(
        x.iter()
            .any(|source| source.occurrence == Some(occurrence(312))
                && source.rank.dynamic_identity().unwrap().lane_id == Uuid::from_u128(21))
    );
    // Select the original member tape's Scale subtree, preserving its Size while releasing
    // the sibling TargetX member. No new source expression or sampled value is supplied.
    incoming.replace_source(0, Some(&member_tape)).unwrap();
    let (value, scratch) = evaluate(conditioned(&incoming), &base, &frame);
    assert_eq!(value, target([5.5, 11., 16.5]));
    let x = scratch
        .family_trace()
        .sources_for_component(
            scratch.family_trace().root().unwrap(),
            ProgrammingComponent::TargetX,
        )
        .unwrap();
    assert!(
        x.iter()
            .any(|source| source.occurrence == Some(occurrence(311))
                && source.rank.dynamic_identity().unwrap().lane_id == Uuid::from_u128(20))
    );
    assert!(
        !x.iter()
            .any(|source| source.occurrence == Some(occurrence(312))),
        "released sibling cannot retain its footprint or occurrence"
    );
    assert_eq!(originals.reads.get(), 0);
    assert!(
        frame.calls.borrow().is_empty(),
        "pure matching-reference Target conditioning needs no fitted Current"
    );
}
