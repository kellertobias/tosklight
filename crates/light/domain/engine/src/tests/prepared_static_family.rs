use super::*;
use light_core::FrameAddressResolver;
use light_core::programming::{PositionIntent, ProgrammingOwner};

fn focus_engine() -> (
    Engine,
    ProgrammerRegistry,
    SessionId,
    FixtureId,
    Arc<ManualClock>,
) {
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    ));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    registry.start(session);
    let (mut patched, fixture) = fixture();
    retarget_only_channel(&mut patched, "focus");
    let engine = Engine::new(registry.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![patched].into(),
            ..Default::default()
        })
        .unwrap();
    registry.set(
        session,
        fixture,
        AttributeKey("focus".into()),
        AttributeValue::Normalized(0.2),
    );
    (engine, registry, session, fixture, clock)
}

fn unknown() -> FamilyProjectionMetadata {
    FamilyProjectionMetadata {
        changed_at: None,
        evidence: FamilyProjectionEvidence::Replace {
            origin: None,
            family_evidence: None,
        },
    }
}

fn source() -> light_playback::SequenceMasterSource {
    light_playback::SequenceMasterSource {
        playback_number: Some(1),
        playback_identity: Some(light_playback::PlaybackIdentity::physical(1).unwrap()),
        cue_list_id: light_core::CueListId::new(),
        temporary: false,
    }
}

fn sample(
    fixture: FixtureId,
    attribute: AttributeKey,
    value: AttributeValue,
    at: chrono::DateTime<Utc>,
) -> TimedValue {
    TimedValue {
        fixture_id: fixture,
        attribute,
        value,
        priority: 200,
        changed_at: at,
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    }
}

#[test]
fn exact_capture_is_required_even_with_identical_time_and_generation() {
    let (engine, _, _, _, _) = focus_engine();
    let capture = engine.prepare_output_frame(Default::default());
    let other = engine.prepare_output_frame(Default::default());
    assert_eq!(capture.sampled_at(), other.sampled_at());
    assert_eq!(capture.generation(), other.generation());
    let revision = engine.capture_output_continuity().0;
    let lane = engine.prepare_static_family_frame(&capture, &[]);
    assert!(matches!(
        engine.render_static_family_frame(&other, lane),
        Err(EngineError::StalePreparedFrame)
    ));
    let lane = engine.prepare_static_family_frame(&capture, &[]);
    assert!(matches!(
        engine.preview_static_family_frame(&other, lane),
        Err(EngineError::StalePreparedFrame)
    ));
    assert_eq!(engine.capture_output_continuity().0, revision);
}

#[test]
fn queued_family_does_not_change_current_or_compete_against_a_newer_static_edit() {
    let (engine, registry, session, fixture, clock) = focus_engine();
    clock.advance_millis(1_000);
    let key = AttributeKey("focus".into());
    registry.set(
        session,
        fixture,
        key.clone(),
        AttributeValue::Normalized(0.7),
    );
    let capture = engine.prepare_output_frame(Default::default());
    let mut lane = engine.prepare_static_family_frame(&capture, &[]);
    let static_at = lane.changed_at(fixture, &key).unwrap();
    assert!(lane.contribution_origin(fixture, &key).is_some());
    assert!(lane.contribution_family_evidence(fixture, &key).is_some());
    lane.project_family(
        fixture,
        ProgrammingOwner::Focus,
        AttributeValue::Normalized(0.4),
        unknown(),
    )
    .unwrap();
    assert_eq!(
        lane.value(fixture, &key),
        Some(&AttributeValue::Normalized(0.7))
    );
    assert_eq!(lane.changed_at(fixture, &key), Some(static_at));
    // Neither a later live edit nor its timestamp can change this captured baseline.
    registry.set(
        session,
        fixture,
        key.clone(),
        AttributeValue::Normalized(0.9),
    );
    let output = engine.render_static_family_frame(&capture, lane).unwrap();
    assert_eq!(
        output.resolved_values.value(fixture, &key),
        Some(&AttributeValue::Normalized(0.4))
    );
    assert_eq!(output.resolved_values.changed_at(fixture, &key), None);
    assert!(
        output
            .resolved_values
            .contribution_origin(fixture, &key)
            .is_none()
    );
    assert!(
        output
            .resolved_values
            .contribution_family_evidence(fixture, &key)
            .is_none()
    );
    assert!(!output.resolved_values.materialised_by_name());
    assert!(
        !output
            .resolved_values
            .changed_at_map()
            .contains_key(&(fixture, key))
    );
}

#[test]
fn dense_and_overflow_keep_explicit_evidence_and_output_time() {
    let (engine, _, _, fixture, _) = focus_engine();
    let capture = engine.prepare_output_frame(Default::default());
    let focus = AttributeKey("focus".into());
    let position = AttributeKey("position".into());
    // TL-639 round 2: every profile head numbers its family owners, so the unnumbered pair is a
    // Position on a target the patch does not hold.
    let unnumbered = FixtureId::new();
    assert!(
        capture
            .frame_addresser()
            .frame_address(fixture, &focus)
            .is_some()
    );
    assert!(
        capture
            .frame_addresser()
            .frame_address(unnumbered, &position)
            .is_none()
    );
    let playback = source();
    let position_value = AttributeValue::Position(Arc::new(PositionIntent::angles(20.0, -10.0)));
    let stamp = light_core::ProgrammerEditStamp {
        changed_at: capture.sampled_at(),
        programmer_order: 7,
    };
    let supplied = Arc::new(ContributionFamilyEvidence::new(vec![
        ContributionFamilyEntry::new(
            ContributionSourceId::playback(playback),
            stamp,
            ContributionFamilyFootprint::Component(
                light_core::programming::ProgrammingComponent::Pan,
            ),
            ContributionFamilyRole::Authored,
        )
        .with_transition_ordinal(Some(8)),
        ContributionFamilyEntry::new(
            ContributionSourceId::programmer(ProgrammerId::new()),
            stamp,
            ContributionFamilyFootprint::Component(
                light_core::programming::ProgrammingComponent::Tilt,
            ),
            ContributionFamilyRole::CalculationDependency,
        ),
    ]));
    let samples = ContributionBatch::new([
        ContributionSample::replacing_playback(
            sample(
                fixture,
                focus.clone(),
                AttributeValue::Normalized(0.8),
                capture.sampled_at(),
            ),
            playback,
            8,
        ),
        ContributionSample::replacing_playback(
            sample(
                unnumbered,
                position.clone(),
                position_value.clone(),
                capture.sampled_at(),
            ),
            playback,
            8,
        )
        .with_family_evidence(supplied),
    ]);
    let mut lane = engine.prepare_static_family_frame(&capture, std::slice::from_ref(&samples));
    let evidence = Arc::clone(
        lane.contribution_family_evidence(unnumbered, &position)
            .unwrap(),
    );
    let origin = Arc::new(
        lane.contribution_origin(unnumbered, &position)
            .unwrap()
            .clone(),
    );
    let output_at = capture.sampled_at() - ChronoDuration::seconds(1);
    lane.project_family(
        fixture,
        ProgrammingOwner::Focus,
        AttributeValue::Normalized(0.6),
        FamilyProjectionMetadata {
            changed_at: Some(output_at),
            evidence: FamilyProjectionEvidence::PreserveBaseline,
        },
    )
    .unwrap();
    lane.project_family(
        unnumbered,
        ProgrammingOwner::Position,
        position_value.clone(),
        FamilyProjectionMetadata {
            changed_at: Some(output_at),
            evidence: FamilyProjectionEvidence::Replace {
                origin: Some(origin),
                family_evidence: Some(Arc::clone(&evidence)),
            },
        },
    )
    .unwrap();
    let revision = engine.capture_output_continuity().0;
    let output = engine.preview_static_family_frame(&capture, lane).unwrap();
    assert_eq!(engine.capture_output_continuity().0, revision);
    let values = &output.resolved_values;
    assert_eq!(values.value(unnumbered, &position), Some(&position_value));
    assert_eq!(values.changed_at(fixture, &focus), Some(output_at));
    assert_eq!(values.changed_at(unnumbered, &position), Some(output_at));
    assert!(Arc::ptr_eq(
        values
            .contribution_family_evidence(unnumbered, &position)
            .unwrap(),
        &evidence
    ));
    assert_eq!(evidence.entries().len(), 2);
    assert_eq!(
        evidence.entries()[1].role(),
        ContributionFamilyRole::CalculationDependency
    );
    assert!(values.contribution_origin(fixture, &focus).is_some());
    assert!(!values.materialised_by_name());
    // The Cue master scales level parameters only; Focus reaches DMX as projected.
    assert!(
        (i16::from(output.universes[&1][0]) - 153).abs() <= 1,
        "a Cue master never scales a non-level parameter"
    );
    let mut lane = engine.prepare_static_family_frame(&capture, &[samples]);
    for (target, owner, value) in [
        (
            fixture,
            ProgrammingOwner::Focus,
            AttributeValue::Normalized(0.6),
        ),
        (unnumbered, ProgrammingOwner::Position, position_value),
    ] {
        lane.project_family(target, owner, value, unknown())
            .unwrap();
    }
    let output = engine.render_static_family_frame(&capture, lane).unwrap();
    for (target, key) in [(fixture, &focus), (unnumbered, &position)] {
        let winner = output
            .resolved_values
            .frame()
            .unwrap()
            .winner(target, key)
            .unwrap();
        assert!(winner.origin.is_none());
        assert!(winner.family_evidence.is_none());
        assert!(output.resolved_values.changed_at(target, key).is_none());
    }
    assert!((i16::from(output.universes[&1][0]) - 153).abs() <= 1);
    assert!(output.resolved_values.changed_at_map().is_empty());
}

#[test]
fn invalid_family_metadata_and_duplicate_outputs_do_not_replace_a_queued_result() {
    let (engine, _, _, fixture, _) = focus_engine();
    let capture = engine.prepare_output_frame(Default::default());
    let mut lane = engine.prepare_static_family_frame(&capture, &[]);
    for value in [
        AttributeValue::Normalized(f32::NAN),
        AttributeValue::Normalized(2.0),
        AttributeValue::Position(Arc::new(PositionIntent::angles(1.0, 2.0))),
    ] {
        assert!(
            lane.project_family(fixture, ProgrammingOwner::Focus, value, unknown())
                .is_err()
        );
    }
    assert!(
        lane.project_family(
            FixtureId::new(),
            ProgrammingOwner::Focus,
            AttributeValue::Normalized(0.5),
            unknown()
        )
        .is_err()
    );
    lane.project_family(
        fixture,
        ProgrammingOwner::Focus,
        AttributeValue::Normalized(0.3),
        unknown(),
    )
    .unwrap();
    assert!(
        lane.project_family(
            fixture,
            ProgrammingOwner::Focus,
            AttributeValue::Normalized(0.9),
            unknown()
        )
        .is_err()
    );
    let result = engine.render_static_family_frame(&capture, lane).unwrap();
    assert_eq!(
        result
            .resolved_values
            .value(fixture, &AttributeKey("focus".into())),
        Some(&AttributeValue::Normalized(0.3))
    );
}

#[test]
fn freeze_holds_the_baseline_and_wins_over_composition_and_clears_projected_sources() {
    let (engine, _, _, fixture, _) = focus_engine();
    let key = AttributeKey("focus".into());
    let mut snapshot = (*engine.snapshot()).clone();
    Arc::make_mut(&mut snapshot.fixtures)[0].freeze = FixtureFreezeState {
        targets: HashMap::from([(
            fixture,
            FrozenFixtureTarget {
                position_native: None,
                full: false,
                families: vec![FreezeFamily::Beam],
                values: HashMap::from([(key.clone(), AttributeValue::Normalized(0.9))]),
            },
        )]),
    };
    engine.replace_snapshot(snapshot).unwrap();
    let capture = engine.prepare_output_frame(Default::default());
    let mut lane = engine.prepare_static_family_frame(&capture, &[]);
    // Freeze holds parameters before DMX (2026-10-05): the baseline already holds the frozen
    // value, so a family adapter renders it.
    assert_eq!(
        lane.value(fixture, &key),
        Some(&AttributeValue::Normalized(0.9))
    );
    lane.project_family(
        fixture,
        ProgrammingOwner::Focus,
        AttributeValue::Normalized(0.4),
        FamilyProjectionMetadata {
            changed_at: None,
            evidence: FamilyProjectionEvidence::PreserveBaseline,
        },
    )
    .unwrap();
    let result = engine.render_static_family_frame(&capture, lane).unwrap();
    assert_eq!(
        result.resolved_values.value(fixture, &key),
        Some(&AttributeValue::Normalized(0.9))
    );
    assert!(
        result
            .resolved_values
            .contribution_origin(fixture, &key)
            .is_none()
    );
    assert!(
        result
            .resolved_values
            .contribution_family_evidence(fixture, &key)
            .is_none()
    );
}

#[test]
fn preview_and_failed_projection_do_not_commit_candidate_fade_history() {
    let (engine, registry, session, fixture, _) = focus_engine();
    registry.set_faded(
        session,
        fixture,
        AttributeKey("focus".into()),
        AttributeValue::Normalized(0.8),
    );
    let mut capture = engine.prepare_output_frame(Default::default());
    let revision = engine.capture_output_continuity().0;
    let lane = engine.prepare_static_family_frame(&capture, &[]);
    engine.preview_static_family_frame(&capture, lane).unwrap();
    assert_eq!(engine.capture_output_continuity().0, revision);
    assert!(
        engine
            .capture_output_continuity()
            .1
            .programmer_transitions
            .is_empty()
    );
    // Deliberately omit compiled projection plans to exercise a real final render failure.
    // The malformed generation stays local to this test capture, never installed into Engine.
    capture.generation = Arc::new(crate::RuntimeGeneration::new(
        (*capture.snapshot()).clone(),
        capture.generation.playback_arc(),
        Default::default(),
        Default::default(),
        Default::default(),
    ));
    let lane = engine.prepare_static_family_frame(&capture, &[]);
    assert!(
        matches!(engine.render_static_family_frame(&capture, lane), Err(EngineError::Invalid(message)) if message.contains("projection plan is missing"))
    );
    assert_eq!(engine.capture_output_continuity().0, revision);
    assert!(
        engine
            .capture_output_continuity()
            .1
            .programmer_transitions
            .is_empty()
    );
    let capture = engine.prepare_output_frame(Default::default());
    let lane = engine.prepare_static_family_frame(&capture, &[]);
    engine.render_static_family_frame(&capture, lane).unwrap();
    assert_ne!(engine.capture_output_continuity().0, revision);
    assert!(
        !engine
            .capture_output_continuity()
            .1
            .programmer_transitions
            .is_empty()
    );
}

#[test]
fn continuity_reset_rejects_a_prepared_family_lane() {
    let (engine, _, _, _, _) = focus_engine();
    let capture = engine.prepare_output_frame(Default::default());
    let lane = engine.prepare_static_family_frame(&capture, &[]);
    engine.clear_programmer_transitions();
    let revision = engine.capture_output_continuity().0;
    assert!(matches!(
        engine.render_static_family_frame(&capture, lane),
        Err(EngineError::StalePreparedFrame)
    ));
    assert_eq!(engine.capture_output_continuity().0, revision);
}
