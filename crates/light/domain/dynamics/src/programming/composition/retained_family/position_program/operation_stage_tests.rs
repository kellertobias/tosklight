//! TL633 acceptance through genuine sampler operations and bound original compositor routes.
//! The algebraic test frame maps Target X/Y to Pan/Tilt; this tests replay, not physical fitting.
use super::*;
use crate::programming::expression_coupled::PositionForestNode;
use crate::*;
use light_core::{AttributeKey, FixtureId};
use std::cell::Cell;
use std::collections::HashMap;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target(reference: u128, x: f32, y: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        if reference == 0 {
            TargetReference::Origin
        } else {
            TargetReference::Point {
                point_id: Uuid::from_u128(reference),
            }
        },
        [x, y, 0.],
    )))
}
fn angle_value(value: &AttributeValue) -> AttributeValue {
    let AttributeValue::Position(position) = value else {
        panic!("Position")
    };
    match position.as_ref() {
        PositionIntent::Angles { .. } => value.clone(),
        PositionIntent::Target { offset_metres, .. } => {
            let [ScalarIntent::Value(x), ScalarIntent::Value(y), _] = offset_metres else {
                panic!("literal offsets")
            };
            angles(*x, *y)
        }
    }
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
struct Sources {
    current: AttributeValue,
    calls: Cell<usize>,
}
impl ScalarSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        panic!("typed sampler")
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}
impl DynamicValueSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        self.calls.set(self.calls.get() + 1);
        Some(DynamicValue::Family(self.current.clone()))
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
struct Sampled {
    expression: Arc<DynamicSampleExpression>,
    rank: FamilySampleRank,
    target: FixtureId,
}
fn sampled(size: f32) -> Sampled {
    let lane_id = Uuid::new_v4();
    let definition = DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 1,
        revision: 1,
        name: "Original operation replay".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::Targetless,
        lanes: vec![DynamicLane {
            id: lane_id,
            body: DynamicLaneBody::Programming(ProgrammingLaneBody {
                address: DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Target { reference: None },
                    component: None,
                },
                configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                    points: vec![
                        DynamicKeyframe {
                            position: 0.,
                            source: DynamicValueSource::Value {
                                value: DynamicValue::Family(target(0, 10., 20.)),
                            },
                            interpolation: crate::ScalarInterpolation::Linear,
                        },
                        DynamicKeyframe {
                            position: 0.5,
                            source: DynamicValueSource::Value {
                                value: DynamicValue::Family(target(9, 30., 40.)),
                            },
                            interpolation: crate::ScalarInterpolation::Linear,
                        },
                    ],
                    size: 1.,
                }),
            }),
            speed_multiplier: Rational::ONE,
            width: 1.,
            phase: None,
            random_group_id: None,
        }],
        random_groups: vec![],
        phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: Default::default(),
        phase: PhaseDistribution {
            ordering: PhaseOrdering::Selection,
            offset_degrees: 0.,
            span_degrees: 0.,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: vec![],
        },
        speed: DynamicSpeed::Fixed {
            duration_millis: 1000,
        },
        overall_speed_multiplier: Rational::ONE,
        run_mode: DynamicRunMode::Loop,
        default_activation: ActivationPolicy::StartNow,
        activation_boundary: ActivationBoundary::Beat,
    };
    let fixture = FixtureId::new();
    let controller = DynamicController {
        id: Uuid::new_v4(),
        source: DynamicControllerSource::Programmer {
            programmer_id: Uuid::new_v4(),
            instance_link: None,
        },
        priority: 10,
        activated_at_millis: 1,
        size,
        speed_multiplier: 1.,
        phase_offset_degrees: 0.,
        paused: false,
    };
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id: definition.id,
            controller,
            target_scope: DynamicTargetScope {
                ordered_targets: vec![fixture],
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 0,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    let sources = Sources {
        current: target(0, 4., 8.),
        calls: Cell::new(0),
    };
    let mut samples = runtime
        .sample_programming(instance, 250, 1000, 10, &sources, &sources)
        .unwrap();
    assert_eq!(samples.len(), 1);
    let sample = samples.remove(0);
    let provenance = sample.expression.operation_provenance().unwrap();
    assert!(provenance.is_complete());
    assert_eq!(provenance.handles().len(), if size == 1. { 1 } else { 2 });
    assert!(
        provenance
            .handles()
            .iter()
            .all(|handle| handle.emission().instance_id() == instance
                && handle.target() == fixture
                && handle.lane_id() == lane_id)
    );
    Sampled {
        expression: Arc::new(sample.expression),
        rank: FamilySampleRank {
            priority: sample.priority,
            changed_at_millis: sample.activated_at_millis,
            changed_at_submillis_nanos: 0,
            stable_order: 0,
            identity: FamilySampleIdentity::Dynamic {
                instance_id: sample.instance_id,
                controller_id: sample.controller_id,
                lane_id: sample.lane_id,
            },
        },
        target: fixture,
    }
}
fn whole(
    expression: Arc<DynamicSampleExpression>,
    rank: FamilySampleRank,
    mix: f32,
) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(
                expression,
                ProgrammingOwner::Position,
                None,
                None,
            )
            .unwrap(),
        ),
        rank,
        activation_mix: mix,
    }
}
fn mask(value: AttributeValue, order: usize, mix: f32) -> FamilyCompositionSample {
    ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Position, None, value)
        .unwrap()
        .compile(
            None,
            &FamilyEditContext::default(),
            FamilySampleRank {
                priority: 10,
                changed_at_millis: 100 + order as u64,
                changed_at_submillis_nanos: 0,
                stable_order: order as u128,
                identity: FamilySampleIdentity::Fixed {
                    source: FamilyFixedSampleSource::Programmer,
                    row_index: order,
                },
            },
            mix,
        )
        .unwrap()
        .into()
}
#[derive(Default)]
struct Frame {
    blocked: bool,
    panic: bool,
    block_to: Option<AttributeValue>,
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for Frame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        op: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        assert!(!self.panic, "intentional operand prefix unwind");
        if self.blocked || self.block_to.as_ref() == Some(to) {
            return Err(TransitionError::Requires(requirement));
        }
        let transition =
            CompiledProgrammingTransition::new(angle_value(from), angle_value(to), None)?;
        match op {
            FamilyExpressionOperation::Transition { progress } => transition.sample(progress),
            FamilyExpressionOperation::Scale { factor } => transition.scale(factor),
        }
    }
}
fn needed(progress: PositionCompositionProgress) -> PositionCompositionRequest {
    let PositionCompositionProgress::NeedsMaterialization(request) = progress else {
        panic!("actual pending operation")
    };
    request
}
fn ready(progress: PositionGraphOperationOperandProgress) -> AttributeValue {
    match progress {
        PositionGraphOperationOperandProgress::OperandReady(value) => value,
        PositionGraphOperationOperandProgress::NeedsMaterialization(request) => {
            panic!(
                "expected original operand, but prefix is pending at request {}",
                request.request_id
            )
        }
        PositionGraphOperationOperandProgress::Inactive => {
            panic!("expected original operand, but its exact lexical operation is inactive")
        }
    }
}
fn suspended(progress: PositionGraphOperationOperandProgress) -> PositionCompositionRequest {
    let PositionGraphOperationOperandProgress::NeedsMaterialization(request) = progress else {
        panic!("actual unresolved prefix")
    };
    request
}
fn ordinary(
    registry: &CapturedPositionProgram,
    scratch: RetainedFamilyCompositionScratch,
) -> (AttributeValue, Option<FamilyTraceQuery>) {
    let context = FamilyCompositionContext::default();
    let mut parent = registry
        .branch()
        .begin_composition(&context, scratch, true)
        .unwrap();
    let PositionCompositionProgress::Complete(value) = parent
        .advance(registry.capture_id(), &context, &Frame::default())
        .unwrap()
    else {
        panic!("available test frame")
    };
    let trace = parent.family_trace();
    let fields = ProgrammingFieldScope::for_value(ProgrammingOwner::Position, &value).unwrap();
    (
        value,
        trace.query_fields_with_base(trace.root().unwrap(), &fields),
    )
}

#[test]
fn genuine_required_and_size_operands_exclude_their_ancestors_and_non_idempotent_suffix() {
    let sample = sampled(1.5);
    let source = whole(Arc::clone(&sample.expression), sample.rank, 1.);
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0., 0.),
        &[source, mask(angles(100., 200.), 1, 0.25)],
    )
    .unwrap();
    let context = FamilyCompositionContext::default();
    let blocked = Frame {
        blocked: true,
        ..Default::default()
    };
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    let required = needed(
        parent
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    let locator = parent
        .pending_graph_operation_locator(required.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(locator.kind(), PositionGraphOperationKind::Required);
    assert!(matches!(
        registry.node_view(locator.operation_node()).unwrap(),
        PositionSourceNodeView::Transition {
            reason: DynamicTransitionReason::Required { .. },
            ..
        }
    ));
    assert!(
        parent
            .pending_graph_operation_locator(Uuid::new_v4())
            .is_err()
    );
    for (operand, expected) in [
        (
            PositionGraphOperationOperand::RequiredOutgoing,
            target(0, 10., 20.),
        ),
        (
            PositionGraphOperationOperand::RequiredIncoming,
            target(9, 30., 40.),
        ),
    ] {
        let mut replay = registry
            .branch()
            .begin_graph_operation_operand(&locator, operand, &context, Default::default(), true)
            .unwrap();
        let calls = blocked.calls.get();
        for _ in 0..2 {
            assert_eq!(
                ready(
                    replay
                        .advance(registry.capture_id(), &context, &blocked)
                        .unwrap()
                ),
                expected
            );
        }
        assert_eq!(
            blocked.calls.get(),
            calls,
            "selected literal operand never executes Required, Size or the mask suffix"
        );
    }
    parent
        .resume_materialization(
            registry.capture_id(),
            required.request_id,
            angles(20., 30.),
            None,
        )
        .unwrap();
    let size_request = needed(
        parent
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    let size = parent
        .pending_graph_operation_locator(size_request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(size.kind(), PositionGraphOperationKind::Size);
    assert!(matches!(
        registry.node_view(size.operation_node()).unwrap(),
        PositionSourceNodeView::Scale { factor: 1.5, .. }
    ));
    let mut baseline = registry
        .branch()
        .begin_graph_operation_operand(
            &size,
            PositionGraphOperationOperand::SizeBaseline,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    let before = blocked.calls.get();
    assert_eq!(
        ready(
            baseline
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        ),
        target(0, 4., 8.)
    );
    assert_eq!(
        blocked.calls.get(),
        before,
        "Size baseline must not wait for its unavailable Required value child"
    );
    let mut value = registry
        .branch()
        .begin_graph_operation_operand(
            &size,
            PositionGraphOperationOperand::SizeValue,
            &context,
            baseline.into_scratch(),
            true,
        )
        .unwrap();
    let dependency = suspended(
        value
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    assert!(
        value
            .resume_materialization(
                Uuid::new_v4(),
                dependency.request_id,
                angles(20., 30.),
                None
            )
            .is_err()
    );
    assert!(
        value
            .resume_materialization(
                registry.capture_id(),
                Uuid::new_v4(),
                angles(20., 30.),
                None
            )
            .is_err()
    );
    assert!(
        value
            .resume_materialization(
                registry.capture_id(),
                dependency.request_id,
                AttributeValue::Normalized(0.5),
                None
            )
            .is_err()
    );
    assert_eq!(
        suspended(
            value
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        )
        .request_id,
        dependency.request_id
    );
    value
        .resume_materialization(
            registry.capture_id(),
            dependency.request_id,
            angles(20., 30.),
            None,
        )
        .unwrap();
    assert_eq!(
        ready(
            value
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        ),
        angles(20., 30.)
    );
    let expected = ordinary(&registry, Default::default());
    assert_eq!(
        expected.0,
        angles(46., 80.75),
        "Required=.5, Size=1.5, then mask=.25 exactly once"
    );
    assert_eq!(
        ordinary(&registry, value.into_scratch()),
        expected,
        "scratch replay preserves full parent value and field trace"
    );
    parent
        .resume_materialization(
            registry.capture_id(),
            size_request.request_id,
            angles(28., 41.),
            None,
        )
        .unwrap();
    assert!(
        matches!(parent.advance(registry.capture_id(),&context,&Frame::default()).unwrap(),PositionCompositionProgress::Complete(result) if result==expected.0)
    );
    assert!(
        parent
            .pending_graph_operation_locator(size_request.request_id)
            .is_err()
    );
}

#[test]
fn changed_required_child_makes_size_synchronous_without_replacing_the_original_operation() {
    let sample = sampled(1.5);
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0., 0.),
        &[whole(sample.expression, sample.rank, 1.)],
    )
    .unwrap();
    let context = FamilyCompositionContext::default();
    let blocked = Frame {
        blocked: true,
        ..Default::default()
    };
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    let required = parent
        .pending_graph_operation_locator(request.request_id)
        .unwrap()
        .unwrap();
    parent
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            angles(20., 30.),
            None,
        )
        .unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    let size = parent
        .pending_graph_operation_locator(request.request_id)
        .unwrap()
        .unwrap();
    let mut changed = registry.branch();
    changed
        .select_local_endpoint(
            required.operation_node(),
            PositionLocalEndpoint::RequiredOutgoing,
        )
        .unwrap();
    let mut replay = changed
        .begin_graph_operation_operand(
            &size,
            PositionGraphOperationOperand::SizeValue,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    let before = blocked.calls.get();
    assert_eq!(
        ready(
            replay
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        ),
        target(0, 10., 20.)
    );
    assert_eq!(
        blocked.calls.get(),
        before,
        "compatible Size still stops before scaling"
    );
    let mut removed = changed
        .begin_graph_operation_operand(
            &required,
            PositionGraphOperationOperand::RequiredOutgoing,
            &context,
            replay.into_scratch(),
            true,
        )
        .unwrap();
    assert!(matches!(
        removed
            .advance(registry.capture_id(), &context, &Frame::default())
            .unwrap(),
        PositionGraphOperationOperandProgress::Inactive
    ));
    assert_eq!(
        needed(
            parent
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        )
        .request_id,
        request.request_id
    );
}

#[test]
fn original_operation_distinguishes_underlay_lexical_use_and_rejects_foreign_registry_or_role() {
    let sample = sampled(1.);
    let mut higher = sample.rank;
    higher.priority += 1;
    let sources = [
        whole(Arc::clone(&sample.expression), sample.rank, 1.),
        whole(leaf(angles(100., 200.)), higher, 0.5),
    ];
    let capture = Uuid::new_v4();
    let registry = CapturedPositionProgram::new(capture, &angles(0., 0.), &sources).unwrap();
    let context = FamilyCompositionContext::default();
    let blocked = Frame {
        blocked: true,
        ..Default::default()
    };
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    let request = needed(parent.advance(capture, &context, &blocked).unwrap());
    let underlay = parent
        .pending_graph_operation_locator(request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(underlay.kind(), PositionGraphOperationKind::Required);
    let foreign = CapturedPositionProgram::new(capture, &angles(0., 0.), &sources).unwrap();
    assert!(
        foreign
            .branch()
            .validate_graph_operation_operand(
                &underlay,
                PositionGraphOperationOperand::RequiredOutgoing
            )
            .is_err(),
        "equal capture UUID and identical retained producer expression are not this registry"
    );
    assert!(
        registry
            .branch()
            .validate_graph_operation_operand(
                &underlay,
                PositionGraphOperationOperand::SizeBaseline
            )
            .is_err()
    );
    let mut without_consumer = registry.branch();
    without_consumer.replace_source(1, None).unwrap();
    let mut other = without_consumer
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    let other_request = needed(other.advance(capture, &context, &blocked).unwrap());
    let final_use = other
        .pending_graph_operation_locator(other_request.request_id)
        .unwrap()
        .unwrap();
    assert!(
        underlay.operation_node() == final_use.operation_node(),
        "same actual original operation"
    );
    assert_ne!(underlay, final_use, "its lexical use is part of authority");
    let mut wrong_use = without_consumer
        .begin_graph_operation_operand(
            &underlay,
            PositionGraphOperationOperand::RequiredOutgoing,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    assert!(matches!(
        wrong_use
            .advance(capture, &context, &Frame::default())
            .unwrap(),
        PositionGraphOperationOperandProgress::Inactive
    ));
    let mut exact = without_consumer
        .begin_graph_operation_operand(
            &final_use,
            PositionGraphOperationOperand::RequiredOutgoing,
            &context,
            wrong_use.into_scratch(),
            true,
        )
        .unwrap();
    assert_eq!(
        ready(exact.advance(capture, &context, &blocked).unwrap()),
        target(0, 10., 20.)
    );
    assert_eq!(
        needed(parent.advance(capture, &context, &blocked).unwrap()).request_id,
        request.request_id
    );
    assert_eq!(
        ordinary(&registry, exact.into_scratch()).0,
        angles(60., 115.),
        "higher activation remains in original parent only"
    );
}

#[test]
fn original_sampled_operation_keeps_nested_sourcecohort_member_identity_after_release() {
    let sample = sampled(1.5);
    let release_id = Uuid::new_v4();
    // Equal-rank Dynamic members arbitrate by lane UUID. Keep this authored releasing
    // member below the real sampled lane so the Required belongs to the final member
    // use, not to an UnderlayFor use that legitimately disappears with the consumer.
    // The producer's original lane and provenance are never rewritten.
    let prior_lane = Uuid::from_u128(1);
    let source_lane = sample.expression.operation_provenance().unwrap().handles()[0].lane_id();
    assert!(
        prior_lane < source_lane,
        "fixture must retain explicit member priority"
    );
    let releasing = Arc::new(DynamicSampleExpression::Transition {
        from: Some(leaf(target(0, 1., 2.))),
        to: None,
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: release_id,
        },
    });
    let compiled = |expression| {
        Arc::new(
            CompiledProgrammingFamilyExpression::new(
                expression,
                ProgrammingOwner::Position,
                None,
                None,
            )
            .unwrap(),
        )
    };
    let members = vec![
        CoupledCohortEndpoint::WholeExpression {
            lane_id: prior_lane,
            expression: compiled(releasing),
        },
        CoupledCohortEndpoint::WholeExpression {
            lane_id: source_lane,
            expression: compiled(Arc::clone(&sample.expression)),
        },
    ];
    let nodes = vec![
        PositionForestNode::SourceCohort(members.into()),
        PositionForestNode::Whole {
            expression: leaf(angles(80., 90.)),
            lane_id: Uuid::new_v4(),
            sources: Arc::from([]),
        },
        PositionForestNode::Transition {
            from: Some(0),
            to: Some(1),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::new_v4(),
            },
        },
    ];
    let source = FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::from_position_forest(Arc::from([]), &nodes, 2).unwrap(),
        ),
        rank: sample.rank,
        activation_mix: 1.,
    };
    let registry =
        CapturedPositionProgram::new(Uuid::new_v4(), &target(0, 0., 0.), &[source]).unwrap();
    let context = FamilyCompositionContext::default();
    let blocked = Frame {
        blocked: true,
        ..Default::default()
    };
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    let PositionCompositionOperation::Base { request: outer, .. } = &request.operation else {
        panic!("base request")
    };
    let PositionCompositionBaseOperation::SourceCohort { request: inner, .. } = &outer.operation
    else {
        panic!("actual raw nested member request, not expanded root slots")
    };
    assert!(
        matches!(&inner.operation,PositionCompositionBaseOperation::Whole(request)
        if matches!(request.operation,FamilyMaterializationOperation::Transition {reason:DynamicTransitionReason::Required {..},..}))
    );
    let locator = parent
        .pending_graph_operation_locator(request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(locator.kind(), PositionGraphOperationKind::Required);
    assert_eq!(
        registry
            .node_source_rank(locator.operation_node())
            .unwrap()
            .identity,
        FamilySampleIdentity::Dynamic {
            instance_id: sample.rank.dynamic_identity().unwrap().instance_id,
            controller_id: sample.rank.dynamic_identity().unwrap().controller_id,
            lane_id: source_lane
        }
    );
    let identity = sample.rank.dynamic_identity().unwrap();
    let mut compacted = registry.branch();
    compacted
        .choose_resume(
            PositionResumeScope {
                instance_id: identity.instance_id,
                controller_id: identity.controller_id,
                occurrence_id: release_id,
            },
            PositionResumeEndpoint::Incoming,
        )
        .unwrap();
    let mut replay = compacted
        .begin_graph_operation_operand(
            &locator,
            PositionGraphOperationOperand::RequiredIncoming,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        ready(
            replay
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        ),
        target(9, 30., 40.)
    );
    assert_eq!(
        needed(
            parent
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        )
        .request_id,
        request.request_id
    );
    parent
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            angles(20., 30.),
            None,
        )
        .unwrap();
    let size_request = needed(
        parent
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    let size = parent
        .pending_graph_operation_locator(size_request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(size.kind(), PositionGraphOperationKind::Size);
    let mut baseline = compacted
        .begin_graph_operation_operand(
            &size,
            PositionGraphOperationOperand::SizeBaseline,
            &context,
            replay.into_scratch(),
            true,
        )
        .unwrap();
    assert_eq!(
        ready(
            baseline
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        ),
        target(0, 4., 8.)
    );
    assert_eq!(
        ordinary(&registry, baseline.into_scratch()).0,
        angles(54., 65.5),
        "outer Resume must remain outside both inner operand replays"
    );
}

#[test]
fn failed_selected_value_child_is_terminal_but_recovered_scratch_keeps_parent_trace() {
    let sample = sampled(1.5);
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0., 0.),
        &[
            whole(sample.expression, sample.rank, 1.),
            mask(angles(100., 200.), 1, 0.25),
        ],
    )
    .unwrap();
    let context = FamilyCompositionContext::default();
    let blocked = Frame {
        blocked: true,
        ..Default::default()
    };
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    parent
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            angles(20., 30.),
            None,
        )
        .unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    let size = parent
        .pending_graph_operation_locator(request.request_id)
        .unwrap()
        .unwrap();
    let mut replay = registry
        .branch()
        .begin_graph_operation_operand(
            &size,
            PositionGraphOperationOperand::SizeValue,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    let panicking = Frame {
        panic: true,
        ..Default::default()
    };
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| replay.advance(
            registry.capture_id(),
            &context,
            &panicking
        )))
        .is_err()
    );
    let resolving = Frame::default();
    assert!(
        replay
            .advance(registry.capture_id(), &context, &resolving)
            .is_err()
    );
    assert_eq!(
        resolving.calls.get(),
        0,
        "poisoned driver must not restart its child"
    );
    let mut fresh = registry
        .branch()
        .begin_graph_operation_operand(
            &size,
            PositionGraphOperationOperand::SizeValue,
            &context,
            replay.into_scratch(),
            true,
        )
        .unwrap();
    assert_eq!(
        ready(
            fresh
                .advance(registry.capture_id(), &context, &resolving)
                .unwrap()
        ),
        angles(20., 30.)
    );
    assert_eq!(resolving.calls.get(), 1);
    assert_eq!(
        ordinary(&registry, fresh.into_scratch()),
        ordinary(&registry, Default::default())
    );
    assert_eq!(
        needed(
            parent
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        )
        .request_id,
        request.request_id
    );
}

#[test]
fn actual_earlier_mask_prefix_pins_current_once_across_operand_suspension_and_retry() {
    let sample = sampled(1.);
    let mut lower_rank = sample.rank;
    lower_rank.priority = 8;
    let lower = whole(leaf(target(0, 5., 7.)), lower_rank, 1.);
    let earlier_mask =
        ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Position, None, angles(9., 11.))
            .unwrap()
            .compile(
                None,
                &FamilyEditContext::default(),
                FamilySampleRank {
                    priority: 9,
                    changed_at_millis: 100,
                    changed_at_submillis_nanos: 0,
                    stable_order: 1,
                    identity: FamilySampleIdentity::Fixed {
                        source: FamilyFixedSampleSource::Programmer,
                        row_index: 0,
                    },
                },
                0.5,
            )
            .unwrap()
            .into();
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0., 0.),
        &[
            lower,
            earlier_mask,
            whole(sample.expression, sample.rank, 0.5),
        ],
    )
    .unwrap();
    let current = Sources {
        current: angles(1., 3.),
        calls: Cell::new(0),
    };
    let control = |rank| {
        if rank == lower_rank {
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.5 }
        } else {
            FamilyEndpointOutputControl::Unchanged
        }
    };
    let context = FamilyCompositionContext {
        endpoint_output: Some(FamilyEndpointOutputContext {
            control: &control,
            target: sample.target,
            current: &current,
            native_models: None,
        }),
        ..Default::default()
    };
    let subject_blocked = Frame {
        block_to: Some(target(9, 30., 40.)),
        ..Default::default()
    };
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &subject_blocked)
            .unwrap(),
    );
    let locator = parent
        .pending_graph_operation_locator(request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(locator.kind(), PositionGraphOperationKind::Required);
    assert_eq!(
        current.calls.get(),
        1,
        "the actual lower Current envelope executes before the original Required"
    );
    let mut replay = registry
        .branch()
        .begin_graph_operation_operand(
            &locator,
            PositionGraphOperationOperand::RequiredIncoming,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        ready(
            replay
                .advance(registry.capture_id(), &context, &subject_blocked)
                .unwrap()
        ),
        target(9, 30., 40.)
    );
    assert_eq!(current.calls.get(), 2);
    for _ in 0..2 {
        assert_eq!(
            ready(
                replay
                    .advance(registry.capture_id(), &context, &subject_blocked)
                    .unwrap()
            ),
            target(9, 30., 40.)
        );
    }
    assert_eq!(
        current.calls.get(),
        2,
        "repeat Ready cannot resample Current or rerun the earlier mask"
    );
    let blocked = Frame {
        blocked: true,
        ..Default::default()
    };
    let mut pending = registry
        .branch()
        .begin_graph_operation_operand(
            &locator,
            PositionGraphOperationOperand::RequiredIncoming,
            &context,
            replay.into_scratch(),
            true,
        )
        .unwrap();
    let prefix = suspended(
        pending
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    assert_eq!(current.calls.get(), 3);
    assert!(
        matches!(&prefix.operation,PositionCompositionOperation::Base {request,..}
        if matches!(&request.operation,PositionCompositionBaseOperation::Completion(value) if value.stage==PositionCompletionStage::EndpointOutput)),
        "suspension is the earlier actual Current envelope, not the selected Required node"
    );
    assert!(
        pending
            .resume_materialization(
                registry.capture_id(),
                prefix.request_id,
                AttributeValue::Normalized(0.5),
                None
            )
            .is_err()
    );
    assert_eq!(
        suspended(
            pending
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        )
        .request_id,
        prefix.request_id
    );
    assert_eq!(current.calls.get(), 3);
    pending
        .resume_materialization(
            registry.capture_id(),
            prefix.request_id,
            angles(3., 5.),
            None,
        )
        .unwrap();
    assert_eq!(
        ready(
            pending
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        ),
        target(9, 30., 40.)
    );
    assert_eq!(
        current.calls.get(),
        3,
        "resume uses the captured prefix Current"
    );
    assert_eq!(
        needed(
            parent
                .advance(registry.capture_id(), &context, &subject_blocked)
                .unwrap()
        )
        .request_id,
        request.request_id
    );
}

#[test]
fn removed_outgoing_whole_required_is_inactive_after_outer_resume_selection() {
    let sample = sampled(1.);
    let source_lane = sample.expression.operation_provenance().unwrap().handles()[0].lane_id();
    let outer_resume = Uuid::new_v4();
    let nodes = vec![
        PositionForestNode::Whole {
            expression: Arc::clone(&sample.expression),
            lane_id: source_lane,
            sources: Arc::from([]),
        },
        PositionForestNode::Whole {
            expression: leaf(angles(80., 90.)),
            lane_id: Uuid::new_v4(),
            sources: Arc::from([]),
        },
        PositionForestNode::Transition {
            from: Some(0),
            to: Some(1),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: outer_resume,
            },
        },
    ];
    let source = FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::from_position_forest(Arc::from([]), &nodes, 2).unwrap(),
        ),
        rank: sample.rank,
        activation_mix: 1.,
    };
    let registry =
        CapturedPositionProgram::new(Uuid::new_v4(), &angles(0., 0.), &[source]).unwrap();
    let context = FamilyCompositionContext::default();
    let blocked = Frame {
        blocked: true,
        ..Default::default()
    };
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    assert!(
        matches!(&request.operation, PositionCompositionOperation::Base { request, .. }
        if matches!(&request.operation, PositionCompositionBaseOperation::Coupled(inner)
            if matches!(&inner.operation, PositionMaterializationOperation::Transition {
                reason: DynamicTransitionReason::Required { .. }, ..
            }))),
        "locator must originate from the genuine sampled Required in the coupled outgoing Whole"
    );
    let locator = parent
        .pending_graph_operation_locator(request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(locator.kind(), PositionGraphOperationKind::Required);
    assert!(matches!(
        registry.node_view(locator.operation_node()).unwrap(),
        PositionSourceNodeView::Transition {
            reason: DynamicTransitionReason::Required { .. },
            ..
        }
    ));
    let mut active = registry
        .branch()
        .begin_graph_operation_operand(
            &locator,
            PositionGraphOperationOperand::RequiredIncoming,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        ready(
            active
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        ),
        target(9, 30., 40.)
    );

    let identity = sample.rank.dynamic_identity().unwrap();
    let mut incoming_only = registry.branch();
    incoming_only
        .choose_resume(
            PositionResumeScope {
                instance_id: identity.instance_id,
                controller_id: identity.controller_id,
                occurrence_id: outer_resume,
            },
            PositionResumeEndpoint::Incoming,
        )
        .unwrap();
    let mut replay = incoming_only
        .begin_graph_operation_operand(
            &locator,
            PositionGraphOperationOperand::RequiredIncoming,
            &context,
            active.into_scratch(),
            true,
        )
        .unwrap();
    let calls = blocked.calls.get();
    for _ in 0..2 {
        assert!(
            matches!(
                replay
                    .advance(registry.capture_id(), &context, &blocked)
                    .unwrap(),
                PositionGraphOperationOperandProgress::Inactive
            ),
            "an original locator cannot revive a detached outgoing Whole node retained in compiled storage"
        );
    }
    assert_eq!(
        blocked.calls.get(),
        calls,
        "removed child cannot call the physical resolver"
    );
    let mut changed_parent = incoming_only
        .begin_composition(&context, replay.into_scratch(), true)
        .unwrap();
    assert!(
        matches!(changed_parent.advance(registry.capture_id(), &context, &blocked).unwrap(),
        PositionCompositionProgress::Complete(value) if value == angles(80., 90.))
    );
    assert_eq!(
        needed(
            parent
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        )
        .request_id,
        request.request_id,
        "branch replay preserves the original pending parent"
    );
    assert_eq!(
        ordinary(&registry, changed_parent.into_scratch()).0,
        angles(50., 60.),
        "original parent still evaluates the actual Required and its outer Resume once"
    );
}

mod graph_discovery_tests;
