//! Captured graph-operand bridge contracts using genuine Required/Size sampler emissions.
//! These checks establish retained authority and scratch lifecycle, not physical fitting.
use super::super::position_batch::CapturedHybridPositionBatchComposer;
use super::*;
use light_core::{ManualClock, NativeColorIdentity, SessionId, programming::*};
use light_dynamics::*;
use light_programmer::ProgrammerRegistry;
use std::cell::Cell;
use std::collections::HashMap;
use uuid::Uuid;

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
struct Sources(Cell<usize>);
impl ScalarSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        panic!("this producer uses typed Position")
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}
impl DynamicValueSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        self.0.set(self.0.get() + 1);
        Some(DynamicValue::Family(target(0, 4., 8.)))
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
    logical: FixtureId,
    samples: Vec<FamilyCompositionSample>,
    current_reads: Cell<usize>,
}
fn sampled(size: f32) -> Sampled {
    let lane_id = Uuid::new_v4();
    let definition = DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 1,
        revision: 1,
        name: "Captured graph bridge".into(),
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
                            interpolation: light_dynamics::ScalarInterpolation::Linear,
                        },
                        DynamicKeyframe {
                            position: 0.5,
                            source: DynamicValueSource::Value {
                                value: DynamicValue::Family(target(9, 30., 40.)),
                            },
                            interpolation: light_dynamics::ScalarInterpolation::Linear,
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
    let logical = FixtureId::new();
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
                ordered_targets: vec![logical],
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
    let sources = Sources(Cell::new(0));
    let mut sampled = runtime
        .sample_programming(instance, 250, 1000, 10, &sources, &sources)
        .unwrap();
    assert_eq!(sampled.len(), 1);
    let sample = sampled.remove(0);
    let provenance = sample.expression.operation_provenance().unwrap();
    assert!(provenance.is_complete());
    assert_eq!(provenance.handles().len(), if size == 1. { 1 } else { 2 });
    assert!(
        provenance
            .handles()
            .iter()
            .all(|handle| handle.target() == logical
                && handle.lane_id() == lane_id
                && handle.emission().instance_id() == instance)
    );
    Sampled {
        logical,
        samples: vec![FamilyCompositionSample::WholeExpression {
            expression: Arc::new(
                CompiledProgrammingFamilyExpression::new(
                    Arc::new(sample.expression),
                    ProgrammingOwner::Position,
                    None,
                    None,
                )
                .unwrap(),
            ),
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
            activation_mix: 1.,
        }],
        current_reads: sources.0,
    }
}
struct NoModels;
impl DynamicNativeModelResolver for NoModels {
    fn resolve(
        &self,
        _: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        panic!("Position graph replay must not request Color models")
    }
}
fn no_current(
    _: FixtureId,
    _: &AttributeValue,
    _: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    panic!("whole Target Current is already captured by the producer")
}
fn no_adoption(
    _: &AttributeValue,
    _: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    panic!("whole graph operands do not perform component adoption")
}
#[derive(Default)]
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
fn with_batch<R>(
    sample: &Sampled,
    action: impl FnOnce(
        &mut dyn HybridPositionBatchComposer<()>,
        &HybridCapturedPositionProgram,
        &Engine,
        &CapturedProgrammingSources<'_, PreparedFamilySources<'_>>,
    ) -> R,
) -> R {
    let programmers = ProgrammerRegistry::with_clock(Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1000).unwrap(),
    )));
    let session = SessionId::new();
    programmers.start(session);
    let base = target(0, 4., 8.);
    programmers.set(
        session,
        sample.logical,
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
    let typed = CapturedProgrammingSources::new(&source, &no_current, None);
    let models = NoModels;
    let frame = HybridFrameContext {
        capture: &capture,
        geometry: &geometry,
        native_models: &models,
        token: &token,
        scalar: &scalar,
    };
    let group = DynamicFamilySampleGroup {
        target: sample.logical,
        owner: ProgrammingOwner::Position,
        samples: sample.samples.clone(),
    };
    let program =
        HybridCapturedPositionProgram::new(&token, sample.logical, &base, &sample.samples).unwrap();
    let control = |_: FamilySampleRank| FamilyEndpointOutputControl::Unchanged;
    let mut batch = CapturedHybridPositionBatchComposer::new_with_pool(
        &typed,
        &[&group],
        frame,
        &scalar,
        &control,
        Default::default(),
        Vec::new(),
    );
    action(&mut batch, &program, &engine, &typed)
}
fn pending(progress: PositionCompositionProgress) -> PositionCompositionRequest {
    let PositionCompositionProgress::NeedsMaterialization(request) = progress else {
        panic!("actual original graph operation must suspend")
    };
    request
}
fn graph_pending(progress: PositionGraphOperationOperandProgress) -> PositionCompositionRequest {
    let PositionGraphOperationOperandProgress::NeedsMaterialization(request) = progress else {
        panic!("actual unresolved operand prefix must suspend")
    };
    request
}
fn ready(progress: PositionGraphOperationOperandProgress) -> AttributeValue {
    let PositionGraphOperationOperandProgress::OperandReady(value) = progress else {
        panic!("expected non-observable graph operand")
    };
    value
}

#[test]
fn graph_bridge_keeps_original_required_and_size_requests_and_retries_only_the_real_prefix() {
    let sample = sampled(1.5);
    assert_eq!(
        sample.current_reads.get(),
        1,
        "Size captures its static whole-family Current once"
    );
    with_batch(&sample, |batch, program, _, _| {
        let frame = Frame::default();
        let destination = FixtureId::new();
        let branch = program.registry().branch();
        let mut parent = batch
            .begin_branch(program, &branch, destination, &no_adoption)
            .unwrap();
        let required = pending(batch.advance(&mut parent, &frame, &no_adoption).unwrap());
        let locator = parent
            .pending_graph_operation_locator(required.request_id)
            .unwrap()
            .unwrap();
        assert_eq!(locator.kind(), PositionGraphOperationKind::Required);
        assert!(
            parent
                .pending_graph_operation_locator(Uuid::new_v4())
                .is_err()
        );
        assert!(
            program
                .registry()
                .source_node_for_origin(required.origin().unwrap())
                .unwrap()
                .as_ref()
                == Some(locator.operation_node())
        );
        let mut incoming = batch
            .begin_graph_branch(
                program,
                &branch,
                &locator,
                PositionGraphOperationOperand::RequiredIncoming,
                destination,
                &no_adoption,
            )
            .unwrap();
        assert_eq!(incoming.frame_token(), program.frame_token());
        assert_eq!(incoming.target(), sample.logical);
        assert_eq!(incoming.destination(), destination);
        incoming.registry().validate_branch(&branch).unwrap();
        let calls = frame.0.get();
        for _ in 0..2 {
            assert_eq!(
                ready(
                    batch
                        .advance_graph(&mut incoming, &frame, &no_adoption)
                        .unwrap()
                ),
                target(9, 30., 40.)
            );
            assert!(incoming.pending_request().is_none());
        }
        assert_eq!(
            frame.0.get(),
            calls,
            "the operand stops before its own Required arithmetic"
        );
        batch.recycle_graph(incoming).unwrap();
        assert_eq!(
            parent.pending_request().unwrap().request_id,
            required.request_id
        );
        batch
            .resume(&mut parent, required.request_id, angles(20., 30.), None)
            .unwrap();
        let size_request = pending(batch.advance(&mut parent, &frame, &no_adoption).unwrap());
        let size = parent
            .pending_graph_operation_locator(size_request.request_id)
            .unwrap()
            .unwrap();
        assert_eq!(size.kind(), PositionGraphOperationKind::Size);
        let mut baseline = batch
            .begin_graph_branch(
                program,
                &branch,
                &size,
                PositionGraphOperationOperand::SizeBaseline,
                destination,
                &no_adoption,
            )
            .unwrap();
        let calls = frame.0.get();
        assert_eq!(
            ready(
                batch
                    .advance_graph(&mut baseline, &frame, &no_adoption)
                    .unwrap()
            ),
            target(0, 4., 8.)
        );
        assert_eq!(
            frame.0.get(),
            calls,
            "baseline does not wait for the unavailable Required child"
        );
        batch.recycle_graph(baseline).unwrap();
        let mut value = batch
            .begin_graph_branch(
                program,
                &branch,
                &size,
                PositionGraphOperationOperand::SizeValue,
                destination,
                &no_adoption,
            )
            .unwrap();
        let dependency = graph_pending(
            batch
                .advance_graph(&mut value, &frame, &no_adoption)
                .unwrap(),
        );
        let calls = frame.0.get();
        assert!(
            batch
                .resume_graph(&mut value, Uuid::new_v4(), angles(25., 35.), None)
                .is_err()
        );
        assert!(
            batch
                .resume_graph(
                    &mut value,
                    dependency.request_id,
                    AttributeValue::Normalized(0.4),
                    None
                )
                .is_err()
        );
        assert_eq!(
            value.pending_request().unwrap().request_id,
            dependency.request_id
        );
        assert_eq!(
            graph_pending(
                batch
                    .advance_graph(&mut value, &frame, &no_adoption)
                    .unwrap()
            )
            .request_id,
            dependency.request_id
        );
        assert_eq!(
            frame.0.get(),
            calls,
            "waiting and rejected responses never rerun the prefix"
        );
        batch
            .resume_graph(&mut value, dependency.request_id, angles(25., 35.), None)
            .unwrap();
        for _ in 0..2 {
            assert_eq!(
                ready(
                    batch
                        .advance_graph(&mut value, &frame, &no_adoption)
                        .unwrap()
                ),
                angles(25., 35.)
            );
        }
        assert_eq!(frame.0.get(), calls);
        assert_eq!(sample.current_reads.get(), 1);
        assert!(parent.completed_value().is_none());
        assert_eq!(
            parent.pending_request().unwrap().request_id,
            size_request.request_id
        );
        let observed = Cell::new(0);
        assert!(
            batch
                .observe(&mut parent, &mut |_| {
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
            "operand readiness never authorizes an original parent observation"
        );
        batch.recycle_graph(value).unwrap();
        let mut again = batch
            .begin_graph_branch(
                program,
                &branch,
                &size,
                PositionGraphOperationOperand::SizeBaseline,
                destination,
                &no_adoption,
            )
            .unwrap();
        assert_eq!(
            ready(
                batch
                    .advance_graph(&mut again, &frame, &no_adoption)
                    .unwrap()
            ),
            target(0, 4., 8.),
            "recycled workspace must not retain another operand's resumed result"
        );
        batch.recycle_graph(again).unwrap();
        batch.recycle(parent).unwrap();
    });
}

#[test]
fn graph_bridge_rejects_foreign_registry_role_target_and_same_time_frame_then_remains_retryable() {
    let sample = sampled(1.);
    with_batch(&sample, |batch, program, engine, _| {
        let frame = Frame::default();
        let destination = FixtureId::new();
        let branch = program.registry().branch();
        let mut parent = batch
            .begin_branch(program, &branch, destination, &no_adoption)
            .unwrap();
        let request = pending(batch.advance(&mut parent, &frame, &no_adoption).unwrap());
        let locator = parent
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .unwrap();
        let foreign = CapturedPositionProgram::new(
            program.registry().capture_id(),
            &target(0, 4., 8.),
            &sample.samples,
        )
        .unwrap();
        assert!(
            batch
                .begin_graph_branch(
                    program,
                    &foreign.branch(),
                    &locator,
                    PositionGraphOperationOperand::RequiredOutgoing,
                    destination,
                    &no_adoption
                )
                .is_err()
        );
        let mut foreign_program = program.clone();
        foreign_program.registry = foreign;
        assert!(
            batch
                .begin_graph_branch(
                    &foreign_program,
                    &foreign_program.registry().branch(),
                    &locator,
                    PositionGraphOperationOperand::RequiredOutgoing,
                    destination,
                    &no_adoption
                )
                .is_err()
        );
        assert!(
            batch
                .begin_graph_branch(
                    program,
                    &branch,
                    &locator,
                    PositionGraphOperationOperand::SizeValue,
                    destination,
                    &no_adoption
                )
                .is_err()
        );
        let mut wrong_target = program.clone();
        wrong_target.target = FixtureId::new();
        assert!(
            batch
                .begin_graph_branch(
                    &wrong_target,
                    &branch,
                    &locator,
                    PositionGraphOperationOperand::RequiredOutgoing,
                    destination,
                    &no_adoption
                )
                .is_err()
        );
        let other_capture = engine.prepare_output_frame(Default::default());
        assert_eq!(
            other_capture.frame_token().sampled_at(),
            program.frame_token().sampled_at()
        );
        let mut wrong_frame = program.clone();
        wrong_frame.token = other_capture.frame_token();
        assert!(
            batch
                .begin_graph_branch(
                    &wrong_frame,
                    &branch,
                    &locator,
                    PositionGraphOperationOperand::RequiredOutgoing,
                    destination,
                    &no_adoption
                )
                .is_err()
        );
        let mut operand = batch
            .begin_graph_branch(
                program,
                &branch,
                &locator,
                PositionGraphOperationOperand::RequiredOutgoing,
                destination,
                &no_adoption,
            )
            .unwrap();
        with_batch(&sample, |foreign_batch, _, _, _| {
            assert!(
                foreign_batch
                    .advance_graph(&mut operand, &frame, &no_adoption)
                    .is_err()
            );
            assert!(
                foreign_batch
                    .resume_graph(&mut operand, Uuid::new_v4(), angles(1., 2.), None)
                    .is_err()
            );
        });
        assert_eq!(
            ready(
                batch
                    .advance_graph(&mut operand, &frame, &no_adoption)
                    .unwrap()
            ),
            target(0, 10., 20.)
        );
        assert_eq!(
            frame.0.get(),
            1,
            "foreign loans do not execute or poison the retained operand"
        );
        assert_eq!(
            parent.pending_request().unwrap().request_id,
            request.request_id
        );
        batch.recycle_graph(operand).unwrap();
        batch.recycle(parent).unwrap();
    });
}

#[test]
fn graph_bridge_source_failure_is_terminal_but_recycles_its_owned_workspace() {
    let sample = sampled(1.);
    with_batch(&sample, |batch, program, _, typed| {
        let frame = Frame::default();
        let branch = program.registry().branch();
        let mut parent = batch
            .begin_branch(program, &branch, program.target(), &no_adoption)
            .unwrap();
        let request = pending(batch.advance(&mut parent, &frame, &no_adoption).unwrap());
        let locator = parent
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .unwrap();
        let mut operand = batch
            .begin_graph_branch(
                program,
                &branch,
                &locator,
                PositionGraphOperationOperand::RequiredIncoming,
                program.target(),
                &no_adoption,
            )
            .unwrap();
        *typed.failure.borrow_mut() =
            Some(IntentError("injected captured source failure".into()).into());
        assert!(
            batch
                .advance_graph(&mut operand, &frame, &no_adoption)
                .is_err()
        );
        assert!(operand.pending_request().is_none());
        *typed.failure.borrow_mut() = None;
        assert!(
            batch
                .advance_graph(&mut operand, &frame, &no_adoption)
                .is_err()
        );
        assert!(
            batch
                .resume_graph(&mut operand, request.request_id, angles(1., 2.), None)
                .is_err()
        );
        batch.recycle_graph(operand).unwrap();
        let mut fresh = batch
            .begin_graph_branch(
                program,
                &branch,
                &locator,
                PositionGraphOperationOperand::RequiredIncoming,
                program.target(),
                &no_adoption,
            )
            .unwrap();
        assert_eq!(
            ready(
                batch
                    .advance_graph(&mut fresh, &frame, &no_adoption)
                    .unwrap()
            ),
            target(9, 30., 40.)
        );
        assert_eq!(
            parent.pending_request().unwrap().request_id,
            request.request_id
        );
        assert_eq!(
            frame.0.get(),
            1,
            "failed operand never reaches a frame resolver or original parent"
        );
        batch.recycle_graph(fresh).unwrap();
        batch.recycle(parent).unwrap();
    });
}

#[path = "graph_nested_tests.rs"]
mod graph_nested_tests;

#[path = "resume_tests.rs"]
mod resume_tests;
