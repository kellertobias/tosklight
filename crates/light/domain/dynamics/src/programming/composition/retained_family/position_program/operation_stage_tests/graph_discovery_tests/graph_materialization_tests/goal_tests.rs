//! TL556 materialization inside Graph, Resume and coupled-forest goals. Separate from the
//! parent only for size; it uses the same genuine runtime fixtures.
use super::*;

#[test]
fn graph_operand_goal_materializes_its_dependency_ready_required_site() {
    // The Size value replay contains the dependency-ready Required site.
    let sample = sampled(1.5);
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0., 0.),
        &[whole(sample.expression, sample.rank, 1.)],
    )
    .unwrap();
    let context = FamilyCompositionContext::default();
    let frame = Frame::default();
    let report = discovered(&registry, &frame);
    let required_site = kind_locator(&report, PositionGraphOperationKind::Required);
    let size_site = kind_locator(&report, PositionGraphOperationKind::Size);
    let mut operand = registry
        .branch()
        .begin_graph_operation_operand(
            &size_site,
            PositionGraphOperationOperand::SizeValue,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    operand
        .enable_graph_materialization(&required_site)
        .unwrap();
    let request = suspended(
        operand
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert!(request.is_materialized_reached_site());
    assert_eq!(
        operand
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .unwrap(),
        required_site
    );
    operand
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            angles(21., 31.),
            None,
        )
        .unwrap();
    assert_eq!(
        ready(
            operand
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        angles(21., 31.),
        "the operand is the answered site, not its inline value"
    );
    assert_eq!(
        operand.graph_materialization_status().unwrap(),
        PositionGraphMaterializationStatus::Resumed
    );
}

#[test]
fn resume_goal_materializes_the_original_member_size_inside_its_outgoing_operand() {
    let context = FamilyCompositionContext::default();
    let hot: HotResume = hot_resume();
    let reads = hot.sources.reads.get();
    let before = hot.runtime.snapshot();
    let hot_registry = hot_registry(&hot, fixed(angles(100., 200.), 20, 2, 0.5));
    let (mut parent, pending, resume) =
        issue_resume(&hot_registry, &hot_registry.branch(), &context);
    let member_size = size_locator(&parent.graph_discovery_report().unwrap());
    let physical = NoPhysicalContext::default();
    let mut plain = hot_registry
        .branch()
        .begin_resume_operand(
            &resume,
            PositionResumeEndpoint::Outgoing,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    let inline = match plain
        .advance(hot_registry.capture_id(), &context, &physical)
        .unwrap()
    {
        PositionResumeOperandProgress::OperandReady(value) => value,
        _ => panic!("synchronous original outgoing operand"),
    };
    let mut armed = hot_registry
        .branch()
        .begin_resume_operand(
            &resume,
            PositionResumeEndpoint::Outgoing,
            &context,
            plain.into_scratch(),
            true,
        )
        .unwrap();
    armed.enable_graph_materialization(&member_size).unwrap();
    let PositionResumeOperandProgress::NeedsMaterialization(request) = armed
        .advance(hot_registry.capture_id(), &context, &physical)
        .unwrap()
    else {
        panic!("armed member Size must materialize")
    };
    assert!(request.is_materialized_reached_site());
    assert_eq!(request.requirement, TransitionRequirement::LiveJointAngles);
    assert_eq!(
        scale(&request),
        (hot.base.clone(), target(0, 20., 40.), 0.5)
    );
    assert_eq!(
        armed
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .unwrap(),
        member_size
    );
    armed
        .resume_materialization(
            hot_registry.capture_id(),
            request.request_id,
            target(0, 12., 24.),
            None,
        )
        .unwrap();
    let PositionResumeOperandProgress::OperandReady(value) = armed
        .advance(hot_registry.capture_id(), &context, &physical)
        .unwrap()
    else {
        panic!("answered member Size resumes the original outgoing goal")
    };
    assert_eq!(
        value, inline,
        "answering the inline result reproduces the original goal exactly"
    );
    assert_eq!(
        armed.graph_materialization_status().unwrap(),
        PositionGraphMaterializationStatus::Resumed
    );
    assert_eq!(hot.sources.reads.get(), reads);
    assert_eq!(hot.runtime.snapshot(), before);
    assert_eq!(
        needed(
            parent
                .advance(hot_registry.capture_id(), &context, &physical)
                .unwrap()
        )
        .request_id,
        pending.request_id
    );
}

#[test]
fn coupled_forest_size_materializes_through_the_coupled_evaluator() {
    // The genuine sampled whole-Angles Size is imported into a Position forest, so its Size is
    // a coupled Scale node. The outer Resume transition is compositor unit assembly only.
    let fixture = size_fixture();
    let sample = fixture.sample(0);
    let nodes = vec![
        PositionForestNode::Whole {
            expression: Arc::new(sample.expression.clone()),
            lane_id: sample.lane_id,
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
                occurrence_id: Uuid::new_v4(),
            },
        },
    ];
    let source = FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::from_position_forest(Arc::from([]), &nodes, 2).unwrap(),
        ),
        rank: sample_rank(sample),
        activation_mix: 1.,
    };
    let registry =
        CapturedPositionProgram::new(Uuid::new_v4(), &fixture.bases[0], &[source]).unwrap();
    let context = FamilyCompositionContext::default();
    let frame = NoPhysicalContext::default();
    let mut plain = begin(&registry);
    plain.enable_graph_discovery(16).unwrap();
    assert_eq!(
        completed(
            plain
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        angles(46., 57.)
    );
    let site = size_locator(&plain.graph_discovery_report().unwrap());
    let mut driver = begin(&registry);
    driver.enable_graph_materialization(&site).unwrap();
    let request = needed(
        driver
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert!(request.is_materialized_reached_site());
    assert_eq!(request.requirement, TransitionRequirement::LiveJointAngles);
    let PositionCompositionOperation::Base { request: base, .. } = &request.operation else {
        panic!("base request")
    };
    assert!(
        matches!(
            &base.operation,
            PositionCompositionBaseOperation::Coupled(_)
        ),
        "the coupled evaluator issued the original Scale"
    );
    assert_eq!(scale(&request), (angles(4., 8.), angles(20., 40.), 0.5));
    assert_eq!(
        driver
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .unwrap(),
        site
    );
    driver
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            angles(14., 30.),
            None,
        )
        .unwrap();
    assert_eq!(
        completed(
            driver
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        angles(47., 60.),
        "the original Resume suffix applies once to the fitted Size result"
    );
    assert_eq!(status(&driver), PositionGraphMaterializationStatus::Resumed);
    assert_eq!(frame.calls.get(), 0);
    assert_eq!(fixture.sources.reads.get(), 2);
}
