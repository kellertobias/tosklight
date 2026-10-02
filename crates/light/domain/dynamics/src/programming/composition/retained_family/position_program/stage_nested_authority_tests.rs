//! Authentic nested cut authority from speculative stage operands. Real sampler emissions
//! and compiled FixAT masks exercise routing/retry; the algebraic frame is not physical fitting.
use super::*;
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
    current_reads: usize,
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
        current_reads: sources.calls.get(),
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
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for Frame {
    fn adopt_position_angles(
        &self,
        value: &AttributeValue,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        if self.blocked {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        }
        Ok(angle_value(value))
    }
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        assert!(!self.panic, "intentional retained prefix unwind");
        if self.blocked {
            return Err(TransitionError::Requires(requirement));
        }
        let transition =
            CompiledProgrammingTransition::new(angle_value(from), angle_value(to), None)?;
        match operation {
            FamilyExpressionOperation::Transition { progress } => transition.sample(progress),
            FamilyExpressionOperation::Scale { factor } => transition.scale(factor),
        }
    }
}
fn adopt(
    value: &AttributeValue,
    address: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    let candidate = angle_value(value);
    if address.matches_authored_source(&candidate) {
        Ok(candidate)
    } else {
        let AttributeValue::Position(position) = candidate else {
            panic!("Angles")
        };
        let PositionIntent::Angles {
            pan_degrees: ScalarIntent::Value(pan),
            tilt_degrees: ScalarIntent::Value(tilt),
        } = position.as_ref()
        else {
            panic!("literal Angles")
        };
        Ok(target(0, *pan, *tilt))
    }
}
fn parent_pending(progress: PositionCompositionProgress) -> PositionCompositionRequest {
    match progress {
        PositionCompositionProgress::NeedsMaterialization(request) => request,
        PositionCompositionProgress::Complete(_) => panic!("original parent must remain pending"),
    }
}
fn pending(progress: PositionStageOperandProgress) -> PositionCompositionRequest {
    match progress {
        PositionStageOperandProgress::NeedsMaterialization(request) => request,
        _ => panic!("original stage prefix must suspend"),
    }
}
fn ready(progress: PositionStageOperandProgress) -> AttributeValue {
    match progress {
        PositionStageOperandProgress::OperandReady(value) => value,
        _ => panic!("original stage must return its operand"),
    }
}
fn assert_refused(driver: &PositionStageOperandContinuation, id: Uuid) {
    assert!(driver.pending_graph_operation_locator(id).is_err());
    assert!(driver.pending_stage_locator(id).is_err());
    assert!(driver.pending_envelope_locator(id).is_err());
    assert!(driver.pending_mask_locator(id).is_err());
}
fn outer_adoption(
    registry: &CapturedPositionProgram,
    context: &FamilyCompositionContext<'_>,
) -> PositionMaskLocator {
    let mut discovery = registry
        .branch()
        .begin_stage_discovery(
            context,
            RetainedFamilyCompositionScratch::default(),
            true,
            64,
        )
        .unwrap();
    let PositionStageDiscoveryProgress::Complete(report) = discovery
        .advance(registry.capture_id(), context, &Frame::default())
        .unwrap()
    else {
        panic!("available frame completes real mask discovery")
    };
    report
        .mask_candidates
        .into_iter()
        .rev()
        .find(|locator| locator.stage() == PositionMaskStage::Adoption)
        .unwrap()
}

#[test]
fn genuine_required_and_size_pending_in_mask_operand_keep_exact_origin_and_parent() {
    let sample = sampled(1.5);
    assert_eq!(
        sample.current_reads, 1,
        "real producer captures Size Current once"
    );
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let sources = vec![
        whole(sample.expression.clone(), sample.rank, 1.),
        mask(target(0, 200., 300.), 1, 0.5),
    ];
    let registry = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    let branch = registry.branch();
    let context = FamilyCompositionContext::default();
    let mut parent = branch
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let outer = parent_pending(
        parent
            .advance(capture, &context, &Frame::default())
            .unwrap(),
    );
    assert!(matches!(
        outer.operation,
        PositionCompositionOperation::MaskAdoption { .. }
    ));
    let mask_locator = parent
        .pending_mask_locator(outer.request_id)
        .unwrap()
        .unwrap();
    let blocked = Frame {
        blocked: true,
        ..Default::default()
    };
    let mut replay = branch
        .begin_mask_operand(
            &mask_locator,
            PositionMaskOperand::AdoptionInput,
            &context,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    let request = pending(replay.advance(capture, &context, &blocked).unwrap());
    let required = replay
        .pending_graph_operation_locator(request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(required.kind(), PositionGraphOperationKind::Required);
    assert_eq!(required.capture_id(), capture);
    assert!(
        registry
            .source_node_for_origin(request.origin().unwrap())
            .unwrap()
            .as_ref()
            == Some(required.operation_node())
    );
    let expression = registry.node_expression(required.operation_node()).unwrap();
    let DynamicSampleExpression::Retained { tape, root } = expression.as_ref() else {
        panic!("exact retained operation")
    };
    let handles = tape.operation_handles(*root).unwrap();
    assert_eq!(handles.len(), 1);
    assert_eq!(handles[0].target(), sample.target);
    assert_eq!(
        handles[0].lane_id(),
        sample.rank.dynamic_identity().unwrap().lane_id
    );
    assert!(
        replay
            .pending_stage_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert!(
        replay
            .pending_envelope_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert!(
        replay
            .pending_mask_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert_refused(&replay, Uuid::new_v4());
    let calls = blocked.calls.get();
    assert!(replay.advance(Uuid::new_v4(), &context, &blocked).is_err());
    assert!(
        replay
            .resume_materialization(Uuid::new_v4(), request.request_id, angles(20., 30.), None)
            .is_err()
    );
    assert!(
        replay
            .resume_materialization(capture, Uuid::new_v4(), angles(20., 30.), None)
            .is_err()
    );
    assert!(
        replay
            .resume_materialization(
                capture,
                request.request_id,
                AttributeValue::Normalized(0.4),
                None
            )
            .is_err()
    );
    for _ in 0..2 {
        assert_eq!(
            pending(replay.advance(capture, &context, &blocked).unwrap()).request_id,
            request.request_id
        );
        assert_eq!(
            replay
                .pending_graph_operation_locator(request.request_id)
                .unwrap(),
            Some(required.clone())
        );
    }
    assert_eq!(
        blocked.calls.get(),
        calls,
        "waiting/rejection never repeats prefix callbacks"
    );
    let foreign = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    assert!(
        foreign
            .branch()
            .validate_graph_operation_operand(
                &required,
                PositionGraphOperationOperand::RequiredIncoming
            )
            .is_err()
    );
    let mut incoming = branch
        .begin_graph_operation_operand(
            &required,
            PositionGraphOperationOperand::RequiredIncoming,
            &context,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    assert!(
        matches!(incoming.advance(capture, &context, &blocked).unwrap(), PositionGraphOperationOperandProgress::OperandReady(value) if value == target(9, 30., 40.))
    );
    incoming.into_scratch();
    replay
        .resume_materialization(capture, request.request_id, angles(20., 30.), None)
        .unwrap();
    let size_request = pending(replay.advance(capture, &context, &blocked).unwrap());
    let size = replay
        .pending_graph_operation_locator(size_request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(size.kind(), PositionGraphOperationKind::Size);
    assert!(size.operation_node() != required.operation_node());
    assert!(
        registry
            .source_node_for_origin(size_request.origin().unwrap())
            .unwrap()
            .as_ref()
            == Some(size.operation_node())
    );
    replay
        .resume_materialization(capture, size_request.request_id, angles(28., 41.), None)
        .unwrap();
    for _ in 0..2 {
        assert_eq!(
            ready(replay.advance(capture, &context, &blocked).unwrap()),
            angles(28., 41.)
        );
        assert_refused(&replay, size_request.request_id);
    }
    assert_eq!(
        parent_pending(
            parent
                .advance(capture, &context, &Frame::default())
                .unwrap()
        )
        .request_id,
        outer.request_id
    );
    assert_eq!(
        parent.pending_mask_locator(outer.request_id).unwrap(),
        Some(mask_locator)
    );
    replay.into_scratch();
    parent.into_scratch();
}

#[test]
fn mask_operand_forwards_only_actual_nested_envelope_segment_and_mask_types() {
    let capture = Uuid::new_v4();
    let resolving = FamilyCompositionContext {
        resolve_adoption: Some(&adopt),
        ..Default::default()
    };
    let deferred = FamilyCompositionContext::default();
    // An actual Known endpoint Activation sits below the outer mask.
    let known = FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Position, &target(0, 10., 20.))
                    .unwrap(),
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Family(target(0, 10., 20.)),
        sampled(1.).rank,
        0.5,
    )
    .unwrap();
    let registry = CapturedPositionProgram::new(
        capture,
        &angles(0., 0.),
        &[known.into(), mask(target(0, 50., 60.), 1, 0.5)],
    )
    .unwrap();
    let outer = outer_adoption(&registry, &resolving);
    let blocked = Frame {
        blocked: true,
        ..Default::default()
    };
    let mut replay = registry
        .branch()
        .begin_mask_operand(
            &outer,
            PositionMaskOperand::AdoptionInput,
            &deferred,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    let request = pending(replay.advance(capture, &deferred, &blocked).unwrap());
    let envelope = replay
        .pending_envelope_locator(request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(envelope.stage(), PositionCompletionStage::Activation);
    assert!(
        replay
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert!(
        replay
            .pending_stage_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert!(
        replay
            .pending_mask_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    let mut input = registry
        .branch()
        .begin_envelope_operand(
            &envelope,
            PositionEnvelopeOperand::ActivationEndpoint,
            &deferred,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        ready(input.advance(capture, &deferred, &blocked).unwrap()),
        target(0, 10., 20.)
    );
    assert_eq!(
        pending(replay.advance(capture, &deferred, &blocked).unwrap()).request_id,
        request.request_id
    );
    input.into_scratch();
    replay.into_scratch();

    // A real component operation requires adopting the lower Target before applying Pan.
    let component = FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Angles,
                    component: Some(ProgrammingComponent::Pan),
                },
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Scalar(90.),
        sampled(1.).rank,
        1.,
    )
    .unwrap();
    let registry = CapturedPositionProgram::new(
        capture,
        &target(0, 10., 20.),
        &[
            component.into_fix_at().into(),
            mask(target(0, 50., 60.), 1, 0.5),
        ],
    )
    .unwrap();
    let outer = outer_adoption(&registry, &resolving);
    let mut replay = registry
        .branch()
        .begin_mask_operand(
            &outer,
            PositionMaskOperand::AdoptionInput,
            &deferred,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    let request = pending(
        replay
            .advance(capture, &deferred, &Frame::default())
            .unwrap(),
    );
    let stage = replay
        .pending_stage_locator(request.request_id)
        .unwrap()
        .unwrap();
    assert!(
        replay
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert!(
        replay
            .pending_envelope_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert!(
        replay
            .pending_mask_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    let mut input = registry
        .branch()
        .begin_stage_operand(
            &stage,
            PositionStageOperand::AdoptionInput,
            &deferred,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        ready(
            input
                .advance(capture, &deferred, &Frame::default())
                .unwrap()
        ),
        target(0, 10., 20.)
    );
    input.into_scratch();
    replay.into_scratch();

    // A lower genuine compiled partial FixAT owns its own MaskAdoption route.
    let registry = CapturedPositionProgram::new(
        capture,
        &angles(0., 0.),
        &[
            mask(target(0, 10., 20.), 1, 0.5),
            mask(angles(50., 60.), 2, 0.5),
        ],
    )
    .unwrap();
    let outer = outer_adoption(&registry, &resolving);
    let mut replay = registry
        .branch()
        .begin_mask_operand(
            &outer,
            PositionMaskOperand::AdoptionInput,
            &deferred,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    let request = pending(
        replay
            .advance(capture, &deferred, &Frame::default())
            .unwrap(),
    );
    let lower = replay
        .pending_mask_locator(request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(lower.stage(), PositionMaskStage::Adoption);
    assert_ne!(lower, outer, "same capture is not same mask boundary");
    assert!(
        replay
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert!(
        replay
            .pending_stage_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert!(
        replay
            .pending_envelope_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    let mut input = registry
        .branch()
        .begin_mask_operand(
            &lower,
            PositionMaskOperand::AdoptionInput,
            &deferred,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        ready(
            input
                .advance(capture, &deferred, &Frame::default())
                .unwrap()
        ),
        angles(0., 0.)
    );
    input.into_scratch();
    replay.into_scratch();
}

struct Current {
    error: Cell<bool>,
    calls: Cell<usize>,
}
impl DynamicValueSourceResolver for Current {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        None
    }
    fn try_current_family_base(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        if self.error.get() {
            return Err(IntentError("captured Current source failed".into()).into());
        }
        Ok(Some(target(0, 4., 8.)))
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
#[test]
fn source_failure_and_unwind_revoke_nested_authority_and_recovered_scratch_is_clean() {
    let sample = sampled(1.);
    let capture = Uuid::new_v4();
    let registry = CapturedPositionProgram::new(
        capture,
        &angles(0., 0.),
        &[
            whole(sample.expression, sample.rank, 1.),
            mask(target(0, 200., 300.), 1, 0.5),
        ],
    )
    .unwrap();
    let current = Current {
        error: Cell::new(false),
        calls: Cell::new(0),
    };
    let controls = |_: FamilySampleRank| FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.5 };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&adopt),
        endpoint_output: Some(FamilyEndpointOutputContext {
            control: &controls,
            target: sample.target,
            current: &current,
            native_models: None,
        }),
        ..Default::default()
    };
    let outer = outer_adoption(&registry, &context);
    let mut scratch = RetainedFamilyCompositionScratch::default();
    for unwind in [false, true] {
        let mut replay = registry
            .branch()
            .begin_mask_operand(
                &outer,
                PositionMaskOperand::AdoptionInput,
                &context,
                scratch,
                true,
            )
            .unwrap();
        let blocked = Frame {
            blocked: true,
            ..Default::default()
        };
        let request = pending(replay.advance(capture, &context, &blocked).unwrap());
        assert!(
            replay
                .pending_graph_operation_locator(request.request_id)
                .unwrap()
                .is_some()
        );
        if unwind {
            let panicking = Frame {
                panic: true,
                ..Default::default()
            };
            // Finish the first held graph request, then fail at the real Current envelope.
            replay
                .resume_materialization(capture, request.request_id, angles(20., 30.), None)
                .unwrap();
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || replay.advance(capture, &context, &panicking)
                ))
                .is_err()
            );
        } else {
            replay
                .resume_materialization(capture, request.request_id, angles(20., 30.), None)
                .unwrap();
            current.error.set(true);
            let calls = current.calls.get();
            assert!(
                replay
                    .advance(capture, &context, &Frame::default())
                    .is_err()
            );
            assert_eq!(current.calls.get(), calls + 1);
            current.error.set(false);
        }
        assert_refused(&replay, request.request_id);
        assert!(
            replay
                .advance(capture, &context, &Frame::default())
                .is_err()
        );
        scratch = replay.into_scratch();
        let mut recovered = registry
            .branch()
            .begin_mask_operand(
                &outer,
                PositionMaskOperand::AdoptionInput,
                &context,
                scratch,
                true,
            )
            .unwrap();
        let value = ready(
            recovered
                .advance(capture, &context, &Frame::default())
                .unwrap(),
        );
        assert_eq!(
            value,
            angles(12., 19.),
            "fresh prefix applies Current envelope once"
        );
        let calls = current.calls.get();
        assert_eq!(
            ready(
                recovered
                    .advance(capture, &context, &Frame::default())
                    .unwrap()
            ),
            value
        );
        assert_eq!(current.calls.get(), calls);
        scratch = recovered.into_scratch();
    }
}

#[test]
fn synthetic_angle_pair_pending_has_no_unrelated_stage_or_graph_authority() {
    let capture = Uuid::new_v4();
    let rank = sampled(1.).rank;
    let component = |component, value, rank| {
        FamilySample::new(
            Arc::new(
                CompiledDynamicValueAddress::new(
                    DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::Angles,
                        component: Some(component),
                    },
                    None,
                )
                .unwrap(),
            ),
            DynamicValue::Scalar(value),
            rank,
            0.5,
        )
        .unwrap()
        .into()
    };
    let sources = [
        component(ProgrammingComponent::Pan, 90., rank),
        component(
            ProgrammingComponent::Tilt,
            40.,
            rank.with_dynamic_lane(Uuid::new_v4()).unwrap(),
        ),
        mask(target(0, 50., 60.), 1, 0.5),
    ];
    let registry = CapturedPositionProgram::new(capture, &target(0, 10., 20.), &sources).unwrap();
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&adopt),
        ..Default::default()
    };
    let outer = outer_adoption(&registry, &context);
    let mut replay = registry
        .branch()
        .begin_mask_operand(
            &outer,
            PositionMaskOperand::AdoptionInput,
            &context,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    let blocked = Frame {
        blocked: true,
        ..Default::default()
    };
    let request = pending(replay.advance(capture, &context, &blocked).unwrap());
    assert!(
        matches!(&request.operation, PositionCompositionOperation::Base { request: base, .. }
        if matches!(&base.operation, PositionCompositionBaseOperation::Completion(completion)
            if completion.stage == PositionCompletionStage::Activation))
    );
    let origin = request.origin().unwrap();
    assert_eq!(origin.original_source_indices().len(), 2);
    assert!(origin.original_source_indices().contains(&0));
    assert!(origin.original_source_indices().contains(&1));
    assert!(
        registry.source_node_for_origin(origin).unwrap().is_none(),
        "the synthetic pair has actual source membership, not an original retained operation"
    );
    assert!(
        replay
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert!(
        replay
            .pending_stage_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert!(
        replay
            .pending_envelope_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    assert!(
        replay
            .pending_mask_locator(request.request_id)
            .unwrap()
            .is_none()
    );
    let calls = blocked.calls.get();
    assert_eq!(
        pending(replay.advance(capture, &context, &blocked).unwrap()).request_id,
        request.request_id
    );
    assert_eq!(blocked.calls.get(), calls);
    replay.into_scratch();
}
