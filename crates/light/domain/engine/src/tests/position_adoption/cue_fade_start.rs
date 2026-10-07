//! TL-552: a Cue that fades Angles in over a fixture with no prior Position starts from the
//! fixture's declared default pose, the cue-fade analogue of the Dynamic declared-default base.
//! Angles never fade from an invented 0°: a fixture whose default cannot be decoded keeps the
//! passive hold until the fade completes.
use super::*;
use chrono::TimeZone;
use light_core::programming::{PositionIntent, ProgrammingOwner, ScalarIntent};
use light_playback::{Cue, CueChange};

const PAN: f32 = 90.;
const TILT: f32 = 30.;

fn cue_engine(fixture: PatchedFixture) -> (Engine, FixtureId, Arc<ManualClock>, CueList) {
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    ));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    registry.start(SessionId::new());
    let root = fixture.fixture_id;
    let angles = AttributeValue::Position(Arc::new(PositionIntent::angles(PAN, TILT)));
    let mut list = test_cue_list("Fade in", vec![]);
    let mut cue = Cue::new(1_u16.into());
    cue.changes = vec![CueChange::set(
        root,
        ProgrammingOwner::Position.key(),
        angles,
    )];
    cue.fade_millis = 1000;
    list.cues = vec![cue];
    let engine = Engine::new(registry);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![list.clone()].into(),
            playbacks: vec![test_playback(1, list.id)].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    (engine, root, clock, list)
}

fn angles(engine: &Engine, owner: FixtureId) -> Option<(f32, f32)> {
    let frame = engine.observe_source_frame(&[]);
    let AttributeValue::Position(position) = frame
        .values()
        .value(owner, &ProgrammingOwner::Position.key())?
    else {
        return None;
    };
    match position.as_ref() {
        PositionIntent::Angles {
            pan_degrees: ScalarIntent::Value(pan),
            tilt_degrees: ScalarIntent::Value(tilt),
        } => Some((*pan, *tilt)),
        _ => None,
    }
}

#[test]
fn cue_angles_fade_in_from_the_declared_default_pose() {
    let (engine, root, clock, _) = cue_engine(mover());
    let default = engine
        .declared_default_position(&engine.snapshot(), root)
        .expect("the U16 mover's default_raw maps into Angles");
    // 32768 of 65535 on a ±720° channel: just off centre, never an invented 0°.
    assert_ne!(default.pan_degrees, 0.0);
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    clock.advance_millis(250);
    let (pan, tilt) = angles(&engine, root).expect("the fade renders from its first frame");
    let lerp = |from: f32, to: f32| from + (to - from) * 0.25;
    assert!(
        (pan - lerp(default.pan_degrees, PAN)).abs() < 1e-3,
        "Pan {pan}"
    );
    assert!(
        (tilt - lerp(default.tilt_degrees, TILT)).abs() < 1e-3,
        "Tilt {tilt}"
    );
    clock.advance_millis(750);
    assert_eq!(angles(&engine, root), Some((PAN, TILT)));
}

#[test]
fn a_go_during_the_fade_in_continues_from_where_the_fade_was() {
    let (engine, root, clock, list) = cue_engine(mover());
    let mut next = Cue::new(2_u16.into());
    next.changes = vec![CueChange::set(
        root,
        ProgrammingOwner::Position.key(),
        AttributeValue::Position(Arc::new(PositionIntent::angles(-PAN, 0.))),
    )];
    next.fade_millis = 1000;
    let mut snapshot = (*engine.snapshot()).clone();
    let mut list = list;
    list.cues.push(next);
    snapshot.cue_lists = vec![list].into();
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    clock.advance_millis(500);
    let (midway, _) = angles(&engine, root).unwrap();
    // The interrupted source a GO captures uses the same declared start as the frame, so the
    // next fade starts at the interior pose instead of jumping back to the default.
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    let (resumed, _) = angles(&engine, root).expect("the second fade renders at once");
    assert!((resumed - midway).abs() < 1e-3, "{resumed} vs {midway}");
    clock.advance_millis(500);
    let (half, _) = angles(&engine, root).unwrap();
    assert!(
        (half - (midway + (-PAN - midway) * 0.5)).abs() < 1e-3,
        "{half}"
    );
}

#[test]
fn undecodable_default_keeps_the_passive_hold_until_the_fade_completes() {
    // Negative control: without a Position physical model there is no declared default pose,
    // so nothing renders mid-fade (never 0°) and the authored Angles land when the fade ends.
    let mut fixture = mover();
    redefine(&mut fixture, |profile| {
        profile.modes[0].position_physical = None
    });
    let (engine, root, clock, _) = cue_engine(fixture);
    assert!(
        engine
            .declared_default_position(&engine.snapshot(), root)
            .is_none()
    );
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    clock.advance_millis(250);
    assert_eq!(angles(&engine, root), None);
    clock.advance_millis(750);
    assert_eq!(angles(&engine, root), Some((PAN, TILT)));
}
