use super::*;
use std::cell::Cell;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn component(
    component: ProgrammingComponent,
    value: f32,
    row: usize,
    mix: f32,
) -> FamilyCompositionSample {
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
        FamilySampleRank {
            priority: 10,
            changed_at_millis: 100,
            changed_at_submillis_nanos: 0,
            stable_order: row as u128,
            identity: FamilySampleIdentity::Fixed {
                source: FamilyFixedSampleSource::Programmer,
                row_index: row,
            },
        },
        mix,
    )
    .unwrap()
    .into_fix_at()
    .into()
}
struct Frame;
impl WholeFamilyExpressionFrameResolver for Frame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("component adoption should use its captured adoption callback: {requirement:?}");
    }
}

#[test]
fn public_component_cut_retains_actual_rank_trace_and_recycles_abandoned_workspace() {
    let capture = Uuid::from_u128(801);
    let base = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [4., 2., 3.],
    )));
    let samples = [
        component(ProgrammingComponent::Pan, 90., 1, 0.5),
        component(ProgrammingComponent::Tilt, 40., 2, 1.),
    ];
    let calls = Cell::new(0);
    let blocked = |from: &AttributeValue, address: &DynamicValueAddress| {
        calls.set(calls.get() + 1);
        assert_eq!(from, &base);
        assert_eq!(address.component, Some(ProgrammingComponent::Pan));
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&blocked),
        ..Default::default()
    };
    let mut driver = begin_retained_position_composition(
        capture,
        &base,
        &samples,
        &context,
        RetainedFamilyCompositionScratch::default(),
        true,
    )
    .unwrap();
    let PositionCompositionProgress::NeedsMaterialization(pending) =
        driver.advance(capture, &context, &Frame).unwrap()
    else {
        panic!()
    };
    assert_eq!(pending.requirement, TransitionRequirement::LiveJointAngles);
    let PositionCompositionOperation::Base {
        source_index,
        rank,
        request,
    } = &pending.operation
    else {
        panic!()
    };
    assert_eq!(
        *source_index,
        usize::MAX,
        "segment request is not an authored source index"
    );
    assert_eq!(*rank, samples[0].rank());
    let base_evaluation::BaseMaterializationOperation::Segment(segment) = &request.operation else {
        panic!()
    };
    assert_eq!(segment.rank, *rank);
    assert!(
        matches!(&segment.operation, PositionSegmentOperation::Adoption { address, .. }
        if address.component == Some(ProgrammingComponent::Pan))
    );
    assert!(
        driver
            .resume_materialization(capture, pending.request_id, base.clone(), None)
            .is_err()
    );
    for _ in 0..3 {
        let PositionCompositionProgress::NeedsMaterialization(repeated) =
            driver.advance(capture, &context, &Frame).unwrap()
        else {
            panic!()
        };
        assert_eq!(repeated.request_id, pending.request_id);
    }
    assert_eq!(calls.get(), 1);
    driver
        .resume_materialization(capture, pending.request_id, angles(10., 20.), None)
        .unwrap();
    let PositionCompositionProgress::Complete(actual) =
        driver.advance(capture, &context, &Frame).unwrap()
    else {
        panic!()
    };
    assert_eq!(actual, angles(50., 40.));
    let adopted = |_: &AttributeValue, _: &DynamicValueAddress| Ok(angles(10., 20.));
    let available = FamilyCompositionContext {
        resolve_adoption: Some(&adopted),
        ..Default::default()
    };
    let mut reference = RetainedFamilyCompositionScratch::default();
    let expected = compose_retained_dynamic_family_traced(
        ProgrammingOwner::Position,
        &base,
        &samples,
        &available,
        &Frame,
        &mut reference,
    )
    .unwrap();
    assert_eq!(actual, expected);
    let fields = ProgrammingFieldScope::for_value(ProgrammingOwner::Position, &actual).unwrap();
    let query =
        |trace: &FamilyTraceArena| trace.query_fields_with_base(trace.root().unwrap(), &fields);
    assert_eq!(
        query(driver.family_trace()),
        query(reference.family_trace())
    );

    let driver = begin_retained_position_composition(
        capture,
        &base,
        &samples,
        &context,
        driver.into_scratch(),
        true,
    )
    .unwrap();
    let mut driver = driver;
    assert!(matches!(
        driver.advance(capture, &context, &Frame).unwrap(),
        PositionCompositionProgress::NeedsMaterialization(_)
    ));
    let scratch = driver.into_scratch();
    assert!(
        scratch.batches.len() > 0,
        "abandoned segment batch returns to pool"
    );
    let mut retry =
        begin_retained_position_composition(capture, &base, &samples, &available, scratch, true)
            .unwrap();
    let PositionCompositionProgress::Complete(reused) =
        retry.advance(capture, &available, &Frame).unwrap()
    else {
        panic!()
    };
    assert_eq!(reused, actual);
    assert_eq!(query(retry.family_trace()), query(reference.family_trace()));
}
