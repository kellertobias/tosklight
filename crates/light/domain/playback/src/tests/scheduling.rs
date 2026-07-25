use super::*;

#[test]
fn backend_speed_groups_drive_assigned_chasers() {
    let mut cue_list = list(vec![Cue::new(1.0), Cue::new(2.0)]);
    cue_list.mode = CueListMode::Chaser;
    cue_list.speed_group = Some("B".into());
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.set_control_timing([60.0, 120.0, 30.0, 15.0, 10.0], 0);
    engine.register(cue_list).unwrap();
    let started = Utc::now();
    engine.go_at(id, started).unwrap();
    engine.tick(started + ChronoDuration::milliseconds(499), None);
    assert_eq!(engine.active()[0].cue_index, 0);
    engine.tick(started + ChronoDuration::milliseconds(500), None);
    assert_eq!(engine.active()[0].cue_index, 1);

    let mut fifth = list(vec![Cue::new(1.0), Cue::new(2.0)]);
    fifth.mode = CueListMode::Chaser;
    fifth.speed_group = Some("E".into());
    let fifth_id = fifth.id;
    engine.register(fifth).unwrap();
    engine.go_at(fifth_id, started).unwrap();
    engine.tick(started + ChronoDuration::milliseconds(5_999), None);
    assert_eq!(
        engine
            .active()
            .iter()
            .find(|active| active.cue_list_id == fifth_id)
            .unwrap()
            .cue_index,
        0
    );
    engine.tick(started + ChronoDuration::milliseconds(6_000), None);
    assert_eq!(
        engine
            .active()
            .iter()
            .find(|active| active.cue_list_id == fifth_id)
            .unwrap()
            .cue_index,
        1
    );
}

#[test]
fn chaser_large_virtual_jump_matches_incremental_phase() {
    let mut cue_list = list(vec![
        Cue::new(1.0),
        Cue::new(2.0),
        Cue::new(3.0),
        Cue::new(4.0),
    ]);
    cue_list.mode = CueListMode::Chaser;
    cue_list.speed_group = Some("A".into());
    cue_list.speed_multiplier = 2.0;
    let id = cue_list.id;
    let started = Utc::now();

    let mut direct = PlaybackEngine::default();
    direct.register(cue_list.clone()).unwrap();
    direct.go_at(id, started).unwrap();
    direct.tick(started + ChronoDuration::milliseconds(1_000), None);

    let mut incremental = PlaybackEngine::default();
    incremental.register(cue_list).unwrap();
    incremental.go_at(id, started).unwrap();
    for millis in [250, 500, 750, 1_000] {
        incremental.tick(started + ChronoDuration::milliseconds(millis), None);
    }

    assert_eq!(
        direct.active()[0].cue_index,
        incremental.active()[0].cue_index
    );
    assert_eq!(
        direct.active()[0].previous_index,
        incremental.active()[0].previous_index
    );
    assert_eq!(
        direct.active()[0].activated_at,
        incremental.active()[0].activated_at
    );
}

#[test]
fn decimal_speed_group_bpm_reaches_chaser_scheduling_without_integer_rounding() {
    let mut cue_list = list(vec![Cue::new(1.0), Cue::new(2.0)]);
    cue_list.mode = CueListMode::Chaser;
    cue_list.speed_group = Some("B".into());
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.set_control_timing([120.0, 127.5, 60.0, 30.0, 15.0], 0);
    engine.register(cue_list).unwrap();
    let started = Utc::now();
    engine.go_at(id, started).unwrap();

    // 60,000 / 127.5 rounds to a 471 ms step. Integer-rounding the BPM to 128 would
    // incorrectly advance at 469 ms instead.
    engine.tick(started + ChronoDuration::milliseconds(470), None);
    assert_eq!(engine.active()[0].cue_index, 0);
    engine.tick(started + ChronoDuration::milliseconds(471), None);
    assert_eq!(engine.active()[0].cue_index, 1);
}

#[test]
fn chaser_bpm_change_preserves_normalized_step_phase() {
    let mut cue_list = list(vec![Cue::new(1.0), Cue::new(2.0)]);
    cue_list.mode = CueListMode::Chaser;
    cue_list.speed_group = Some("A".into());
    let id = cue_list.id;
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(cue_list).unwrap();
    engine.go_at(id, started).unwrap();

    clock.set(started + ChronoDuration::milliseconds(250));
    engine.tick(started + ChronoDuration::milliseconds(250), None);
    engine.set_control_timing([60.0, 90.0, 60.0, 30.0, 15.0], 0);
    engine.tick(started + ChronoDuration::milliseconds(749), None);
    assert_eq!(engine.active()[0].cue_index, 0);
    engine.tick(started + ChronoDuration::milliseconds(750), None);
    assert_eq!(engine.active()[0].cue_index, 1);
    assert_eq!(
        engine.active()[0].activated_at,
        started + ChronoDuration::milliseconds(750)
    );
}

#[test]
fn sequence_master_fade_only_fills_missing_cue_fades() {
    let fixture = FixtureId::new();
    let mut fallback = Cue::new(1.0);
    fallback.changes.push(value(fixture, "intensity", 1.0));
    let mut cue_list = list(vec![fallback]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.set_control_timing([120.0, 90.0, 60.0, 30.0, 15.0], 1_000);
    engine.register(cue_list.clone()).unwrap();
    let started = Utc::now();
    engine.go_at(id, started).unwrap();
    assert!(
        (contribution_level(
            &engine,
            started + ChronoDuration::milliseconds(500),
            fixture
        ) - 0.5)
            .abs()
            < 0.01
    );

    cue_list.cues[0].fade_millis = 2_000;
    let mut explicit = PlaybackEngine::default();
    explicit.set_control_timing([120.0, 90.0, 60.0, 30.0, 15.0], 1_000);
    explicit.register(cue_list).unwrap();
    explicit.go_at(id, started).unwrap();
    assert!(
        (contribution_level(
            &explicit,
            started + ChronoDuration::milliseconds(500),
            fixture
        ) - 0.25)
            .abs()
            < 0.01
    );
}
