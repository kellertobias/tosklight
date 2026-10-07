//! Discovery exercises the actual retained driver; candidates need separate operand replay.
use super::*;
use crate::{CapturedPositionProgram, DynamicSampleExpression, DynamicTransitionReason};
use std::cell::Cell;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [4., 8., 0.],
    )))
}
fn rank(order: u128) -> FamilySampleRank {
    FamilySampleRank {
        priority: 10,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: order,
        identity: crate::FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(1000 + order),
            controller_id: Uuid::from_u128(2000 + order),
            lane_id: Uuid::from_u128(3000 + order),
        },
    }
}
fn component(
    component: ProgrammingComponent,
    value: f32,
    order: u128,
    mix: f32,
) -> FamilyCompositionSample {
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: crate::DynamicFamilyRepresentation::Angles,
                    component: Some(component),
                },
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Scalar(value),
        rank(order),
        mix,
    )
    .unwrap()
    .into_fix_at()
    .into()
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
fn complete(progress: PositionStageDiscoveryProgress) -> PositionStageDiscoveryReport {
    match progress {
        PositionStageDiscoveryProgress::Complete(report) => report,
        PositionStageDiscoveryProgress::NeedsMaterialization(_) => {
            panic!("synchronous discovery must complete")
        }
    }
}
fn pending(progress: PositionStageDiscoveryProgress) -> PositionCompositionRequest {
    match progress {
        PositionStageDiscoveryProgress::NeedsMaterialization(request) => request,
        PositionStageDiscoveryProgress::Complete(_) => {
            panic!("nested transition must remain pending")
        }
    }
}

#[test]
fn discovers_synchronous_conversion_without_a_deferred_parent_request() {
    let capture = Uuid::new_v4();
    let base = target();
    let samples = [component(ProgrammingComponent::Pan, 90., 1, 1.)];
    let registry = CapturedPositionProgram::new(capture, &base, &samples).unwrap();
    let calls = Cell::new(0);
    let adoption = |_: &AttributeValue, _: &DynamicValueAddress| {
        calls.set(calls.get() + 1);
        Ok(angles(10., 20.))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&adoption),
        ..Default::default()
    };
    let frame = Frame {
        calls: Cell::new(0),
    };
    let branch = registry.branch();
    let mut discovery = branch
        .begin_stage_discovery(
            &context,
            RetainedFamilyCompositionScratch::default(),
            true,
            20,
        )
        .unwrap();
    assert!(!discovery.report().unwrap().complete);
    assert!(discovery.advance(Uuid::new_v4(), &context, &frame).is_err());
    let report = complete(discovery.advance(capture, &context, &frame).unwrap());
    assert!(report.complete);
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.inspected_routes, 1);
    assert_eq!(report.unmapped_routes, 0);
    assert_eq!(calls.get(), 1);
    assert_eq!(frame.calls.get(), 0);
    let repeated = complete(discovery.advance(capture, &context, &frame).unwrap());
    assert_eq!(repeated.candidates, report.candidates);
    assert_eq!(repeated.inspected_routes, report.inspected_routes);
    assert_eq!(
        calls.get(),
        1,
        "repeated completion must not replay conversion"
    );
    let mut replay = branch
        .begin_stage_operand(
            &report.candidates[0],
            PositionStageOperand::AdoptionInput,
            &context,
            discovery.into_scratch(),
            true,
        )
        .unwrap();
    let PositionStageOperandProgress::OperandReady(value) =
        replay.advance(capture, &context, &frame).unwrap()
    else {
        panic!("actual adoption cut")
    };
    assert_eq!(value, base);
    assert_eq!(
        calls.get(),
        1,
        "operand stops before synchronous conversion"
    );
}

#[test]
fn partial_prefix_waits_retries_and_discovers_later_stages_after_nested_response() {
    let capture = Uuid::new_v4();
    let expression = Arc::new(DynamicSampleExpression::Transition {
        from: Some(leaf(target())),
        to: Some(leaf(angles(40., 50.))),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(501),
        },
    });
    let samples = [
        component(ProgrammingComponent::Pan, 10., 1, 0.5),
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
            rank: rank(2),
            activation_mix: 0.5,
        },
        component(ProgrammingComponent::Tilt, 60., 3, 1.),
    ];
    let base = angles(0., 0.);
    let registry = CapturedPositionProgram::new(capture, &base, &samples).unwrap();
    let context = FamilyCompositionContext::default();
    let frame = Frame {
        calls: Cell::new(0),
    };
    let mut discovery = registry
        .branch()
        .begin_stage_discovery(
            &context,
            RetainedFamilyCompositionScratch::default(),
            true,
            100,
        )
        .unwrap();
    let request = pending(discovery.advance(capture, &context, &frame).unwrap());
    let prefix = discovery.report().unwrap();
    assert!(!prefix.complete);
    assert!(
        !prefix.candidates.is_empty(),
        "actual underlay prefix is enumerated before nested endpoint materialization"
    );
    let calls = frame.calls.get();
    assert!(
        discovery
            .resume_materialization(Uuid::new_v4(), request.request_id, angles(20., 25.), None)
            .is_err()
    );
    assert!(
        discovery
            .resume_materialization(capture, Uuid::new_v4(), angles(20., 25.), None)
            .is_err()
    );
    // Non-Position transfer identities are invalid and must not consume the request/prefix.
    let foreign = ProgrammingTransitionTrace {
        from: ProgrammingFieldTransfer {
            identity: ProgrammingFieldScope::new([ProgrammingTraceField::Uv]),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        discovery
            .resume_materialization(capture, request.request_id, angles(20., 25.), Some(foreign))
            .is_err()
    );
    let repeated = pending(discovery.advance(capture, &context, &frame).unwrap());
    assert_eq!(request.request_id, repeated.request_id);
    assert_eq!(discovery.report().unwrap().candidates, prefix.candidates);
    assert_eq!(frame.calls.get(), calls);
    discovery
        .resume_materialization(capture, request.request_id, angles(20., 25.), None)
        .unwrap();
    let report = complete(discovery.advance(capture, &context, &frame).unwrap());
    assert!(report.complete);
    assert!(
        report.inspected_routes > prefix.inspected_routes,
        "completion must include stages reached after the retained nested request"
    );
    assert_eq!(
        &report.candidates[..prefix.candidates.len()],
        &prefix.candidates
    );
}

#[test]
fn equal_capture_foreign_registry_cannot_replay_a_discovered_locator() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let samples = [component(ProgrammingComponent::Pan, 90., 1, 1.)];
    let registry = CapturedPositionProgram::new(capture, &base, &samples).unwrap();
    let foreign = CapturedPositionProgram::new(capture, &base, &samples).unwrap();
    let context = FamilyCompositionContext::default();
    let frame = Frame {
        calls: Cell::new(0),
    };
    let mut discovery = registry
        .branch()
        .begin_stage_discovery(
            &context,
            RetainedFamilyCompositionScratch::default(),
            false,
            10,
        )
        .unwrap();
    let report = complete(discovery.advance(capture, &context, &frame).unwrap());
    assert!(
        foreign
            .branch()
            .begin_stage_operand(
                &report.candidates[0],
                PositionStageOperand::AdoptionInput,
                &context,
                discovery.into_scratch(),
                false
            )
            .is_err()
    );
}

#[test]
fn covered_candidate_is_inactive_while_winning_candidate_has_an_operand() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let samples = [
        component(ProgrammingComponent::Pan, 10., 1, 0.5),
        component(ProgrammingComponent::Pan, 90., 2, 1.),
    ];
    let registry = CapturedPositionProgram::new(capture, &base, &samples).unwrap();
    let context = FamilyCompositionContext::default();
    let frame = Frame {
        calls: Cell::new(0),
    };
    let branch = registry.branch();
    let mut discovery = branch
        .begin_stage_discovery(
            &context,
            RetainedFamilyCompositionScratch::default(),
            false,
            10,
        )
        .unwrap();
    let report = complete(discovery.advance(capture, &context, &frame).unwrap());
    assert_eq!(
        report.candidates.len(),
        2,
        "syntactic candidates are not active-operation proof"
    );
    let mut inactive = 0;
    let mut ready = 0;
    for candidate in &report.candidates {
        let mut replay = branch
            .begin_stage_operand(
                candidate,
                PositionStageOperand::AdoptionInput,
                &context,
                RetainedFamilyCompositionScratch::default(),
                false,
            )
            .unwrap();
        match replay.advance(capture, &context, &frame).unwrap() {
            PositionStageOperandProgress::Inactive => inactive += 1,
            PositionStageOperandProgress::OperandReady(value) => {
                assert_eq!(value, base);
                ready += 1;
            }
            PositionStageOperandProgress::NeedsMaterialization(_) => panic!("compatible angles"),
        }
    }
    assert_eq!((inactive, ready), (1, 1));
}

#[test]
fn route_limit_exhaustion_and_failure_never_certify_enumeration() {
    let capture = Uuid::new_v4();
    let base = angles(0., 0.);
    let samples = [
        component(ProgrammingComponent::Pan, 10., 1, 1.),
        component(ProgrammingComponent::Tilt, 20., 2, 1.),
    ];
    let registry = CapturedPositionProgram::new(capture, &base, &samples).unwrap();
    let context = FamilyCompositionContext::default();
    let frame = Frame {
        calls: Cell::new(0),
    };
    assert!(
        registry
            .branch()
            .begin_stage_discovery(
                &context,
                RetainedFamilyCompositionScratch::default(),
                false,
                0
            )
            .is_err()
    );
    let mut limited = registry
        .branch()
        .begin_stage_discovery(
            &context,
            RetainedFamilyCompositionScratch::default(),
            false,
            1,
        )
        .unwrap();
    assert!(limited.advance(capture, &context, &frame).is_err());
    assert!(limited.report().is_err());
    assert!(limited.advance(capture, &context, &frame).is_err());
    let mut reusable = registry
        .branch()
        .begin_stage_discovery(&context, limited.into_scratch(), false, 10)
        .unwrap();
    assert!(complete(reusable.advance(capture, &context, &frame).unwrap()).complete);
}

#[test]
fn ordinary_complete_dynamic_angle_pair_reports_unmapped_synthetic_assembly() {
    let capture = Uuid::new_v4();
    let base = angles(10., 20.);
    let mut pan = match component(ProgrammingComponent::Pan, 90., 1, 0.5) {
        FamilyCompositionSample::Known(sample) => sample,
        _ => unreachable!(),
    };
    let mut tilt = match component(ProgrammingComponent::Tilt, 45., 1, 0.5) {
        FamilyCompositionSample::Known(sample) => sample,
        _ => unreachable!(),
    };
    pan.fix_at = false;
    tilt.fix_at = false;
    // Both axes belong to the same real controller/rank, with distinct original lane IDs.
    tilt.rank.identity = crate::FamilySampleIdentity::Dynamic {
        instance_id: Uuid::from_u128(1001),
        controller_id: Uuid::from_u128(2001),
        lane_id: Uuid::from_u128(3002),
    };
    let samples = [pan.into(), tilt.into()];
    let registry = CapturedPositionProgram::new(capture, &base, &samples).unwrap();
    // A compatible baseline isolates assembly identity from deferred physical conversion.
    let context = FamilyCompositionContext::default();
    let frame = Frame {
        calls: Cell::new(0),
    };
    let mut discovery = registry
        .branch()
        .begin_stage_discovery(
            &context,
            RetainedFamilyCompositionScratch::default(),
            true,
            20,
        )
        .unwrap();
    let report = complete(discovery.advance(capture, &context, &frame).unwrap());
    assert!(report.complete);
    assert!(
        report.inspected_routes > 0,
        "the complete Dynamic pair reaches actual assembly"
    );
    assert_eq!(
        frame.calls.get(),
        0,
        "compatible paired assembly remains synchronous"
    );
    assert!(
        report.candidates.is_empty(),
        "two original contributors cannot manufacture one exact source stage"
    );
    assert_eq!(report.inspected_routes, report.unmapped_routes);
}

#[test]
fn scalar_coupled_endpoint_reports_unsupported_lexical_route_without_authority() {
    let capture = Uuid::new_v4();
    let base = target();
    let scalar = Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: crate::DynamicFamilyRepresentation::Target {
                reference: Some(TargetReference::Origin),
            },
            component: Some(ProgrammingComponent::TargetX),
        }),
        value: DynamicValue::Scalar(10.),
        occurrence: None,
        dependency_occurrence: None,
    });
    let expression = Arc::new(DynamicSampleExpression::Transition {
        from: Some(scalar),
        to: Some(leaf(target())),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(555),
        },
    });
    let source = FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(CompiledCoupledExpression::new(expression, None).unwrap()),
        rank: rank(1),
        activation_mix: 1.,
    };
    let registry = CapturedPositionProgram::new(capture, &base, &[source]).unwrap();
    let context = FamilyCompositionContext::default();
    let frame = Frame {
        calls: Cell::new(0),
    };
    let mut discovery = registry
        .branch()
        .begin_stage_discovery(
            &context,
            RetainedFamilyCompositionScratch::default(),
            false,
            20,
        )
        .unwrap();
    let report = complete(discovery.advance(capture, &context, &frame).unwrap());
    assert!(report.complete);
    assert!(
        report.unmapped_routes > 0,
        "actual scalar endpoint lexical use remains unsupported"
    );
    assert!(report.inspected_routes > report.candidates.len());
}
