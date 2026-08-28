//! A 3D Point that a tracking system holds, and everything that must not move it.
//!
//! The attribute here is `point.position.x` because that is what a 3D Point actually carries, but
//! nothing in the engine is fussy about the name: what is being tested is that an override
//! outranks each source in turn, and that dropping the binding hands the attribute straight back.

use super::*;

fn tracked_point() -> (PatchedFixture, FixtureId) {
    schema_v2_fixture(&[("point.position.x", false, false, false, false, false)])
}

fn held(fixture_id: FixtureId, value: f32) -> crate::TrackedOverride {
    crate::TrackedOverride::new(
        fixture_id,
        AttributeKey("point.position.x".into()),
        AttributeValue::Normalized(value),
    )
}

fn point_at(engine: &Engine, fixture_id: FixtureId) -> f32 {
    normalized(&engine.resolved_values(), fixture_id, "point.position.x")
}

#[test]
fn a_bound_point_ignores_the_programmer() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = tracked_point();
    programmers.set(
        session,
        fixture_id,
        AttributeKey("point.position.x".into()),
        AttributeValue::Normalized(0.2),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    assert_eq!(point_at(&engine, fixture_id), 0.2);

    engine.set_tracked_overrides([held(fixture_id, 0.75)]);
    assert_eq!(point_at(&engine, fixture_id), 0.75);
}

#[test]
fn a_bound_point_ignores_a_running_cue() {
    let (fixture, fixture_id) = tracked_point();
    let cue_list = test_cue_list(
        "Positions",
        vec![CueChange::set(
            fixture_id,
            AttributeKey("point.position.x".into()),
            AttributeValue::Normalized(0.1),
        )],
    );
    let mut playback = test_playback(1, cue_list.id);
    playback.auto_off = false;
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![cue_list].into(),
            playbacks: vec![playback].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([held(fixture_id, 0.6)]);

    // The cue goes live against a point that is already bound: the marker keeps it.
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    assert_eq!(point_at(&engine, fixture_id), 0.6);
}

#[test]
fn unbinding_hands_the_point_back() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = tracked_point();
    programmers.set(
        session,
        fixture_id,
        AttributeKey("point.position.x".into()),
        AttributeValue::Normalized(0.2),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([held(fixture_id, 0.75)]);
    assert_eq!(point_at(&engine, fixture_id), 0.75);

    // Unbinding is the operator's way of taking the point back, and it is immediate: the
    // programmer value that was underneath all along is what the point reads again.
    engine.clear_tracked_overrides();
    assert_eq!(point_at(&engine, fixture_id), 0.2);
    assert!(engine.tracked_overrides().is_empty());
}

#[test]
fn a_source_that_goes_quiet_holds_where_it_last_was() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = tracked_point();
    programmers.set(
        session,
        fixture_id,
        AttributeKey("point.position.x".into()),
        AttributeValue::Normalized(0.2),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([held(fixture_id, 0.8)]);

    // No further frames arrive. Nothing tells the engine so, which is the point: the last
    // position stays held rather than snapping back to the programmer mid-show.
    for _ in 0..3 {
        assert_eq!(point_at(&engine, fixture_id), 0.8);
    }
}

#[test]
fn only_the_bound_point_is_held() {
    let (bound, bound_id) = tracked_point();
    let (mut loose, loose_id) = tracked_point();
    // A second point of the same kind, patched somewhere else in the rig.
    loose.fixture_number = Some(2);
    loose.address = Some(10);
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    for fixture_id in [bound_id, loose_id] {
        programmers.set(
            session,
            fixture_id,
            AttributeKey("point.position.x".into()),
            AttributeValue::Normalized(0.3),
        );
    }
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![bound, loose].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([held(bound_id, 0.9)]);

    assert_eq!(point_at(&engine, bound_id), 0.9);
    assert_eq!(point_at(&engine, loose_id), 0.3);
}

#[test]
fn the_render_path_holds_the_point_as_well_as_the_read_path() {
    let (fixture, fixture_id) = tracked_point();
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([held(fixture_id, 0.5)]);

    let rendered = engine.render(RenderOptions::default()).unwrap();
    let value = rendered.resolved_values[&(fixture_id, AttributeKey("point.position.x".into()))]
        .normalized()
        .unwrap();
    assert_eq!(value, 0.5);
}
