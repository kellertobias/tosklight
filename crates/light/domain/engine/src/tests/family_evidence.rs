use super::*;
use light_core::programming::ProgrammingComponent;

mod ordinary;
mod programmer_history;

fn observed_engine(frozen: bool) -> (Engine, FixtureId) {
    let (mut fixture, logical) = fixture();
    retarget_only_channel(&mut fixture, "focus");
    if frozen {
        fixture.freeze = FixtureFreezeState {
            targets: HashMap::from([(
                logical,
                FrozenFixtureTarget {
                    position_native: None,
                    full: false,
                    families: vec![FreezeFamily::Beam],
                    values: HashMap::from([(
                        AttributeKey("focus".into()),
                        AttributeValue::Normalized(0.7),
                    )]),
                },
            )]),
        };
    }
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    (engine, logical)
}

fn sampled(
    fixture: FixtureId,
    attribute: &str,
    at: chrono::DateTime<Utc>,
    priority: i16,
) -> ContributionSample {
    ContributionSample::independent(TimedValue {
        fixture_id: fixture,
        attribute: AttributeKey(attribute.into()),
        value: AttributeValue::Normalized(0.4),
        priority,
        changed_at: at,
        programmer_order: 1,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    })
}

fn evidence(
    source: ContributionSourceId,
    at: chrono::DateTime<Utc>,
) -> Arc<ContributionFamilyEvidence> {
    Arc::new(ContributionFamilyEvidence::new(vec![
        ContributionFamilyEntry::new(
            source.clone(),
            light_core::ProgrammerEditStamp {
                changed_at: at,
                programmer_order: 8,
            },
            ContributionFamilyFootprint::Component(ProgrammingComponent::Focus),
            ContributionFamilyRole::Authored,
        ),
        ContributionFamilyEntry::new(
            source,
            light_core::ProgrammerEditStamp {
                changed_at: at,
                programmer_order: 3,
            },
            ContributionFamilyFootprint::Whole,
            ContributionFamilyRole::CalculationDependency,
        ),
    ]))
}

#[test]
fn equal_payloads_keep_only_the_dense_winners_explicit_family_evidence() {
    let (engine, fixture) = observed_engine(false);
    let at = Utc::now();
    let first = evidence(ContributionSourceId::programmer(ProgrammerId::new()), at);
    let second = evidence(ContributionSourceId::preload(ProgrammerId::new()), at);
    let batch = ContributionBatch::new([
        sampled(fixture, "focus", at, 1).with_family_evidence(Arc::clone(&first)),
        sampled(fixture, "focus", at + chrono::Duration::milliseconds(1), 1)
            .with_family_evidence(Arc::clone(&second)),
    ]);
    let frame = engine.observe_source_frame(&[batch]);
    let kept = frame
        .values()
        .contribution_family_evidence(fixture, &AttributeKey("focus".into()))
        .unwrap();
    assert!(Arc::ptr_eq(kept, &second));
    assert!(!Arc::ptr_eq(kept, &first));
    assert_eq!(kept.entries().len(), 2);
    assert_eq!(kept.entries()[0].role(), ContributionFamilyRole::Authored);
    assert_eq!(
        kept.entries()[0].footprint(),
        ContributionFamilyFootprint::Component(ProgrammingComponent::Focus)
    );
    assert_eq!(
        kept.entries()[1].role(),
        ContributionFamilyRole::CalculationDependency
    );
    assert_eq!(kept.entries()[1].stamp().programmer_order, 3);
    assert!(
        frame
            .values()
            .contribution_origin(fixture, &AttributeKey("focus".into()))
            .is_none()
    );
    assert!(!frame.values().materialised_by_name());
}

#[test]
fn evidence_does_not_replace_a_stronger_sample_and_losing_evidence_is_pruned() {
    let (engine, fixture) = observed_engine(false);
    let at = Utc::now();
    let losing = evidence(ContributionSourceId::preload(ProgrammerId::new()), at);
    let high = sampled(fixture, "focus", at, 10);
    let low = sampled(fixture, "focus", at + chrono::Duration::seconds(1), 1)
        .with_family_evidence(losing);
    assert!(low.replacement_source().is_none());
    let frame = engine.observe_source_frame(&[ContributionBatch::new([high, low])]);
    assert_eq!(
        frame.values().value(fixture, &AttributeKey("focus".into())),
        Some(&AttributeValue::Normalized(0.4))
    );
    assert!(
        frame
            .values()
            .contribution_family_evidence(fixture, &AttributeKey("focus".into()))
            .is_none()
    );
}

#[test]
fn overflow_winner_retains_evidence_and_dense_freeze_or_overflow_override_clears_it() {
    let (engine, fixture) = observed_engine(false);
    let at = Utc::now();
    let kept = evidence(ContributionSourceId::programmer(ProgrammerId::new()), at);
    let unknown = AttributeKey("focus.unlisted".into());
    let batch = ContributionBatch::new([
        sampled(fixture, "focus.unlisted", at, 1).with_family_evidence(Arc::clone(&kept)),
        sampled(
            fixture,
            "focus.unlisted",
            at - chrono::Duration::seconds(1),
            0,
        )
        .with_family_evidence(evidence(
            ContributionSourceId::preload(ProgrammerId::new()),
            at,
        )),
    ]);
    let frame = engine.observe_source_frame(&[batch]);
    assert!(frame.values().has_unnumbered_values());
    assert!(Arc::ptr_eq(
        frame
            .values()
            .contribution_family_evidence(fixture, &unknown)
            .unwrap(),
        &kept
    ));

    let slots = engine.generation.load_full().slots().clone();
    let mut resolver = crate::EngineContributionResolver::unpooled(&slots).tracing_sources();
    resolver.extend_borrowed_samples(
        [sampled(fixture, "focus.unlisted", at, 1).with_family_evidence(Arc::clone(&kept))].iter(),
    );
    let mut resolved = resolver.finish();
    resolved.override_value(fixture, &unknown, AttributeValue::Normalized(0.7), None);
    let overridden = resolved.named_values();
    assert_eq!(
        overridden.value(fixture, &unknown),
        Some(&AttributeValue::Normalized(0.7))
    );
    assert!(
        overridden
            .contribution_family_evidence(fixture, &unknown)
            .is_none()
    );

    let (frozen_engine, frozen_fixture) = observed_engine(true);
    let frozen = frozen_engine.observe_source_frame(&[ContributionBatch::new([sampled(
        frozen_fixture,
        "focus",
        at,
        1,
    )
    .with_family_evidence(kept)])]);
    assert_eq!(
        frozen
            .values()
            .value(frozen_fixture, &AttributeKey("focus".into())),
        Some(&AttributeValue::Normalized(0.7))
    );
    assert!(
        frozen
            .values()
            .contribution_family_evidence(frozen_fixture, &AttributeKey("focus".into()))
            .is_none()
    );
}
