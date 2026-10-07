//! Deferred Target Current uses the original captured family and the existing destination
//! evaluator. No scalar adoption, phase advance or provenance reconstruction is allowed.
use super::*;
use std::cell::RefCell;

fn occurrence(id: u128) -> DynamicSourceOccurrenceId {
    DynamicSourceOccurrenceId::new(Uuid::from_u128(id)).unwrap()
}
struct CapturedCurrent {
    original: AttributeValue,
    reads: Cell<usize>,
}
impl DynamicValueSourceResolver for CapturedCurrent {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        panic!("captured Target/Angle Current must not ask for scalar adoption")
    }
    fn try_position_current_family(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.reads.set(self.reads.get() + 1);
        Ok(Some(self.original.clone()))
    }
    fn current_family_occurrence(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Option<DynamicSourceOccurrenceId> {
        Some(occurrence(500))
    }
    fn current_dependency(&self, _: FixtureId, _: &DynamicValueAddress) -> DynamicSourceDependency {
        panic!("Current dependency must not trigger root adoption")
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
struct DestinationFrame {
    adopted: AttributeValue,
    originals: RefCell<Vec<AttributeValue>>,
}
impl WholeFamilyExpressionFrameResolver for DestinationFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        Err(TransitionError::Requires(requirement))
    }
    fn adopt_position_angles(
        &self,
        original: &AttributeValue,
    ) -> Result<AttributeValue, TransitionError> {
        self.originals.borrow_mut().push(original.clone());
        Ok(self.adopted.clone())
    }
}
fn frame(pan: f32, tilt: f32) -> DestinationFrame {
    DestinationFrame {
        adopted: angles(pan, tilt),
        originals: RefCell::new(Vec::new()),
    }
}
fn source(original: AttributeValue) -> CapturedCurrent {
    CapturedCurrent {
        original,
        reads: Cell::new(0),
    }
}
fn target_current() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(700),
        },
        [1., 2., 3.],
    )))
}
fn pair() -> Vec<DynamicRuntimeSample> {
    let mut pan = angle(ProgrammingComponent::Pan, 90.);
    if let E::Programming {
        occurrence: slot, ..
    } = &mut pan
    {
        *slot = Some(occurrence(100));
    }
    vec![
        sample(10, pan),
        sample(11, current(ProgrammingComponent::Tilt)),
    ]
}
fn compose(
    group: &DynamicFamilySampleGroup,
    base: &AttributeValue,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<AttributeValue, TransitionError> {
    compose_retained_dynamic_family_traced(
        group.owner,
        base,
        &group.samples,
        &FamilyCompositionContext::default(),
        frame,
        scratch,
    )
}

#[test]
fn target_current_angle_pair_is_deferred_until_each_destination_evaluation() {
    let source = source(target_current());
    let samples = pair();
    let before = samples
        .iter()
        .map(|sample| sample.expression.clone())
        .collect::<Vec<_>>();
    let mut preparation = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&samples, &source, None, &mut preparation).unwrap();
    assert!(prepared.requirements.is_empty());
    assert_eq!(prepared.families.len(), 1);
    assert_eq!(source.reads.get(), 1);
    let mut scratch = RetainedFamilyCompositionScratch::default();
    for tilt in [30., 70.] {
        let frame = frame(-20., tilt);
        assert_eq!(
            compose(&prepared.families[0], &angles(0., 0.), &frame, &mut scratch).unwrap(),
            angles(90., tilt)
        );
        assert_eq!(*frame.originals.borrow(), vec![source.original.clone()]);
        let trace = scratch.family_trace();
        let root = trace.root().unwrap();
        let pan = trace
            .sources_for_component(root, ProgrammingComponent::Pan)
            .unwrap();
        assert_eq!(pan.len(), 1);
        assert_eq!(pan[0].role, FamilyTraceRole::Authored);
        assert_eq!(pan[0].occurrence, Some(occurrence(100)));
        // Destination adoption carries no exact Target->Tilt field-transfer evidence.
        assert!(
            trace
                .sources_for_component(root, ProgrammingComponent::Tilt)
                .is_none()
        );
    }
    assert_eq!(
        source.reads.get(),
        1,
        "destination evaluation does not recapture Current"
    );
    assert_eq!(
        before,
        samples
            .iter()
            .map(|sample| sample.expression.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(source.original, target_current());
}

#[test]
fn original_angle_current_keeps_compatible_axis_source_scope_without_frame_adoption() {
    let source = source(angles(10., 30.));
    let mut preparation = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&pair(), &source, None, &mut preparation).unwrap();
    let frame = frame(999., 999.);
    let mut scratch = RetainedFamilyCompositionScratch::default();
    assert_eq!(
        compose(&prepared.families[0], &angles(0., 0.), &frame, &mut scratch).unwrap(),
        angles(90., 30.)
    );
    assert!(frame.originals.borrow().is_empty());
    let trace = scratch.family_trace();
    let tilt = trace
        .sources_for_component(trace.root().unwrap(), ProgrammingComponent::Tilt)
        .unwrap();
    assert_eq!(tilt.len(), 1);
    assert_eq!(tilt[0].role, FamilyTraceRole::CalculationDependency);
    assert_eq!(tilt[0].occurrence, Some(occurrence(500)));
    assert_eq!(
        tilt[0].rank.dynamic_identity().unwrap().lane_id,
        Uuid::from_u128(11)
    );
    assert_eq!(source.reads.get(), 1);
}

#[test]
fn held_resume_angle_pair_reuses_phase_but_resolves_current_in_each_destination() {
    let source = source(target_current());
    let samples = vec![
        sample(
            10,
            resume(
                Some(angle(ProgrammingComponent::Pan, 90.)),
                Some(angle(ProgrammingComponent::Pan, 30.)),
                0.25,
            ),
        ),
        sample(
            11,
            resume(
                Some(current(ProgrammingComponent::Tilt)),
                Some(current(ProgrammingComponent::Tilt)),
                0.25,
            ),
        ),
    ];
    let before = samples
        .iter()
        .map(|sample| sample.expression.clone())
        .collect::<Vec<_>>();
    let mut preparation = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&samples, &source, None, &mut preparation).unwrap();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    for tilt in [30., 70.] {
        let frame = frame(10., tilt);
        assert_eq!(
            compose(&prepared.families[0], &angles(0., 0.), &frame, &mut scratch).unwrap(),
            angles(75., tilt)
        );
        assert!(
            frame
                .originals
                .borrow()
                .iter()
                .all(|value| value == &source.original)
        );
    }
    assert_eq!(source.reads.get(), 1);
    assert_eq!(
        before,
        samples
            .iter()
            .map(|sample| sample.expression.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn whole_target_current_adopts_both_axes_once_and_missing_destination_stays_a_requirement() {
    let source = source(target_current());
    let samples = vec![sample(
        10,
        E::AngleCurrent {
            address: Arc::new(DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Angles,
                component: None,
            }),
        },
    )];
    let mut preparation = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&samples, &source, None, &mut preparation).unwrap();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    assert_eq!(
        compose(&prepared.families[0], &angles(0., 0.), &Frame, &mut scratch),
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles
        ))
    );
    let frame = frame(12., 34.);
    assert_eq!(
        compose(&prepared.families[0], &angles(0., 0.), &frame, &mut scratch).unwrap(),
        angles(12., 34.)
    );
    assert_eq!(*frame.originals.borrow(), vec![source.original.clone()]);
    assert_eq!(source.reads.get(), 1);
}
