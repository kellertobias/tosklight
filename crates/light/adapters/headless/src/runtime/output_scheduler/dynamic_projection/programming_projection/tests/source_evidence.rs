use super::*;
use crate::runtime::dynamic_source_origins::DynamicStaticProgrammerLane;
use light_engine::{
    ContributionFamilyEntry, ContributionFamilyEvidence, ContributionFamilyFootprint,
    ContributionFamilyRole, ContributionSourceId,
};

fn dependency_source(
    origins: &DynamicSourceOrigins,
    id: DynamicSourceOccurrenceId,
) -> &DynamicStaticSource {
    let DynamicSourceOrigin::StaticBaseline { sources } = &origins.get(id).unwrap().origin else {
        panic!("expected static dependency")
    };
    assert_eq!(sources.len(), 1);
    &sources[0].source
}

#[test]
fn live_and_preload_current_bind_their_captured_sources_without_observing_later_edits() {
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
    let live = TickSources::prepared(&engine, &frame, &[]);
    let pending = PreloadTickSources {
        engine: &engine,
        input: &input,
        state: &state,
        before_release: false,
        baseline_samples: &[],
        values: OnceLock::new(),
    };
    let mut live_origins = DynamicSourceOrigins::default();
    let mut pending_origins = live_origins.clone();
    // Both observations are still lazy; later edits must affect neither value nor provenance.
    assert!(live.values.get().is_none());
    assert!(pending.values.get().is_none());
    clock.advance_millis(10);
    programmers.set(
        session,
        target,
        ProgrammingOwner::Position.key(),
        position(90., 60.),
    );
    let current = CapturedProgrammingSources::new(&live, &no_adoption, None)
        .with_source_transaction(&mut live_origins);
    let preview = CapturedProgrammingSources::new(&pending, &no_adoption, None)
        .with_source_transaction(&mut pending_origins);
    let address = axis(ProgrammingComponent::Tilt);
    assert_eq!(
        current.current(target, &address),
        Some(DynamicValue::Scalar(20.))
    );
    assert_eq!(
        preview.current(target, &address),
        Some(DynamicValue::Scalar(40.))
    );
    let live_id = current.current_family_occurrence(target, &address).unwrap();
    let pending_id = preview.current_family_occurrence(target, &address).unwrap();
    assert_ne!(live_id, pending_id);
    assert_eq!(
        current.current_family_occurrence(target, &axis(ProgrammingComponent::Pan)),
        Some(live_id)
    );
    current.check().unwrap();
    preview.check().unwrap();
    current.finish_source_bindings().unwrap();
    preview.finish_source_bindings().unwrap();
    drop((current, preview));
    assert!(matches!(
        dependency_source(&live_origins, live_id),
        DynamicStaticSource::Programmer {
            lane: DynamicStaticProgrammerLane::Live,
            ..
        }
    ));
    assert!(matches!(
        dependency_source(&pending_origins, pending_id),
        DynamicStaticSource::Programmer {
            lane: DynamicStaticProgrammerLane::Preload,
            ..
        }
    ));
    assert!(live_origins.get(pending_id).is_none());
    assert!(pending_origins.get(live_id).is_none());
    let unchanged = pending_origins.clone();
    let repeat = CapturedProgrammingSources::new(&pending, &no_adoption, None)
        .with_source_transaction(&mut pending_origins);
    assert_eq!(
        repeat.current_family_occurrence(target, &address),
        Some(pending_id)
    );
    drop(repeat);
    assert!(unchanged.shares_storage(&pending_origins));
    let unused = CapturedProgrammingSources::new(&pending, &no_adoption, None)
        .with_source_transaction(&mut pending_origins);
    unused.finish_source_bindings().unwrap();
    drop(unused);
    assert!(
        pending_origins
            .binding(&DynamicSourceBinding::StaticBaseline {
                target,
                owner: ProgrammingOwner::Position
            })
            .is_none()
    );
    assert!(
        pending_origins.get(pending_id).is_some(),
        "unqueried sources retire without rewriting retained history"
    );
}

struct EvidenceSource {
    base: StaticSource,
    evidence: Option<Arc<ContributionFamilyEvidence>>,
    queries: Cell<usize>,
}

#[test]
fn singleton_fast_path_changes_identity_for_action_ordinal_footprint_and_role() {
    let target = FixtureId::new();
    let source = ContributionSourceId::playback(light_playback::SequenceMasterSource {
        playback_number: None,
        playback_identity: None,
        cue_list_id: light_core::CueListId::new(),
        temporary: false,
    });
    let stamp = light_core::ProgrammerEditStamp {
        changed_at: chrono::DateTime::from_timestamp(100, 123).unwrap(),
        programmer_order: 0,
    };
    let mut origins = DynamicSourceOrigins::default();
    let mut previous = None;
    for (ordinal, footprint, role) in [
        (
            41,
            ContributionFamilyFootprint::Whole,
            ContributionFamilyRole::Authored,
        ),
        (
            42,
            ContributionFamilyFootprint::Whole,
            ContributionFamilyRole::Authored,
        ),
        (
            42,
            ContributionFamilyFootprint::Component(ProgrammingComponent::Pan),
            ContributionFamilyRole::Authored,
        ),
        (
            42,
            ContributionFamilyFootprint::Component(ProgrammingComponent::Pan),
            ContributionFamilyRole::CalculationDependency,
        ),
    ] {
        let entry = ContributionFamilyEntry::new(source.clone(), stamp, footprint, role)
            .with_transition_ordinal(Some(ordinal));
        let source = EvidenceSource {
            base: StaticSource {
                target,
                family: position(30., 15.),
            },
            evidence: Some(Arc::new(ContributionFamilyEvidence::new(vec![entry]))),
            queries: Cell::new(0),
        };
        let typed = CapturedProgrammingSources::new(&source, &no_adoption, None)
            .with_source_transaction(&mut origins);
        let id = typed
            .current_family_occurrence(target, &axis(ProgrammingComponent::Pan))
            .unwrap();
        typed.check().unwrap();
        drop(typed);
        assert_ne!(
            Some(id),
            previous,
            "equal payload/source/time is insufficient for occurrence identity"
        );
        let unchanged = origins.clone();
        let repeat = CapturedProgrammingSources::new(&source, &no_adoption, None)
            .with_source_transaction(&mut origins);
        assert_eq!(
            repeat.current_family_occurrence(target, &axis(ProgrammingComponent::Pan)),
            Some(id)
        );
        drop(repeat);
        assert!(unchanged.shares_storage(&origins));
        previous = Some(id);
    }
}
impl ScalarSourceResolver for EvidenceSource {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}
impl DynamicTickSource for EvidenceSource {
    fn value(&self, target: FixtureId, attribute: &AttributeKey) -> Option<&AttributeValue> {
        self.base.value(target, attribute)
    }
    fn family_evidence(
        &self,
        target: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&Arc<ContributionFamilyEvidence>> {
        self.queries.set(self.queries.get() + 1);
        self.base
            .value(target, attribute)
            .and(self.evidence.as_ref())
    }
}

#[test]
fn explicit_component_and_dependency_evidence_survives_and_unknown_clears_only_the_live_lookup() {
    let target = FixtureId::new();
    let stamp = light_core::ProgrammerEditStamp {
        changed_at: chrono::DateTime::from_timestamp(100, 123456).unwrap(),
        programmer_order: 9,
    };
    let entries = vec![
        ContributionFamilyEntry::new(
            ContributionSourceId::programmer_group(light_core::ProgrammerId::new(), "front/é"),
            stamp,
            ContributionFamilyFootprint::Component(ProgrammingComponent::Pan),
            ContributionFamilyRole::Authored,
        ),
        ContributionFamilyEntry::new(
            ContributionSourceId::preload(light_core::ProgrammerId::new()),
            stamp,
            ContributionFamilyFootprint::Component(ProgrammingComponent::Tilt),
            ContributionFamilyRole::CalculationDependency,
        ),
    ];
    let mut source = EvidenceSource {
        base: StaticSource {
            target,
            family: position(30., 15.),
        },
        evidence: Some(Arc::new(ContributionFamilyEvidence::new(entries.clone()))),
        queries: Cell::new(0),
    };
    let mut origins = DynamicSourceOrigins::default();
    let typed = CapturedProgrammingSources::new(&source, &no_adoption, None)
        .with_source_transaction(&mut origins);
    let id = typed
        .current_family_occurrence(target, &axis(ProgrammingComponent::Pan))
        .unwrap();
    assert_eq!(
        typed.current_family_occurrence(target, &axis(ProgrammingComponent::Tilt)),
        Some(id)
    );
    assert_eq!(
        source.queries.get(),
        1,
        "all components share one captured family dependency"
    );
    typed.check().unwrap();
    drop(typed);
    let record = origins.get(id).unwrap().clone();
    let DynamicSourceOrigin::StaticBaseline { sources } = &record.origin else {
        panic!()
    };
    assert_eq!(sources.len(), 2);
    for entry in &entries {
        assert!(
            sources.contains(
                &DynamicStaticSourceEntry::from_evidence(
                    DynamicStaticSource::from_contribution(entry.source()),
                    entry
                )
                .unwrap()
            )
        );
    }
    let unchanged = origins.clone();
    let repeat = CapturedProgrammingSources::new(&source, &no_adoption, None)
        .with_source_transaction(&mut origins);
    assert_eq!(
        repeat.current_family_occurrence(target, &axis(ProgrammingComponent::Tilt)),
        Some(id)
    );
    repeat.finish_source_bindings().unwrap();
    drop(repeat);
    assert!(
        origins.shares_storage(&unchanged),
        "another frame reuses the immutable multi-source evidence without catalogue writes"
    );
    source.evidence = None;
    let typed = CapturedProgrammingSources::new(&source, &no_adoption, None)
        .with_source_transaction(&mut origins);
    assert_eq!(
        typed.current(target, &axis(ProgrammingComponent::Tilt)),
        Some(DynamicValue::Scalar(15.))
    );
    assert_eq!(
        typed.current_family_occurrence(target, &axis(ProgrammingComponent::Pan)),
        None
    );
    typed.check().unwrap();
    drop(typed);
    assert_eq!(
        origins.binding(&DynamicSourceBinding::StaticBaseline {
            target,
            owner: ProgrammingOwner::Position
        }),
        None
    );
    assert!(
        Arc::ptr_eq(origins.get(id).unwrap(), &record),
        "held history retains the immutable old record"
    );
}

#[test]
fn incompatible_source_footprint_rejects_the_frame_instead_of_fabricating_a_dependency() {
    let target = FixtureId::new();
    let source = EvidenceSource {
        base: StaticSource {
            target,
            family: position(30., 15.),
        },
        queries: Cell::new(0),
        evidence: Some(Arc::new(ContributionFamilyEvidence::new(vec![
            ContributionFamilyEntry::new(
                ContributionSourceId::programmer(light_core::ProgrammerId::new()),
                light_core::ProgrammerEditStamp {
                    changed_at: chrono::Utc::now(),
                    programmer_order: 1,
                },
                ContributionFamilyFootprint::Component(ProgrammingComponent::Focus),
                ContributionFamilyRole::Authored,
            ),
        ]))),
    };
    let mut origins = DynamicSourceOrigins::default();
    let before = origins.clone();
    let typed = CapturedProgrammingSources::new(&source, &no_adoption, None)
        .with_source_transaction(&mut origins);
    assert_eq!(
        typed.current_family_occurrence(target, &axis(ProgrammingComponent::Pan)),
        None
    );
    assert!(typed.check().is_err());
    drop(typed);
    assert!(before.shares_storage(&origins));
}
