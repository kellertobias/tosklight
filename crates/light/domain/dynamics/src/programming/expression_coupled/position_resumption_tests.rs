//! Position-only continuation contracts; these do not claim outer cohort scheduling exists.
use super::*;
use std::cell::{Cell, RefCell};
use uuid::Uuid;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target(x: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [x, 2., 3.],
    )))
}
fn whole(value: AttributeValue) -> Arc<DynamicSampleExpression> {
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
    from: Arc<DynamicSampleExpression>,
    to: Arc<DynamicSampleExpression>,
    occurrence: u128,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from: Some(from),
        to: Some(to),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(occurrence),
        },
    })
}

struct NoUnderlay;
impl CoupledExpressionContext for NoUnderlay {
    fn materialize_base(
        &self,
        _: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("complete leaf should not request another underlay");
    }
    fn orthogonal_underlay(
        &self,
        _: ColorComponent,
        _: &AttributeValue,
    ) -> Result<f32, TransitionError> {
        panic!("Position has no Color orthogonal");
    }
}
#[derive(Default)]
struct Unavailable {
    operations: Cell<usize>,
    adoptions: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for Unavailable {
    fn adopt_position_angles(&self, _: &AttributeValue) -> Result<AttributeValue, TransitionError> {
        self.adoptions.set(self.adoptions.get() + 1);
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    }
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.operations.set(self.operations.get() + 1);
        Err(TransitionError::Requires(requirement))
    }
}
#[derive(Default)]
struct Observed {
    nodes: Vec<usize>,
    angle_sources: Vec<CoupledComponentEndpoint>,
    source_cohort_lanes: Vec<Uuid>,
}
impl CoupledEvaluationObserver for Observed {
    fn evaluated(
        &mut self,
        node: usize,
        step: CoupledEvaluationStep<'_>,
        _: &AttributeValue,
    ) -> Result<(), TransitionError> {
        assert!(
            !self.nodes.contains(&node),
            "completed graph nodes must never be observed twice"
        );
        self.nodes.push(node);
        match step {
            CoupledEvaluationStep::AnglePair { sources } => self.angle_sources = sources.to_vec(),
            CoupledEvaluationStep::SourceCohort { sources } => {
                self.source_cohort_lanes =
                    sources.iter().map(CoupledCohortEndpoint::lane_id).collect();
            }
            _ => {}
        }
        Ok(())
    }
}
fn needed(progress: PositionEvaluationProgress) -> PositionMaterializationRequest {
    let PositionEvaluationProgress::NeedsMaterialization(request) = progress else {
        panic!("expected yield");
    };
    request
}
fn complete(progress: PositionEvaluationProgress) -> AttributeValue {
    let PositionEvaluationProgress::Complete(value) = progress else {
        panic!("expected completion");
    };
    value
}

#[test]
fn yield_preserves_completed_children_original_resume_reason_and_rejects_invalid_responses() {
    let child = transition(whole(angles(10., 0.)), whole(angles(20., 0.)), 11);
    let mixed = transition(whole(target(4.)), child, 22);
    let expression = Arc::new(
        CompiledCoupledExpression::new(transition(mixed, whole(angles(60., 0.)), 33), None)
            .unwrap(),
    );
    let mut evaluation = expression.begin_position_evaluation().unwrap();
    let frame = Unavailable::default();
    let mut observer = Observed::default();
    let request = needed(
        evaluation
            .advance(&NoUnderlay, &frame, Some(&mut observer))
            .unwrap(),
    );
    assert_eq!(frame.operations.get(), 1);
    let completed_nodes = observer.nodes.clone();
    assert_eq!(
        completed_nodes.len(),
        4,
        "Target and three completed Angle subtree nodes are retained"
    );
    let PositionMaterializationOperation::Transition {
        from,
        to,
        progress,
        reason,
        from_node,
        to_node,
    } = &request.operation
    else {
        panic!("expected transition materialization");
    };
    assert_eq!(*from, target(4.));
    assert_eq!(*to, angles(15., 0.));
    assert_eq!(*progress, 0.5);
    assert_eq!(
        *reason,
        DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(22)
        }
    );
    assert_eq!(expression.transition_reason(request.node), Some(*reason));
    assert!(completed_nodes.contains(from_node));
    assert!(completed_nodes.contains(to_node));
    assert_eq!(
        needed(
            evaluation
                .advance(&NoUnderlay, &frame, Some(&mut observer))
                .unwrap()
        )
        .node,
        request.node
    );
    assert_eq!(
        frame.operations.get(),
        1,
        "waiting cannot call the resolver again"
    );
    assert_eq!(observer.nodes, completed_nodes);

    assert!(
        evaluation
            .resume_materialization(usize::MAX, angles(40., 0.))
            .is_err()
    );
    assert!(
        evaluation
            .resume_materialization(request.node, AttributeValue::Normalized(0.5))
            .is_err()
    );
    assert!(
        evaluation
            .resume_materialization(request.node, angles(f32::NAN, 0.))
            .is_err()
    );
    assert_eq!(
        evaluation.pending_materialization().unwrap().node,
        request.node,
        "wrong node, wrong family and nonfinite replies leave the exact pending request intact"
    );
    evaluation
        .resume_materialization(request.node, angles(40., 0.))
        .unwrap();
    assert!(evaluation.pending_materialization().is_none());
    let value = complete(
        evaluation
            .advance(&NoUnderlay, &frame, Some(&mut observer))
            .unwrap(),
    );
    assert_eq!(value, angles(50., 0.));
    assert_eq!(frame.operations.get(), 1);
    assert_eq!(
        &observer.nodes[..completed_nodes.len()],
        completed_nodes.as_slice()
    );
    let observed_count = observer.nodes.len();
    assert_eq!(
        complete(
            evaluation
                .advance(&NoUnderlay, &frame, Some(&mut observer))
                .unwrap()
        ),
        value
    );
    assert_eq!(
        observer.nodes.len(),
        observed_count,
        "completed continuation is observationally stable"
    );

    struct Ready;
    impl WholeFamilyExpressionFrameResolver for Ready {
        fn resolve(
            &self,
            _: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            Ok(angles(40., 0.))
        }
    }
    assert_eq!(
        expression.evaluate_base(&NoUnderlay, &Ready).unwrap(),
        Some(value),
        "the existing evaluator and continuation share the same successful transition arithmetic"
    );
}

#[test]
fn angle_current_resume_uses_original_pair_arithmetic_and_preserves_current_source_role() {
    let original = Arc::new(CapturedPositionCurrent {
        value: target(4.),
        occurrence: None,
    });
    let pan_lane = Uuid::from_u128(100);
    let tilt_lane = Uuid::from_u128(200);
    let address = |component| {
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Angles,
                    component: Some(component),
                },
                None,
            )
            .unwrap(),
        )
    };
    let pair = Arc::new(PositionAnglePairEndpoint {
        axes: [
            PositionAngleAxis::Materialized(CoupledComponentEndpoint {
                lane_id: pan_lane,
                address: address(ProgrammingComponent::Pan),
                value: DynamicValue::Scalar(120.),
                role: CoupledLeafRole::Authored,
                occurrence: None,
                dependency_occurrence: None,
            }),
            PositionAngleAxis::Current {
                lane_id: tilt_lane,
                address: address(ProgrammingComponent::Tilt),
                original: original.clone(),
            },
        ],
    });
    let expression = Arc::new(
        CompiledCoupledExpression::from_position_forest(
            Arc::from([]),
            &[PositionForestNode::AnglePair(pair)],
            0,
        )
        .unwrap(),
    );
    let mut evaluation = expression.begin_position_evaluation().unwrap();
    let frame = Unavailable::default();
    let mut observer = Observed::default();
    let request = needed(
        evaluation
            .advance(&NoUnderlay, &frame, Some(&mut observer))
            .unwrap(),
    );
    let PositionMaterializationOperation::AngleCurrent {
        original: captured,
        original_occurrence,
    } = &request.operation
    else {
        panic!("expected Current adoption");
    };
    assert_eq!(*captured, original.value);
    assert_eq!(*original_occurrence, original.occurrence);
    assert_eq!(frame.adoptions.get(), 1);
    assert!(observer.nodes.is_empty());
    assert!(
        evaluation
            .resume_materialization(request.node, target(90.))
            .is_err()
    );
    assert!(evaluation.pending_materialization().is_some());
    evaluation
        .resume_materialization(request.node, angles(75., -5.))
        .unwrap();
    assert_eq!(
        complete(
            evaluation
                .advance(&NoUnderlay, &frame, Some(&mut observer))
                .unwrap()
        ),
        angles(120., -5.)
    );
    assert_eq!(
        frame.adoptions.get(),
        1,
        "adopted response is reused without a second physical callback"
    );
    assert_eq!(observer.angle_sources.len(), 2);
    assert_eq!(observer.angle_sources[0].lane_id, pan_lane);
    assert_eq!(observer.angle_sources[0].role, CoupledLeafRole::Authored);
    assert_eq!(observer.angle_sources[1].lane_id, tilt_lane);
    assert_eq!(observer.angle_sources[1].role, CoupledLeafRole::Current);
    assert!(observer.angle_sources[1].dependency_occurrence.is_some());
}

#[test]
fn source_cohort_handles_survive_two_yields_without_restarting_completed_context_work() {
    let source_lane = Uuid::from_u128(300);
    let whole_lane = Uuid::from_u128(400);
    let components = CoupledComponentEndpoint {
        lane_id: source_lane,
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
        value: DynamicValue::Scalar(5.),
        role: CoupledLeafRole::Authored,
        occurrence: None,
        dependency_occurrence: None,
    };
    let deferred = Arc::new(
        super::super::super::CompiledProgrammingFamilyExpression::new(
            whole(target(10.)),
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    );
    let sources: Arc<[CoupledCohortEndpoint]> = Arc::from([
        CoupledCohortEndpoint::Materialized(components),
        CoupledCohortEndpoint::WholeExpression {
            lane_id: whole_lane,
            expression: deferred.clone(),
        },
    ]);
    let forest = [
        PositionForestNode::SourceCohort(sources.clone()),
        PositionForestNode::Whole {
            expression: whole(angles(30., 0.)),
            lane_id: Uuid::from_u128(500),
            sources: Arc::from([]),
        },
        PositionForestNode::Transition {
            from: Some(0),
            to: Some(1),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(44),
            },
        },
    ];
    let expression = Arc::new(
        CompiledCoupledExpression::from_position_forest(Arc::from([]), &forest, 2).unwrap(),
    );
    struct CohortContext {
        calls: Cell<usize>,
        lanes: RefCell<Vec<Uuid>>,
    }
    impl CoupledExpressionContext for CohortContext {
        fn materialize_base(
            &self,
            _: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
        ) -> Result<AttributeValue, TransitionError> {
            panic!("SourceCohort has its own materialization boundary");
        }
        fn materialize_cohort_sources(
            &self,
            sources: &[CoupledCohortEndpoint],
        ) -> Result<AttributeValue, TransitionError> {
            self.calls.set(self.calls.get() + 1);
            self.lanes
                .replace(sources.iter().map(CoupledCohortEndpoint::lane_id).collect());
            Err(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            ))
        }
        fn orthogonal_underlay(
            &self,
            _: ColorComponent,
            _: &AttributeValue,
        ) -> Result<f32, TransitionError> {
            unreachable!()
        }
    }
    let context = CohortContext {
        calls: Cell::new(0),
        lanes: RefCell::new(vec![]),
    };
    let frame = Unavailable::default();
    let mut observer = Observed::default();
    let mut evaluation = expression.begin_position_evaluation().unwrap();
    let first = needed(
        evaluation
            .advance(&context, &frame, Some(&mut observer))
            .unwrap(),
    );
    let PositionMaterializationOperation::Cohort(CoupledBaseEndpoint::Sources(retained)) =
        &first.operation
    else {
        panic!("expected original SourceCohort");
    };
    assert!(Arc::ptr_eq(retained, &sources));
    let CoupledCohortEndpoint::WholeExpression {
        expression: retained_whole,
        ..
    } = &retained[1]
    else {
        panic!();
    };
    assert!(Arc::ptr_eq(retained_whole, &deferred));
    evaluation
        .resume_materialization(first.node, target(20.))
        .unwrap();
    let second = needed(
        evaluation
            .advance(&context, &frame, Some(&mut observer))
            .unwrap(),
    );
    let PositionMaterializationOperation::Transition {
        from, to, reason, ..
    } = &second.operation
    else {
        panic!();
    };
    assert_eq!(*from, target(20.));
    assert_eq!(*to, angles(30., 0.));
    assert_eq!(
        *reason,
        DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(44)
        }
    );
    assert_eq!(context.calls.get(), 1);
    assert_eq!(observer.source_cohort_lanes, [source_lane, whole_lane]);
    evaluation
        .resume_materialization(second.node, angles(25., 0.))
        .unwrap();
    assert_eq!(
        complete(
            evaluation
                .advance(&context, &frame, Some(&mut observer))
                .unwrap()
        ),
        angles(25., 0.)
    );
    assert_eq!(context.calls.get(), 1);
    assert_eq!(frame.operations.get(), 1);
}

#[test]
fn size_yield_retains_original_baseline_factor_and_already_evaluated_child() {
    let base = target(2.);
    let source = angles(10., 4.);
    let expression = Arc::new(
        CompiledCoupledExpression::new(
            Arc::new(DynamicSampleExpression::Scale {
                // Size declares the evaluated child's address. Its captured baseline may retain
                // another Position representation and is converted only at evaluation time.
                address: Arc::new(
                    DynamicValueAddress::whole_family(ProgrammingOwner::Position, &source).unwrap(),
                ),
                base: DynamicValue::Family(base.clone()),
                value: whole(source.clone()),
                factor: 1.5,
                baseline_occurrence: None,
            }),
            None,
        )
        .unwrap(),
    );
    let frame = Unavailable::default();
    let mut observer = Observed::default();
    let mut evaluation = expression.begin_position_evaluation().unwrap();
    let request = needed(
        evaluation
            .advance(&NoUnderlay, &frame, Some(&mut observer))
            .unwrap(),
    );
    let PositionMaterializationOperation::Scale {
        base: retained,
        value_node,
        value,
        factor,
        baseline_occurrence,
    } = &request.operation
    else {
        panic!("expected Size request");
    };
    let (_, DynamicValue::Family(retained)) = retained.materialized().unwrap() else {
        panic!();
    };
    assert_eq!(*retained, base);
    assert_eq!(*value, source);
    assert_eq!(*factor, 1.5);
    assert_eq!(*baseline_occurrence, None);
    assert_eq!(observer.nodes, [*value_node]);
    assert_eq!(frame.operations.get(), 1);
    evaluation
        .resume_materialization(request.node, angles(30., 6.))
        .unwrap();
    assert_eq!(
        complete(
            evaluation
                .advance(&NoUnderlay, &frame, Some(&mut observer))
                .unwrap()
        ),
        angles(30., 6.)
    );
    assert_eq!(frame.operations.get(), 1);
    assert_eq!(observer.nodes.len(), 2);
}

#[test]
fn caught_frame_and_observer_unwinds_leave_position_evaluation_terminal() {
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
            panic!("Position frame callback unwound");
        }
    }
    struct PanicObserver(Cell<usize>);
    impl CoupledEvaluationObserver for PanicObserver {
        fn evaluated(
            &mut self,
            _: usize,
            _: CoupledEvaluationStep<'_>,
            _: &AttributeValue,
        ) -> Result<(), TransitionError> {
            self.0.set(self.0.get() + 1);
            panic!("Position observer unwound");
        }
    }
    let expression = Arc::new(
        CompiledCoupledExpression::new(
            transition(whole(target(4.)), whole(angles(10., 20.)), 701),
            None,
        )
        .unwrap(),
    );
    let mut evaluation = expression.begin_position_evaluation().unwrap();
    let frame = PanicFrame(Cell::new(0));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            evaluation.advance(&NoUnderlay, &frame, None).unwrap();
        }))
        .is_err()
    );
    assert!(evaluation.advance(&NoUnderlay, &frame, None).is_err());
    assert!(
        evaluation
            .resume_materialization(0, angles(1., 2.))
            .is_err()
    );
    assert_eq!(frame.0.get(), 1);
    let mut evaluation = expression.begin_position_evaluation().unwrap();
    let mut observer = PanicObserver(Cell::new(0));
    let unavailable = Unavailable::default();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            evaluation
                .advance(&NoUnderlay, &unavailable, Some(&mut observer))
                .unwrap();
        }))
        .is_err()
    );
    assert!(
        evaluation
            .advance(&NoUnderlay, &unavailable, Some(&mut observer))
            .is_err()
    );
    assert!(
        evaluation
            .resume_materialization(0, angles(1., 2.))
            .is_err()
    );
    assert_eq!(observer.0.get(), 1);
    assert_eq!(unavailable.operations.get(), 0);
}
