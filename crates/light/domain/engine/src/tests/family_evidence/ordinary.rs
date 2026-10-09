use super::*;
use crate::{ContributionProgrammerLane as Lane, ContributionSourceDescriptor as Source};
use light_core::programming::PositionIntent;

#[test]
fn source_descriptors_preserve_every_identity_and_borrow_lane_names() {
    let programmer = ProgrammerId::new();
    for (id, expected) in [
        (ContributionSourceId::programmer(programmer), Lane::Live),
        (ContributionSourceId::preload(programmer), Lane::Preload),
        (
            ContributionSourceId::programmer_transient(programmer, "momentary"),
            Lane::Transient("momentary"),
        ),
        (
            ContributionSourceId::programmer_group(programmer, "front"),
            Lane::Group("front"),
        ),
        (
            ContributionSourceId::preload_group(programmer, "back"),
            Lane::PreloadGroup("back"),
        ),
    ] {
        assert_eq!(
            id.descriptor(),
            Source::Programmer {
                programmer_id: programmer,
                lane: expected
            }
        );
    }
    let name: Arc<str> = Arc::from("shared group name");
    let id = ContributionSourceId::programmer_group(programmer, Arc::clone(&name));
    let Source::Programmer {
        lane: Lane::Group(borrowed),
        ..
    } = id.descriptor()
    else {
        panic!()
    };
    assert_eq!(borrowed.as_ptr(), name.as_ptr());
    let source = light_playback::SequenceMasterSource {
        playback_number: Some(1601),
        playback_identity: Some(light_playback::PlaybackIdentity::Virtual(
            light_playback::VirtualPlaybackAddress::new(3, 1601).unwrap(),
        )),
        cue_list_id: light_core::CueListId::new(),
        temporary: true,
    };
    assert_eq!(
        ContributionSourceId::playback(source).descriptor(),
        Source::Playback(source)
    );
}

pub(super) fn focus_engine() -> (
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
    (engine, registry, session, fixture, clock)
}

fn whole_entry<'a>(
    frame: &'a ObservedSourceFrame,
    fixture: FixtureId,
    attribute: &AttributeKey,
) -> &'a ContributionFamilyEntry {
    let evidence = frame
        .values()
        .contribution_family_evidence(fixture, attribute)
        .expect("complete authored family evidence");
    assert_eq!(evidence.entries().len(), 1);
    let entry = &evidence.entries()[0];
    assert_eq!(entry.footprint(), ContributionFamilyFootprint::Whole);
    assert_eq!(entry.role(), ContributionFamilyRole::Authored);
    entry
}

#[test]
fn ordinary_programmer_keeps_complete_position_author_and_exact_edit_stamp() {
    let (engine, registry, session, fixture, _) = focus_engine();
    let attribute = AttributeKey("position".into());
    let value = AttributeValue::Position(Arc::new(PositionIntent::angles(37.0, -12.0)));
    registry.set(session, fixture, attribute.clone(), value.clone());
    let capture = engine.prepare_output_frame(Default::default());
    let stored = &capture.programmer().output_states[0].values[0];
    let observed = engine.observe_prepared_frame(&capture, &[]);
    assert_eq!(observed.values().value(fixture, &attribute), Some(&value));
    let entry = whole_entry(&observed, fixture, &attribute);
    assert_eq!(
        entry.source().descriptor(),
        Source::Programmer {
            programmer_id: capture.programmer().identity.unwrap(),
            lane: Lane::Live,
        }
    );
    assert_eq!(entry.stamp().changed_at, stored.changed_at);
    assert_eq!(entry.stamp().programmer_order, stored.programmer_order);
    assert_eq!(entry.transition_ordinal(), None);
    assert_eq!(
        observed
            .values()
            .contribution_origin(fixture, &attribute)
            .unwrap()
            .transition_ordinal(),
        None
    );
    assert!(!observed.values().materialised_by_name());
    assert!(
        engine
            .render_prepared(&capture, &[])
            .unwrap()
            .resolved_values
            .contribution_family_evidence(fixture, &attribute)
            .is_none(),
        "ordinary output does not publish observer evidence"
    );
}

#[test]
fn equal_group_family_values_retain_the_actual_winner_across_same_capture_release() {
    let (engine, registry, session, fixture, _) = focus_engine();
    let mut snapshot = (*engine.snapshot()).clone();
    snapshot.groups = ["first", "second"]
        .into_iter()
        .map(|id| GroupDefinition {
            replacement_projections: Default::default(),
            id: id.into(),
            name: id.into(),
            fixtures: vec![fixture],
            ..Default::default()
        })
        .collect::<Vec<_>>()
        .into();
    engine.replace_snapshot(snapshot).unwrap();
    let attribute = AttributeKey("focus".into());
    for group in ["first", "second"] {
        registry.set_group(
            session,
            group.into(),
            attribute.clone(),
            AttributeValue::Normalized(0.4),
        );
    }
    let capture = engine.prepare_output_frame(Default::default());
    let before = engine.observe_prepared_frame(&capture, &[]);
    let entry = whole_entry(&before, fixture, &attribute);
    let programmer = capture.programmer().identity.unwrap();
    assert_eq!(
        entry.source().descriptor(),
        Source::Programmer {
            programmer_id: programmer,
            lane: Lane::Group("second")
        }
    );
    let release =
        ContributionBatch::excluding([(entry.source().clone(), fixture, attribute.clone())]);
    let after = engine.observe_prepared_frame(&capture, &[release]);
    assert_eq!(
        before.values().value(fixture, &attribute),
        after.values().value(fixture, &attribute)
    );
    assert_eq!(
        whole_entry(&after, fixture, &attribute)
            .source()
            .descriptor(),
        Source::Programmer {
            programmer_id: programmer,
            lane: Lane::Group("first")
        }
    );
    assert_eq!(
        whole_entry(&before, fixture, &attribute)
            .source()
            .descriptor(),
        Source::Programmer {
            programmer_id: programmer,
            lane: Lane::Group("second")
        }
    );
}

#[test]
fn programmer_fade_retains_previous_and_blended_history_until_the_target_is_reached() {
    let (engine, registry, session, fixture, clock) = focus_engine();
    let attribute = AttributeKey("focus".into());
    registry.set(
        session,
        fixture,
        attribute.clone(),
        AttributeValue::Normalized(0.25),
    );
    engine.render(Default::default()).unwrap();
    let before = engine.observe_source_frame(&[]);
    let previous = before
        .values()
        .contribution_family_evidence(fixture, &attribute)
        .unwrap();
    clock.advance_millis(1);
    registry.set_faded_with_timing(
        session,
        fixture,
        attribute.clone(),
        AttributeValue::Normalized(0.75),
        Some(1000),
        Some(200),
    );
    let expected_target = super::programmer_history::stored_entry(&engine, fixture, &attribute);
    engine.render(Default::default()).unwrap();
    for _ in 0..2 {
        clock.advance_millis(100);
        let observed = engine.observe_source_frame(&[]);
        assert_eq!(
            observed.values().value(fixture, &attribute),
            Some(&AttributeValue::Normalized(0.25))
        );
        assert!(Arc::ptr_eq(
            observed
                .values()
                .contribution_family_evidence(fixture, &attribute)
                .unwrap(),
            previous
        ));
    }
    clock.advance_millis(500);
    let interior = engine.observe_source_frame(&[]);
    assert_eq!(
        interior.values().value(fixture, &attribute),
        Some(&AttributeValue::Normalized(0.5))
    );
    super::programmer_history::assert_entries(
        interior
            .values()
            .contribution_family_evidence(fixture, &attribute)
            .unwrap(),
        &[previous.entries()[0].clone(), expected_target.clone()],
    );
    clock.advance_millis(500);
    let observed = engine.observe_source_frame(&[]);
    assert_eq!(
        observed.values().value(fixture, &attribute),
        Some(&AttributeValue::Normalized(0.75))
    );
    super::programmer_history::assert_entries(
        observed
            .values()
            .contribution_family_evidence(fixture, &attribute)
            .unwrap(),
        &[expected_target],
    );
}

#[test]
fn playback_retains_distinct_authors_through_equal_values_and_interrupted_blends() {
    let (engine, _, _, fixture, clock) = focus_engine();
    let attribute = AttributeKey("focus".into());
    let change = |value| {
        CueChange::set(
            fixture,
            attribute.clone(),
            AttributeValue::Normalized(value),
        )
    };
    let mut list = test_cue_list("Family evidence", vec![change(0.4)]);
    let mut second = Cue::new(2_u16.into());
    second.changes = vec![change(0.4)];
    second.fade_millis = 1000;
    let mut third = Cue::new(3_u16.into());
    third.changes = vec![change(0.8)];
    third.fade_millis = 1000;
    list.cues.extend([second, third]);
    let mut snapshot = (*engine.snapshot()).clone();
    snapshot.cue_lists = vec![list.clone()].into();
    snapshot.playbacks = vec![test_playback(1, list.id)].into();
    engine.replace_snapshot(snapshot).unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    let first = engine.observe_source_frame(&[]);
    let entry = whole_entry(&first, fixture, &attribute);
    assert!(
        matches!(entry.source().descriptor(), Source::Playback(source) if source.cue_list_id == list.id)
    );
    let stamp = first
        .values()
        .contribution_origin(fixture, &attribute)
        .unwrap()
        .stamp();
    assert_eq!(entry.stamp().changed_at, stamp.changed_at);
    assert_eq!(entry.stamp().programmer_order, stamp.programmer_order);
    assert_eq!(entry.authored_cue_id(), Some(list.cues[0].id));
    clock.advance_millis(1);
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    clock.advance_millis(500);
    let equal_blend = engine.observe_source_frame(&[]);
    assert_eq!(
        equal_blend.values().value(fixture, &attribute),
        first.values().value(fixture, &attribute)
    );
    let history = equal_blend
        .values()
        .contribution_family_evidence(fixture, &attribute)
        .unwrap();
    assert_eq!(
        history.entries().len(),
        2,
        "equal values retain both actual action occurrences"
    );
    assert_eq!(
        history.entries()[0].authored_cue_id(),
        Some(list.cues[0].id)
    );
    assert_eq!(
        history.entries()[1].authored_cue_id(),
        Some(list.cues[1].id)
    );
    assert_eq!(history.entries()[0].stamp().changed_at, stamp.changed_at);
    let repeated = engine.observe_source_frame(&[]);
    assert!(Arc::ptr_eq(
        history,
        repeated
            .values()
            .contribution_family_evidence(fixture, &attribute)
            .unwrap()
    ));
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    clock.advance_millis(500);
    let interrupted = engine.observe_source_frame(&[]);
    let retained = interrupted
        .values()
        .contribution_family_evidence(fixture, &attribute)
        .unwrap();
    assert_eq!(retained.entries().len(), 3);
    for (entry, cue) in retained.entries().iter().zip(&list.cues) {
        assert_eq!(entry.authored_cue_id(), Some(cue.id));
    }
    clock.advance_millis(500);
    let completed = engine.observe_source_frame(&[]);
    assert_eq!(
        whole_entry(&completed, fixture, &attribute).authored_cue_id(),
        Some(list.cues[2].id)
    );
    assert_eq!(
        completed.values().value(fixture, &attribute),
        Some(&AttributeValue::Normalized(0.8))
    );
}

#[test]
fn same_time_playback_actions_keep_distinct_authored_occurrences() {
    let (engine, _, _, fixture, _) = focus_engine();
    let attribute = AttributeKey("focus".into());
    let change = CueChange::set(fixture, attribute.clone(), AttributeValue::Normalized(0.4));
    let mut list = test_cue_list("Same-time evidence", vec![change.clone()]);
    let mut second = Cue::new(2_u16.into());
    second.changes = vec![change];
    list.cues.push(second);
    let mut snapshot = (*engine.snapshot()).clone();
    snapshot.cue_lists = vec![list.clone()].into();
    snapshot.playbacks = vec![test_playback(1, list.id)].into();
    engine.replace_snapshot(snapshot).unwrap();

    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    let first = engine.observe_source_frame(&[]);
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    let second = engine.observe_source_frame(&[]);
    let first_entry = whole_entry(&first, fixture, &attribute);
    let second_entry = whole_entry(&second, fixture, &attribute);
    assert_eq!(
        first.values().value(fixture, &attribute),
        second.values().value(fixture, &attribute)
    );
    assert_eq!(first_entry.source(), second_entry.source());
    assert_eq!(
        first_entry.stamp().changed_at,
        second_entry.stamp().changed_at
    );
    assert_eq!(first_entry.stamp().programmer_order, 0);
    assert_eq!(second_entry.stamp().programmer_order, 0);
    assert!(first_entry.transition_ordinal().is_some());
    assert!(second_entry.transition_ordinal().is_some());
    assert_ne!(
        first_entry.transition_ordinal(),
        second_entry.transition_ordinal()
    );
    for (frame, entry) in [(&first, first_entry), (&second, second_entry)] {
        assert_eq!(
            frame
                .values()
                .contribution_origin(fixture, &attribute)
                .unwrap()
                .transition_ordinal(),
            entry.transition_ordinal()
        );
    }
}

#[test]
fn playback_samples_keep_origin_ordinals_and_explicit_evidence_ordinals_independently() {
    let (engine, fixture) = observed_engine(false);
    let attribute = AttributeKey("focus".into());
    let at = Utc::now();
    let source = light_playback::SequenceMasterSource {
        playback_number: Some(1),
        playback_identity: Some(light_playback::PlaybackIdentity::physical(1).unwrap()),
        cue_list_id: light_core::CueListId::new(),
        temporary: false,
    };
    let sample = ContributionSample::replacing_playback(
        sampled(fixture, "focus", at, 1).value().clone(),
        source,
        91,
    );
    let unknown = engine.observe_source_frame(&[ContributionBatch::new([sample.clone()])]);
    assert_eq!(
        unknown
            .values()
            .contribution_origin(fixture, &attribute)
            .unwrap()
            .transition_ordinal(),
        Some(91)
    );
    assert!(
        unknown
            .values()
            .contribution_family_evidence(fixture, &attribute)
            .is_none()
    );
    let explicit = Arc::new(ContributionFamilyEvidence::new(vec![
        ContributionFamilyEntry::new(
            ContributionSourceId::playback(source),
            light_core::ProgrammerEditStamp {
                changed_at: at,
                programmer_order: 0,
            },
            ContributionFamilyFootprint::Component(
                light_core::programming::ProgrammingComponent::Focus,
            ),
            ContributionFamilyRole::Authored,
        )
        .with_transition_ordinal(Some(88)),
    ]));
    let observed = engine.observe_source_frame(&[ContributionBatch::new([
        sample.with_family_evidence(Arc::clone(&explicit))
    ])]);
    let kept = observed
        .values()
        .contribution_family_evidence(fixture, &attribute)
        .unwrap();
    assert!(Arc::ptr_eq(kept, &explicit));
    assert_eq!(kept.entries()[0].transition_ordinal(), Some(88));
    assert_eq!(
        observed
            .values()
            .contribution_origin(fixture, &attribute)
            .unwrap()
            .transition_ordinal(),
        Some(91)
    );
}

#[test]
fn ordinary_evidence_is_cleared_by_freeze_override_and_unknown_samples() {
    let (engine, registry, session, fixture, _) = focus_engine();
    let attribute = AttributeKey("focus".into());
    registry.set(
        session,
        fixture,
        attribute.clone(),
        AttributeValue::Normalized(0.4),
    );
    let before = engine.observe_source_frame(&[]);
    whole_entry(&before, fixture, &attribute);
    let unknown = sampled(fixture, "focus", Utc::now(), i16::MAX);
    let replaced = engine.observe_source_frame(&[ContributionBatch::new([unknown])]);
    assert!(
        replaced
            .values()
            .contribution_family_evidence(fixture, &attribute)
            .is_none()
    );
    let capture = engine.prepare_output_frame(Default::default());
    let mut continuity = capture.continuity.clone();
    let mut resolved = engine.resolve_prepared_lane_attributes(
        &capture,
        &[],
        &mut continuity,
        true,
        crate::resolution::ProgrammerLaneInputs {
            playback: &capture.playback,
            states: &capture.programmer.output_states,
            releases: &capture.releases,
            addresses: &engine.programmer_addresses,
        },
    );
    resolved.override_value(fixture, &attribute, AttributeValue::Normalized(0.7), None);
    assert!(
        resolved
            .named_values()
            .contribution_family_evidence(fixture, &attribute)
            .is_none()
    );
    let mut snapshot = (*engine.snapshot()).clone();
    Arc::make_mut(&mut snapshot.fixtures)[0].freeze = FixtureFreezeState {
        targets: HashMap::from([(
            fixture,
            FrozenFixtureTarget {
                position_native: None,
                full: false,
                families: vec![FreezeFamily::Beam],
                values: HashMap::from([(attribute.clone(), AttributeValue::Normalized(0.7))]),
            },
        )]),
    };
    engine.replace_snapshot(snapshot).unwrap();
    assert!(
        engine
            .observe_source_frame(&[])
            .values()
            .contribution_family_evidence(fixture, &attribute)
            .is_none()
    );
}
