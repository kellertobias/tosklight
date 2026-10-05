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
fn sample(pan: f32) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(
                Arc::new(DynamicSampleExpression::Transition {
                    from: Some(leaf(target())),
                    to: Some(leaf(angles(pan))),
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
fn metadata() -> FamilyProjectionMetadata {
    FamilyProjectionMetadata {
        changed_at: None,
        evidence: light_engine::FamilyProjectionEvidence::PreserveBaseline,
    }
}
struct NoModels;
impl DynamicNativeModelResolver for NoModels {
    fn resolve(
        &self,
        _: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        panic!("Position service must not request a Color model")
    }
}
fn no_current(
    _: FixtureId,
    _: &AttributeValue,
    _: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    panic!("the captured service examples have no component Current")
}
fn no_adoption(
    _: &AttributeValue,
    _: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    panic!("whole Position examples need no component adoption")
}
struct Frame(Cell<usize>);
impl WholeFamilyExpressionFrameResolver for Frame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.0.set(self.0.get() + 1);
        Err(TransitionError::Requires(requirement))
    }
}
fn needed(progress: PositionCompositionProgress) -> PositionCompositionRequest {
    let PositionCompositionProgress::NeedsMaterialization(request) = progress else {
        panic!("expected pending mixed Position operation")
    };
    request
}

fn with_batch<R>(
    targets: [FixtureId; 2],
    samples: [Vec<FamilyCompositionSample>; 2],
    wrong_baseline: bool,
    action: impl FnOnce(
        &mut dyn HybridPositionBatchComposer<()>,
        &[HybridCapturedPositionProgram; 2],
        &[DynamicFamilySampleGroup; 2],
        &Engine,
    ) -> R,
) -> R {
    with_batch_workspaces(
        targets,
        samples,
        wrong_baseline,
        RetainedFamilyCompositionScratch::default(),
        Vec::new(),
        action,
    )
    .0
}

fn with_batch_workspaces<R>(
    targets: [FixtureId; 2],
    samples: [Vec<FamilyCompositionSample>; 2],
    wrong_baseline: bool,
    scratch: RetainedFamilyCompositionScratch,
    pool: Vec<RetainedFamilyCompositionScratch>,
    action: impl FnOnce(
        &mut dyn HybridPositionBatchComposer<()>,
        &[HybridCapturedPositionProgram; 2],
        &[DynamicFamilySampleGroup; 2],
        &Engine,
    ) -> R,
) -> (
    R,
    RetainedFamilyCompositionScratch,
    Vec<RetainedFamilyCompositionScratch>,
) {
    with_batch_control_workspaces(
        targets,
        samples,
        wrong_baseline,
        scratch,
        pool,
        &|_| FamilyEndpointOutputControl::Unchanged,
        action,
    )
}

fn with_batch_control_workspaces<R>(
    targets: [FixtureId; 2],
    samples: [Vec<FamilyCompositionSample>; 2],
    wrong_baseline: bool,
    scratch: RetainedFamilyCompositionScratch,
    pool: Vec<RetainedFamilyCompositionScratch>,
    control: &dyn Fn(FamilySampleRank) -> FamilyEndpointOutputControl,
    action: impl FnOnce(
        &mut dyn HybridPositionBatchComposer<()>,
        &[HybridCapturedPositionProgram; 2],
        &[DynamicFamilySampleGroup; 2],
        &Engine,
    ) -> R,
) -> (
    R,
    RetainedFamilyCompositionScratch,
    Vec<RetainedFamilyCompositionScratch>,
) {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock);
    let session = SessionId::new();
    programmers.start(session);
    for (index, id) in targets.iter().enumerate() {
        programmers.set(
            session,
            *id,
            ProgrammingOwner::Position.key(),
            angles(index as f32 * 10.),
        );
    }
    let engine =
        Engine::with_programming_contract_support(programmers, PROGRAMMING_CONTRACT_VERSION);
    let capture = engine.prepare_output_frame(Default::default());
    let token = capture.frame_token();
    let mut scalar = engine.prepare_static_family_frame(&capture, &[]);
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .unwrap();
    let foreign_capture = engine.prepare_output_frame(Default::default());
    let foreign_scalar = engine.prepare_static_family_frame(&foreign_capture, &[]);
    let source = PreparedFamilySources(&scalar);
    let typed = CapturedProgrammingSources::new(&source, &no_current, None);
    let models = NoModels;
    let frame = HybridFrameContext {
        capture: &capture,
        geometry: &geometry,
        native_models: &models,
        token: &token,
        scalar: &scalar,
    };
    let groups = std::array::from_fn(|index| DynamicFamilySampleGroup {
        target: targets[index],
        owner: ProgrammingOwner::Position,
        samples: samples[index].clone(),
    });
    let programs = std::array::from_fn(|index| {
        HybridCapturedPositionProgram::new(
            &token,
            targets[index],
            &angles(index as f32 * 10.),
            &samples[index],
        )
        .unwrap()
    });
    let mut batch = CapturedHybridPositionBatchComposer::new_with_pool(
        &typed,
        &[&groups[0], &groups[1]],
        frame,
        if wrong_baseline {
            &foreign_scalar
        } else {
            &scalar
        },
        control,
        scratch,
        pool,
    );
    let result = action(&mut batch, &programs, &groups, &engine);
    let (scratch, pool) = batch.into_workspaces();
    (result, scratch, pool)
}

#[test]
fn noninterpolating_gate_query_is_exact_and_preserves_unchanged_query() {
    for (control, unchanged, noninterpolating) in [
        (FamilyEndpointOutputControl::Unchanged, true, true),
        (FamilyEndpointOutputControl::Suppressed, false, true),
        (
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: 1.0 },
            false,
            true,
        ),
        (
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.999 },
            false,
            false,
        ),
        (
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.5 },
            false,
            false,
        ),
        (
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.0 },
            false,
            false,
        ),
        (
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: f32::NAN },
            false,
            false,
        ),
        (
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: 1.001 },
            false,
            false,
        ),
    ] {
        with_batch_control_workspaces(
            [FixtureId::new(), FixtureId::new()],
            [vec![sample(90.)], vec![]],
            false,
            RetainedFamilyCompositionScratch::default(),
            Vec::new(),
            &|_| control,
            |batch, programs, _, engine| {
                assert_eq!(batch.unchanged_controls(&programs[0]).unwrap(), unchanged);
                assert_eq!(
                    batch.noninterpolating_controls(&programs[0]).unwrap(),
                    noninterpolating
                );
                assert!(
                    batch.noninterpolating_controls(&programs[1]).unwrap(),
                    "an empty static peer introduces no gate"
                );
                let foreign_capture = engine.prepare_output_frame(Default::default());
                let foreign = HybridCapturedPositionProgram::new(
                    &foreign_capture.frame_token(),
                    programs[0].target(),
                    &angles(0.),
                    &[sample(90.)],
                )
                .unwrap();
                assert!(batch.noninterpolating_controls(&foreign).is_err());
                let foreign_target = HybridCapturedPositionProgram::new(
                    programs[0].frame_token(),
                    FixtureId::new(),
                    &angles(0.),
                    &[sample(90.)],
                )
                .unwrap();
                assert!(batch.noninterpolating_controls(&foreign_target).is_err());
            },
        );
    }
}

#[test]
fn repeated_frames_retain_the_multi_evaluation_pool_without_growing_empty_placeholders() {
    let targets = [FixtureId::new(), FixtureId::new()];
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let mut pool = Vec::new();
    let mut retained_count = None;
    let mut previous_token = None;
    for frame_number in 0..4 {
        let expected = [
            angles(40. + frame_number as f32),
            angles(70. + frame_number as f32),
        ];
        let (token, next_scratch, next_pool) = with_batch_workspaces(
            targets,
            [vec![sample(90.)], vec![sample(120.)]],
            false,
            scratch,
            pool,
            |batch, programs, _, _| {
                let token = programs[0].frame_token().clone();
                let frame = Frame(Cell::new(0));
                let mut evaluations = programs
                    .iter()
                    .map(|program| {
                        batch
                            .begin_branch(
                                program,
                                &program.registry().branch(),
                                program.target(),
                                &no_adoption,
                            )
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                let requests = evaluations
                    .iter_mut()
                    .map(|evaluation| {
                        needed(batch.advance(evaluation, &frame, &no_adoption).unwrap())
                    })
                    .collect::<Vec<_>>();
                assert!(
                    evaluations
                        .iter()
                        .all(|evaluation| evaluation.pending_request().is_some())
                );
                for (index, evaluation) in evaluations.iter_mut().enumerate().rev() {
                    batch
                        .resume(
                            evaluation,
                            requests[index].request_id,
                            expected[index].clone(),
                            None,
                        )
                        .unwrap();
                    assert!(
                        matches!(batch.advance(evaluation,&frame,&no_adoption).unwrap(),PositionCompositionProgress::Complete(value) if value==expected[index])
                    );
                    let row = batch
                        .observe(evaluation, &mut |observation| {
                            assert_eq!(observation.target, targets[index]);
                            assert_eq!(observation.frame.token, &token);
                            assert_eq!(
                                observation.value, &expected[index],
                                "recycled workspaces must not leak a preceding frame's result"
                            );
                            Ok((metadata(), ()))
                        })
                        .unwrap();
                    assert_eq!(row.value, expected[index]);
                }
                for evaluation in evaluations {
                    batch.recycle(evaluation).unwrap();
                }
                assert_eq!(frame.0.get(), 2);
                token
            },
        );
        if let Some(previous) = previous_token {
            assert_ne!(token, previous, "each factory receives a fresh capture");
        }
        previous_token = Some(token);
        let count = 1 + next_pool.len();
        assert!(
            count >= 2,
            "retain multiple concurrently used workspaces, not only the last one"
        );
        assert!(next_pool.len() <= 256);
        if let Some(previous) = retained_count {
            assert_eq!(
                count, previous,
                "a steady two-owner workload must reuse its pool without retaining consumed empty placeholders"
            );
        }
        retained_count = Some(count);
        scratch = next_scratch;
        pool = next_pool;
    }
}

#[test]
fn two_pending_position_evaluations_keep_their_own_requests_traces_and_completion() {
    let targets = [FixtureId::new(), FixtureId::new()];
    with_batch(
        targets,
        [vec![sample(90.)], vec![sample(120.)]],
        false,
        |batch, programs, groups, _| {
            assert_eq!(batch.eligible_targets(), targets);
            for program in programs {
                assert!(batch.unchanged_controls(program).unwrap());
            }
            let mut first = batch
                .begin_branch(
                    &programs[0],
                    &programs[0].registry().branch(),
                    targets[0],
                    &no_adoption,
                )
                .unwrap();
            let mut second = batch
                .begin_branch(
                    &programs[1],
                    &programs[1].registry().branch(),
                    targets[1],
                    &no_adoption,
                )
                .unwrap();
            let frame = Frame(Cell::new(0));
            let a = needed(batch.advance(&mut first, &frame, &no_adoption).unwrap());
            let b = needed(batch.advance(&mut second, &frame, &no_adoption).unwrap());
            assert_ne!(a.request_id, b.request_id);
            assert_ne!(a.capture_id, b.capture_id);
            assert_eq!(first.frame_token(), second.frame_token());
            assert_eq!(first.pending_request().unwrap().request_id, a.request_id);
            assert_eq!(second.pending_request().unwrap().request_id, b.request_id);
            let observed = Cell::new(0);
            let mut observe = |observation: HybridFamilyObservation<'_>| {
                observed.set(observed.get() + 1);
                let expected = if observation.target == targets[0] {
                    angles(50.)
                } else {
                    angles(65.)
                };
                assert_eq!(observation.value, &expected);
                let fields = ProgrammingFieldScope::for_value(
                    ProgrammingOwner::Position,
                    observation.value,
                )?;
                observation
                    .project_fields(&fields, &mut DynamicFamilySourceProjection::default())?;
                observation.controls_for_fields(&fields);
                Ok((metadata(), ()))
            };
            assert!(batch.observe(&mut first, &mut observe).is_err());
            assert!(batch.observe(&mut second, &mut observe).is_err());
            assert_eq!(observed.get(), 0);
            assert!(
                batch
                    .resume(&mut first, b.request_id, angles(65.), None)
                    .is_err()
            );
            assert_eq!(first.pending_request().unwrap().request_id, a.request_id);
            // Resolve in the opposite order to preparation: one borrowed scratch slot must not
            // replace the other evaluation's pending request or retained source trace.
            batch
                .resume(&mut second, b.request_id, angles(65.), None)
                .unwrap();
            assert!(
                matches!(batch.advance(&mut second,&frame,&no_adoption).unwrap(),PositionCompositionProgress::Complete(value) if value==angles(65.))
            );
            let second_row = batch.observe(&mut second, &mut observe).unwrap();
            assert_eq!(first.pending_request().unwrap().request_id, a.request_id);
            batch
                .resume(&mut first, a.request_id, angles(50.), None)
                .unwrap();
            assert!(
                matches!(batch.advance(&mut first,&frame,&no_adoption).unwrap(),PositionCompositionProgress::Complete(value) if value==angles(50.))
            );
            let first_row = batch.observe(&mut first, &mut observe).unwrap();
            assert_eq!(observed.get(), 2);
            assert_eq!(
                frame.0.get(),
                2,
                "each original mixed operation invokes the frame once"
            );
            let result = HybridPositionBatchResult {
                handled: targets.to_vec(),
                projections: vec![second_row, first_row],
                requirements: vec![],
            };
            validate_result(&result, &[&groups[0], &groups[1]]).unwrap();
            batch.recycle(second).unwrap();
            batch.recycle(first).unwrap();
            let mut reused = batch
                .begin_branch(
                    &programs[0],
                    &programs[0].registry().branch(),
                    targets[0],
                    &no_adoption,
                )
                .unwrap();
            assert!(matches!(
                batch.advance(&mut reused, &frame, &no_adoption).unwrap(),
                PositionCompositionProgress::NeedsMaterialization(_)
            ));
            batch.recycle(reused).unwrap();
        },
    );
}

#[test]
fn foreign_conditioned_branches_capture_tokens_and_baselines_never_reach_observation() {
    let targets = [FixtureId::new(), FixtureId::new()];
    with_batch(
        targets,
        [vec![sample(90.)], vec![sample(120.)]],
        false,
        |batch, programs, groups, engine| {
            let program = &programs[0];
            let foreign = CapturedPositionProgram::new(
                program.registry().capture_id(),
                &angles(0.),
                &groups[0].samples,
            )
            .unwrap();
            assert!(
                batch
                    .begin_branch(program, &foreign.branch(), targets[0], &no_adoption)
                    .is_err()
            );
            assert!(
                batch
                    .begin_branch(
                        program,
                        &programs[1].registry().branch(),
                        targets[0],
                        &no_adoption
                    )
                    .is_err()
            );
            let capture = engine.prepare_output_frame(Default::default());
            let foreign_program = HybridCapturedPositionProgram::new(
                &capture.frame_token(),
                targets[0],
                &angles(0.),
                &groups[0].samples,
            )
            .unwrap();
            assert_eq!(
                foreign_program.frame_token().sampled_at(),
                program.frame_token().sampled_at()
            );
            assert!(batch.unchanged_controls(&foreign_program).is_err());
            assert!(
                batch
                    .begin_branch(
                        &foreign_program,
                        &foreign_program.registry().branch(),
                        targets[0],
                        &no_adoption
                    )
                    .is_err()
            );
            let frame = Frame(Cell::new(0));
            let mut foreign_evaluation = with_batch(
                targets,
                [vec![sample(90.)], vec![]],
                false,
                |other, programs, _, _| {
                    let mut evaluation = other
                        .begin_branch(
                            &programs[0],
                            &programs[0].registry().branch(),
                            targets[0],
                            &no_adoption,
                        )
                        .unwrap();
                    other
                        .advance(&mut evaluation, &frame, &no_adoption)
                        .unwrap();
                    evaluation
                },
            );
            let pending = foreign_evaluation.pending_request().unwrap().request_id;
            let observed = Cell::new(0);
            assert!(
                batch
                    .advance(&mut foreign_evaluation, &frame, &no_adoption)
                    .is_err()
            );
            assert!(
                batch
                    .resume(&mut foreign_evaluation, pending, angles(50.), None)
                    .is_err()
            );
            assert!(
                batch
                    .observe(&mut foreign_evaluation, &mut |_| {
                        observed.set(observed.get() + 1);
                        Ok((metadata(), ()))
                    })
                    .is_err()
            );
            assert_eq!(observed.get(), 0);
            assert_eq!(
                frame.0.get(),
                1,
                "only the original foreign composer reached its resolver"
            );
            assert_eq!(
                foreign_evaluation.pending_request().unwrap().request_id,
                pending
            );
            assert!(batch.recycle(foreign_evaluation).is_err());
        },
    );
    with_batch(targets, [vec![], vec![]], true, |batch, programs, _, _| {
        for program in programs {
            assert!(
                batch
                    .begin_branch(
                        program,
                        &program.registry().branch(),
                        program.target(),
                        &no_adoption
                    )
                    .is_err(),
                "a matching token cannot bless a foreign scalar baseline"
            );
        }
    });
}

#[test]
fn batch_result_membership_requires_exact_owned_rows_or_explicit_passive_requirements() {
    let targets = [FixtureId::new(), FixtureId::new()];
    with_batch(
        targets,
        [vec![], vec![]],
        false,
        |batch, programs, groups, _| {
            let frame = Frame(Cell::new(0));
            let mut rows = Vec::new();
            for program in programs {
                let mut evaluation = batch
                    .begin_branch(
                        program,
                        &program.registry().branch(),
                        program.target(),
                        &no_adoption,
                    )
                    .unwrap();
                assert!(matches!(
                    batch
                        .advance(&mut evaluation, &frame, &no_adoption)
                        .unwrap(),
                    PositionCompositionProgress::Complete(_)
                ));
                rows.push(
                    batch
                        .observe(&mut evaluation, &mut |_| Ok((metadata(), ())))
                        .unwrap(),
                );
                batch.recycle(evaluation).unwrap();
            }
            let groups = [&groups[0], &groups[1]];
            let mut result = HybridPositionBatchResult {
                handled: targets.to_vec(),
                projections: rows,
                requirements: vec![],
            };
            validate_result(&result, &groups).unwrap();
            let missing = result.projections.pop().unwrap();
            assert!(
                validate_result(&result, &groups).is_err(),
                "handled owner cannot silently disappear"
            );
            result.requirements.push(HybridFamilyRequirement {
                target: targets[1],
                owner: ProgrammingOwner::Position,
                reason: HybridFamilyRequirementReason::Composition(
                    TransitionRequirement::LiveJointAngles,
                ),
            });
            validate_result(&result, &groups).unwrap();
            result.requirements[0].target = FixtureId::new();
            assert!(validate_result(&result, &groups).is_err());
            result.requirements[0].target = targets[1];
            result.requirements[0].owner = ProgrammingOwner::Color;
            assert!(validate_result(&result, &groups).is_err());
            result.requirements.clear();
            result.projections.push(missing);
            result.handled.push(targets[0]);
            assert!(
                validate_result(&result, &groups).is_err(),
                "duplicate handled owner"
            );
            result.handled.pop();
            result.projections[1].target = targets[0];
            assert!(
                validate_result(&result, &groups).is_err(),
                "duplicate projected owner"
            );
            result.projections[1].target = FixtureId::new();
            assert!(
                validate_result(&result, &groups).is_err(),
                "foreign projected owner"
            );
            result.projections[1].target = targets[1];
            result.projections[1].owner = ProgrammingOwner::Color;
            assert!(
                validate_result(&result, &groups).is_err(),
                "foreign projected family"
            );
            result.projections[1].owner = ProgrammingOwner::Position;
            result.handled[1] = FixtureId::new();
            assert!(
                validate_result(&result, &groups).is_err(),
                "unavailable handled owner"
            );
            assert_eq!(
                frame.0.get(),
                0,
                "membership validation never invokes physical work"
            );
        },
    );
}

#[test]
fn concurrent_stage_replays_keep_parent_cuts_and_reuse_the_batch_pool() {
    let targets = [FixtureId::new(), FixtureId::new()];
    let copies = [FixtureId::new(), FixtureId::new()];
    let samples = std::array::from_fn(|_| {
        vec![
            ProgrammingFamilyFixAt::from_family(
                ProgrammingOwner::Position,
                Some(ProgrammingComponent::TargetX),
                AttributeValue::Position(Arc::new(PositionIntent::target(
                    TargetReference::Origin,
                    [90., 2., 3.],
                ))),
            )
            .unwrap()
            .compile(
                None,
                &FamilyEditContext::default(),
                FamilySampleRank {
                    priority: 10,
                    changed_at_millis: 1000,
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
            .into(),
        ]
    });
    let blocked_calls = Cell::new(0);
    let blocked = |_: &AttributeValue, _: &DynamicValueAddress| {
        blocked_calls.set(blocked_calls.get() + 1);
        Err(TransitionError::Requires(
            TransitionRequirement::LiveTargetPoints,
        ))
    };
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let mut pool = Vec::new();
    let mut retained_count = None;
    for _ in 0..4 {
        let (_, next_scratch, next_pool) = with_batch_workspaces(
            targets,
            samples.clone(),
            false,
            scratch,
            pool,
            |batch, programs, _, _| {
                let frame = Frame(Cell::new(0));
                let mut parents = programs
                    .iter()
                    .enumerate()
                    .map(|(index, program)| {
                        batch
                            .begin_branch(
                                program,
                                &program.registry().branch(),
                                copies[index],
                                &blocked,
                            )
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                let cuts = parents
                    .iter_mut()
                    .map(|parent| needed(batch.advance(parent, &frame, &blocked).unwrap()))
                    .collect::<Vec<_>>();
                let locators = parents
                    .iter()
                    .zip(&cuts)
                    .map(|(parent, cut)| {
                        parent
                            .pending_stage_locator(cut.request_id)
                            .unwrap()
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                let before = blocked_calls.get();
                let mut operands = programs
                    .iter()
                    .enumerate()
                    .map(|(index, program)| {
                        batch
                            .begin_stage_branch(
                                program,
                                &program.registry().branch(),
                                &locators[index],
                                PositionStageOperand::AdoptionInput,
                                copies[index],
                                &blocked,
                            )
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                for (index, operand) in operands.iter_mut().enumerate().rev() {
                    assert_eq!(
                        operand.destination(),
                        copies[index],
                        "destination retained as dispatch metadata"
                    );
                    assert!(
                        matches!(batch.advance_stage(operand, &frame, &blocked).unwrap(), PositionStageOperandProgress::OperandReady(value) if value == angles(index as f32 * 10.))
                    );
                    assert!(
                        matches!(batch.advance_stage(operand, &frame, &blocked).unwrap(), PositionStageOperandProgress::OperandReady(value) if value == angles(index as f32 * 10.))
                    );
                    assert_eq!(
                        parents[index].pending_request().unwrap().request_id,
                        cuts[index].request_id
                    );
                }
                assert_eq!(
                    blocked_calls.get(),
                    before,
                    "operand stops before adoption callback"
                );
                let foreign = CapturedPositionProgram::new(
                    programs[0].registry().capture_id(),
                    &angles(0.),
                    &samples[0],
                )
                .unwrap();
                assert!(
                    batch
                        .begin_stage_branch(
                            &programs[0],
                            &foreign.branch(),
                            &locators[0],
                            PositionStageOperand::AdoptionInput,
                            copies[0],
                            &blocked
                        )
                        .is_err()
                );
                let mut removed = programs[0].registry().branch();
                removed.replace_source(0, None).unwrap();
                let mut inactive = batch
                    .begin_stage_branch(
                        &programs[0],
                        &removed,
                        &locators[0],
                        PositionStageOperand::AdoptionInput,
                        copies[0],
                        &blocked,
                    )
                    .unwrap();
                assert!(matches!(
                    batch
                        .advance_stage(&mut inactive, &frame, &blocked)
                        .unwrap(),
                    PositionStageOperandProgress::Inactive
                ));
                assert!(matches!(
                    batch
                        .advance_stage(&mut inactive, &frame, &blocked)
                        .unwrap(),
                    PositionStageOperandProgress::Inactive
                ));
                batch.recycle_stage(inactive).unwrap();
                for operand in operands {
                    batch.recycle_stage(operand).unwrap();
                }
                for parent in parents {
                    batch.recycle(parent).unwrap();
                }
                assert_eq!(frame.0.get(), 0, "no transition or physical solver used");
            },
        );
        let count = 1 + next_pool.len();
        if let Some(previous) = retained_count {
            assert_eq!(
                count, previous,
                "steady parent/operand workload must reuse the same pool"
            );
        }
        retained_count = Some(count);
        scratch = next_scratch;
        pool = next_pool;
    }
}

#[test]
fn discovery_batch_keeps_parent_requests_independent_and_reuses_all_owned_workspaces() {
    let targets = [FixtureId::new(), FixtureId::new()];
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let mut pool = Vec::new();
    let mut retained = None;
    for _ in 0..4 {
        let (_, next_scratch, next_pool) = with_batch_workspaces(
            targets,
            [vec![sample(90.)], vec![sample(120.)]],
            false,
            scratch,
            pool,
            |batch, programs, _, _| {
                let frame = Frame(Cell::new(0));
                let mut parents = Vec::new();
                let mut discoveries = Vec::new();
                for program in programs {
                    let mut parent = batch
                        .begin_branch(
                            program,
                            &program.registry().branch(),
                            program.target(),
                            &no_adoption,
                        )
                        .unwrap();
                    let request = needed(batch.advance(&mut parent, &frame, &no_adoption).unwrap());
                    let mut discovery = batch
                        .begin_discovery_branch(
                            program,
                            &program.registry().branch(),
                            FixtureId::new(),
                            &no_adoption,
                            64,
                        )
                        .unwrap();
                    let PositionStageDiscoveryProgress::NeedsMaterialization(pending) = batch
                        .advance_discovery(&mut discovery, &frame, &no_adoption)
                        .unwrap()
                    else {
                        panic!("original nested transition must stay pending");
                    };
                    let prefix = batch.discovery_report(&mut discovery).unwrap();
                    assert!(!prefix.complete);
                    assert_ne!(pending.request_id, request.request_id);
                    assert!(
                        batch
                            .resume_discovery(
                                &mut discovery,
                                uuid::Uuid::new_v4(),
                                angles(40.),
                                None
                            )
                            .is_err()
                    );
                    assert_eq!(
                        discovery.pending_request().unwrap().request_id,
                        pending.request_id
                    );
                    batch
                        .resume_discovery(&mut discovery, pending.request_id, angles(40.), None)
                        .unwrap();
                    let PositionStageDiscoveryProgress::Complete(report) = batch
                        .advance_discovery(&mut discovery, &frame, &no_adoption)
                        .unwrap()
                    else {
                        panic!("resolved discovery must complete");
                    };
                    assert!(report.complete);
                    assert!(report.inspected_routes > 0);
                    assert!(
                        report
                            .candidates
                            .iter()
                            .all(|candidate| candidate.capture_id()
                                == program.registry().capture_id())
                    );
                    assert_eq!(
                        batch.discovery_report(&mut discovery).unwrap().candidates,
                        report.candidates
                    );
                    assert_eq!(
                        parent.pending_request().unwrap().request_id,
                        request.request_id,
                        "speculative discovery cannot consume parent response"
                    );
                    assert!(matches!(
                        batch
                            .advance_discovery(&mut discovery, &frame, &no_adoption)
                            .unwrap(),
                        PositionStageDiscoveryProgress::Complete(_)
                    ));
                    parents.push(parent);
                    discoveries.push(discovery);
                }
                // A removed branch is valid and complete but cannot recover removed stages.
                let mut removed = programs[0].registry().branch();
                removed.replace_source(0, None).unwrap();
                let mut empty = batch
                    .begin_discovery_branch(&programs[0], &removed, targets[0], &no_adoption, 64)
                    .unwrap();
                let PositionStageDiscoveryProgress::Complete(report) = batch
                    .advance_discovery(&mut empty, &frame, &no_adoption)
                    .unwrap()
                else {
                    panic!("empty branch completes");
                };
                assert!(report.complete && report.candidates.is_empty());
                batch.recycle_discovery(empty).unwrap();
                for discovery in discoveries {
                    batch.recycle_discovery(discovery).unwrap();
                }
                for parent in parents {
                    batch.recycle(parent).unwrap();
                }
            },
        );
        let count = 1 + next_pool.len();
        if let Some(previous) = retained {
            assert_eq!(
                count, previous,
                "discovery must not retain empty transfer placeholders"
            );
        }
        retained = Some(count);
        scratch = next_scratch;
        pool = next_pool;
    }
}

#[test]
fn original_suppression_query_uses_every_captured_source_and_rejects_crossfade_zero() {
    for (control, expected) in [
        (FamilyEndpointOutputControl::Suppressed, true),
        (FamilyEndpointOutputControl::Unchanged, false),
        (
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0. },
            false,
        ),
        (
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: 1. },
            false,
        ),
    ] {
        let first = sample(90.);
        let mut rank = match &first {
            FamilyCompositionSample::WholeExpression { rank, .. } => *rank,
            _ => unreachable!(),
        };
        rank.stable_order = 2;
        rank.identity = FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(11),
            controller_id: Uuid::from_u128(12),
            lane_id: Uuid::from_u128(13),
        };
        let narrow = FamilySample::new(
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
            DynamicValue::Scalar(35.),
            rank,
            0.2,
        )
        .unwrap()
        .into();
        with_batch_control_workspaces(
            [FixtureId::new(), FixtureId::new()],
            [vec![first, narrow], vec![]],
            false,
            Default::default(),
            Vec::new(),
            &|_| control,
            |batch, programs, _, _| {
                assert_eq!(
                    batch
                        .original_sources_suppressed(programs[0].target())
                        .unwrap(),
                    expected
                );
                assert!(
                    !batch
                        .original_sources_suppressed(programs[1].target())
                        .unwrap()
                );
                assert!(batch.original_sources_suppressed(FixtureId::new()).is_err());
            },
        );
    }
    with_batch_control_workspaces(
        [FixtureId::new(), FixtureId::new()],
        [vec![sample(90.)], vec![]],
        true,
        Default::default(),
        Vec::new(),
        &|_| FamilyEndpointOutputControl::Suppressed,
        |batch, programs, _, _| {
            assert!(
                batch
                    .original_sources_suppressed(programs[0].target())
                    .is_err()
            )
        },
    );
    let same = FixtureId::new();
    with_batch_control_workspaces(
        [same, same],
        [vec![sample(90.)], vec![]],
        false,
        Default::default(),
        Vec::new(),
        &|_| FamilyEndpointOutputControl::Suppressed,
        |batch, _, _, _| assert!(batch.original_sources_suppressed(same).is_err()),
    );
}

#[test]
fn envelope_operands_keep_exact_parent_authority_and_do_not_observe() {
    let targets = [FixtureId::new(), FixtureId::new()];
    let samples = std::array::from_fn(|_| {
        let mut sample = sample(10.);
        let FamilyCompositionSample::WholeExpression {
            expression,
            activation_mix,
            ..
        } = &mut sample
        else {
            unreachable!()
        };
        *expression = Arc::new(
            CompiledProgrammingFamilyExpression::new(
                leaf(target()),
                ProgrammingOwner::Position,
                None,
                None,
            )
            .unwrap(),
        );
        *activation_mix = 0.25;
        vec![sample]
    });
    with_batch(targets, samples, false, |batch, programs, _, _| {
        let frame = Frame(Cell::new(0));
        let mut parents = Vec::new();
        for program in programs {
            let mut parent = batch
                .begin_branch(
                    program,
                    &program.registry().branch(),
                    program.target(),
                    &no_adoption,
                )
                .unwrap();
            let request = needed(batch.advance(&mut parent, &frame, &no_adoption).unwrap());
            let locator = parent
                .pending_envelope_locator(request.request_id)
                .unwrap()
                .unwrap();
            assert_eq!(locator.stage(), PositionCompletionStage::Activation);
            assert!(parent.pending_envelope_locator(Uuid::new_v4()).is_err());
            assert!(
                batch
                    .begin_envelope_branch(
                        program,
                        &program.registry().branch(),
                        &locator,
                        PositionEnvelopeOperand::EndpointCurrent,
                        program.target(),
                        &no_adoption
                    )
                    .is_err()
            );
            let foreign = &programs[usize::from(program.target() == targets[0])];
            assert!(
                batch
                    .begin_envelope_branch(
                        foreign,
                        &foreign.registry().branch(),
                        &locator,
                        PositionEnvelopeOperand::ActivationUnderlay,
                        foreign.target(),
                        &no_adoption
                    )
                    .is_err()
            );
            for (operand, expected) in [
                (
                    PositionEnvelopeOperand::ActivationUnderlay,
                    angles(if program.target() == targets[0] {
                        0.
                    } else {
                        10.
                    }),
                ),
                (PositionEnvelopeOperand::ActivationEndpoint, target()),
            ] {
                let mut child = batch
                    .begin_envelope_branch(
                        program,
                        &program.registry().branch(),
                        &locator,
                        operand,
                        program.target(),
                        &no_adoption,
                    )
                    .unwrap();
                for _ in 0..2 {
                    assert!(
                        matches!(batch.advance_stage(&mut child, &frame, &no_adoption).unwrap(),
                        PositionStageOperandProgress::OperandReady(value) if value == expected)
                    );
                }
                assert_eq!(
                    parent.pending_request().unwrap().request_id,
                    request.request_id
                );
                assert!(parent.completed_value().is_none());
                batch.recycle_stage(child).unwrap();
            }
            assert!(
                batch
                    .resume(
                        &mut parent,
                        request.request_id,
                        AttributeValue::Normalized(0.5),
                        None
                    )
                    .is_err()
            );
            assert_eq!(
                parent.pending_request().unwrap().request_id,
                request.request_id
            );
            batch
                .resume(&mut parent, request.request_id, angles(7.), None)
                .unwrap();
            assert!(
                matches!(batch.advance(&mut parent, &frame, &no_adoption).unwrap(), PositionCompositionProgress::Complete(value) if value == angles(7.))
            );
            assert!(parent.pending_envelope_locator(request.request_id).is_err());
            parents.push(parent);
        }
        for parent in parents {
            batch.recycle(parent).unwrap();
        }
    });
}

#[test]
fn mask_operands_replay_the_original_prefix_and_leave_the_parent_and_suffix_owned() {
    let targets = [FixtureId::new(), FixtureId::new()];
    let blocked = |_: &AttributeValue, _: &DynamicValueAddress| {
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    };
    let fixed =
        |value: AttributeValue, component: Option<ProgrammingComponent>, order: usize, mix: f32| {
            FamilyCompositionSample::Known(
                ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Position, component, value)
                    .unwrap()
                    .compile(
                        None,
                        &FamilyEditContext::default(),
                        FamilySampleRank {
                            priority: 10,
                            changed_at_millis: 1000 + order as u64,
                            changed_at_submillis_nanos: 0,
                            stable_order: order as u128,
                            identity: FamilySampleIdentity::Fixed {
                                source: FamilyFixedSampleSource::Programmer,
                                row_index: order,
                            },
                        },
                        mix,
                    )
                    .unwrap(),
            )
        };
    for transition in [false, true] {
        let endpoint = if transition {
            AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Point {
                    point_id: Uuid::new_v4(),
                },
                [1., 2., 3.],
            )))
        } else {
            angles(40.)
        };
        let samples = std::array::from_fn(|_| {
            let mut prefix = sample(0.);
            let FamilyCompositionSample::WholeExpression { expression, .. } = &mut prefix else {
                unreachable!()
            };
            *expression = Arc::new(
                CompiledProgrammingFamilyExpression::new(
                    leaf(target()),
                    ProgrammingOwner::Position,
                    None,
                    None,
                )
                .unwrap(),
            );
            let mut values = vec![prefix, fixed(endpoint.clone(), None, 1, 0.25)];
            if !transition {
                values.push(fixed(
                    AttributeValue::Position(Arc::new(PositionIntent::angles(999., 44.))),
                    Some(ProgrammingComponent::Tilt),
                    2,
                    0.5,
                ));
            }
            values
        });
        with_batch(targets, samples, false, |batch, programs, _, _| {
            let frame = Frame(Cell::new(0));
            let program = &programs[0];
            let mut parent = batch
                .begin_branch(
                    program,
                    &program.registry().branch(),
                    program.target(),
                    &blocked,
                )
                .unwrap();
            let request = needed(batch.advance(&mut parent, &frame, &blocked).unwrap());
            assert!(matches!(
                (&request.operation, transition),
                (PositionCompositionOperation::MaskAdoption { .. }, false)
                    | (PositionCompositionOperation::MaskTransition { .. }, true)
            ));
            let locator = parent
                .pending_mask_locator(request.request_id)
                .unwrap()
                .unwrap();
            assert_eq!(
                locator.stage(),
                if transition {
                    PositionMaskStage::Transition
                } else {
                    PositionMaskStage::Adoption
                }
            );
            assert!(parent.pending_mask_locator(Uuid::new_v4()).is_err());
            let operands = if transition {
                vec![
                    (PositionMaskOperand::TransitionFrom, target()),
                    (PositionMaskOperand::TransitionTo, endpoint.clone()),
                ]
            } else {
                vec![(PositionMaskOperand::AdoptionInput, target())]
            };
            let wrong = if transition {
                PositionMaskOperand::AdoptionInput
            } else {
                PositionMaskOperand::TransitionTo
            };
            assert!(
                batch
                    .begin_mask_branch(
                        program,
                        &program.registry().branch(),
                        &locator,
                        wrong,
                        program.target(),
                        &blocked
                    )
                    .is_err()
            );
            assert!(
                batch
                    .begin_mask_branch(
                        &programs[1],
                        &programs[1].registry().branch(),
                        &locator,
                        operands[0].0,
                        programs[1].target(),
                        &blocked
                    )
                    .is_err()
            );
            for (operand, expected) in operands {
                let mut child = batch
                    .begin_mask_branch(
                        program,
                        &program.registry().branch(),
                        &locator,
                        operand,
                        program.target(),
                        &blocked,
                    )
                    .unwrap();
                for _ in 0..2 {
                    assert!(
                        matches!(batch.advance_stage(&mut child, &frame, &blocked).unwrap(), PositionStageOperandProgress::OperandReady(value) if value == expected)
                    );
                }
                assert_eq!(
                    parent.pending_request().unwrap().request_id,
                    request.request_id
                );
                assert!(parent.completed_value().is_none());
                batch.recycle_stage(child).unwrap();
            }
            assert!(
                batch
                    .resume(
                        &mut parent,
                        request.request_id,
                        AttributeValue::Normalized(0.5),
                        None
                    )
                    .is_err()
            );
            assert_eq!(
                parent.pending_request().unwrap().request_id,
                request.request_id
            );
            batch
                .resume(&mut parent, request.request_id, angles(8.), None)
                .unwrap();
            // The component suffix is a real 50% FixAT: 20 -> 44 gives 32.
            // Executing that suffix twice would give 38, so an idempotent overwrite
            // can no longer conceal accidental ancestor/suffix replay.
            let expected = if transition {
                angles(8.)
            } else {
                AttributeValue::Position(Arc::new(PositionIntent::angles(16., 32.)))
            };
            assert!(
                matches!(batch.advance(&mut parent, &frame, &blocked).unwrap(), PositionCompositionProgress::Complete(value) if value == expected)
            );
            assert!(parent.pending_mask_locator(request.request_id).is_err());
            batch.recycle(parent).unwrap();
        });
    }
}
