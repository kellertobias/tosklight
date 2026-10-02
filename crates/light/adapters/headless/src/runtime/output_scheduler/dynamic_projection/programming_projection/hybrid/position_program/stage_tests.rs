//! Actual captured hybrid bridge tests; speculative operands never observe or accept output.
use super::*;
use light_core::{ManualClock, NativeColorIdentity, SessionId, programming::*};
use light_dynamics::*;
use light_programmer::ProgrammerRegistry;
use std::cell::Cell;
use uuid::Uuid;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target(x: f32, y: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [x, y, 0.],
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
fn source(
    value: DynamicValue,
    component: Option<ProgrammingComponent>,
    order: u128,
    mix: f32,
) -> FamilyCompositionSample {
    let family = match (value, component) {
        (DynamicValue::Family(family), _) => family,
        (DynamicValue::Scalar(value), Some(ProgrammingComponent::Pan)) => angles(value, 0.),
        (DynamicValue::Scalar(value), Some(ProgrammingComponent::Tilt)) => angles(0., value),
        _ => panic!("this fixture uses whole Position or scalar Pan/Tilt FixAT"),
    };
    ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Position, component, family)
        .unwrap()
        .compile(None, &FamilyEditContext::default(), rank(order), mix)
        .unwrap()
        .into()
}
fn sources() -> Vec<FamilyCompositionSample> {
    let expression = Arc::new(DynamicSampleExpression::Transition {
        from: Some(leaf(target(10., 20.))),
        to: Some(leaf(target(30., 40.))),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(5001),
        },
    });
    vec![
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
            rank: rank(1),
            activation_mix: 1.,
        },
        source(DynamicValue::Family(target(40., 60.)), None, 2, 0.5),
        source(
            DynamicValue::Scalar(90.),
            Some(ProgrammingComponent::Pan),
            3,
            0.5,
        ),
        source(
            DynamicValue::Scalar(80.),
            Some(ProgrammingComponent::Tilt),
            4,
            1.,
        ),
        source(DynamicValue::Family(angles(300., 400.)), None, 5, 0.5),
    ]
}
fn scope() -> PositionResumeScope {
    PositionResumeScope {
        instance_id: Uuid::from_u128(1001),
        controller_id: Uuid::from_u128(2001),
        occurrence_id: Uuid::from_u128(5001),
    }
}
struct NoModels;
impl DynamicNativeModelResolver for NoModels {
    fn resolve(
        &self,
        _: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        panic!("Position bridge does not resolve Color models")
    }
}
fn no_current_adoption(
    _: FixtureId,
    _: &AttributeValue,
    _: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    panic!("these bridge examples keep compatible original Current")
}
struct Frame {
    calls: Cell<usize>,
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
        Err(TransitionError::Requires(requirement))
    }
}
fn with_composer<R>(
    target: FixtureId,
    base: AttributeValue,
    samples: &[FamilyCompositionSample],
    action: impl FnOnce(
        &mut CapturedHybridProgramComposer<'_, '_, PreparedFamilySources<'_>>,
        &HybridCapturedPositionProgram,
        &Engine,
    ) -> R,
) -> R {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock);
    let session = SessionId::new();
    programmers.start(session);
    programmers.set(
        session,
        target,
        ProgrammingOwner::Position.key(),
        base.clone(),
    );
    let engine =
        Engine::with_programming_contract_support(programmers, PROGRAMMING_CONTRACT_VERSION);
    let capture = engine.prepare_output_frame(Default::default());
    let token = capture.frame_token();
    let mut scalar = engine.prepare_static_family_frame(&capture, &[]);
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .unwrap();
    let source = PreparedFamilySources(&scalar);
    let typed = CapturedProgrammingSources::new(&source, &no_current_adoption, None);
    let models = NoModels;
    let frame = HybridFrameContext {
        capture: &capture,
        geometry: &geometry,
        native_models: &models,
        token: &token,
        scalar: &scalar,
    };
    let group = DynamicFamilySampleGroup {
        target,
        owner: ProgrammingOwner::Position,
        samples: samples.to_vec(),
    };
    let control = |_: FamilySampleRank| FamilyEndpointOutputControl::Unchanged;
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let mut composer = CapturedHybridProgramComposer {
        typed: &typed,
        group: &group,
        frame,
        baseline: &scalar,
        control: &control,
        scratch: &mut scratch,
    };
    let program = HybridCapturedPositionProgram::new(&token, target, &base, samples).unwrap();
    action(&mut composer, &program, &engine)
}

fn blocked(_: &AttributeValue, _: &DynamicValueAddress) -> Result<AttributeValue, TransitionError> {
    Err(TransitionError::Requires(
        TransitionRequirement::LiveJointAngles,
    ))
}
fn pending(progress: PositionCompositionProgress) -> PositionCompositionRequest {
    match progress {
        PositionCompositionProgress::NeedsMaterialization(request) => request,
        _ => panic!("expected original pending operation"),
    }
}
fn stage_pending(progress: PositionStageOperandProgress) -> PositionCompositionRequest {
    match progress {
        PositionStageOperandProgress::NeedsMaterialization(request) => request,
        _ => panic!("expected nested operand dependency"),
    }
}
fn ready(progress: PositionStageOperandProgress) -> AttributeValue {
    match progress {
        PositionStageOperandProgress::OperandReady(value) => value,
        _ => panic!("expected operand"),
    }
}

#[test]
fn captured_stage_replays_changed_prefix_repeats_ready_and_never_observes_parent() {
    let logical = FixtureId::new();
    let destination = FixtureId::new();
    with_composer(
        logical,
        target(0., 0.),
        &sources(),
        |composer, program, _| {
            let frame = Frame {
                calls: Cell::new(0),
            };
            let mut parent = begin_branch(
                composer,
                program,
                &program.registry().branch(),
                destination,
                &blocked,
            )
            .unwrap();
            let request = pending(advance(composer, &mut parent, &frame, &blocked).unwrap());
            let locator = parent
                .pending_stage_locator(request.request_id)
                .unwrap()
                .unwrap();
            assert!(parent.pending_stage_locator(Uuid::new_v4()).is_err());
            let mut branch = program.registry().branch();
            branch
                .choose_resume(scope(), PositionResumeEndpoint::Incoming)
                .unwrap();
            let adopted = Cell::new(0);
            let available = |_: &AttributeValue, _: &DynamicValueAddress| {
                adopted.set(adopted.get() + 1);
                Ok(angles(35., 50.))
            };
            let mut operand = begin_stage_branch(
                composer,
                program,
                &branch,
                &locator,
                PositionStageOperand::AdoptionInput,
                destination,
                &available,
            )
            .unwrap();
            assert_eq!(operand.frame_token(), program.frame_token());
            assert_eq!(operand.target(), logical);
            assert_eq!(operand.destination(), destination);
            assert_eq!(
                operand.registry().capture_id(),
                program.registry().capture_id()
            );
            operand.registry().validate_branch(&branch).unwrap();
            for _ in 0..3 {
                assert_eq!(
                    ready(advance_stage(composer, &mut operand, &frame, &available).unwrap()),
                    target(35., 50.)
                );
                assert!(operand.pending_request().is_none());
            }
            assert_eq!(
                adopted.get(),
                0,
                "operand stops before even an available physical adoption"
            );
            assert_eq!(frame.calls.get(), 0);
            assert_eq!(
                parent.pending_request().unwrap().request_id,
                request.request_id
            );
            assert!(parent.completed_value().is_none());
            let observed = Cell::new(0);
            assert!(
                observe(composer, &mut parent, &mut |_| {
                    observed.set(observed.get() + 1);
                    Ok((
                        FamilyProjectionMetadata {
                            changed_at: None,
                            evidence: light_engine::FamilyProjectionEvidence::PreserveBaseline,
                            master: light_engine::FamilyProjectionMaster::PreserveBaseline,
                        },
                        (),
                    ))
                })
                .is_err()
            );
            assert_eq!(
                observed.get(),
                0,
                "speculative ready does not complete or accept the original output"
            );
            recycle_stage(composer, operand).unwrap();
            let mut original = begin_stage_branch(
                composer,
                program,
                &program.registry().branch(),
                &locator,
                PositionStageOperand::AdoptionInput,
                destination,
                &blocked,
            )
            .unwrap();
            assert_eq!(
                ready(advance_stage(composer, &mut original, &frame, &blocked).unwrap()),
                target(30., 45.),
                "returned scratch must not retain the child branch prefix"
            );
            recycle_stage(composer, original).unwrap();
            *composer.scratch = parent.into_scratch();
        },
    );
}

fn point_target(x: f32, y: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(91),
        },
        [x, y, 0.],
    )))
}
fn nested_sources() -> Vec<FamilyCompositionSample> {
    vec![
        FamilyCompositionSample::WholeExpression {
            expression: Arc::new(
                CompiledProgrammingFamilyExpression::new(
                    Arc::new(DynamicSampleExpression::Transition {
                        from: Some(leaf(target(10., 20.))),
                        to: Some(leaf(point_target(30., 40.))),
                        progress: 0.5,
                        reason: DynamicTransitionReason::Resume {
                            occurrence_id: Uuid::from_u128(5001),
                        },
                    }),
                    ProgrammingOwner::Position,
                    None,
                    None,
                )
                .unwrap(),
            ),
            rank: rank(1),
            activation_mix: 1.,
        },
        // This whole mask consumes the lower expression as its real underlay before the
        // later Angle component starts a new segment. Without it, that incompatible
        // lower Target expression is correctly excluded by component arbitration.
        source(DynamicValue::Family(point_target(40., 60.)), None, 2, 0.5),
        source(
            DynamicValue::Scalar(90.),
            Some(ProgrammingComponent::Pan),
            3,
            0.5,
        ),
    ]
}

#[test]
fn captured_stage_retains_nested_pending_and_retries_invalid_materialization() {
    with_composer(
        FixtureId::new(),
        target(0., 0.),
        &nested_sources(),
        |composer, program, _| {
            let frame = Frame {
                calls: Cell::new(0),
            };
            let destination = FixtureId::new();
            let mut parent = begin_branch(
                composer,
                program,
                &program.registry().branch(),
                destination,
                &blocked,
            )
            .unwrap();
            let prefix = pending(advance(composer, &mut parent, &frame, &blocked).unwrap());
            resume(
                composer,
                &mut parent,
                prefix.request_id,
                point_target(20., 30.),
                None,
            )
            .unwrap();
            let request = pending(advance(composer, &mut parent, &frame, &blocked).unwrap());
            let PositionCompositionOperation::Base {
                request: base_request,
                ..
            } = &request.operation
            else {
                panic!("later actual component stage must suspend");
            };
            let PositionCompositionBaseOperation::Segment(segment) = &base_request.operation else {
                panic!("locator must name the Pan segment, not the earlier expression or mask");
            };
            assert!(
                matches!(&segment.operation, PositionSegmentOperation::Adoption { from, address }
            if from == &point_target(30., 45.) && address.component == Some(ProgrammingComponent::Pan))
            );
            let locator = parent
                .pending_stage_locator(request.request_id)
                .unwrap()
                .expect("later Pan adoption has its own original stage");
            let mut operand = begin_stage_branch(
                composer,
                program,
                &program.registry().branch(),
                &locator,
                PositionStageOperand::AdoptionInput,
                destination,
                &blocked,
            )
            .unwrap();
            let nested =
                stage_pending(advance_stage(composer, &mut operand, &frame, &blocked).unwrap());
            assert!(
                matches!(&nested.operation, PositionCompositionOperation::Base { request, .. }
            if matches!(&request.operation, PositionCompositionBaseOperation::Whole(_))),
                "operand replay first needs the original whole expression's reference transition"
            );
            assert_eq!(nested.capture_id, program.registry().capture_id());
            assert_eq!(nested.origin().unwrap().original_source_indices(), &[0]);
            program
                .registry()
                .source_node_for_origin(nested.origin().unwrap())
                .unwrap();
            for _ in 0..3 {
                assert_eq!(
                    stage_pending(advance_stage(composer, &mut operand, &frame, &blocked).unwrap())
                        .request_id,
                    nested.request_id
                );
            }
            assert_eq!(
                frame.calls.get(),
                2,
                "one callback in the parent and one in the independent operand driver"
            );
            assert!(
                resume_stage(
                    composer,
                    &mut operand,
                    Uuid::new_v4(),
                    point_target(25., 35.),
                    None
                )
                .is_err()
            );
            assert!(
                resume_stage(
                    composer,
                    &mut operand,
                    nested.request_id,
                    AttributeValue::Normalized(0.4),
                    None
                )
                .is_err()
            );
            assert_eq!(
                operand.pending_request().unwrap().request_id,
                nested.request_id
            );
            assert_eq!(
                stage_pending(advance_stage(composer, &mut operand, &frame, &blocked).unwrap())
                    .request_id,
                nested.request_id
            );
            resume_stage(
                composer,
                &mut operand,
                nested.request_id,
                point_target(25., 35.),
                None,
            )
            .unwrap();
            assert!(operand.pending_request().is_none());
            for _ in 0..2 {
                assert_eq!(
                    ready(advance_stage(composer, &mut operand, &frame, &blocked).unwrap()),
                    point_target(32.5, 47.5),
                    "materialized prefix passes through the intervening half-weight whole mask once"
                );
            }
            assert_eq!(frame.calls.get(), 2);
            assert_eq!(
                parent.pending_request().unwrap().request_id,
                request.request_id
            );
            assert!(parent.completed_value().is_none());
            recycle_stage(composer, operand).unwrap();
            *composer.scratch = parent.into_scratch();
        },
    );
}

#[test]
fn captured_stage_rejects_foreign_registry_target_and_frame_without_poisoning_original() {
    let logical = FixtureId::new();
    let samples = sources();
    with_composer(
        logical,
        target(0., 0.),
        &samples,
        |composer, program, engine| {
            let frame = Frame {
                calls: Cell::new(0),
            };
            let destination = FixtureId::new();
            let mut parent = begin_branch(
                composer,
                program,
                &program.registry().branch(),
                destination,
                &blocked,
            )
            .unwrap();
            let request = pending(advance(composer, &mut parent, &frame, &blocked).unwrap());
            let locator = parent
                .pending_stage_locator(request.request_id)
                .unwrap()
                .unwrap();
            let foreign_registry = CapturedPositionProgram::new(
                program.registry().capture_id(),
                &target(0., 0.),
                &samples,
            )
            .unwrap();
            assert!(
                begin_stage_branch(
                    composer,
                    program,
                    &foreign_registry.branch(),
                    &locator,
                    PositionStageOperand::AdoptionInput,
                    destination,
                    &blocked
                )
                .is_err()
            );
            let mut foreign_program = program.clone();
            foreign_program.registry = foreign_registry;
            assert!(
                begin_stage_branch(
                    composer,
                    &foreign_program,
                    &foreign_program.registry().branch(),
                    &locator,
                    PositionStageOperand::AdoptionInput,
                    destination,
                    &blocked
                )
                .is_err(),
                "same capture UUID and values do not authorize a foreign locator registry"
            );
            let mut other_target = program.clone();
            other_target.target = FixtureId::new();
            assert!(
                begin_stage_branch(
                    composer,
                    &other_target,
                    &other_target.registry().branch(),
                    &locator,
                    PositionStageOperand::AdoptionInput,
                    destination,
                    &blocked
                )
                .is_err()
            );
            let foreign_capture = engine.prepare_output_frame(Default::default());
            assert_eq!(
                foreign_capture.frame_token().sampled_at(),
                program.frame_token().sampled_at()
            );
            let mut other_frame = program.clone();
            other_frame.token = foreign_capture.frame_token();
            assert!(
                begin_stage_branch(
                    composer,
                    &other_frame,
                    &other_frame.registry().branch(),
                    &locator,
                    PositionStageOperand::AdoptionInput,
                    destination,
                    &blocked
                )
                .is_err()
            );
            assert!(
                begin_stage_branch(
                    composer,
                    program,
                    &program.registry().branch(),
                    &locator,
                    PositionStageOperand::TransitionFrom,
                    destination,
                    &blocked
                )
                .is_err()
            );
            let mut operand = begin_stage_branch(
                composer,
                program,
                &program.registry().branch(),
                &locator,
                PositionStageOperand::AdoptionInput,
                destination,
                &blocked,
            )
            .unwrap();
            with_composer(
                logical,
                target(0., 0.),
                &samples,
                |foreign_composer, _, _| {
                    assert!(
                        advance_stage(foreign_composer, &mut operand, &frame, &blocked).is_err()
                    );
                    assert!(
                        resume_stage(
                            foreign_composer,
                            &mut operand,
                            Uuid::new_v4(),
                            angles(1., 2.),
                            None
                        )
                        .is_err()
                    );
                },
            );
            assert_eq!(
                operand.destination(),
                destination,
                "destination is retained metadata, not a physical-copy resolver authority"
            );
            assert_eq!(
                ready(advance_stage(composer, &mut operand, &frame, &blocked).unwrap()),
                target(30., 45.)
            );
            assert_eq!(frame.calls.get(), 0);
            assert_eq!(
                parent.pending_request().unwrap().request_id,
                request.request_id
            );
            recycle_stage(composer, operand).unwrap();
            *composer.scratch = parent.into_scratch();
        },
    );
}

#[test]
fn parent_source_freshness_failure_is_terminal_for_advance_resume_and_observe() {
    for fail_during_resume in [false, true] {
        with_composer(
            FixtureId::new(),
            target(0., 0.),
            &sources(),
            |composer, program, _| {
                let frame = Frame {
                    calls: Cell::new(0),
                };
                let mut parent = begin_branch(
                    composer,
                    program,
                    &program.registry().branch(),
                    program.target(),
                    &blocked,
                )
                .unwrap();
                let request = pending(advance(composer, &mut parent, &frame, &blocked).unwrap());
                assert!(
                    parent
                        .pending_stage_locator(request.request_id)
                        .unwrap()
                        .is_some()
                );
                *composer.typed.failure.borrow_mut() =
                    Some(IntentError("injected captured-source freshness failure".into()).into());
                if fail_during_resume {
                    assert!(
                        resume(
                            composer,
                            &mut parent,
                            request.request_id,
                            angles(30., 45.),
                            None
                        )
                        .is_err()
                    );
                } else {
                    assert!(advance(composer, &mut parent, &frame, &blocked).is_err());
                }
                assert!(parent.failed);
                assert!(parent.completed_value().is_none());
                assert!(parent.pending_stage_locator(request.request_id).is_err());
                // Clearing the source failure simulates a subsequent safe wrapper. It cannot
                // resurrect the driver that already crossed a failed captured-source boundary.
                *composer.typed.failure.borrow_mut() = None;
                assert!(advance(composer, &mut parent, &frame, &blocked).is_err());
                assert!(
                    resume(
                        composer,
                        &mut parent,
                        request.request_id,
                        angles(30., 45.),
                        None
                    )
                    .is_err()
                );
                assert!(parent.pending_stage_locator(request.request_id).is_err());
                assert!(
                    observe::<_, ()>(composer, &mut parent, &mut |_| {
                        panic!("a source-failed pending evaluation cannot loan an observation")
                    })
                    .is_err()
                );
                assert_eq!(frame.calls.get(), 0);
                *composer.scratch = parent.into_scratch();
            },
        );
    }
    with_composer(
        FixtureId::new(),
        angles(30., 45.),
        &[],
        |composer, program, _| {
            let frame = Frame {
                calls: Cell::new(0),
            };
            let mut parent = begin_branch(
                composer,
                program,
                &program.registry().branch(),
                program.target(),
                &blocked,
            )
            .unwrap();
            assert!(matches!(
                advance(composer, &mut parent, &frame, &blocked).unwrap(),
                PositionCompositionProgress::Complete(_)
            ));
            assert_eq!(parent.completed_value(), Some(&angles(30., 45.)));
            *composer.typed.failure.borrow_mut() =
                Some(IntentError("injected observation freshness failure".into()).into());
            assert!(
                observe::<_, ()>(composer, &mut parent, &mut |_| {
                    panic!("initial freshness failure must precede the observer")
                })
                .is_err()
            );
            assert!(parent.failed);
            assert!(
                parent.completed_value().is_none(),
                "the cached completed value loses publication authority"
            );
            *composer.typed.failure.borrow_mut() = None;
            assert!(parent.pending_stage_locator(Uuid::new_v4()).is_err());
            assert!(advance(composer, &mut parent, &frame, &blocked).is_err());
            assert!(
                observe::<_, ()>(composer, &mut parent, &mut |_| {
                    panic!("a healthy later source wrapper cannot reloan failed output")
                })
                .is_err()
            );
            assert_eq!(frame.calls.get(), 0);
            *composer.scratch = parent.into_scratch();
        },
    );
}

#[path = "stage_nested_bridge_tests.rs"]
mod stage_nested_bridge_tests;
