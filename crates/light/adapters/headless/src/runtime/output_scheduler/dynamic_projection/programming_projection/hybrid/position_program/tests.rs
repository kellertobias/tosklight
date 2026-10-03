use super::*;
use light_core::{ManualClock, NativeColorIdentity, SessionId, programming::*};
use light_dynamics::*;
use light_programmer::ProgrammerRegistry;
use std::cell::Cell;
use uuid::Uuid;

fn angles(pan: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, 20.)))
}
fn target() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [4., 2., 3.],
    )))
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
fn mixed_source() -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(
                Arc::new(DynamicSampleExpression::Transition {
                    from: Some(leaf(target())),
                    to: Some(leaf(angles(90.))),
                    progress: 0.5,
                    reason: DynamicTransitionReason::Resume {
                        occurrence_id: Uuid::from_u128(40),
                    },
                }),
                ProgrammingOwner::Position,
                None,
                None,
            )
            .unwrap(),
        ),
        rank: FamilySampleRank {
            priority: 10,
            changed_at_millis: 1000,
            changed_at_submillis_nanos: 0,
            stable_order: 1,
            identity: FamilySampleIdentity::Dynamic {
                instance_id: Uuid::from_u128(1),
                controller_id: Uuid::from_u128(2),
                lane_id: Uuid::from_u128(3),
            },
        },
        activation_mix: 1.,
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
fn no_adoption(
    _: &AttributeValue,
    _: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    panic!("whole sampled examples do not adopt a component underlay")
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
fn metadata() -> FamilyProjectionMetadata {
    FamilyProjectionMetadata {
        changed_at: None,
        evidence: light_engine::FamilyProjectionEvidence::PreserveBaseline,
        master: light_engine::FamilyProjectionMaster::PreserveBaseline,
    }
}
fn needed(progress: PositionCompositionProgress) -> PositionCompositionRequest {
    let PositionCompositionProgress::NeedsMaterialization(request) = progress else {
        panic!("expected retained pending operation")
    };
    request
}
fn with_composer<R>(
    target: FixtureId,
    base: AttributeValue,
    samples: &[FamilyCompositionSample],
    action: impl FnOnce(
        &mut dyn HybridProgramComposer<()>,
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
        static_rows: None,
    };
    let program = HybridCapturedPositionProgram::new(&token, target, &base, samples).unwrap();
    action(&mut composer, &program, &engine)
}

#[test]
fn owned_static_position_completes_before_one_observation_and_recycles() {
    let logical = FixtureId::new();
    let destination = FixtureId::new();
    with_composer(logical, angles(30.), &[], |composer, program, _| {
        assert!(!program.registry().capture_id().is_nil());
        let mut evaluation = composer
            .begin_position(program, destination, &no_adoption)
            .unwrap();
        assert_eq!(evaluation.target(), logical);
        assert_eq!(evaluation.destination(), destination);
        assert_eq!(evaluation.frame_token(), program.frame_token());
        let calls = Cell::new(0);
        let mut observe = |observation: HybridFamilyObservation<'_>| {
            calls.set(calls.get() + 1);
            assert_eq!(observation.target, logical);
            assert_eq!(observation.value, &angles(30.));
            let fields =
                ProgrammingFieldScope::for_value(ProgrammingOwner::Position, observation.value)?;
            let mut projection = DynamicFamilySourceProjection::default();
            observation.project_fields(&fields, &mut projection)?;
            observation.controls_for_fields(&fields);
            Ok((metadata(), ()))
        };
        assert!(
            composer
                .observe_position(&mut evaluation, &mut observe)
                .is_err()
        );
        assert_eq!(
            calls.get(),
            0,
            "pending state cannot loan a trace or publish a sidecar"
        );
        let frame = Frame {
            calls: Cell::new(0),
        };
        let PositionCompositionProgress::Complete(value) = composer
            .advance_position(&mut evaluation, &frame, &no_adoption)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(value, angles(30.));
        assert_eq!(evaluation.completed_value(), Some(&value));
        let row = composer
            .observe_position(&mut evaluation, &mut observe)
            .unwrap();
        assert_eq!(row.target, logical);
        assert_eq!(row.value, value);
        assert_eq!(calls.get(), 1);
        assert_eq!(frame.calls.get(), 0);
        assert!(
            composer
                .observe_position(&mut evaluation, &mut observe)
                .is_err()
        );
        assert!(
            composer
                .advance_position(&mut evaluation, &frame, &no_adoption)
                .is_err()
        );
        composer.recycle_position(evaluation);
        let mut reused = composer
            .begin_position(program, destination, &no_adoption)
            .unwrap();
        assert!(matches!(
            composer
                .advance_position(&mut reused, &frame, &no_adoption)
                .unwrap(),
            PositionCompositionProgress::Complete(_)
        ));
        composer.recycle_position(reused);
    });
}

#[test]
fn pending_original_registry_and_retryable_responses_do_not_replay_frame_callbacks() {
    let logical = FixtureId::new();
    with_composer(
        logical,
        angles(0.),
        &[mixed_source()],
        |composer, program, _| {
            let frame = Frame {
                calls: Cell::new(0),
            };
            let mut evaluation = composer
                .begin_position(program, FixtureId::new(), &no_adoption)
                .unwrap();
            let request = needed(
                composer
                    .advance_position(&mut evaluation, &frame, &no_adoption)
                    .unwrap(),
            );
            let origin = request.origin().unwrap();
            assert_eq!(origin.original_source_indices(), &[0]);
            let node = program
                .registry()
                .source_node_for_origin(origin)
                .unwrap()
                .unwrap();
            assert_eq!(
                program
                    .registry()
                    .resume_scope(&node)
                    .unwrap()
                    .unwrap()
                    .occurrence_id,
                Uuid::from_u128(40)
            );
            for _ in 0..3 {
                let repeated = needed(
                    composer
                        .advance_position(&mut evaluation, &frame, &no_adoption)
                        .unwrap(),
                );
                assert_eq!(repeated.request_id, request.request_id);
            }
            assert_eq!(frame.calls.get(), 1);
            assert!(
                composer
                    .resume_position(&mut evaluation, Uuid::new_v4(), angles(50.), None)
                    .is_err()
            );
            assert!(
                composer
                    .resume_position(
                        &mut evaluation,
                        request.request_id,
                        AttributeValue::Normalized(0.5),
                        None
                    )
                    .is_err()
            );
            assert_eq!(
                evaluation.pending_request().unwrap().request_id,
                request.request_id
            );
            composer
                .resume_position(&mut evaluation, request.request_id, angles(50.), None)
                .unwrap();
            let PositionCompositionProgress::Complete(value) = composer
                .advance_position(&mut evaluation, &frame, &no_adoption)
                .unwrap()
            else {
                panic!()
            };
            assert_eq!(value, angles(50.));
            assert_eq!(
                frame.calls.get(),
                1,
                "resumed complete children and callbacks remain retained"
            );
            composer
                .observe_position(&mut evaluation, &mut |_| Ok((metadata(), ())))
                .unwrap();
            assert!(
                composer
                    .resume_position(&mut evaluation, request.request_id, angles(60.), None)
                    .is_err()
            );
            composer.recycle_position(evaluation);
        },
    );
}

#[test]
fn equal_time_foreign_captures_and_other_logical_targets_cannot_begin_or_resume() {
    let logical = FixtureId::new();
    let samples = [mixed_source()];
    with_composer(
        logical,
        angles(0.),
        &samples,
        |composer, program, engine| {
            let foreign_capture = engine.prepare_output_frame(Default::default());
            let foreign = HybridCapturedPositionProgram::new(
                &foreign_capture.frame_token(),
                logical,
                &angles(0.),
                &samples,
            )
            .unwrap();
            assert_eq!(
                foreign.frame_token().sampled_at(),
                program.frame_token().sampled_at()
            );
            assert!(
                composer
                    .begin_position(&foreign, logical, &no_adoption)
                    .is_err()
            );
            let other = HybridCapturedPositionProgram::new(
                program.frame_token(),
                FixtureId::new(),
                &angles(0.),
                &samples,
            )
            .unwrap();
            assert!(
                composer
                    .begin_position(&other, logical, &no_adoption)
                    .is_err()
            );
            let frame = Frame {
                calls: Cell::new(0),
            };
            let mut foreign_evaluation = with_composer(
                logical,
                angles(0.),
                &samples,
                |foreign_composer, foreign_program, _| {
                    let mut evaluation = foreign_composer
                        .begin_position(foreign_program, logical, &no_adoption)
                        .unwrap();
                    foreign_composer
                        .advance_position(&mut evaluation, &frame, &no_adoption)
                        .unwrap();
                    evaluation
                },
            );
            let pending = foreign_evaluation.pending_request().unwrap().request_id;
            assert!(
                composer
                    .resume_position(&mut foreign_evaluation, pending, angles(50.), None)
                    .is_err()
            );
            assert!(
                composer
                    .advance_position(&mut foreign_evaluation, &frame, &no_adoption)
                    .is_err()
            );
            assert_eq!(
                foreign_evaluation.pending_request().unwrap().request_id,
                pending
            );
            assert_eq!(
                frame.calls.get(),
                1,
                "foreign operations never invoke this frame's callbacks"
            );
        },
    );
}

#[test]
fn failed_observer_is_terminal_and_never_repeats_its_side_effects() {
    with_composer(FixtureId::new(), angles(0.), &[], |composer, program, _| {
        let frame = Frame {
            calls: Cell::new(0),
        };
        let mut evaluation = composer
            .begin_position(program, program.target(), &no_adoption)
            .unwrap();
        composer
            .advance_position(&mut evaluation, &frame, &no_adoption)
            .unwrap();
        let calls = Cell::new(0);
        let mut fail = |_: HybridFamilyObservation<'_>| {
            calls.set(calls.get() + 1);
            Err(IntentError("injected observation failure".into()).into())
        };
        assert!(
            composer
                .observe_position(&mut evaluation, &mut fail)
                .is_err()
        );
        assert!(
            composer
                .observe_position(&mut evaluation, &mut fail)
                .is_err()
        );
        assert!(evaluation.completed_value().is_none());
        assert_eq!(calls.get(), 1);
        composer.recycle_position(evaluation);
    });
}

#[test]
fn conditioned_bridge_rejects_a_foreign_branch_even_with_the_same_capture_uuid() {
    let logical = FixtureId::new();
    let samples = [mixed_source()];
    with_composer(logical, angles(0.), &samples, |composer, program, _| {
        let foreign =
            CapturedPositionProgram::new(program.registry().capture_id(), &angles(0.), &samples)
                .unwrap();
        assert!(
            composer
                .begin_position_branch(program, &foreign.branch(), logical, &no_adoption)
                .is_err()
        );
        let mut branch = program.registry().branch();
        branch
            .choose_resume(
                PositionResumeScope {
                    instance_id: Uuid::from_u128(1),
                    controller_id: Uuid::from_u128(2),
                    occurrence_id: Uuid::from_u128(40),
                },
                PositionResumeEndpoint::Incoming,
            )
            .unwrap();
        let mut evaluation = composer
            .begin_position_branch(program, &branch, logical, &no_adoption)
            .unwrap();
        let frame = Frame {
            calls: Cell::new(0),
        };
        let PositionCompositionProgress::Complete(value) = composer
            .advance_position(&mut evaluation, &frame, &no_adoption)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(value, angles(90.));
        assert_eq!(frame.calls.get(), 0);
        composer.recycle_position(evaluation);
    });
}

#[test]
fn conditioned_bridge_keeps_original_slots_and_nested_pending_origin() {
    let logical = FixtureId::new();
    let FamilyCompositionSample::WholeExpression {
        expression: inner,
        rank,
        activation_mix,
    } = mixed_source()
    else {
        panic!()
    };
    let wrapped = FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(
                Arc::new(DynamicSampleExpression::Transition {
                    from: Some(leaf(angles(10.))),
                    to: Some(Arc::new(inner.expression().clone())),
                    progress: 0.5,
                    reason: DynamicTransitionReason::Resume {
                        occurrence_id: Uuid::from_u128(39),
                    },
                }),
                ProgrammingOwner::Position,
                None,
                None,
            )
            .unwrap(),
        ),
        rank,
        activation_mix,
    };
    let lower = FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(
                leaf(angles(20.)),
                ProgrammingOwner::Position,
                None,
                None,
            )
            .unwrap(),
        ),
        rank: FamilySampleRank {
            priority: 1,
            ..rank
        },
        activation_mix: 1.,
    };
    with_composer(
        logical,
        angles(0.),
        &[lower, wrapped],
        |composer, program, _| {
            let registry = program.registry();
            let PositionSourceNodeView::Transition {
                to: Some(original_inner),
                ..
            } = registry
                .node_view(&registry.source_root(1).unwrap().unwrap())
                .unwrap()
            else {
                panic!()
            };
            let mut branch = registry.branch();
            branch
                .choose_resume(
                    PositionResumeScope {
                        instance_id: Uuid::from_u128(1),
                        controller_id: Uuid::from_u128(2),
                        occurrence_id: Uuid::from_u128(39),
                    },
                    PositionResumeEndpoint::Incoming,
                )
                .unwrap();
            branch.replace_source(0, None).unwrap();
            let mut evaluation = composer
                .begin_position_branch(program, &branch, logical, &no_adoption)
                .unwrap();
            let frame = Frame {
                calls: Cell::new(0),
            };
            let request = needed(
                composer
                    .advance_position(&mut evaluation, &frame, &no_adoption)
                    .unwrap(),
            );
            let origin = request.origin().unwrap();
            assert_eq!(origin.original_source_indices(), &[1]);
            let actual = registry.source_node_for_origin(origin).unwrap().unwrap();
            assert!(
                actual == original_inner,
                "conditioned compilation retains original nested operation identity"
            );
            assert_eq!(
                registry
                    .resume_scope(&actual)
                    .unwrap()
                    .unwrap()
                    .occurrence_id,
                Uuid::from_u128(40)
            );
            composer
                .resume_position(&mut evaluation, request.request_id, angles(50.), None)
                .unwrap();
            assert!(
                matches!(composer.advance_position(&mut evaluation, &frame, &no_adoption).unwrap(), PositionCompositionProgress::Complete(value) if value == angles(50.))
            );
            assert_eq!(frame.calls.get(), 1);
            composer.recycle_position(evaluation);
        },
    );
}
