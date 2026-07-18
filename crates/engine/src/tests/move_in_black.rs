use super::*;

#[test]
fn hazardous_fixture_defaults_to_immediate_safe_on_control_loss() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    let (mut fixture, logical) = fixture();
    fixture.definition.hazardous = true;
    fixture
        .definition
        .safe_values
        .insert(AttributeKey::intensity(), AttributeValue::Normalized(0.0));
    programmers.set(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            cue_lists: vec![],
            playbacks: vec![],
            playback_pages: vec![],
            routes: vec![],
            control_mappings: vec![],
            groups: vec![],
            revision: 1,
        })
        .unwrap();
    let rendered = engine
        .render(RenderOptions {
            grand_master: 1.0,
            blackout: false,
            control_loss_progress: Some(0.0),
        })
        .unwrap();
    assert_eq!(rendered.universes[&1][0], 0);
}

#[test]
fn move_in_black_waits_for_resolved_darkness_then_prepositions_only_enabled_fixture() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let (enabled_fixture, enabled) = moving_fixture(1, true, 1_000);
    let (disabled_fixture, disabled) = moving_fixture(10, false, 1_000);
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(mib_snapshot(
            vec![enabled_fixture, disabled_fixture],
            &[enabled, disabled],
        ))
        .unwrap();
    engine.playback().write().go_playback(1).unwrap();
    engine.playback().write().go_playback(1).unwrap();

    clock.set(started + ChronoDuration::milliseconds(1_999));
    let values = engine.resolved_values();
    assert!(normalized(&values, enabled, "intensity") > 0.0);
    assert_eq!(normalized(&values, enabled, "pan"), 0.2);
    let runtime = engine.move_in_black_runtime();
    assert_eq!(
        runtime
            .iter()
            .find(|item| item.fixture_id == enabled)
            .unwrap()
            .state,
        MoveInBlackState::Blocked
    );

    clock.set(started + ChronoDuration::milliseconds(2_000));
    let values = engine.resolved_values();
    assert_eq!(normalized(&values, enabled, "intensity"), 0.0);
    let runtime = engine.move_in_black_runtime();
    let enabled_runtime = runtime
        .iter()
        .find(|item| item.fixture_id == enabled)
        .unwrap();
    assert_eq!(enabled_runtime.state, MoveInBlackState::Delaying);
    assert_eq!(enabled_runtime.dark_since, Some(clock.now()));
    assert_eq!(
        enabled_runtime.delay_deadline,
        Some(started + ChronoDuration::milliseconds(3_000))
    );
    assert_eq!(
        runtime
            .iter()
            .find(|item| item.fixture_id == disabled)
            .unwrap()
            .state,
        MoveInBlackState::Disabled
    );

    clock.set(started + ChronoDuration::milliseconds(2_999));
    assert_eq!(normalized(&engine.resolved_values(), enabled, "pan"), 0.2);
    clock.set(started + ChronoDuration::milliseconds(3_000));
    assert_eq!(normalized(&engine.resolved_values(), enabled, "pan"), 0.2);
    assert_eq!(
        engine
            .move_in_black_runtime()
            .iter()
            .find(|item| item.fixture_id == enabled)
            .unwrap()
            .movement_started_at,
        Some(started + ChronoDuration::milliseconds(3_000))
    );

    clock.set(started + ChronoDuration::milliseconds(4_500));
    let values = engine.resolved_values();
    assert!((normalized(&values, enabled, "pan") - 0.5).abs() < 0.001);
    assert_eq!(normalized(&values, disabled, "pan"), 0.2);

    clock.set(started + ChronoDuration::milliseconds(6_000));
    let values = engine.resolved_values();
    assert!((normalized(&values, enabled, "pan") - 0.8).abs() < 0.001);
    assert_eq!(normalized(&values, disabled, "pan"), 0.2);
    assert_eq!(
        engine
            .move_in_black_runtime()
            .iter()
            .find(|item| item.fixture_id == enabled)
            .unwrap()
            .state,
        MoveInBlackState::Completed
    );

    engine.playback().write().go_playback(1).unwrap();
    let values = engine.resolved_values();
    assert!(
        (normalized(&values, enabled, "pan") - 0.8).abs() < 0.001,
        "the completed hidden move must hand off without jumping back"
    );
    assert_eq!(normalized(&values, disabled, "pan"), 0.2);
}

#[test]
fn move_in_black_is_blocked_and_restarts_its_delay_after_intensity_returns() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    let (fixture, logical) = moving_fixture(1, true, 1_000);
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(mib_snapshot(vec![fixture], &[logical]))
        .unwrap();
    engine.playback().write().go_playback(1).unwrap();
    engine.playback().write().go_playback(1).unwrap();
    programmers.set(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    );

    clock.set(started + ChronoDuration::milliseconds(5_000));
    engine.resolved_values();
    let runtime = engine.move_in_black_runtime();
    let runtime = runtime
        .iter()
        .find(|item| item.fixture_id == logical)
        .unwrap();
    assert_eq!(runtime.state, MoveInBlackState::Blocked);
    assert_eq!(runtime.dark_since, None);

    programmers.clear(session);
    engine.resolved_values();
    let runtime = engine.move_in_black_runtime();
    let runtime = runtime
        .iter()
        .find(|item| item.fixture_id == logical)
        .unwrap();
    assert_eq!(runtime.dark_since, Some(clock.now()));
    assert_eq!(
        runtime.delay_deadline,
        Some(started + ChronoDuration::milliseconds(6_000))
    );

    clock.set(started + ChronoDuration::milliseconds(5_500));
    programmers.start(session, UserId::new());
    programmers.set(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    );
    engine.resolved_values();
    assert_eq!(
        engine
            .move_in_black_runtime()
            .iter()
            .find(|item| item.fixture_id == logical)
            .unwrap()
            .state,
        MoveInBlackState::Blocked
    );

    clock.set(started + ChronoDuration::milliseconds(6_000));
    programmers.clear(session);
    engine.resolved_values();
    let runtime = engine.move_in_black_runtime();
    let runtime = runtime
        .iter()
        .find(|item| item.fixture_id == logical)
        .unwrap();
    assert_eq!(runtime.dark_since, Some(clock.now()));
    assert_eq!(
        runtime.delay_deadline,
        Some(started + ChronoDuration::milliseconds(7_000)),
        "returning to dark starts a fresh complete delay"
    );
}

#[test]
fn move_in_black_obeys_same_priority_ltp_and_numeric_priority() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let (fixture, logical) = moving_fixture(1, true, 0);
    let mut snapshot = mib_snapshot(vec![fixture], &[logical]);

    let mut newer_list = snapshot.cue_lists[0].clone();
    newer_list.id = light_core::CueListId::new();
    newer_list.name = "Newer MIB".into();
    newer_list.cues[2]
        .changes
        .iter_mut()
        .find(|change| change.attribute == AttributeKey("pan".into()))
        .unwrap()
        .value = Some(AttributeValue::Normalized(0.4));
    let mut newer_playback = snapshot.playbacks[0].clone();
    newer_playback.number = 2;
    newer_playback.name = "Newer MIB".into();
    newer_playback.target = PlaybackTarget::CueList {
        cue_list_id: newer_list.id,
    };
    snapshot.cue_lists.push(newer_list);
    snapshot.playbacks.push(newer_playback);

    let engine = Engine::new(programmers);
    engine.replace_snapshot(snapshot.clone()).unwrap();
    for playback in [1, 2] {
        engine.playback().write().go_playback(playback).unwrap();
        engine.playback().write().go_playback(playback).unwrap();
    }

    clock.set(started + ChronoDuration::milliseconds(2_000));
    engine.resolved_values();
    clock.set(started + ChronoDuration::milliseconds(5_000));
    engine.resolved_values();

    snapshot.cue_lists[1].cues[2]
        .changes
        .iter_mut()
        .find(|change| change.attribute == AttributeKey("pan".into()))
        .unwrap()
        .value = Some(AttributeValue::Normalized(0.6));
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot.clone()).unwrap();
    engine.resolved_values();

    clock.set(started + ChronoDuration::milliseconds(6_500));
    let values = engine.resolved_values();
    assert!(
        (normalized(&values, logical, "pan") - 0.5).abs() < 0.001,
        "the recalculated same-priority MIB target is the newer LTP source"
    );

    snapshot.cue_lists[0].priority = 20;
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    let values = engine.resolved_values();
    assert!(
        (normalized(&values, logical, "pan") - 0.8).abs() < 0.001,
        "numeric priority overrides a newer lower-priority MIB source"
    );
}
