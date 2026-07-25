use super::*;

#[test]
fn tracked_direct_jump_equals_sequential_state() {
    let fixture = FixtureId::new();
    let mut one = Cue::new(1.0);
    one.changes.push(value(fixture, "intensity", 1.0));
    let mut two = Cue::new(2.0);
    two.changes.push(value(fixture, "pan", 0.5));
    let three = Cue::new(3.0);
    let list = list(vec![one, two, three]);
    assert_eq!(list.state_at_number(3.0), list.state_at_index(2));
    assert_eq!(list.state_at_index(2).len(), 2);
}

#[test]
fn zero_delay_zero_fade_cue_is_active_at_go_timestamp() {
    let fixture = FixtureId::new();
    let mut cue = Cue::new(1.0);
    cue.changes.push(value(fixture, "pan", 0.25));
    let cue_list = list(vec![cue]);
    let cue_list_id = cue_list.id;
    let now = Utc::now();
    let mut playback = PlaybackEngine::default();
    playback.register(cue_list).unwrap();
    playback.go_at(cue_list_id, now).unwrap();
    let contribution = playback.contributions_at(now);
    assert_eq!(contribution.len(), 1);
    assert_eq!(contribution[0].value, AttributeValue::Normalized(0.25));
}

#[test]
fn cue_only_restores_previous_value_in_following_cue() {
    let fixture = FixtureId::new();
    let mut one = Cue::new(1.0);
    one.changes.push(value(fixture, "intensity", 0.2));
    let two = Cue::new(2.0);
    let three = Cue::new(3.0);
    let mut list = list(vec![one, two, three]);
    list.store_cue_only(1, vec![value(fixture, "intensity", 1.0)])
        .unwrap();
    assert!(list.cues[1].cue_only);
    assert!(list.cues[2].changes[0].automatic_restore);
    assert_eq!(
        list.state_at_index(1)[&(fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(1.0)
    );
    assert_eq!(
        list.state_at_index(2)[&(fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(0.2)
    );
}

#[test]
fn cue_only_releases_new_attribute_in_following_cue() {
    let fixture = FixtureId::new();
    let mut list = list(vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)]);
    list.store_cue_only(1, vec![value(fixture, "intensity", 1.0)])
        .unwrap();
    assert!(list.state_at_index(2).is_empty());
}

#[test]
fn legacy_cues_default_cue_only_and_group_restore_metadata_to_false() {
    let mut body = serde_json::to_value(Cue::new(1.0)).unwrap();
    body.as_object_mut().unwrap().remove("cue_only");
    body["group_changes"] = serde_json::json!([{
        "group_id": "1",
        "attribute": "intensity",
        "value": { "kind": "normalized", "value": 0.5 }
    }]);
    let cue: Cue = serde_json::from_value(body).unwrap();
    assert!(!cue.cue_only);
    assert!(!cue.group_changes[0].automatic_restore);
}

#[test]
fn explicit_next_cue_change_beats_automatic_restore() {
    let fixture = FixtureId::new();
    let one = Cue::new(1.0);
    let two = Cue::new(2.0);
    let mut three = Cue::new(3.0);
    three.changes.push(value(fixture, "intensity", 0.7));
    let mut list = list(vec![one, two, three]);
    list.store_cue_only(1, vec![value(fixture, "intensity", 1.0)])
        .unwrap();
    assert_eq!(
        list.state_at_index(2)[&(fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(0.7)
    );
}

#[test]
fn priority_then_htp_resolution() {
    let fixture = FixtureId::new();
    let now = Utc::now();
    let make = |level, priority| TimedValue {
        fixture_id: fixture,
        attribute: AttributeKey::intensity(),
        value: AttributeValue::Normalized(level),
        priority,
        changed_at: now,
        programmer_order: 0,
        merge_mode: MergeMode::Htp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    };
    assert_eq!(
        resolve([make(1.0, 1), make(0.2, 2)])[&(fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(0.2)
    );
    assert_eq!(
        resolve([make(0.4, 2), make(0.8, 2)])[&(fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(0.8)
    );
}

#[test]
fn phaser_interpolates_and_distributes_phase() {
    let phaser = Phaser {
        mode: PhaserMode::Absolute,
        steps: vec![
            PhaserStep {
                position: 0.0,
                value: 0.0,
                curve_to_next: PhaserCurve::Linear,
            },
            PhaserStep {
                position: 0.5,
                value: 1.0,
                curve_to_next: PhaserCurve::Linear,
            },
        ],
        cycles_per_minute: 60.0,
        phase_start_degrees: 0.0,
        phase_end_degrees: 180.0,
        width: 1.0,
    };
    assert!((phaser.sample(0.25, 0, 2) - 0.5).abs() < 0.001);
    assert!((phaser.sample(0.0, 1, 2) - 1.0).abs() < 0.001);
}

#[test]
fn fades_from_zero_and_between_tracked_states() {
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.fade_millis = 1_000;
    first.changes.push(value(fixture, "intensity", 1.0));
    let mut second = Cue::new(2.0);
    second.fade_millis = 1_000;
    second.changes.push(value(fixture, "intensity", 0.0));
    let cue_list = list(vec![first, second]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
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
    engine
        .go_at(id, started + ChronoDuration::seconds(1))
        .unwrap();
    assert!(
        (contribution_level(
            &engine,
            started + ChronoDuration::milliseconds(1_500),
            fixture
        ) - 0.5)
            .abs()
            < 0.01
    );
}

#[test]
fn cue_changes_keep_independent_fade_and_delay_times() {
    let first = FixtureId::new();
    let second = FixtureId::new();
    let mut cue = Cue::new(1.0);
    let mut immediate = value(first, "intensity", 1.0);
    immediate.fade_millis = Some(1_000);
    immediate.delay_millis = Some(0);
    cue.changes.push(immediate);
    let mut delayed = value(second, "intensity", 1.0);
    delayed.fade_millis = Some(1_000);
    delayed.delay_millis = Some(500);
    cue.changes.push(delayed);
    let cue_list = list(vec![cue]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    let started = Utc::now();
    engine.go_at(id, started).unwrap();
    assert!(
        (contribution_level(&engine, started + ChronoDuration::milliseconds(500), first) - 0.5)
            .abs()
            < 0.01
    );
    assert!(
        contribution_level(&engine, started + ChronoDuration::milliseconds(500), second).abs()
            < 0.01
    );
    assert!(
        (contribution_level(
            &engine,
            started + ChronoDuration::milliseconds(1_000),
            second
        ) - 0.5)
            .abs()
            < 0.01
    );
}

#[test]
fn pause_freezes_and_resume_continues_fade() {
    let fixture = FixtureId::new();
    let mut cue = Cue::new(1.0);
    cue.fade_millis = 1_000;
    cue.changes.push(value(fixture, "intensity", 1.0));
    let cue_list = list(vec![cue]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    let started = Utc::now();
    engine.go_at(id, started).unwrap();
    engine
        .pause_at(id, started + ChronoDuration::milliseconds(250))
        .unwrap();
    assert!(
        (contribution_level(
            &engine,
            started + ChronoDuration::milliseconds(800),
            fixture
        ) - 0.25)
            .abs()
            < 0.01
    );
    engine
        .go_at(id, started + ChronoDuration::milliseconds(800))
        .unwrap();
    assert!(
        (contribution_level(
            &engine,
            started + ChronoDuration::milliseconds(1_050),
            fixture
        ) - 0.5)
            .abs()
            < 0.01
    );
}
