use super::*;

#[test]
fn move_in_black_looks_through_dark_cues_and_uses_future_position_timing() {
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(fixture, "intensity", 1.0));
    first.changes.push(value(fixture, "pan", 0.2));
    first.changes.push(value(fixture, "color.red", 0.1));
    let mut dark = Cue::new(2.0);
    dark.changes.push(value(fixture, "intensity", 0.0));
    let another_dark = Cue::new(2.5);
    let mut lit = Cue::new(3.0);
    lit.changes.push(value(fixture, "intensity", 1.0));
    let mut pan = value(fixture, "pan", 0.8);
    pan.fade_millis = Some(3_000);
    lit.changes.push(pan);
    lit.changes.push(value(fixture, "color.red", 0.9));
    let cue_list = list(vec![first, dark, another_dark, lit]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();

    let candidates = engine.move_in_black_candidates();
    assert_eq!(candidates.len(), 1);
    let candidate = &candidates[0];
    assert_eq!(candidate.current_cue_number, 2.0);
    assert_eq!(candidate.target_cue_number, 3.0);
    assert_eq!(
        candidate.values.len(),
        1,
        "only Position-family values move early"
    );
    assert_eq!(candidate.values[0].attribute.0, "pan");
    assert_eq!(candidate.values[0].current, AttributeValue::Normalized(0.2));
    assert_eq!(candidate.values[0].target, AttributeValue::Normalized(0.8));
    assert_eq!(candidate.values[0].fade_millis, 3_000);
}

#[test]
fn move_in_black_does_not_look_across_the_end_of_a_cuelist() {
    let fixture = FixtureId::new();
    let mut lit = Cue::new(1.0);
    lit.changes.push(value(fixture, "intensity", 1.0));
    lit.changes.push(value(fixture, "pan", 0.8));
    let mut dark = Cue::new(2.0);
    dark.changes.push(value(fixture, "intensity", 0.0));
    dark.changes.push(value(fixture, "pan", 0.2));
    let mut cue_list = list(vec![lit, dark]);
    cue_list.wrap_mode = Some(WrapMode::Tracking);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();
    assert!(engine.move_in_black_candidates().is_empty());
}

#[test]
fn snap_attributes_bypass_cue_crossfades() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let fixture = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.changes.push(value(fixture, "pan", 0.0));
    first.changes.push(value(fixture, "tilt", 0.0));
    let mut second = Cue::new(2.0);
    second.fade_millis = 1_000;
    second.changes.push(value(fixture, "pan", 1.0));
    second.changes.push(value(fixture, "tilt", 1.0));
    let cue_list = list(vec![first, second]);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(cue_list).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    engine.go_playback(1).unwrap();
    engine.go_playback(1).unwrap();

    let halfway = started + ChronoDuration::milliseconds(500);
    clock.set(halfway);
    let values =
        resolve(engine.contributions_at_with_snap(halfway, |_, attribute| attribute.0 == "pan"));
    assert_eq!(
        values[&(fixture, AttributeKey("pan".into()))],
        AttributeValue::Normalized(1.0)
    );
    assert_eq!(
        values[&(fixture, AttributeKey("tilt".into()))],
        AttributeValue::Normalized(0.5)
    );
}

#[test]
fn snap_attributes_bypass_playback_master_crossfades() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let snap_fixture = FixtureId::new();
    let faded_fixture = FixtureId::new();
    let mut cue = Cue::new(1.0);
    for fixture in [snap_fixture, faded_fixture] {
        cue.changes.push(value(fixture, "intensity", 1.0));
    }
    let cue_list = list(vec![cue]);
    let cue_list_id = cue_list.id;
    let mut playback = definition(1, cue_list_id);
    playback.xfade_millis = 1_000;
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(cue_list).unwrap();
    engine.register_definition(playback).unwrap();
    engine.xfade(1, true).unwrap();

    let halfway = started + ChronoDuration::milliseconds(500);
    clock.set(halfway);
    engine.tick(halfway, None);
    let values =
        resolve(engine.contributions_at_with_snap(halfway, |fixture, _| fixture == snap_fixture));
    assert_eq!(
        values[&(snap_fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(1.0)
    );
    assert_eq!(
        values[&(faded_fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(0.5)
    );
}
