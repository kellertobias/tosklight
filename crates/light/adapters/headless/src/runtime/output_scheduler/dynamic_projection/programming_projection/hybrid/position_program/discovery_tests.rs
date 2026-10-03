//! Actual captured discovery loans: registry-local candidates never observe or accept output.
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
        static_rows: None,
    };
    let program = HybridCapturedPositionProgram::new(&token, target, &base, samples).unwrap();
    action(&mut composer, &program, &engine)
}

fn blocked(_: &AttributeValue, _: &DynamicValueAddress) -> Result<AttributeValue, TransitionError> {
    Err(TransitionError::Requires(
        TransitionRequirement::LiveJointAngles,
    ))
}
fn pending_discovery(progress: PositionStageDiscoveryProgress) -> PositionCompositionRequest {
    match progress {
        PositionStageDiscoveryProgress::NeedsMaterialization(request) => request,
        PositionStageDiscoveryProgress::Complete(_) => {
            panic!("expected retained discovery request")
        }
    }
}
fn completed_discovery(progress: PositionStageDiscoveryProgress) -> PositionStageDiscoveryReport {
    match progress {
        PositionStageDiscoveryProgress::Complete(report) => report,
        PositionStageDiscoveryProgress::NeedsMaterialization(_) => {
            panic!("expected completed synchronous discovery")
        }
    }
}
fn simple_sources() -> Vec<FamilyCompositionSample> {
    vec![
        source(
            DynamicValue::Scalar(90.),
            Some(ProgrammingComponent::Pan),
            1,
            0.5,
        ),
        source(
            DynamicValue::Scalar(80.),
            Some(ProgrammingComponent::Tilt),
            2,
            1.,
        ),
    ]
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
fn captured_discovery_synchronous_conversion_repeats_reports_and_recycles_changed_branch() {
    let logical = FixtureId::new();
    let destination = FixtureId::new();
    with_composer(
        logical,
        target(0., 0.),
        &simple_sources(),
        |composer, program, _| {
            let frame = Frame {
                calls: Cell::new(0),
            };
            let calls = Cell::new(0);
            let available = |_: &AttributeValue, _: &DynamicValueAddress| {
                calls.set(calls.get() + 1);
                Ok(angles(10., 20.))
            };
            let branch = program.registry().branch();
            let mut evaluation =
                begin_discovery_branch(composer, program, &branch, destination, &available, 30)
                    .unwrap();
            assert_eq!(evaluation.frame_token(), program.frame_token());
            assert_eq!(evaluation.target(), logical);
            assert_eq!(evaluation.destination(), destination);
            assert_eq!(
                evaluation.registry().capture_id(),
                program.registry().capture_id()
            );
            let initial = discovery_report(composer, &mut evaluation).unwrap();
            assert!(!initial.complete);
            assert!(initial.candidates.is_empty());
            let report = completed_discovery(
                advance_discovery(composer, &mut evaluation, &frame, &available).unwrap(),
            );
            assert!(report.complete);
            assert_eq!(report.candidates.len(), 2);
            assert_eq!(report.unmapped_routes, 0);
            assert_eq!(calls.get(), 1);
            assert!(evaluation.pending_request().is_none());
            for _ in 0..3 {
                let snapshot = discovery_report(composer, &mut evaluation).unwrap();
                assert!(snapshot.complete);
                assert_eq!(snapshot.candidates, report.candidates);
                assert_eq!(snapshot.inspected_routes, report.inspected_routes);
                let repeated = completed_discovery(
                    advance_discovery(composer, &mut evaluation, &frame, &available).unwrap(),
                );
                assert_eq!(repeated.candidates, report.candidates);
            }
            assert_eq!(calls.get(), 1);
            assert_eq!(frame.calls.get(), 0);
            // A candidate remains only a local recipe: actual operand replay proves reachability.
            let mut operand = begin_stage_branch(
                composer,
                program,
                &branch,
                &report.candidates[0],
                PositionStageOperand::AdoptionInput,
                destination,
                &available,
            )
            .unwrap();
            let PositionStageOperandProgress::OperandReady(value) =
                advance_stage(composer, &mut operand, &frame, &available).unwrap()
            else {
                panic!("original first component cut")
            };
            assert_eq!(value, target(0., 0.));
            assert_eq!(calls.get(), 1);
            recycle_stage(composer, operand).unwrap();
            recycle_discovery(composer, evaluation).unwrap();
            let mut changed = branch.clone();
            changed.replace_source(0, None).unwrap();
            let mut changed_evaluation =
                begin_discovery_branch(composer, program, &changed, destination, &available, 30)
                    .unwrap();
            let changed_report = completed_discovery(
                advance_discovery(composer, &mut changed_evaluation, &frame, &available).unwrap(),
            );
            assert_eq!(changed_report.candidates.len(), 1);
            assert_eq!(changed_report.candidates[0], report.candidates[1]);
            assert_eq!(
                calls.get(),
                2,
                "recycled workspace cannot retain completed source cursor"
            );
            recycle_discovery(composer, changed_evaluation).unwrap();
        },
    );
}

#[test]
fn captured_discovery_preserves_pending_prefix_and_invalid_response_retry() {
    with_composer(
        FixtureId::new(),
        target(0., 0.),
        &nested_sources(),
        |composer, program, _| {
            let frame = Frame {
                calls: Cell::new(0),
            };
            let mut evaluation = begin_discovery_branch(
                composer,
                program,
                &program.registry().branch(),
                FixtureId::new(),
                &blocked,
                60,
            )
            .unwrap();
            let nested = pending_discovery(
                advance_discovery(composer, &mut evaluation, &frame, &blocked).unwrap(),
            );
            let prefix = discovery_report(composer, &mut evaluation).unwrap();
            assert!(!prefix.complete);
            assert_eq!(nested.capture_id, program.registry().capture_id());
            program
                .registry()
                .source_node_for_origin(nested.origin().unwrap())
                .unwrap();
            for _ in 0..2 {
                assert_eq!(
                    pending_discovery(
                        advance_discovery(composer, &mut evaluation, &frame, &blocked).unwrap()
                    )
                    .request_id,
                    nested.request_id
                );
                assert_eq!(
                    discovery_report(composer, &mut evaluation)
                        .unwrap()
                        .candidates,
                    prefix.candidates
                );
            }
            assert_eq!(frame.calls.get(), 1);
            assert!(
                resume_discovery(
                    composer,
                    &mut evaluation,
                    Uuid::new_v4(),
                    point_target(20., 30.),
                    None
                )
                .is_err()
            );
            assert!(
                resume_discovery(
                    composer,
                    &mut evaluation,
                    nested.request_id,
                    AttributeValue::Normalized(0.4),
                    None
                )
                .is_err()
            );
            assert_eq!(
                evaluation.pending_request().unwrap().request_id,
                nested.request_id
            );
            assert_eq!(
                pending_discovery(
                    advance_discovery(composer, &mut evaluation, &frame, &blocked).unwrap()
                )
                .request_id,
                nested.request_id
            );
            assert_eq!(frame.calls.get(), 1);
            resume_discovery(
                composer,
                &mut evaluation,
                nested.request_id,
                point_target(20., 30.),
                None,
            )
            .unwrap();
            let component = pending_discovery(
                advance_discovery(composer, &mut evaluation, &frame, &blocked).unwrap(),
            );
            assert_ne!(component.request_id, nested.request_id);
            let later = discovery_report(composer, &mut evaluation).unwrap();
            assert!(!later.complete);
            assert!(later.inspected_routes > prefix.inspected_routes);
            assert!(!later.candidates.is_empty());
            resume_discovery(
                composer,
                &mut evaluation,
                component.request_id,
                angles(30., 45.),
                None,
            )
            .unwrap();
            let report = completed_discovery(
                advance_discovery(composer, &mut evaluation, &frame, &blocked).unwrap(),
            );
            assert!(report.complete);
            assert_eq!(report.candidates, later.candidates);
            assert_eq!(
                frame.calls.get(),
                1,
                "responses resume owned tasks without replaying nested frame callbacks"
            );
            recycle_discovery(composer, evaluation).unwrap();
        },
    );
}

#[test]
fn captured_discovery_rejects_foreign_registry_target_and_frame_without_poisoning() {
    let logical = FixtureId::new();
    let samples = simple_sources();
    with_composer(
        logical,
        target(0., 0.),
        &samples,
        |composer, program, engine| {
            let frame = Frame {
                calls: Cell::new(0),
            };
            let destination = FixtureId::new();
            let foreign = CapturedPositionProgram::new(
                program.registry().capture_id(),
                &target(0., 0.),
                &samples,
            )
            .unwrap();
            assert!(
                begin_discovery_branch(
                    composer,
                    program,
                    &foreign.branch(),
                    destination,
                    &blocked,
                    30
                )
                .is_err()
            );
            let mut wrong_target = program.clone();
            wrong_target.target = FixtureId::new();
            assert!(
                begin_discovery_branch(
                    composer,
                    &wrong_target,
                    &wrong_target.registry().branch(),
                    destination,
                    &blocked,
                    30
                )
                .is_err()
            );
            let mut wrong_frame = program.clone();
            wrong_frame.token = engine
                .prepare_output_frame(Default::default())
                .frame_token();
            assert_eq!(wrong_frame.token.sampled_at(), program.token.sampled_at());
            assert!(
                begin_discovery_branch(
                    composer,
                    &wrong_frame,
                    &wrong_frame.registry().branch(),
                    destination,
                    &blocked,
                    30
                )
                .is_err()
            );
            assert!(
                begin_discovery_branch(
                    composer,
                    program,
                    &program.registry().branch(),
                    destination,
                    &blocked,
                    0
                )
                .is_err()
            );
            let mut evaluation = begin_discovery_branch(
                composer,
                program,
                &program.registry().branch(),
                destination,
                &blocked,
                30,
            )
            .unwrap();
            with_composer(logical, target(0., 0.), &samples, |other, _, _| {
                assert!(advance_discovery(other, &mut evaluation, &frame, &blocked).is_err());
                assert!(discovery_report(other, &mut evaluation).is_err());
                assert!(
                    resume_discovery(other, &mut evaluation, Uuid::new_v4(), angles(1., 2.), None)
                        .is_err()
                );
            });
            let pending = pending_discovery(
                advance_discovery(composer, &mut evaluation, &frame, &blocked).unwrap(),
            );
            let snapshot = discovery_report(composer, &mut evaluation).unwrap();
            assert!(!snapshot.complete);
            assert_eq!(
                evaluation.destination(),
                destination,
                "free bridge retains copy metadata; batch admission supplies physical-copy authority"
            );
            resume_discovery(
                composer,
                &mut evaluation,
                pending.request_id,
                angles(1., 2.),
                None,
            )
            .unwrap();
            assert!(
                completed_discovery(
                    advance_discovery(composer, &mut evaluation, &frame, &blocked).unwrap()
                )
                .complete
            );
            recycle_discovery(composer, evaluation).unwrap();
        },
    );
}

#[test]
fn captured_discovery_freshness_failure_never_resurrects_report_or_pending_authority() {
    for fail_in in ["advance", "resume", "report", "completed_report"] {
        with_composer(
            FixtureId::new(),
            target(0., 0.),
            &simple_sources(),
            |composer, program, _| {
                let frame = Frame {
                    calls: Cell::new(0),
                };
                let mut evaluation = begin_discovery_branch(
                    composer,
                    program,
                    &program.registry().branch(),
                    program.target(),
                    &blocked,
                    30,
                )
                .unwrap();
                let pending = pending_discovery(
                    advance_discovery(composer, &mut evaluation, &frame, &blocked).unwrap(),
                );
                if fail_in == "completed_report" {
                    resume_discovery(
                        composer,
                        &mut evaluation,
                        pending.request_id,
                        angles(1., 2.),
                        None,
                    )
                    .unwrap();
                    assert!(
                        completed_discovery(
                            advance_discovery(composer, &mut evaluation, &frame, &blocked).unwrap()
                        )
                        .complete
                    );
                }
                *composer.typed.failure.borrow_mut() =
                    Some(IntentError("injected captured discovery freshness loss".into()).into());
                match fail_in {
                    "advance" => {
                        assert!(
                            advance_discovery(composer, &mut evaluation, &frame, &blocked).is_err()
                        );
                    }
                    "resume" => {
                        assert!(
                            resume_discovery(
                                composer,
                                &mut evaluation,
                                pending.request_id,
                                angles(1., 2.),
                                None
                            )
                            .is_err()
                        );
                    }
                    _ => {
                        assert!(discovery_report(composer, &mut evaluation).is_err());
                    }
                }
                assert!(evaluation.pending_request().is_none());
                *composer.typed.failure.borrow_mut() = None;
                assert!(discovery_report(composer, &mut evaluation).is_err());
                assert!(advance_discovery(composer, &mut evaluation, &frame, &blocked).is_err());
                assert!(
                    resume_discovery(
                        composer,
                        &mut evaluation,
                        pending.request_id,
                        angles(1., 2.),
                        None
                    )
                    .is_err()
                );
                assert_eq!(frame.calls.get(), 0);
                recycle_discovery(composer, evaluation).unwrap();
            },
        );
    }
}
