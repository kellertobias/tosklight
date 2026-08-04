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
fn intensity_uses_independent_in_and_out_master_timing() {
    let incoming = FixtureId::new();
    let outgoing = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(incoming, "intensity", 0.0));
    first.changes.push(value(outgoing, "intensity", 1.0));
    let mut second = Cue::new(2.0);
    second.fade_millis = 1_000;
    second.delay_millis = 0;
    second.out_fade_millis = Some(2_000);
    second.out_delay_millis = Some(500);
    second.changes.push(value(incoming, "intensity", 1.0));
    second.changes.push(value(outgoing, "intensity", 0.0));
    let cue_list = list(vec![first, second]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    let started = Utc::now();
    engine.go_at(id, started).unwrap();
    engine.go_at(id, started).unwrap();

    let halfway_in = started + ChronoDuration::milliseconds(500);
    assert!((contribution_level(&engine, halfway_in, incoming) - 0.5).abs() < 0.01);
    assert!((contribution_level(&engine, halfway_in, outgoing) - 1.0).abs() < 0.01);

    let halfway_out = started + ChronoDuration::milliseconds(1_500);
    assert_eq!(contribution_level(&engine, halfway_out, incoming), 1.0);
    assert!((contribution_level(&engine, halfway_out, outgoing) - 0.5).abs() < 0.01);
}

#[test]
fn interrupted_go_fades_from_the_current_resolved_intensity() {
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(fixture, "intensity", 0.0));
    let mut second = Cue::new(2.0);
    second.fade_millis = 10_000;
    second.changes.push(value(fixture, "intensity", 1.0));
    let mut third = Cue::new(3.0);
    third.out_fade_millis = Some(10_000);
    third.changes.push(value(fixture, "intensity", 0.0));
    let cue_list = list(vec![first, second, third]);
    let id = cue_list.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list.clone()).unwrap();
    engine.go_at(id, started).unwrap();
    engine.go_at(id, started).unwrap();

    let interrupted_at = started + ChronoDuration::seconds(5);
    assert!((contribution_level(&engine, interrupted_at, fixture) - 0.5).abs() < 0.01);
    engine.go_at(id, interrupted_at).unwrap();
    assert!((contribution_level(&engine, interrupted_at, fixture) - 0.5).abs() < 0.01);
    assert!(
        (contribution_level(
            &engine,
            interrupted_at + ChronoDuration::seconds(5),
            fixture,
        ) - 0.25)
            .abs()
            < 0.01
    );

    let persisted = serde_json::to_value(engine.runtime()).unwrap();
    let runtime: Vec<ActivePlayback> = serde_json::from_value(persisted).unwrap();
    let mut restored = PlaybackEngine::default();
    restored.register(cue_list).unwrap();
    restored.restore_active(runtime);
    assert!(
        (contribution_level(
            &restored,
            interrupted_at + ChronoDuration::seconds(5),
            fixture,
        ) - 0.25)
            .abs()
            < 0.01
    );
}

#[test]
fn legacy_cue_out_timing_follows_existing_fade_and_delay() {
    let mut body = serde_json::to_value(Cue::new(1.0)).unwrap();
    let object = body.as_object_mut().unwrap();
    object.remove("out_fade_millis");
    object.remove("out_delay_millis");
    object.insert("fade_millis".into(), serde_json::json!(1_250));
    object.insert("delay_millis".into(), serde_json::json!(300));

    let cue: Cue = serde_json::from_value(body).unwrap();
    assert_eq!(cue.out_fade_millis, None);
    assert_eq!(cue.out_delay_millis, None);
}

#[test]
fn absent_out_fade_follows_the_effective_sequence_master_but_explicit_zero_snaps() {
    let inherited = FixtureId::new();
    let snapped = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(inherited, "intensity", 1.0));
    first.changes.push(value(snapped, "intensity", 1.0));
    let mut second = Cue::new(2.0);
    second.changes.push(value(inherited, "intensity", 0.0));
    second.changes.push(value(snapped, "intensity", 0.0));
    let mut third = Cue::new(3.0);
    third.out_fade_millis = Some(0);
    third.changes.push(value(inherited, "intensity", 1.0));
    third.changes.push(value(snapped, "intensity", 1.0));
    let mut fourth = Cue::new(4.0);
    fourth.out_fade_millis = Some(0);
    fourth.changes.push(value(inherited, "intensity", 0.0));
    fourth.changes.push(value(snapped, "intensity", 0.0));
    let cue_list = list(vec![first, second, third, fourth]);
    let id = cue_list.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.set_control_timing([120.0; 5], 3_000);
    engine.register(cue_list).unwrap();
    engine.go_at(id, started).unwrap();
    let second_at = started + ChronoDuration::seconds(3);
    engine.go_at(id, second_at).unwrap();
    assert!(
        (contribution_level(
            &engine,
            second_at + ChronoDuration::milliseconds(1_500),
            inherited,
        ) - 0.5)
            .abs()
            < 0.01
    );

    let third_at = second_at + ChronoDuration::seconds(3);
    engine.go_at(id, third_at).unwrap();
    let fourth_at = third_at + ChronoDuration::seconds(3);
    engine.go_at(id, fourth_at).unwrap();
    assert_eq!(contribution_level(&engine, fourth_at, snapped), 0.0);
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
fn per_value_force_and_disable_precedence_apply_to_outgoing_intensity() {
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(fixture, "intensity", 1.0));
    let mut second = Cue::new(2.0);
    second.out_delay_millis = Some(1_000);
    second.out_fade_millis = Some(4_000);
    let mut outgoing = value(fixture, "intensity", 0.0);
    outgoing.delay_millis = Some(0);
    outgoing.fade_millis = Some(500);
    second.changes.push(outgoing);
    let base = list(vec![first, second]);
    let started = Utc::now();

    let level_for = |cue_list: CueList, elapsed_millis| {
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine.go_at(id, started).unwrap();
        engine.go_at(id, started).unwrap();
        contribution_level(
            &engine,
            started + ChronoDuration::milliseconds(elapsed_millis),
            fixture,
        )
    };

    assert!((level_for(base.clone(), 250) - 0.5).abs() < 0.01);
    let mut forced = base.clone();
    forced.force_cue_timing = true;
    assert_eq!(level_for(forced, 500), 1.0);
    let mut disabled = base;
    disabled.disable_cue_timing = true;
    assert_eq!(level_for(disabled, 0), 0.0);
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
