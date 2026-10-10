use super::*;

#[test]
fn programmer_fade_starts_from_unowned_fixture_profile_default() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    programmers.start(session);
    let (mut fixture, logical) = fixture();
    fixture.definition.heads[0].parameters[0].default = 0.8;
    let engine = Engine::new(programmers.clone());
    engine.set_control_timing([120.0; 5], 1_000, 0, 0);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    programmers.set_faded(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    );
    assert!((normalized(&engine.resolved_values(), logical, "intensity") - 0.8).abs() < 0.001);
    clock.set(started + ChronoDuration::milliseconds(500));
    assert!((normalized(&engine.resolved_values(), logical, "intensity") - 0.5).abs() < 0.001);
}

#[test]
fn programmer_fade_starts_from_current_immediate_programmer_value() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, logical) = fixture();
    let engine = Engine::new(programmers.clone());
    engine.set_control_timing([120.0; 5], 1_000, 0, 0);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    programmers.set(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    // Output owns the current fade endpoint; an observational values read must not seed it.
    let values = engine
        .render(RenderOptions::default())
        .unwrap()
        .resolved_values;
    assert!((normalized(&values, logical, "intensity") - 0.8).abs() < 0.001);
    clock.set(started + ChronoDuration::milliseconds(100));
    programmers.set_faded(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    );
    let values = engine
        .render(RenderOptions::default())
        .unwrap()
        .resolved_values;
    assert!((normalized(&values, logical, "intensity") - 0.8).abs() < 0.001);
    clock.set(started + ChronoDuration::milliseconds(600));
    let values = engine
        .render(RenderOptions::default())
        .unwrap()
        .resolved_values;
    assert!((normalized(&values, logical, "intensity") - 0.5).abs() < 0.001);
}

#[test]
fn programmer_fade_starts_from_resolved_playback_underlay_and_release_reveals_it() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, logical) = fixture();
    let mut snapshot = mib_snapshot(vec![fixture], &[logical]);
    Arc::make_mut(&mut snapshot.cue_lists)[0].cues[0]
        .changes
        .iter_mut()
        .find(|change| change.attribute.is_intensity())
        .unwrap()
        .value = Some(AttributeValue::Normalized(0.25));
    let engine = Engine::new(programmers.clone());
    engine.set_control_timing([120.0; 5], 1_000, 0, 0);
    engine.replace_snapshot(snapshot).unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);

    clock.set(started + ChronoDuration::seconds(5));
    assert!((normalized(&engine.resolved_values(), logical, "intensity") - 0.25).abs() < 0.001);
    programmers.set_faded(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    assert!((normalized(&engine.resolved_values(), logical, "intensity") - 0.25).abs() < 0.001);

    clock.set(started + ChronoDuration::milliseconds(5_500));
    assert!(
        (normalized(&engine.resolved_values(), logical, "intensity") - 0.525).abs() < 0.001,
        "the programmer transition interpolates from the live playback, not zero"
    );
    programmers.clear(session);
    assert!(
        (normalized(&engine.resolved_values(), logical, "intensity") - 0.25).abs() < 0.001,
        "release immediately reveals the unchanged playback underlay"
    );
}

#[test]
fn immediate_programmer_value_bypasses_non_zero_master_fade_without_zero_override() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, logical) = fixture();
    let engine = Engine::new(programmers.clone());
    engine.set_control_timing([120.0; 5], 5_000, 0, 0);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    programmers.set(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    let stored = &programmers.get(session).unwrap().values[0];
    assert!(!stored.fade);
    assert_eq!(stored.fade_millis, None);
    assert!((normalized(&engine.resolved_values(), logical, "intensity") - 0.8).abs() < 0.001);

    clock.set(started + ChronoDuration::milliseconds(2_500));
    assert!((normalized(&engine.resolved_values(), logical, "intensity") - 0.8).abs() < 0.001);
}

#[test]
fn overlapping_preload_group_fades_keep_edit_order_at_one_commit_timestamp() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, logical) = fixture();
    let engine = Engine::new(programmers.clone());
    engine.set_control_timing([120.0; 5], 3_000, 0, 0);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            groups: vec![
                GroupDefinition {
                    replacement_projections: Default::default(),
                    id: "1".into(),
                    name: "Broad".into(),
                    fixtures: vec![logical],
                    ..Default::default()
                },
                GroupDefinition {
                    replacement_projections: Default::default(),
                    id: "2".into(),
                    name: "Subset".into(),
                    fixtures: vec![logical],
                    ..Default::default()
                },
            ]
            .into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    assert!(programmers.arm_preload(session, true));
    assert!(programmers.set_group_faded(
        session,
        "1".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    ));
    assert!(programmers.set_group_faded_with_timing(
        session,
        "2".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.7),
        Some(1_000),
        None,
    ));
    let committed_at = started + ChronoDuration::seconds(2);
    assert!(programmers.activate_preload_at(session, committed_at));
    let active = programmers.get(session).unwrap();
    assert_eq!(
        active.preload_group_active["1"][&AttributeKey::intensity()].changed_at,
        committed_at
    );
    assert_eq!(
        active.preload_group_active["2"][&AttributeKey::intensity()].changed_at,
        committed_at
    );
    assert!(
        active.preload_group_active["2"][&AttributeKey::intensity()].programmer_order
            > active.preload_group_active["1"][&AttributeKey::intensity()].programmer_order
    );

    for millis in (2_000..=3_000).step_by(25) {
        clock.set(started + ChronoDuration::milliseconds(millis));
        engine.resolved_values();
    }
    assert!(
        (normalized(&engine.resolved_values(), logical, "intensity") - 0.7).abs() < 0.001,
        "rendering one group must not continually restart another group's explicit fade"
    );
}

#[test]
fn programmer_master_fade_interpolates_live_values() {
    let engine = Engine::new(ProgrammerRegistry::default());
    engine.set_control_timing([120.0, 90.0, 60.0, 30.0, 15.0], 1_000, 0, 0);
    let now = chrono::Utc::now();
    let value = TimedValue {
        fixture_id: FixtureId::new(),
        attribute: AttributeKey::intensity(),
        value: AttributeValue::Normalized(1.0),
        priority: 100,
        changed_at: now - chrono::Duration::milliseconds(500),
        programmer_order: 0,
        merge_mode: MergeMode::Htp,
        fade: true,
        fade_millis: None,
        delay_millis: None,
    };
    let faded = engine
        .faded_programmer_value(
            value,
            now,
            None,
            ProgrammerId::new(),
            ProgrammerTransitionSource::Programmer,
            false,
        )
        .unwrap();
    assert!(
        faded
            .value
            .normalized()
            .is_some_and(|level| (level - 0.5).abs() < 0.02)
    );
}

#[test]
fn interrupted_programmer_fade_samples_old_timing_before_new_delay_and_duration() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let programmer = ProgrammerId::new();
    let mut value = TimedValue {
        fixture_id: FixtureId::new(),
        attribute: AttributeKey("focus".into()),
        value: AttributeValue::Normalized(1.0),
        priority: 100,
        changed_at: start,
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: true,
        fade_millis: Some(1000),
        delay_millis: Some(200),
    };
    let sample = |value: TimedValue, millis| {
        engine
            .faded_programmer_value(
                value,
                start + ChronoDuration::milliseconds(millis),
                Some(&AttributeValue::Normalized(0.0)),
                programmer,
                ProgrammerTransitionSource::Programmer,
                false,
            )
            .unwrap()
            .value
            .normalized()
            .unwrap()
    };
    assert_eq!(sample(value.clone(), 0), 0.0);
    assert_eq!(sample(value.clone(), 700), 0.5);
    value.value = AttributeValue::Normalized(0.0);
    value.changed_at = start + ChronoDuration::milliseconds(700);
    value.fade_millis = Some(2000);
    value.delay_millis = Some(100);
    assert_eq!(sample(value.clone(), 700), 0.5);
    assert_eq!(sample(value.clone(), 799), 0.5);
    assert_eq!(sample(value.clone(), 1800), 0.25);
    assert_eq!(sample(value.clone(), 2800), 0.0);

    value.fixture_id = FixtureId::new();
    value.changed_at = start;
    value.value = AttributeValue::Normalized(1.0);
    value.fade_millis = Some(1000);
    value.delay_millis = Some(0);
    assert_eq!(sample(value.clone(), 0), 0.0);
    value.changed_at = start + ChronoDuration::milliseconds(700);
    value.value = AttributeValue::Normalized(0.0);
    assert!(
        (sample(value, 800) - 0.63).abs() < 0.0001,
        "the new fade must not double-advance when rendering follows the edit boundary"
    );
}

#[test]
fn delayed_snap_keeps_the_underlay_live_until_its_boundary() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let start = Utc::now();
    let programmer = ProgrammerId::new();
    let timed = TimedValue {
        fixture_id: FixtureId::new(),
        attribute: AttributeKey("media.play_mode".into()),
        value: AttributeValue::Normalized(1.0),
        priority: 100,
        changed_at: start,
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: true,
        fade_millis: Some(2000),
        delay_millis: Some(1000),
    };
    for (millis, underlay, expected) in [(0, 0.2, 0.2), (500, 0.8, 0.8), (1000, 0.4, 1.0)] {
        let sample = engine
            .faded_programmer_value(
                timed.clone(),
                start + ChronoDuration::milliseconds(millis),
                Some(&AttributeValue::Normalized(underlay)),
                programmer,
                ProgrammerTransitionSource::Programmer,
                true,
            )
            .unwrap();
        assert_eq!(sample.value, AttributeValue::Normalized(expected));
    }
}

#[test]
fn semantic_programmer_fade_honors_delay_turns_and_missing_underlay() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let position = |pan| {
        AttributeValue::Position(Arc::new(light_core::programming::PositionIntent::angles(
            pan, 0.0,
        )))
    };
    let value = TimedValue {
        fixture_id: FixtureId::new(),
        attribute: light_core::programming::ProgrammingOwner::Position.key(),
        value: position(720.0),
        priority: 100,
        changed_at: start,
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: true,
        fade_millis: Some(1000),
        delay_millis: Some(500),
    };
    let programmer = ProgrammerId::new();
    let sample = |value: TimedValue, millis, underlay: Option<&AttributeValue>, programmer| {
        engine
            .faded_programmer_value(
                value,
                start + ChronoDuration::milliseconds(millis),
                underlay,
                programmer,
                ProgrammerTransitionSource::Programmer,
                false,
            )
            .map(|v| v.value)
    };
    assert_eq!(
        sample(value.clone(), 0, Some(&position(0.0)), programmer),
        Some(position(0.0))
    );
    assert_eq!(
        sample(value.clone(), 400, Some(&position(0.0)), programmer),
        Some(position(0.0))
    );
    assert_eq!(
        sample(value.clone(), 1000, Some(&position(0.0)), programmer),
        Some(position(360.0))
    );
    assert_eq!(
        sample(value.clone(), 1500, Some(&position(0.0)), programmer),
        Some(position(720.0))
    );
    let no_underlay = ProgrammerId::new();
    assert_eq!(sample(value.clone(), 1000, None, no_underlay), None);
    assert_eq!(
        sample(value.clone(), 1500, None, no_underlay),
        Some(position(720.0))
    );
    let mut delayed = value;
    delayed.fade_millis = Some(0);
    let zero_fade = ProgrammerId::new();
    assert_eq!(
        sample(delayed.clone(), 499, Some(&position(30.0)), zero_fade),
        Some(position(30.0))
    );
    assert_eq!(
        sample(delayed, 500, Some(&position(30.0)), zero_fade),
        Some(position(720.0))
    );
}

#[test]
fn legacy_raw_and_discrete_programmer_values_keep_their_immediate_policy() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let now = Utc::now();
    for value in [
        AttributeValue::RawDmx(192),
        AttributeValue::RawDmxExact(u32::MAX - 1),
        AttributeValue::Discrete("open".into()),
    ] {
        let timed = TimedValue {
            fixture_id: FixtureId::new(),
            attribute: AttributeKey("raw.channel".into()),
            value: value.clone(),
            priority: 100,
            changed_at: now,
            programmer_order: 0,
            merge_mode: MergeMode::Ltp,
            fade: true,
            fade_millis: Some(1000),
            delay_millis: Some(500),
        };
        let sampled = engine
            .faded_programmer_value(
                timed,
                now,
                None,
                ProgrammerId::new(),
                ProgrammerTransitionSource::Programmer,
                false,
            )
            .unwrap();
        assert_eq!(sampled.value, value);
    }
}

/// An indexed attribute names a state rather than a level, so the Programmer must arrive at the
/// operator's choice in the frame they make it. Fading one walks an Audio Player's play mode
/// through Stop and Pause on its way to Play, and its media address through every slot in between.
#[test]
fn an_indexed_attribute_snaps_in_the_programmer_whatever_the_profile_flag_says() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    programmers.start(session);
    let play_mode = AttributeKey("media.play_mode".into());
    let (mut fixture, logical) = fixture();
    // The profile leaves the channel unflagged; the attribute registry is what makes it snap.
    retarget_only_channel(&mut fixture, "media.play_mode");
    let engine = Engine::new(programmers.clone());
    engine.set_control_timing([120.0; 5], 1_000, 0, 0);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    programmers.set_faded(
        session,
        logical,
        play_mode.clone(),
        AttributeValue::Normalized(0.85),
    );

    assert!(
        (normalized(&engine.resolved_values(), logical, "media.play_mode") - 0.85).abs() < 0.001,
        "the chosen play mode has to resolve in the same frame it was chosen"
    );
}
