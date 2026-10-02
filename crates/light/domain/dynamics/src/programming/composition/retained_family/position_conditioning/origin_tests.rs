//! Deferred requests refer to original captured operations, never prepared or folded indices.
use super::*;
use crate::{
    CoupledCohortEndpoint, CoupledComponentEndpoint, CoupledLeafRole, DynamicPresetSourceBinding,
    DynamicValueSourceResolver, WholeFamilyExpressionFrameResolver,
};
use light_core::FixtureId;
use std::cell::Cell;

fn rank(lane: u128) -> FamilySampleRank {
    FamilySampleRank {
        priority: 3,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: 0,
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
fn target(point: u128) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        if point == 0 {
            TargetReference::Origin
        } else {
            TargetReference::Point {
                point_id: Uuid::from_u128(point),
            }
        },
        [1., 2., 3.],
    )))
}
fn angles() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(10., 20.)))
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
fn compiled_whole(
    expression: Arc<DynamicSampleExpression>,
) -> Arc<CompiledProgrammingFamilyExpression> {
    Arc::new(
        CompiledProgrammingFamilyExpression::new(
            expression,
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    )
}
fn whole(
    expression: Arc<DynamicSampleExpression>,
    lane: u128,
    mix: f32,
) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: compiled_whole(expression),
        rank: rank(lane),
        activation_mix: mix,
    }
}
fn coupled(expression: Arc<DynamicSampleExpression>, lane: u128) -> FamilyCompositionSample {
    FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(CompiledCoupledExpression::new(expression, None).unwrap()),
        rank: rank(lane),
        activation_mix: 1.,
    }
}
fn forest(nodes: Vec<PositionForestNode>, root: usize) -> FamilyCompositionSample {
    FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::from_position_forest(Arc::from([]), &nodes, root).unwrap(),
        ),
        rank: rank(90),
        activation_mix: 1.,
    }
}
fn known(lane: u128) -> FamilyCompositionSample {
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Position, &angles()).unwrap(),
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Family(angles()),
        rank(lane),
        1.,
    )
    .unwrap()
    .into()
}
struct Blocked {
    calls: Cell<usize>,
}
impl Blocked {
    fn new() -> Self {
        Self {
            calls: Cell::new(0),
        }
    }
}
impl WholeFamilyExpressionFrameResolver for Blocked {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        Err(TransitionError::Requires(requirement))
    }
}
fn needed(progress: PositionCompositionProgress) -> PositionCompositionRequest {
    match progress {
        PositionCompositionProgress::NeedsMaterialization(request) => request,
        PositionCompositionProgress::Complete(_) => panic!("expected deferred operation"),
    }
}
fn node(
    registry: &CapturedPositionProgram,
    request: &PositionCompositionRequest,
    indices: &[usize],
) -> PositionSourceNode {
    let origin = request.origin().expect("bound original registry origin");
    assert_eq!(origin.capture_id(), registry.capture_id());
    assert_eq!(origin.original_source_indices(), indices);
    registry
        .source_node_for_origin(origin)
        .unwrap()
        .expect("actual original retained operation")
}
fn assert_resume(
    registry: &CapturedPositionProgram,
    node: &PositionSourceNode,
    source: usize,
    lane: u128,
    id: u128,
) {
    assert_eq!(node.source_index(), source);
    assert_eq!(registry.node_source_rank(node).unwrap(), rank(lane));
    assert_eq!(registry.resume_scope(node).unwrap(), Some(scope(id)));
    assert!(
        matches!(registry.node_view(node).unwrap(), PositionSourceNodeView::Transition {
        progress: 0.5, reason: DynamicTransitionReason::Resume { occurrence_id }, ..
    } if occurrence_id == Uuid::from_u128(id))
    );
}

#[test]
fn mixed_whole_and_coupled_requests_keep_original_slots_after_sorting_and_release_hole() {
    let capture = Uuid::from_u128(920);
    let sources = vec![
        whole(
            resume(Some(leaf(target(11))), Some(leaf(target(12))), 70),
            20,
            0.5,
        ),
        known(5),
        coupled(
            resume(Some(leaf(target(13))), Some(leaf(target(14))), 71),
            10,
        ),
    ];
    let registry = CapturedPositionProgram::new(capture, &target(0), &sources).unwrap();
    let mut branch = registry.branch();
    branch.replace_source(1, None).unwrap();
    let context = FamilyCompositionContext::default();
    let frame = Blocked::new();
    let mut evaluation = branch
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let first = needed(evaluation.advance(capture, &context, &frame).unwrap());
    let first_node = node(&registry, &first, &[2]);
    assert_resume(&registry, &first_node, 2, 10, 71);
    assert!(matches!(
        first.operation,
        PositionCompositionOperation::Base {
            source_index: 1,
            ..
        }
    ));
    let repeated = needed(evaluation.advance(capture, &context, &frame).unwrap());
    assert_eq!(first.request_id, repeated.request_id);
    assert_resume(&registry, &node(&registry, &repeated, &[2]), 2, 10, 71);
    assert_eq!(
        frame.calls.get(),
        1,
        "origin lookup cannot replay suspended callbacks"
    );
    let foreign = CapturedPositionProgram::new(capture, &target(0), &sources).unwrap();
    assert!(
        foreign
            .source_node_for_origin(first.origin().unwrap())
            .is_err(),
        "equal UUID and values do not authorize another registry"
    );
    evaluation
        .resume_materialization(capture, first.request_id, target(0), None)
        .unwrap();
    let second = needed(evaluation.advance(capture, &context, &frame).unwrap());
    assert_resume(&registry, &node(&registry, &second, &[0]), 0, 20, 70);
    assert!(matches!(
        second.operation,
        PositionCompositionOperation::Base {
            source_index: 0,
            ..
        }
    ));
    let mut raw = begin_retained_position_composition(
        capture,
        &target(0),
        &sources,
        &context,
        RetainedFamilyCompositionScratch::default(),
        true,
    )
    .unwrap();
    assert!(
        needed(raw.advance(capture, &context, &frame).unwrap())
            .origin()
            .is_none(),
        "legacy unbound preparation cannot fabricate registry authority"
    );
}

#[test]
fn folded_outer_forest_resume_routes_nested_whole_tape_request_to_original_lane_and_scope() {
    let nested = resume(Some(leaf(target(21))), Some(leaf(target(22))), 81);
    let source = forest(
        vec![
            PositionForestNode::Whole {
                expression: nested,
                lane_id: Uuid::from_u128(11),
                sources: Arc::from([]),
            },
            PositionForestNode::Whole {
                expression: leaf(target(23)),
                lane_id: Uuid::from_u128(12),
                sources: Arc::from([]),
            },
            PositionForestNode::Transition {
                from: Some(0),
                to: Some(1),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: Uuid::from_u128(80),
                },
            },
        ],
        2,
    );
    let capture = Uuid::from_u128(921);
    let registry = CapturedPositionProgram::new(capture, &target(0), &[source]).unwrap();
    let original_root = registry.source_root(0).unwrap().unwrap();
    let PositionSourceNodeView::Transition {
        from: Some(outgoing),
        ..
    } = registry.node_view(&original_root).unwrap()
    else {
        panic!("original forest Resume")
    };
    let PositionSourceNodeView::Whole {
        root: original_nested,
        ..
    } = registry.node_view(&outgoing).unwrap()
    else {
        panic!("original imported Whole")
    };
    assert_resume(&registry, &original_nested, 0, 11, 81);
    let mut branch = registry.branch();
    branch
        .choose_resume(scope(80), PositionResumeEndpoint::Outgoing)
        .unwrap();
    let context = FamilyCompositionContext::default();
    let mut evaluation = branch
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let request = needed(
        evaluation
            .advance(capture, &context, &Blocked::new())
            .unwrap(),
    );
    let routed = node(&registry, &request, &[0]);
    assert_resume(&registry, &routed, 0, 11, 81);
    assert_eq!(
        routed.node, original_nested.node,
        "folded local IDs resolve to the exact original imported tape root"
    );
}
fn size(point: u128) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &target(0)).unwrap(),
        ),
        base: DynamicValue::Family(target(0)),
        value: leaf(target(point)),
        factor: 1.5,
        baseline_occurrence: None,
    })
}
fn member(lane: u128, expression: Arc<DynamicSampleExpression>) -> CoupledCohortEndpoint {
    CoupledCohortEndpoint::WholeExpression {
        lane_id: Uuid::from_u128(lane),
        expression: compiled_whole(expression),
    }
}
fn component(lane: u128) -> CoupledCohortEndpoint {
    CoupledCohortEndpoint::Materialized(CoupledComponentEndpoint {
        lane_id: Uuid::from_u128(lane),
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
        value: DynamicValue::Scalar(99.),
        role: CoupledLeafRole::Authored,
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn whole_root(
    registry: &CapturedPositionProgram,
    member: &PositionSourceNode,
) -> PositionSourceNode {
    let PositionSourceNodeView::Whole { root, .. } = registry.node_view(member).unwrap() else {
        panic!("original Whole member")
    };
    root
}
fn assert_size(registry: &CapturedPositionProgram, routed: &PositionSourceNode, lane: u128) {
    assert_eq!(registry.node_source_rank(routed).unwrap(), rank(lane));
    assert_eq!(registry.resume_scope(routed).unwrap(), None);
    assert!(matches!(
        registry.node_view(routed).unwrap(),
        PositionSourceNodeView::Scale { factor: 1.5, .. }
    ));
}
#[test]
fn nested_sourcecohort_size_request_keeps_original_member_after_earlier_member_release() {
    let capture = Uuid::from_u128(922);
    let source = forest(
        vec![
            PositionForestNode::SourceCohort(
                vec![
                    member(10, resume(Some(leaf(target(31))), None, 82)),
                    member(20, size(32)),
                    component(21),
                ]
                .into(),
            ),
            PositionForestNode::SourceCohort(
                vec![member(30, leaf(target(33))), component(31)].into(),
            ),
            PositionForestNode::Transition {
                from: Some(0),
                to: Some(1),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: Uuid::from_u128(83),
                },
            },
        ],
        2,
    );
    let registry = CapturedPositionProgram::new(capture, &target(0), &[known(5), source]).unwrap();
    let root = registry.source_root(1).unwrap().unwrap();
    let PositionSourceNodeView::Transition {
        from: Some(outgoing),
        ..
    } = registry.node_view(&root).unwrap()
    else {
        panic!("original forest")
    };
    let PositionSourceNodeView::SourceCohort { members } = registry.node_view(&outgoing).unwrap()
    else {
        panic!("original member list")
    };
    let original_size = whole_root(&registry, &members[1]);
    assert_size(&registry, &original_size, 20);
    let mut branch = registry.branch();
    branch.replace_source(0, None).unwrap();
    branch
        .choose_resume(scope(82), PositionResumeEndpoint::Incoming)
        .unwrap();
    let context = FamilyCompositionContext::default();
    let mut evaluation = branch
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let request = needed(
        evaluation
            .advance(capture, &context, &Blocked::new())
            .unwrap(),
    );
    let routed = node(&registry, &request, &[1]);
    assert_eq!(routed.source_index(), 1);
    assert_size(&registry, &routed, 20);
    assert_eq!(
        routed.node, original_size.node,
        "member compaction preserves the original member-one tape root"
    );
    assert!(
        matches!(&request.operation, PositionCompositionOperation::Base { request, .. }
        if matches!(request.operation, PositionCompositionBaseOperation::SourceCohort { .. }))
    );
}
#[test]
fn exact_expanded_whole_member_size_request_keeps_original_forest_member_lane() {
    let capture = Uuid::from_u128(923);
    let source = forest(
        vec![PositionForestNode::SourceCohort(
            vec![component(21), member(20, size(42))].into(),
        )],
        0,
    );
    let registry = CapturedPositionProgram::new(capture, &target(0), &[known(5), source]).unwrap();
    let root = registry.source_root(1).unwrap().unwrap();
    let PositionSourceNodeView::SourceCohort { members } = registry.node_view(&root).unwrap()
    else {
        panic!("original exact cohort")
    };
    let original_size = whole_root(&registry, &members[1]);
    let mut branch = registry.branch();
    branch.replace_source(0, None).unwrap();
    let context = FamilyCompositionContext::default();
    let mut evaluation = branch
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let request = needed(
        evaluation
            .advance(capture, &context, &Blocked::new())
            .unwrap(),
    );
    let routed = node(&registry, &request, &[1]);
    assert_eq!(routed.source_index(), 1);
    assert_size(&registry, &routed, 20);
    assert_eq!(
        routed.node, original_size.node,
        "expanded local Whole routes to the original member-one tape root"
    );
}
struct CapturedCurrent;
impl DynamicValueSourceResolver for CapturedCurrent {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        None
    }
    fn try_current_family_base(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        Ok(Some(target(0)))
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
fn axis(lane: u128, component: ProgrammingComponent) -> FamilyCompositionSample {
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Angles,
                    component: Some(component),
                },
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Scalar(lane as f32),
        rank(lane),
        0.5,
    )
    .unwrap()
    .into()
}
#[test]
fn synthetic_angle_pair_completion_has_original_slot_aggregate_without_retained_node_authority() {
    let capture = Uuid::from_u128(924);
    let sources = [
        axis(12, ProgrammingComponent::Tilt),
        known(5),
        axis(11, ProgrammingComponent::Pan),
    ];
    let registry = CapturedPositionProgram::new(capture, &target(0), &sources).unwrap();
    let mut branch = registry.branch();
    branch.replace_source(1, None).unwrap();
    let current = CapturedCurrent;
    let control = |_| FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.5 };
    let context = FamilyCompositionContext {
        endpoint_output: Some(FamilyEndpointOutputContext {
            control: &control,
            target: FixtureId(Uuid::from_u128(3)),
            current: &current,
            native_models: None,
        }),
        ..Default::default()
    };
    let frame = Blocked::new();
    let mut evaluation = branch
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let first = needed(evaluation.advance(capture, &context, &frame).unwrap());
    let origin = first.origin().unwrap();
    assert_eq!(origin.original_source_indices(), &[2, 0]);
    assert!(
        registry.source_node_for_origin(origin).unwrap().is_none(),
        "a synthetic pair is not an original retained cut operation"
    );
    assert!(
        matches!(&first.operation, PositionCompositionOperation::Base { request, .. }
        if matches!(&request.operation, PositionCompositionBaseOperation::Completion(completion)
            if completion.stage == PositionCompletionStage::EndpointOutput))
    );
    evaluation
        .resume_materialization(capture, first.request_id, angles(), None)
        .unwrap();
    let second = needed(evaluation.advance(capture, &context, &frame).unwrap());
    assert_eq!(second.origin().unwrap().original_source_indices(), &[2, 0]);
    assert!(
        registry
            .source_node_for_origin(second.origin().unwrap())
            .unwrap()
            .is_none()
    );
    assert!(
        matches!(&second.operation, PositionCompositionOperation::Base { request, .. }
        if matches!(&request.operation, PositionCompositionBaseOperation::Completion(completion)
            if completion.stage == PositionCompletionStage::Activation))
    );
}

#[test]
fn dropped_first_member_cannot_steal_surviving_interior_resume_origin_in_either_cohort_route() {
    for nested in [false, true] {
        let capture = Uuid::from_u128(if nested { 926 } else { 925 });
        let mut nodes = vec![PositionForestNode::SourceCohort(
            vec![
                member(10, resume(Some(leaf(target(51))), None, 85)),
                member(20, resume(Some(leaf(target(52))), Some(leaf(angles())), 86)),
            ]
            .into(),
        )];
        let root = if nested {
            nodes.push(PositionForestNode::SourceCohort(
                vec![member(30, leaf(target(53)))].into(),
            ));
            nodes.push(PositionForestNode::Transition {
                from: Some(0),
                to: Some(1),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: Uuid::from_u128(87),
                },
            });
            2
        } else {
            0
        };
        let source = forest(nodes, root);
        let registry = CapturedPositionProgram::new(capture, &target(0), &[source]).unwrap();
        let original_root = registry.source_root(0).unwrap().unwrap();
        let cohort = if nested {
            let PositionSourceNodeView::Transition {
                from: Some(cohort), ..
            } = registry.node_view(&original_root).unwrap()
            else {
                panic!("original enclosing forest Resume")
            };
            cohort
        } else {
            original_root
        };
        let PositionSourceNodeView::SourceCohort { members } = registry.node_view(&cohort).unwrap()
        else {
            panic!("original cohort members")
        };
        let original_resume = whole_root(&registry, &members[1]);
        assert_resume(&registry, &original_resume, 0, 20, 86);
        let mut branch = registry.branch();
        branch
            .choose_resume(scope(85), PositionResumeEndpoint::Incoming)
            .unwrap();
        let context = FamilyCompositionContext::default();
        let mut evaluation = branch
            .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
            .unwrap();
        let request = needed(
            evaluation
                .advance(capture, &context, &Blocked::new())
                .unwrap(),
        );
        let routed = node(&registry, &request, &[0]);
        assert_resume(&registry, &routed, 0, 20, 86);
        assert_eq!(
            routed.node, original_resume.node,
            "local member zero routes to original member one after release"
        );
        assert!(matches!(routed.node, NodeLocation::MemberTape(0, 1, _)));
        let PositionCompositionOperation::Base { request, .. } = &request.operation else {
            panic!("base request")
        };
        assert_eq!(
            matches!(
                request.operation,
                PositionCompositionBaseOperation::SourceCohort { .. }
            ),
            nested
        );
    }
}
