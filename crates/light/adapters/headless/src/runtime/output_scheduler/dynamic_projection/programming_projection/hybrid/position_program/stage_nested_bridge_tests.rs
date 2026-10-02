//! Nested original operand authority through real captured compositions. These examples
//! prove request identity, retry and scratch lifecycle; they do not establish physical fitting.
use super::*;

fn required_size_source() -> FamilyCompositionSample {
    let baseline = target(4., 8.);
    let required = Arc::new(DynamicSampleExpression::Transition {
        from: Some(leaf(target(10., 20.))),
        to: Some(leaf(point_target(30., 40.))),
        progress: 0.5,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::LiveJointAngles,
        },
    });
    FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(
                Arc::new(DynamicSampleExpression::Scale {
                    address: Arc::new(DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::Target { reference: None },
                        component: None,
                    }),
                    base: DynamicValue::Family(baseline),
                    value: required,
                    factor: 1.5,
                    baseline_occurrence: None,
                }),
                ProgrammingOwner::Position,
                None,
                None,
            )
            .unwrap(),
        ),
        rank: rank(1),
        activation_mix: 1.,
    }
}
fn graph_pending(progress: PositionGraphOperationOperandProgress) -> PositionCompositionRequest {
    match progress {
        PositionGraphOperationOperandProgress::NeedsMaterialization(request) => request,
        _ => panic!("original graph prefix must suspend"),
    }
}
fn all_stage_locators_reject(driver: &HybridPositionStageEvaluation, request_id: Uuid) {
    assert!(driver.pending_graph_operation_locator(request_id).is_err());
    assert!(driver.pending_stage_locator(request_id).is_err());
    assert!(driver.pending_envelope_locator(request_id).is_err());
    assert!(driver.pending_mask_locator(request_id).is_err());
}
fn all_graph_locators_reject(driver: &HybridPositionGraphEvaluation, request_id: Uuid) {
    assert!(driver.pending_graph_operation_locator(request_id).is_err());
    assert!(driver.pending_stage_locator(request_id).is_err());
    assert!(driver.pending_envelope_locator(request_id).is_err());
    assert!(driver.pending_mask_locator(request_id).is_err());
}

#[test]
fn captured_stage_and_graph_prefixes_issue_their_own_nested_graph_authority() {
    let samples = vec![
        required_size_source(),
        source(DynamicValue::Family(point_target(40., 60.)), None, 2, 0.5),
        source(
            DynamicValue::Scalar(90.),
            Some(ProgrammingComponent::Pan),
            3,
            0.5,
        ),
    ];
    with_composer(
        FixtureId::new(),
        target(0., 0.),
        &samples,
        |composer, program, _| {
            let frame = Frame {
                calls: Cell::new(0),
            };
            let destination = FixtureId::new();
            let branch = program.registry().branch();
            let mut parent =
                begin_branch(composer, program, &branch, destination, &blocked).unwrap();
            let required_request =
                pending(advance(composer, &mut parent, &frame, &blocked).unwrap());
            let required = parent
                .pending_graph_operation_locator(required_request.request_id)
                .unwrap()
                .expect("actual original Required operation");
            assert_eq!(required.kind(), PositionGraphOperationKind::Required);
            resume(
                composer,
                &mut parent,
                required_request.request_id,
                point_target(20., 30.),
                None,
            )
            .unwrap();
            let size_request = pending(advance(composer, &mut parent, &frame, &blocked).unwrap());
            let size = parent
                .pending_graph_operation_locator(size_request.request_id)
                .unwrap()
                .expect("actual original Size operation");
            assert_eq!(size.kind(), PositionGraphOperationKind::Size);
            resume(
                composer,
                &mut parent,
                size_request.request_id,
                point_target(25., 35.),
                None,
            )
            .unwrap();
            let outer_request = pending(advance(composer, &mut parent, &frame, &blocked).unwrap());
            let outer = parent
                .pending_stage_locator(outer_request.request_id)
                .unwrap()
                .expect("later original Pan adoption stage");

            let mut stage = begin_stage_branch(
                composer,
                program,
                &branch,
                &outer,
                PositionStageOperand::AdoptionInput,
                destination,
                &blocked,
            )
            .unwrap();
            let nested =
                stage_pending(advance_stage(composer, &mut stage, &frame, &blocked).unwrap());
            assert_eq!(nested.capture_id, program.registry().capture_id());
            let nested_required = stage
                .pending_graph_operation_locator(nested.request_id)
                .unwrap()
                .expect("stage prefix retains original Required authority");
            assert_eq!(nested_required, required);
            assert!(
                stage
                    .pending_stage_locator(nested.request_id)
                    .unwrap()
                    .is_none()
            );
            assert!(
                stage
                    .pending_envelope_locator(nested.request_id)
                    .unwrap()
                    .is_none()
            );
            assert!(
                stage
                    .pending_mask_locator(nested.request_id)
                    .unwrap()
                    .is_none()
            );
            all_stage_locators_reject(&stage, Uuid::new_v4());
            let calls = frame.calls.get();
            assert!(
                resume_stage(
                    composer,
                    &mut stage,
                    nested.request_id,
                    AttributeValue::Normalized(0.4),
                    None
                )
                .is_err()
            );
            assert_eq!(
                stage_pending(advance_stage(composer, &mut stage, &frame, &blocked).unwrap())
                    .request_id,
                nested.request_id
            );
            assert_eq!(
                stage
                    .pending_graph_operation_locator(nested.request_id)
                    .unwrap(),
                Some(required.clone())
            );
            assert_eq!(
                frame.calls.get(),
                calls,
                "locator queries and retries never re-evaluate prefix"
            );

            let mut graph = begin_graph_branch(
                composer,
                program,
                &branch,
                &size,
                PositionGraphOperationOperand::SizeValue,
                destination,
                &blocked,
            )
            .unwrap();
            let graph_dependency =
                graph_pending(advance_graph(composer, &mut graph, &frame, &blocked).unwrap());
            assert_eq!(
                graph
                    .pending_graph_operation_locator(graph_dependency.request_id)
                    .unwrap(),
                Some(required.clone())
            );
            assert!(
                graph
                    .pending_stage_locator(graph_dependency.request_id)
                    .unwrap()
                    .is_none()
            );
            assert!(
                graph
                    .pending_envelope_locator(graph_dependency.request_id)
                    .unwrap()
                    .is_none()
            );
            assert!(
                graph
                    .pending_mask_locator(graph_dependency.request_id)
                    .unwrap()
                    .is_none()
            );
            all_graph_locators_reject(&graph, nested.request_id);
            all_graph_locators_reject(&graph, Uuid::new_v4());
            assert!(
                resume_graph(composer, &mut graph, Uuid::new_v4(), angles(1., 2.), None).is_err()
            );
            assert_eq!(
                graph_pending(advance_graph(composer, &mut graph, &frame, &blocked).unwrap())
                    .request_id,
                graph_dependency.request_id
            );
            resume_graph(
                composer,
                &mut graph,
                graph_dependency.request_id,
                point_target(22., 32.),
                None,
            )
            .unwrap();
            match advance_graph(composer, &mut graph, &frame, &blocked).unwrap() {
                PositionGraphOperationOperandProgress::OperandReady(value) => {
                    assert_eq!(value, point_target(22., 32.));
                }
                _ => panic!("Size Value stops before its original arithmetic"),
            }
            all_graph_locators_reject(&graph, graph_dependency.request_id);
            *composer.scratch = graph.into_scratch();

            // Source failure revokes every kind of pending authority, even after recovery.
            *composer.typed.failure.borrow_mut() =
                Some(IntentError("injected source failure".into()).into());
            assert!(advance_stage(composer, &mut stage, &frame, &blocked).is_err());
            *composer.typed.failure.borrow_mut() = None;
            all_stage_locators_reject(&stage, nested.request_id);
            recycle_stage(composer, stage).unwrap();
            let mut graph = begin_graph_branch(
                composer,
                program,
                &branch,
                &size,
                PositionGraphOperationOperand::SizeValue,
                destination,
                &blocked,
            )
            .unwrap();
            let failed =
                graph_pending(advance_graph(composer, &mut graph, &frame, &blocked).unwrap());
            *composer.typed.failure.borrow_mut() =
                Some(IntentError("injected graph source failure".into()).into());
            assert!(advance_graph(composer, &mut graph, &frame, &blocked).is_err());
            *composer.typed.failure.borrow_mut() = None;
            all_graph_locators_reject(&graph, failed.request_id);
            *composer.scratch = graph.into_scratch();
            assert_eq!(
                parent.pending_request().unwrap().request_id,
                outer_request.request_id
            );
            assert!(
                parent.completed_value().is_none(),
                "no speculative child publishes the parent"
            );
            *composer.scratch = parent.into_scratch();
        },
    );
}

#[test]
fn captured_mask_operand_retains_a_different_lower_masks_original_authority() {
    let samples = vec![
        source(DynamicValue::Family(angles(20., 30.)), None, 1, 0.5),
        source(DynamicValue::Family(point_target(40., 60.)), None, 2, 0.5),
    ];
    with_composer(
        FixtureId::new(),
        target(0., 0.),
        &samples,
        |composer, program, _| {
            let frame = Frame {
                calls: Cell::new(0),
            };
            let destination = FixtureId::new();
            let branch = program.registry().branch();
            let mut parent =
                begin_branch(composer, program, &branch, destination, &blocked).unwrap();
            let lower_request = pending(advance(composer, &mut parent, &frame, &blocked).unwrap());
            let lower = parent
                .pending_mask_locator(lower_request.request_id)
                .unwrap()
                .expect("original lower Angle mask adoption");
            assert_eq!(lower.stage(), PositionMaskStage::Adoption);
            resume(
                composer,
                &mut parent,
                lower_request.request_id,
                angles(0., 0.),
                None,
            )
            .unwrap();
            let upper_request = pending(advance(composer, &mut parent, &frame, &blocked).unwrap());
            let upper = parent
                .pending_mask_locator(upper_request.request_id)
                .unwrap()
                .expect("original upper Target mask adoption");
            assert_ne!(
                lower, upper,
                "equal stages are still distinct original masks"
            );
            let mut operand = begin_mask_branch(
                composer,
                program,
                &branch,
                &upper,
                PositionMaskOperand::AdoptionInput,
                destination,
                &blocked,
            )
            .unwrap();
            let dependency =
                stage_pending(advance_stage(composer, &mut operand, &frame, &blocked).unwrap());
            assert_eq!(
                operand.pending_mask_locator(dependency.request_id).unwrap(),
                Some(lower)
            );
            assert!(
                operand
                    .pending_stage_locator(dependency.request_id)
                    .unwrap()
                    .is_none()
            );
            assert!(
                operand
                    .pending_envelope_locator(dependency.request_id)
                    .unwrap()
                    .is_none()
            );
            assert!(
                operand
                    .pending_graph_operation_locator(dependency.request_id)
                    .unwrap()
                    .is_none()
            );
            all_stage_locators_reject(&operand, upper_request.request_id);
            assert!(
                resume_stage(composer, &mut operand, Uuid::new_v4(), angles(0., 0.), None).is_err()
            );
            assert_eq!(
                stage_pending(advance_stage(composer, &mut operand, &frame, &blocked).unwrap())
                    .request_id,
                dependency.request_id
            );
            resume_stage(
                composer,
                &mut operand,
                dependency.request_id,
                angles(0., 0.),
                None,
            )
            .unwrap();
            assert_eq!(
                ready(advance_stage(composer, &mut operand, &frame, &blocked).unwrap()),
                angles(10., 15.),
                "only the original lower mask's activation has run"
            );
            all_stage_locators_reject(&operand, dependency.request_id);
            assert_eq!(
                parent.pending_request().unwrap().request_id,
                upper_request.request_id
            );
            assert_eq!(
                parent
                    .pending_mask_locator(upper_request.request_id)
                    .unwrap(),
                Some(upper)
            );
            assert!(parent.completed_value().is_none());
            recycle_stage(composer, operand).unwrap();
            *composer.scratch = parent.into_scratch();
        },
    );
}
