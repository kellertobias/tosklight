//! Capture-local original Current reuse. This tests authoring/source reads, not per-copy
//! physical adoption; destination-specific solving remains in the Position adapter.
use super::*;
use crate::runtime::dynamic_source_origins::DynamicStaticProgrammerLane;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::tests::field;
use light_core::OpeningConvention;
use light_engine::{
    ContributionFamilyEntry, ContributionFamilyEvidence, ContributionFamilyFootprint,
    ContributionFamilyRole, ContributionSourceId,
};
use std::collections::HashMap;

struct Sources {
    values: HashMap<(FixtureId, AttributeKey), AttributeValue>,
    evidence: Arc<ContributionFamilyEvidence>,
    programmer_id: light_core::ProgrammerId,
}
impl Sources {
    fn new(values: impl IntoIterator<Item = ((FixtureId, AttributeKey), AttributeValue)>) -> Self {
        let programmer_id = light_core::ProgrammerId::new();
        Self {
            programmer_id,
            values: values.into_iter().collect(),
            evidence: Arc::new(ContributionFamilyEvidence::new(vec![
                ContributionFamilyEntry::new(
                    ContributionSourceId::programmer(programmer_id),
                    light_core::ProgrammerEditStamp {
                        changed_at: chrono::DateTime::from_timestamp_millis(1000).unwrap(),
                        programmer_order: 9,
                    },
                    ContributionFamilyFootprint::Whole,
                    ContributionFamilyRole::Authored,
                ),
            ])),
        }
    }
}
impl ScalarSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}
impl DynamicTickSource for Sources {
    fn value(&self, target: FixtureId, key: &AttributeKey) -> Option<&AttributeValue> {
        self.values.get(&(target, key.clone()))
    }
    fn family_evidence(
        &self,
        target: FixtureId,
        key: &AttributeKey,
    ) -> Option<&Arc<ContributionFamilyEvidence>> {
        self.values
            .contains_key(&(target, key.clone()))
            .then_some(&self.evidence)
    }
}
struct Counting<'a, S> {
    inner: &'a S,
    reads: RefCell<HashMap<(FixtureId, AttributeKey), usize>>,
    evidence_reads: Cell<usize>,
}
impl<'a, S> Counting<'a, S> {
    fn new(inner: &'a S) -> Self {
        Self {
            inner,
            reads: Default::default(),
            evidence_reads: Cell::new(0),
        }
    }
    fn reads(&self, target: FixtureId, owner: ProgrammingOwner) -> usize {
        self.reads
            .borrow()
            .get(&(target, owner.key()))
            .copied()
            .unwrap_or(0)
    }
}
impl<S: DynamicTickSource> ScalarSourceResolver for Counting<'_, S> {
    fn current(&self, target: FixtureId, key: &AttributeKey) -> Option<f32> {
        self.inner.current(target, key)
    }
    fn preset(&self, id: &str, target: FixtureId, key: &AttributeKey) -> Option<f32> {
        self.inner.preset(id, target, key)
    }
}
impl<S: DynamicTickSource> DynamicTickSource for Counting<'_, S> {
    fn value(&self, target: FixtureId, key: &AttributeKey) -> Option<&AttributeValue> {
        *self
            .reads
            .borrow_mut()
            .entry((target, key.clone()))
            .or_default() += 1;
        self.inner.value(target, key)
    }
    fn family_evidence(
        &self,
        target: FixtureId,
        key: &AttributeKey,
    ) -> Option<&Arc<ContributionFamilyEvidence>> {
        self.evidence_reads.set(self.evidence_reads.get() + 1);
        self.inner.family_evidence(target, key)
    }
}
fn dependency_lane(
    origins: &DynamicSourceOrigins,
    occurrence: DynamicSourceOccurrenceId,
) -> DynamicStaticProgrammerLane {
    let DynamicSourceOrigin::StaticBaseline { sources } = &origins.get(occurrence).unwrap().origin
    else {
        panic!("original static evidence")
    };
    assert_eq!(sources.len(), 1);
    let DynamicStaticSource::Programmer { lane, .. } = &sources[0].source else {
        panic!("Programmer origin")
    };
    lane.clone()
}

#[test]
fn original_family_is_reused_across_addresses_without_caching_adopted_values_as_the_baseline() {
    let first = FixtureId::new();
    let second = FixtureId::new();
    let original = target_source().family;
    let color = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }));
    let source = Sources::new([
        ((first, ProgrammingOwner::Position.key()), original.clone()),
        (
            (second, ProgrammingOwner::Position.key()),
            position(10., 20.),
        ),
        ((first, ProgrammingOwner::Color.key()), color.clone()),
    ]);
    let counted = Counting::new(&source);
    let adopted = Cell::new(0);
    let adoption = |_, _: &AttributeValue, _: &DynamicValueAddress| {
        adopted.set(adopted.get() + 1);
        Ok(position(450., -30.))
    };
    let mut origins = DynamicSourceOrigins::default();
    let captured = CapturedProgrammingSources::new(&counted, &adoption, None)
        .with_source_transaction(&mut origins);
    let pan = axis(ProgrammingComponent::Pan);
    let tilt = axis(ProgrammingComponent::Tilt);
    assert_eq!(
        captured.try_current_family_base(first, &pan).unwrap(),
        Some(original.clone())
    );
    let occurrence = captured.current_family_occurrence(first, &pan).unwrap();
    for _ in 0..3 {
        assert_eq!(
            captured.try_current(first, &pan).unwrap(),
            Some(DynamicValue::Scalar(450.))
        );
        assert_eq!(
            captured.try_current(first, &tilt).unwrap(),
            Some(DynamicValue::Scalar(-30.))
        );
        for address in [&pan, &tilt] {
            assert_eq!(
                captured.try_current_family_base(first, address).unwrap(),
                Some(original.clone())
            );
            let dependency = captured.current_dependency(first, address);
            assert_eq!(dependency.occurrence, Some(occurrence));
            assert!(matches!(
                dependency.transfer,
                DynamicSourceTransfer::Unknown
            ));
        }
    }
    assert_eq!(
        adopted.get(),
        2,
        "adoption is still address-specific and reused once per address"
    );
    assert_eq!(
        captured.try_current(second, &pan).unwrap(),
        Some(DynamicValue::Scalar(10.))
    );
    let color_address = DynamicValueAddress::whole_family(ProgrammingOwner::Color, &color).unwrap();
    assert_eq!(
        captured
            .try_current_family_base(first, &color_address)
            .unwrap(),
        Some(color)
    );
    for (target, owner) in [
        (first, ProgrammingOwner::Position),
        (second, ProgrammingOwner::Position),
        (first, ProgrammingOwner::Color),
    ] {
        assert_eq!(
            counted.reads(target, owner),
            1,
            "logical target and owner remain separate cache keys"
        );
    }
    assert_eq!(
        counted.evidence_reads.get(),
        1,
        "repeated addresses preserve one original occurrence binding"
    );
    captured.check().unwrap();
    captured.finish_source_bindings().unwrap();
    drop(captured);
    let DynamicSourceOrigin::StaticBaseline { sources } = &origins.get(occurrence).unwrap().origin
    else {
        panic!("unchanged original evidence")
    };
    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0].source,
        DynamicStaticSource::Programmer {
            programmer_id: source.programmer_id,
            lane: DynamicStaticProgrammerLane::Live
        }
    );
}

#[test]
fn absence_is_memoized_but_a_fresh_captured_source_has_its_own_cache() {
    let target = FixtureId::new();
    let source = Sources::new([]);
    let counted = Counting::new(&source);
    let mut origins = DynamicSourceOrigins::default();
    let captured = CapturedProgrammingSources::new(&counted, &no_adoption, None)
        .with_source_transaction(&mut origins);
    for _ in 0..3 {
        for component in [ProgrammingComponent::Pan, ProgrammingComponent::Tilt] {
            let address = axis(component);
            assert!(
                captured
                    .try_current_family_base(target, &address)
                    .unwrap()
                    .is_none()
            );
            assert!(captured.try_current(target, &address).unwrap().is_none());
            assert!(
                captured
                    .current_family_occurrence(target, &address)
                    .is_none()
            );
        }
    }
    assert_eq!(counted.reads(target, ProgrammingOwner::Position), 1);
    assert_eq!(counted.evidence_reads.get(), 0);
    assert!(captured.requirements().is_empty());
    captured.finish_source_bindings().unwrap();
    drop(captured);
    assert!(
        origins
            .binding(&DynamicSourceBinding::StaticBaseline {
                target,
                owner: ProgrammingOwner::Position
            })
            .is_none()
    );
    let fresh = CapturedProgrammingSources::new(&counted, &no_adoption, None);
    assert!(
        fresh
            .try_current_family_base(target, &axis(ProgrammingComponent::Pan))
            .unwrap()
            .is_none()
    );
    assert_eq!(counted.reads(target, ProgrammingOwner::Position), 2);
}

#[test]
fn actual_live_and_preload_frames_keep_separate_original_values_and_evidence_during_replay() {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    let target = FixtureId::new();
    programmers.start(session);
    programmers.set(
        session,
        target,
        ProgrammingOwner::Position.key(),
        position(10., 20.),
    );
    let engine = Engine::with_programming_contract_support(
        programmers.clone(),
        PROGRAMMING_CONTRACT_VERSION,
    );
    programmers.arm_preload(session, true);
    clock.advance_millis(10);
    programmers.set(
        session,
        target,
        ProgrammingOwner::Position.key(),
        position(60., 40.),
    );
    let frame = engine.prepare_output_frame(Default::default());
    let input = engine.prepare_preload_frame(&frame, None);
    let state = light_engine::PreloadFrameState::default();
    let live_source = TickSources::prepared(&engine, &frame, &[]);
    let pending_source = PreloadTickSources {
        engine: &engine,
        input: &input,
        state: &state,
        before_release: false,
        baseline_samples: &[],
        values: OnceLock::new(),
    };
    let live = Counting::new(&live_source);
    let pending = Counting::new(&pending_source);
    let mut live_origins = DynamicSourceOrigins::default();
    let mut pending_origins = DynamicSourceOrigins::default();
    let current = CapturedProgrammingSources::new(&live, &no_adoption, None)
        .with_source_transaction(&mut live_origins);
    let preview = CapturedProgrammingSources::new(&pending, &no_adoption, None)
        .with_source_transaction(&mut pending_origins);
    // The underlying captured engine sources are still lazy when the later edit happens.
    clock.advance_millis(10);
    programmers.set(
        session,
        target,
        ProgrammingOwner::Position.key(),
        position(90., 60.),
    );
    let pan = axis(ProgrammingComponent::Pan);
    let tilt = axis(ProgrammingComponent::Tilt);
    for _ in 0..3 {
        assert_eq!(
            current.try_current_family_base(target, &pan).unwrap(),
            Some(position(10., 20.))
        );
        assert_eq!(
            preview.try_current_family_base(target, &tilt).unwrap(),
            Some(position(60., 40.))
        );
        assert_eq!(
            current.try_current(target, &pan).unwrap(),
            Some(DynamicValue::Scalar(10.))
        );
        assert_eq!(
            preview.try_current(target, &tilt).unwrap(),
            Some(DynamicValue::Scalar(40.))
        );
    }
    let live_id = current.current_family_occurrence(target, &pan).unwrap();
    let pending_id = preview.current_family_occurrence(target, &tilt).unwrap();
    assert_ne!(live_id, pending_id);
    assert_eq!(live.reads(target, ProgrammingOwner::Position), 1);
    assert_eq!(pending.reads(target, ProgrammingOwner::Position), 1);
    current.finish_source_bindings().unwrap();
    preview.finish_source_bindings().unwrap();
    drop((current, preview));
    assert!(matches!(
        dependency_lane(&live_origins, live_id),
        DynamicStaticProgrammerLane::Live
    ));
    assert!(matches!(
        dependency_lane(&pending_origins, pending_id),
        DynamicStaticProgrammerLane::Preload
    ));
    assert!(live_origins.get(pending_id).is_none());
    assert!(pending_origins.get(live_id).is_none());
    let fresh_frame = engine.prepare_output_frame(Default::default());
    let fresh_input = engine.prepare_preload_frame(&fresh_frame, None);
    let fresh_source = PreloadTickSources {
        engine: &engine,
        input: &fresh_input,
        state: &state,
        before_release: false,
        baseline_samples: &[],
        values: OnceLock::new(),
    };
    let fresh = Counting::new(&fresh_source);
    let next = CapturedProgrammingSources::new(&fresh, &no_adoption, None);
    assert_eq!(
        next.try_current_family_base(target, &pan).unwrap(),
        Some(position(90., 60.))
    );
    assert_eq!(fresh.reads(target, ProgrammingOwner::Position), 1);
}

#[test]
fn legacy_zoom_family_requirement_does_not_poison_addressed_scalar_adoption() {
    let target = FixtureId::new();
    let source = Sources::new([(
        (target, ProgrammingOwner::Zoom.key()),
        AttributeValue::RawDmxExact(16384),
    )]);
    let counted = Counting::new(&source);
    let calls = Cell::new(0);
    let adopt = |_, original: &AttributeValue, _: &DynamicValueAddress| {
        assert_eq!(original, &AttributeValue::RawDmxExact(16384));
        calls.set(calls.get() + 1);
        Ok(field(35.))
    };
    let captured = CapturedProgrammingSources::new(&counted, &adopt, None);
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Zoom {
            convention: OpeningConvention::Field,
        },
        component: Some(ProgrammingComponent::Zoom),
    };
    for _ in 0..3 {
        assert!(matches!(
            captured.try_current_family_base(target, &address),
            Err(TransitionError::Requires(
                TransitionRequirement::ZoomConvention
            ))
        ));
        assert_eq!(
            captured.try_current(target, &address).unwrap(),
            Some(DynamicValue::Scalar(35.))
        );
    }
    assert_eq!(calls.get(), 1);
    assert_eq!(counted.reads(target, ProgrammingOwner::Zoom), 1);
    assert_eq!(captured.requirements().len(), 1);
    assert_eq!(
        captured.requirements()[0].requirement,
        TransitionRequirement::ZoomConvention
    );
    captured.check().unwrap();
}
