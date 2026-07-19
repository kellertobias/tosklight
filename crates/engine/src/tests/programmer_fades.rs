use super::*;

#[test]
fn programmer_fade_starts_from_resolved_playback_underlay_and_release_reveals_it() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    let (fixture, logical) = fixture();
    let mut snapshot = mib_snapshot(vec![fixture], &[logical]);
    snapshot.cue_lists[0].cues[0]
        .changes
        .iter_mut()
        .find(|change| change.attribute.is_intensity())
        .unwrap()
        .value = Some(AttributeValue::Normalized(0.25));
    let engine = Engine::new(programmers.clone());
    engine.set_control_timing([120.0; 5], 1_000, 0);
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
fn overlapping_preload_group_fades_keep_edit_order_at_one_commit_timestamp() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    let (fixture, logical) = fixture();
    let engine = Engine::new(programmers.clone());
    engine.set_control_timing([120.0; 5], 3_000, 0);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            groups: vec![
                GroupDefinition {
                    id: "1".into(),
                    name: "Broad".into(),
                    fixtures: vec![logical],
                    ..Default::default()
                },
                GroupDefinition {
                    id: "2".into(),
                    name: "Subset".into(),
                    fixtures: vec![logical],
                    ..Default::default()
                },
            ],
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
    engine.set_control_timing([120.0, 90.0, 60.0, 30.0, 15.0], 1_000, 0);
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
    let faded = engine.faded_programmer_value(
        value,
        now,
        None,
        ProgrammerId::new(),
        ProgrammerTransitionSource::Programmer,
        false,
    );
    assert!(
        faded
            .value
            .normalized()
            .is_some_and(|level| (level - 0.5).abs() < 0.02)
    );
}
