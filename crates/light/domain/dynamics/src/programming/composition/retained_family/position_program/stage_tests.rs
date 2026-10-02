//! Public stage replay follows original source routes through the real mask/prefix compositor.
use super::*;
use crate::{
    CapturedPositionProgram, DynamicSampleExpression, DynamicTransitionReason,
    PositionResumeEndpoint, PositionResumeScope, PositionStageOperand,
    PositionStageOperandProgress,
};
use std::cell::Cell;

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
        identity: crate::FamilySampleIdentity::Dynamic {
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
    let address = match &value {
        DynamicValue::Family(value) => {
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, value).unwrap()
        }
        _ => DynamicValueAddress {
            representation: crate::DynamicFamilyRepresentation::Angles,
            component,
        },
    };
    FamilySample::new(
        Arc::new(CompiledDynamicValueAddress::new(address, None).unwrap()),
        value,
        rank(order),
        mix,
    )
    .unwrap()
    .into_fix_at()
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
struct Frame;
impl WholeFamilyExpressionFrameResolver for Frame {
    fn resolve(
        &self,
        _: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("this test must reach actual segment adoption, not another transition stage")
    }
}
fn needed(progress: PositionCompositionProgress) -> PositionCompositionRequest {
    let PositionCompositionProgress::NeedsMaterialization(request) = progress else {
        panic!("pending component adoption")
    };
    let PositionCompositionOperation::Base { request: base, .. } = &request.operation else {
        panic!("not a mask or completion")
    };
    let base_evaluation::BaseMaterializationOperation::Segment(segment) = &base.operation else {
        panic!("actual segment stage")
    };
    assert!(
        matches!(&segment.operation, PositionSegmentOperation::Adoption { address, .. } if address.component == Some(ProgrammingComponent::Pan))
    );
    request
}
fn operand(progress: PositionStageOperandProgress) -> AttributeValue {
    match progress {
        PositionStageOperandProgress::OperandReady(value) => value,
        PositionStageOperandProgress::NeedsMaterialization(_) => {
            panic!("no unrelated materialization precedes this operand")
        }
        PositionStageOperandProgress::Inactive => panic!("original stage remains reachable"),
    }
}

#[test]
fn public_stage_rebind_replays_changed_resume_and_prior_mask_but_excludes_trigger_and_suffix() {
    let capture = Uuid::new_v4();
    let base = target(0., 0.);
    let sources = sources();
    let registry = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    let branch = registry.branch();
    let blocked_calls = Cell::new(0);
    let blocked = |from: &AttributeValue, _: &DynamicValueAddress| {
        blocked_calls.set(blocked_calls.get() + 1);
        assert_eq!(
            from,
            &target(30., 45.),
            "original Resume and preceding partial whole FixAT are already composed"
        );
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&blocked),
        ..Default::default()
    };
    let mut parent = branch
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let request = needed(parent.advance(capture, &context, &Frame).unwrap());
    let locator = parent
        .pending_stage_locator(request.request_id)
        .unwrap()
        .expect("segment owns an executable original stage");
    assert!(parent.pending_stage_locator(Uuid::new_v4()).is_err());
    let mut changed = branch.clone();
    changed
        .choose_resume(scope(), PositionResumeEndpoint::Incoming)
        .unwrap();
    let available_calls = Cell::new(0);
    let available = |_: &AttributeValue, _: &DynamicValueAddress| {
        available_calls.set(available_calls.get() + 1);
        Ok(angles(35., 50.))
    };
    let available_context = FamilyCompositionContext {
        resolve_adoption: Some(&available),
        ..Default::default()
    };
    let mut replay = changed
        .begin_stage_operand(
            &locator,
            PositionStageOperand::AdoptionInput,
            &available_context,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    assert!(
        replay
            .advance(Uuid::new_v4(), &available_context, &Frame)
            .is_err(),
        "foreign capture cannot consume or poison the original replay"
    );
    assert!(
        changed
            .begin_stage_operand(
                &locator,
                PositionStageOperand::TransitionFrom,
                &available_context,
                RetainedFamilyCompositionScratch::default(),
                true
            )
            .is_err(),
        "an adoption locator cannot authorize a transition endpoint"
    );
    assert_eq!(
        operand(replay.advance(capture, &available_context, &Frame).unwrap()),
        target(35., 50.)
    );
    assert_eq!(
        available_calls.get(),
        0,
        "the cut intercepts the stage before a now-synchronous adoption"
    );
    assert_eq!(
        operand(replay.advance(capture, &available_context, &Frame).unwrap()),
        target(35., 50.),
        "operand result is stable on repeated reads"
    );
    assert_eq!(
        needed(parent.advance(capture, &context, &Frame).unwrap()).request_id,
        request.request_id
    );
    assert_eq!(
        blocked_calls.get(),
        1,
        "child replay never advances or retries the original parent"
    );
    parent
        .resume_materialization(capture, request.request_id, angles(30., 45.), None)
        .unwrap();
    let PositionCompositionProgress::Complete(value) =
        parent.advance(capture, &context, &Frame).unwrap()
    else {
        panic!("parent completes once")
    };
    assert_eq!(
        value,
        angles(180., 240.),
        "Pan half blend, Tilt write and later whole mask each apply only in parent"
    );
    let original = branch.conditioned_sources().unwrap();
    let (
        FamilyCompositionSample::WholeExpression {
            expression: before, ..
        },
        Some(FamilyCompositionSample::WholeExpression {
            expression: after, ..
        }),
    ) = (&sources[0], &original[0])
    else {
        panic!()
    };
    assert!(
        Arc::ptr_eq(before, after),
        "the captured original source is never rewritten by child conditioning"
    );
    let mut original_replay = branch
        .begin_stage_operand(
            &locator,
            PositionStageOperand::AdoptionInput,
            &available_context,
            replay.into_scratch(),
            true,
        )
        .unwrap();
    assert_eq!(
        operand(
            original_replay
                .advance(capture, &available_context, &Frame)
                .unwrap()
        ),
        target(30., 45.),
        "reused workspace cannot retain the child branch prefix"
    );
}

#[test]
fn public_stage_locator_rejects_equal_foreign_registry_and_removed_trigger_is_inactive() {
    let capture = Uuid::new_v4();
    let base = target(0., 0.);
    let sources = sources();
    let registry = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    let blocked = |_: &AttributeValue, _: &DynamicValueAddress| {
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&blocked),
        ..Default::default()
    };
    let mut parent = registry
        .branch()
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), false)
        .unwrap();
    let request = needed(parent.advance(capture, &context, &Frame).unwrap());
    let locator = parent
        .pending_stage_locator(request.request_id)
        .unwrap()
        .unwrap();
    let foreign = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    assert!(
        foreign
            .branch()
            .begin_stage_operand(
                &locator,
                PositionStageOperand::AdoptionInput,
                &context,
                RetainedFamilyCompositionScratch::default(),
                false
            )
            .is_err(),
        "same capture UUID, Arcs, ranks and values do not authorize a foreign registry"
    );
    let available = |_: &AttributeValue, _: &DynamicValueAddress| Ok(angles(30., 45.));
    let available_context = FamilyCompositionContext {
        resolve_adoption: Some(&available),
        ..Default::default()
    };
    let mut removed = registry.branch();
    removed.replace_source(2, None).unwrap();
    let mut replay = removed
        .begin_stage_operand(
            &locator,
            PositionStageOperand::AdoptionInput,
            &available_context,
            RetainedFamilyCompositionScratch::default(),
            false,
        )
        .unwrap();
    assert!(
        matches!(
            replay.advance(capture, &available_context, &Frame).unwrap(),
            PositionStageOperandProgress::Inactive
        ),
        "another remaining component cannot impersonate the removed Pan adoption"
    );
    assert_eq!(
        needed(parent.advance(capture, &context, &Frame).unwrap()).request_id,
        request.request_id
    );
}

#[test]
fn public_stage_locator_keeps_original_cohort_member_after_preceding_member_is_released() {
    assert_cohort_member_compaction(false);
}

#[test]
fn public_stage_locator_replays_actual_nested_sourcecohort_segment_after_member_release() {
    assert_cohort_member_compaction(true);
}

fn assert_cohort_member_compaction(nested: bool) {
    use crate::programming::expression_coupled::PositionForestNode;
    use crate::{CoupledCohortEndpoint, CoupledComponentEndpoint, CoupledLeafRole};
    let capture = Uuid::new_v4();
    let base = target(4., 8.);
    let released = Arc::new(DynamicSampleExpression::Transition {
        from: Some(leaf(target(10., 20.))),
        to: None,
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(5001),
        },
    });
    let address = DynamicValueAddress {
        representation: crate::DynamicFamilyRepresentation::Target {
            reference: Some(TargetReference::Point {
                point_id: Uuid::from_u128(42),
            }),
        },
        component: Some(ProgrammingComponent::TargetX),
    };
    let members: Arc<[CoupledCohortEndpoint]> = vec![
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(10),
            expression: Arc::new(
                CompiledProgrammingFamilyExpression::new(
                    released,
                    ProgrammingOwner::Position,
                    None,
                    None,
                )
                .unwrap(),
            ),
        },
        CoupledCohortEndpoint::Materialized(CoupledComponentEndpoint {
            lane_id: Uuid::from_u128(20),
            address: Arc::new(CompiledDynamicValueAddress::new(address, None).unwrap()),
            value: DynamicValue::Scalar(90.),
            role: CoupledLeafRole::Authored,
            occurrence: None,
            dependency_occurrence: None,
        }),
    ]
    .into();
    let mut nodes = vec![PositionForestNode::SourceCohort(members)];
    if nested {
        nodes.push(PositionForestNode::Whole {
            expression: leaf(target(70., 80.)),
            lane_id: Uuid::from_u128(30),
            sources: Arc::from([]),
        });
        nodes.push(PositionForestNode::Transition {
            from: Some(0),
            to: Some(1),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(5002),
            },
        });
    }
    let root = nodes.len() - 1;
    let source = FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::from_position_forest(Arc::from([]), &nodes, root).unwrap(),
        ),
        rank: rank(1),
        activation_mix: 1.,
    };
    let registry = CapturedPositionProgram::new(capture, &base, &[source]).unwrap();
    let blocked = |_: &AttributeValue, address: &DynamicValueAddress| {
        assert_eq!(address.component, Some(ProgrammingComponent::TargetX));
        Err(TransitionError::Requires(
            TransitionRequirement::LiveTargetPoints,
        ))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&blocked),
        ..Default::default()
    };
    let mut parent = registry
        .branch()
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    let PositionCompositionProgress::NeedsMaterialization(request) =
        parent.advance(capture, &context, &Frame).unwrap()
    else {
        panic!("original second member requires reference adoption")
    };
    if nested {
        let PositionCompositionOperation::Base { request: base, .. } = &request.operation else {
            panic!("actual nested base request")
        };
        let base_evaluation::BaseMaterializationOperation::SourceCohort { request: child, .. } =
            &base.operation
        else {
            panic!(
                "outer interior forest must retain a raw SourceCohort request, not expand to top-level sources"
            )
        };
        let base_evaluation::BaseMaterializationOperation::Segment(segment) = &child.operation
        else {
            panic!("nested actual segment stage")
        };
        assert!(
            matches!(&segment.operation, PositionSegmentOperation::Adoption { address, .. } if address.component == Some(ProgrammingComponent::TargetX))
        );
    }
    let locator = parent
        .pending_stage_locator(request.request_id)
        .unwrap()
        .expect("original member stage locator");
    let mut compacted = registry.branch();
    compacted
        .choose_resume(scope(), PositionResumeEndpoint::Incoming)
        .unwrap();
    let mut replay = compacted
        .begin_stage_operand(
            &locator,
            PositionStageOperand::AdoptionInput,
            &context,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        operand(replay.advance(capture, &context, &Frame).unwrap()),
        base,
        "compacted member zero still names original member one, and excludes the incoming TargetX write"
    );
    let PositionCompositionProgress::NeedsMaterialization(repeated) =
        parent.advance(capture, &context, &Frame).unwrap()
    else {
        panic!()
    };
    assert_eq!(repeated.request_id, request.request_id);
}

#[test]
fn public_stage_locator_distinguishes_underlay_use_from_final_use_of_same_original_trigger() {
    let capture = Uuid::new_v4();
    let base = target(4., 8.);
    let sources = vec![
        source(
            DynamicValue::Scalar(90.),
            Some(ProgrammingComponent::Pan),
            1,
            0.5,
        ),
        FamilyCompositionSample::WholeExpression {
            expression: Arc::new(
                CompiledProgrammingFamilyExpression::new(
                    leaf(angles(100., 120.)),
                    ProgrammingOwner::Position,
                    None,
                    None,
                )
                .unwrap(),
            ),
            rank: rank(2),
            activation_mix: 0.5,
        },
    ];
    let registry = CapturedPositionProgram::new(capture, &base, &sources).unwrap();
    let blocked = |_: &AttributeValue, _: &DynamicValueAddress| {
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&blocked),
        ..Default::default()
    };
    let mut parent = registry
        .branch()
        .begin_composition(&context, RetainedFamilyCompositionScratch::default(), true)
        .unwrap();
    // Resolving the partial higher Whole first composes its lower Pan underlay. This is
    // the same original Pan source later used as a final segment when that Whole is removed.
    let request = needed(parent.advance(capture, &context, &Frame).unwrap());
    let locator = parent
        .pending_stage_locator(request.request_id)
        .unwrap()
        .unwrap();
    let available_calls = Cell::new(0);
    let available = |_: &AttributeValue, _: &DynamicValueAddress| {
        available_calls.set(available_calls.get() + 1);
        Ok(angles(10., 20.))
    };
    let available_context = FamilyCompositionContext {
        resolve_adoption: Some(&available),
        ..Default::default()
    };
    let mut original = registry
        .branch()
        .begin_stage_operand(
            &locator,
            PositionStageOperand::AdoptionInput,
            &available_context,
            RetainedFamilyCompositionScratch::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        operand(
            original
                .advance(capture, &available_context, &Frame)
                .unwrap()
        ),
        base
    );
    assert_eq!(available_calls.get(), 0);
    let mut no_consumer = registry.branch();
    no_consumer.replace_source(1, None).unwrap();
    let mut final_use = no_consumer
        .begin_stage_operand(
            &locator,
            PositionStageOperand::AdoptionInput,
            &available_context,
            original.into_scratch(),
            true,
        )
        .unwrap();
    assert!(
        matches!(
            final_use
                .advance(capture, &available_context, &Frame)
                .unwrap(),
            PositionStageOperandProgress::Inactive
        ),
        "same original source, rank and operands in a different lexical use cannot satisfy the underlay locator"
    );
    assert_eq!(
        available_calls.get(),
        1,
        "the remaining final Pan stage really executes, but never impersonates the absent underlay stage"
    );
    assert_eq!(
        needed(parent.advance(capture, &context, &Frame).unwrap()).request_id,
        request.request_id
    );
}
