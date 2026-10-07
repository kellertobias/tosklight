use super::*;
use crate::{DynamicSampleExpression, DynamicTransitionReason};
use std::cell::Cell;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [4., 2., 3.],
    )))
}
fn rank(order: u128) -> FamilySampleRank {
    FamilySampleRank {
        priority: 10,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: order,
        identity: FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(1000 + order),
            controller_id: Uuid::from_u128(2000 + order),
            lane_id: Uuid::from_u128(3000 + order),
        },
    }
}
fn whole_leaf(value: AttributeValue) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn history() -> Arc<CompiledCoupledExpression> {
    // A successful Angle child precedes the mixed Target/Angle cut. Its arithmetic and
    // observer nodes must survive the outer compositor's heap-task suspension.
    let child = Arc::new(DynamicSampleExpression::Transition {
        from: Some(whole_leaf(angles(10., 20.))),
        to: Some(whole_leaf(angles(30., 40.))),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(90),
        },
    });
    Arc::new(
        CompiledCoupledExpression::new(
            Arc::new(DynamicSampleExpression::Transition {
                from: Some(whole_leaf(target())),
                to: Some(child),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: Uuid::from_u128(91),
                },
            }),
            None,
        )
        .unwrap(),
    )
}
fn sources() -> Vec<FamilyCompositionSample> {
    let lower = coupled::verified_endpoint(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Position, &angles(30., 40.))
                    .unwrap(),
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Family(angles(30., 40.)),
        rank(1),
        0.5,
    )
    .unwrap();
    let mut upper = coupled::verified_endpoint(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Angles,
                    component: Some(ProgrammingComponent::Tilt),
                },
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Scalar(22.),
        rank(3),
        0.5,
    )
    .unwrap();
    // FixAT keeps an explicitly partial component mask rather than creating a new Angle
    // Dynamic without its required Pan partner.
    upper.fix_at = true;
    vec![
        lower.into(),
        FamilyCompositionSample::CoupledExpression {
            expression: history(),
            rank: rank(2),
            activation_mix: 0.5,
        },
        upper.into(),
    ]
}
fn prepared(
    sources: &[FamilyCompositionSample],
) -> (RetainedFamilyCompositionScratch, BaseEvaluation) {
    let base = angles(0., 0.);
    let mut scratch = RetainedFamilyCompositionScratch::default();
    scratch.trace_enabled = true;
    scratch.sources = sources.to_vec();
    scratch.resolved.resize(sources.len(), None);
    let base_trace = scratch.trace.base();
    let evaluation =
        BaseEvaluation::begin(&base, Some(base_trace), &[0, 1, 2], &base, &mut scratch);
    (scratch, evaluation)
}
struct Frame {
    calls: Cell<usize>,
    available: bool,
    fatal: bool,
}
impl WholeFamilyExpressionFrameResolver for Frame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        if self.fatal {
            return Err(IntentError("fatal captured frame failure".into()).into());
        }
        if self.available {
            Ok(angles(70., 80.))
        } else {
            Err(TransitionError::Requires(requirement))
        }
    }
}
fn suspended(
    evaluation: &mut BaseEvaluation,
    scratch: &mut RetainedFamilyCompositionScratch,
    frame: &Frame,
) -> (usize, usize) {
    for _ in 0..128 {
        match evaluation
            .advance(&FamilyCompositionContext::default(), frame, scratch)
            .unwrap()
        {
            BaseEvaluationProgress::Pending => {}
            BaseEvaluationProgress::OperandReady(_) => {
                panic!("ordinary evaluator cannot yield operands")
            }
            BaseEvaluationProgress::NeedsMaterialization {
                source_index,
                request,
            } => {
                assert_eq!(request.requirement, TransitionRequirement::LiveJointAngles);
                return (source_index, request.node);
            }
            BaseEvaluationProgress::Complete(_) => panic!("mixed cut must suspend"),
        }
    }
    panic!("bounded driver did not reach the materialization cut")
}
fn finish(
    evaluation: &mut BaseEvaluation,
    scratch: &mut RetainedFamilyCompositionScratch,
    frame: &Frame,
) -> TracedValue {
    for _ in 0..128 {
        match evaluation
            .advance(&FamilyCompositionContext::default(), frame, scratch)
            .unwrap()
        {
            BaseEvaluationProgress::Pending => {}
            BaseEvaluationProgress::OperandReady(_) => {
                panic!("ordinary evaluator cannot yield operands")
            }
            BaseEvaluationProgress::Complete(value) => return value,
            BaseEvaluationProgress::NeedsMaterialization { .. } => panic!("unexpected second cut"),
        }
    }
    panic!("bounded driver did not complete")
}
fn observer_snapshot(
    evaluation: &BaseEvaluation,
) -> (usize, usize, Vec<Option<FamilyTraceNodeId>>) {
    let Some(BaseTask::Coupled { step, inputs, .. }) = evaluation.tasks.last() else {
        panic!("suspended coupled task must retain inputs")
    };
    assert!(
        inputs.underlay.is_some(),
        "partial activation lower prefix is prepared before graph evaluation"
    );
    (*step, inputs.values.len(), inputs.observer_nodes.clone())
}

#[test]
fn outer_yield_retains_candidate_lower_cache_observers_and_rejects_wrong_responses() {
    let sources = sources();
    let (mut scratch, mut evaluation) = prepared(&sources);
    let frame = Frame {
        calls: Cell::new(0),
        available: false,
        fatal: false,
    };
    let (source_index, node) = suspended(&mut evaluation, &mut scratch, &frame);
    assert_eq!(source_index, 1);
    assert_eq!(frame.calls.get(), 1);
    assert!(
        evaluation
            .tasks
            .iter()
            .any(|task| matches!(task, BaseTask::Candidate { .. })),
        "newer partial Tilt winner must retain its scan/candidate stack while lower Coupled source yields"
    );
    let lower = scratch.resolved[0]
        .as_ref()
        .expect("lower partial whole source already resolved")
        .clone();
    let observed = observer_snapshot(&evaluation);
    assert!(
        observed.2.iter().filter(|id| id.is_some()).count() >= 4,
        "completed Target and Angle children already have provenance"
    );
    for (wrong_source, wrong_node, value) in [
        (usize::MAX, node, angles(70., 80.)),
        (source_index, usize::MAX, angles(70., 80.)),
        (source_index, node, AttributeValue::Normalized(0.5)),
        (source_index, node, angles(f32::NAN, 0.)),
    ] {
        assert!(
            evaluation
                .resume_materialization(wrong_source, wrong_node, value)
                .is_err()
        );
        let (index, request) = evaluation.materialization.as_ref().unwrap();
        assert_eq!((*index, request.node), (source_index, node));
    }
    for _ in 0..4 {
        let BaseEvaluationProgress::NeedsMaterialization {
            source_index: index,
            request,
        } = evaluation
            .advance(&FamilyCompositionContext::default(), &frame, &mut scratch)
            .unwrap()
        else {
            panic!("pending request must remain stable")
        };
        assert_eq!((index, request.node), (source_index, node));
        assert_eq!(observer_snapshot(&evaluation), observed);
        assert_eq!(
            scratch.resolved[0].as_ref().unwrap().trace_node,
            lower.trace_node
        );
        assert_eq!(
            scratch.resolved[0].as_ref().unwrap().materialized_value(),
            lower.materialized_value()
        );
    }
    assert_eq!(
        frame.calls.get(),
        1,
        "waiting cannot repeat frame resolution"
    );
    evaluation
        .resume_materialization(source_index, node, angles(70., 80.))
        .unwrap();
    let result = finish(&mut evaluation, &mut scratch, &frame);
    assert_eq!(frame.calls.get(), 1);
    let ready = Frame {
        calls: Cell::new(0),
        available: true,
        fatal: false,
    };
    let mut synchronous = RetainedFamilyCompositionScratch::default();
    let expected = compose_retained_dynamic_family_traced(
        ProgrammingOwner::Position,
        &angles(0., 0.),
        &sources,
        &FamilyCompositionContext::default(),
        &ready,
        &mut synchronous,
    )
    .unwrap();
    assert_eq!(
        result.value, expected,
        "resumed rank composition equals uninterrupted composition"
    );
    for component in [ProgrammingComponent::Pan, ProgrammingComponent::Tilt] {
        assert_eq!(
            scratch
                .trace
                .sources_for_component(result.trace.unwrap(), component),
            synchronous
                .trace
                .sources_for_component(synchronous.trace.root().unwrap(), component),
            "successful children and partial winner retain the same provenance"
        );
    }
}

#[test]
fn completed_outer_evaluation_is_idempotent_and_cannot_accept_more_materialization() {
    let (mut scratch, mut evaluation) = prepared(&sources());
    let ready = Frame {
        calls: Cell::new(0),
        available: true,
        fatal: false,
    };
    let result = finish(&mut evaluation, &mut scratch, &ready);
    let calls = ready.calls.get();
    for _ in 0..4 {
        let BaseEvaluationProgress::Complete(repeated) = evaluation
            .advance(&FamilyCompositionContext::default(), &ready, &mut scratch)
            .unwrap()
        else {
            panic!("completed evaluator must remain complete")
        };
        assert_eq!(repeated.value, result.value);
        assert_eq!(repeated.trace, result.trace);
    }
    assert_eq!(ready.calls.get(), calls);
    assert!(
        evaluation
            .resume_materialization(1, 0, angles(1., 2.))
            .is_err()
    );
}

#[test]
fn fatal_outer_error_rejects_further_advance_without_reentering_the_frame() {
    let (mut scratch, mut evaluation) = prepared(&sources());
    let fatal = Frame {
        calls: Cell::new(0),
        available: false,
        fatal: true,
    };
    let mut failed = false;
    for _ in 0..128 {
        if evaluation
            .advance(&FamilyCompositionContext::default(), &fatal, &mut scratch)
            .is_err()
        {
            failed = true;
            break;
        }
    }
    assert!(failed);
    let calls = fatal.calls.get();
    assert_eq!(calls, 1);
    for _ in 0..3 {
        assert!(
            evaluation
                .advance(&FamilyCompositionContext::default(), &fatal, &mut scratch)
                .is_err()
        );
    }
    assert_eq!(fatal.calls.get(), calls);
}

fn source_cohort_history() -> Arc<CompiledCoupledExpression> {
    use crate::CoupledCohortEndpoint;
    use crate::programming::expression_coupled::PositionForestNode;
    let lower = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            Arc::new(DynamicSampleExpression::Transition {
                from: Some(whole_leaf(angles(10., 20.))),
                to: Some(whole_leaf(angles(30., 40.))),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: Uuid::from_u128(190),
                },
            }),
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    );
    let mixed = Arc::new(DynamicSampleExpression::Transition {
        from: Some(whole_leaf(target())),
        to: Some(whole_leaf(angles(50., 60.))),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(191),
        },
    });
    let deferred = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            Arc::new(DynamicSampleExpression::Transition {
                from: None,
                to: Some(mixed),
                progress: 0.5,
                reason: DynamicTransitionReason::Resume {
                    occurrence_id: Uuid::from_u128(192),
                },
            }),
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    );
    let endpoints: Arc<[CoupledCohortEndpoint]> = Arc::from([
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(300),
            expression: lower,
        },
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(400),
            expression: deferred,
        },
    ]);
    let forest = [
        PositionForestNode::SourceCohort(endpoints),
        PositionForestNode::Whole {
            expression: whole_leaf(angles(90., 100.)),
            lane_id: Uuid::from_u128(500),
            sources: Arc::from([]),
        },
        PositionForestNode::Transition {
            from: Some(0),
            to: Some(1),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(193),
            },
        },
    ];
    Arc::new(CompiledCoupledExpression::from_position_forest(Arc::from([]), &forest, 2).unwrap())
}

fn nested_snapshot(
    evaluation: &BaseEvaluation,
) -> (usize, usize, usize, Vec<Option<FamilyTraceNodeId>>) {
    let Some(BaseTask::Coupled { inputs, .. }) = evaluation.tasks.last() else {
        panic!("outer SourceCohort task must remain suspended")
    };
    let child = inputs
        .source_cohort
        .as_ref()
        .expect("nested evaluation is retained");
    let Some(BaseTask::EvaluateWhole { inputs: whole, .. }) = child.evaluation.tasks.last() else {
        panic!("nested deferred Whole graph must remain suspended")
    };
    assert!(
        whole.underlay.is_some(),
        "earlier source cohort sibling has supplied its eligible lower prefix"
    );
    assert!(
        child.scratch.resolved[0].is_some(),
        "completed earlier Whole source remains cached"
    );
    (
        &*child.scratch as *const RetainedFamilyCompositionScratch as usize,
        child.scratch.sources.len(),
        child.evaluation.tasks.len(),
        whole.observer_nodes.clone(),
    )
}

#[test]
fn deferred_source_cohort_whole_yield_preserves_nested_tasks_cache_and_rebases_once() {
    let mut sources = sources();
    sources[1] = FamilyCompositionSample::CoupledExpression {
        expression: source_cohort_history(),
        rank: rank(2),
        activation_mix: 0.5,
    };
    let (mut scratch, mut evaluation) = prepared(&sources);
    let unavailable = Frame {
        calls: Cell::new(0),
        available: false,
        fatal: false,
    };
    let (source_index, node) = suspended(&mut evaluation, &mut scratch, &unavailable);
    assert_eq!(source_index, 1);
    let (_, request) = evaluation.materialization.as_ref().unwrap();
    let BaseMaterializationOperation::SourceCohort {
        endpoint_index,
        child_source_index,
        request: nested,
        source_rank,
    } = &request.operation
    else {
        panic!("yield must be forwarded from actual nested source composition")
    };
    assert_eq!(*endpoint_index, 0);
    assert_eq!(*child_source_index, 1);
    assert_eq!(
        *source_rank,
        rank(2).with_dynamic_lane(Uuid::from_u128(400)).unwrap(),
        "nested request retains the deferred child lane source lineage"
    );
    assert!(matches!(
        nested.operation,
        BaseMaterializationOperation::Whole(_)
    ));
    let before = nested_snapshot(&evaluation);
    assert_eq!(before.1, 2);
    assert!(
        before.3.iter().filter(|entry| entry.is_some()).count() >= 3,
        "Whole underlay and completed mixed children retain their trace node mappings"
    );
    for (wrong_source, wrong_node, response) in [
        (source_index + 1, node, angles(70., 80.)),
        (source_index, node + 1000, angles(70., 80.)),
        (source_index, node, AttributeValue::Normalized(0.5)),
    ] {
        assert!(
            evaluation
                .resume_materialization(wrong_source, wrong_node, response)
                .is_err()
        );
        assert_eq!(nested_snapshot(&evaluation), before);
    }
    for _ in 0..5 {
        let BaseEvaluationProgress::NeedsMaterialization {
            source_index: index,
            request,
        } = evaluation
            .advance(
                &FamilyCompositionContext::default(),
                &unavailable,
                &mut scratch,
            )
            .unwrap()
        else {
            panic!("nested yield must stay pending")
        };
        assert_eq!((index, request.node), (source_index, node));
        assert_eq!(nested_snapshot(&evaluation), before);
    }
    assert_eq!(unavailable.calls.get(), 1);
    evaluation
        .resume_materialization(source_index, node, angles(70., 80.))
        .unwrap();
    let result = finish(&mut evaluation, &mut scratch, &unavailable);
    assert_eq!(
        unavailable.calls.get(),
        1,
        "completion cannot refit the already answered nested cut"
    );
    let recycled = scratch
        .source_cohort_scratch
        .as_ref()
        .expect("completed child buffers returned to parent");
    assert_eq!(
        &**recycled as *const RetainedFamilyCompositionScratch as usize,
        before.0
    );
    assert!(
        recycled.resolved.iter().all(Option::is_some),
        "both original whole siblings completed once"
    );
    let ready = Frame {
        calls: Cell::new(0),
        available: true,
        fatal: false,
    };
    let mut synchronous = RetainedFamilyCompositionScratch::default();
    let expected = compose_retained_dynamic_family_traced(
        ProgrammingOwner::Position,
        &angles(0., 0.),
        &sources,
        &FamilyCompositionContext::default(),
        &ready,
        &mut synchronous,
    )
    .unwrap();
    assert_eq!(result.value, expected);
    assert_eq!(ready.calls.get(), 1);
    for component in [ProgrammingComponent::Pan, ProgrammingComponent::Tilt] {
        assert_eq!(
            scratch
                .trace
                .sources_for_component(result.trace.unwrap(), component),
            synchronous
                .trace
                .sources_for_component(synchronous.trace.root().unwrap(), component)
        );
    }
    let trace = result.trace;
    assert!(
        matches!(evaluation.advance(&FamilyCompositionContext::default(), &unavailable, &mut scratch).unwrap(),
        BaseEvaluationProgress::Complete(TracedValue { trace: repeated, .. }) if repeated == trace),
        "completion must not append/rebase the child's trace a second time"
    );
}

#[test]
fn callback_unwind_restores_stack_ownership_and_makes_outer_evaluation_terminal() {
    struct PanicFrame(Cell<usize>);
    impl WholeFamilyExpressionFrameResolver for PanicFrame {
        fn resolve(
            &self,
            _: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            self.0.set(self.0.get() + 1);
            panic!("frame callback unwound");
        }
    }
    let (mut scratch, mut evaluation) = prepared(&sources());
    let frame = PanicFrame(Cell::new(0));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for _ in 0..128 {
            evaluation
                .advance(&FamilyCompositionContext::default(), &frame, &mut scratch)
                .unwrap();
        }
    }));
    assert!(caught.is_err());
    assert_eq!(frame.0.get(), 1);
    assert!(
        scratch.tasks.is_empty(),
        "temporary task loan must return on unwind"
    );
    assert!(
        !evaluation.tasks.is_empty(),
        "pending outer scan stack stays with its owner"
    );
    for _ in 0..3 {
        assert!(
            evaluation
                .advance(&FamilyCompositionContext::default(), &frame, &mut scratch)
                .is_err()
        );
        assert!(
            evaluation
                .resume_materialization(1, 0, angles(1., 2.))
                .is_err()
        );
    }
    assert_eq!(frame.0.get(), 1);
    evaluation.recycle(&mut scratch);
    assert!(scratch.tasks.is_empty());
}
