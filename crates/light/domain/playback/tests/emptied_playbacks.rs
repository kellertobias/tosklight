//! A Cuelist that no longer holds any parameter turns itself off.

use chrono::{Duration as ChronoDuration, Utc};
use light_core::{AttributeKey, AttributeValue, CueListId, FixtureId};
use light_playback::{
    Cue, CueChange, CueList, CueListMode, IntensityPriorityMode, PlaybackEngine, RestartMode,
    WrapMode,
};

fn cue_list(cues: Vec<Cue>) -> CueList {
    CueList {
        id: CueListId::new(),
        name: "Release test".into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 100,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Off),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: None,
        speed_multiplier: 1.0,
        cues,
    }
}

fn enabled(playback: &PlaybackEngine, id: CueListId) -> bool {
    playback
        .active()
        .iter()
        .any(|active| active.cue_list_id == id && active.enabled)
}

#[test]
fn a_cuelist_turns_itself_off_once_its_release_has_faded_out() {
    let fixture = FixtureId::new();
    let color = AttributeKey("color.red".into());
    let mut set = Cue::new(1_u16.into());
    set.changes.push(CueChange::set(
        fixture,
        color.clone(),
        AttributeValue::Normalized(1.0),
    ));
    let mut release = Cue::new(2_u16.into());
    release.fade_millis = 500;
    release.changes.push(CueChange {
        value: None,
        ..CueChange::set(fixture, color, AttributeValue::Normalized(0.0))
    });
    let list = cue_list(vec![set, release]);
    let id = list.id;
    let started = Utc::now();
    let at = |millis| started + ChronoDuration::milliseconds(millis);
    let mut playback = PlaybackEngine::default();
    playback.register(list).unwrap();

    playback.go_at(id, started).unwrap();
    playback.tick(at(1_000), None);
    assert!(enabled(&playback, id), "Cue 1 holds a value");

    playback.go_at(id, at(1_000)).unwrap();
    playback.tick(at(1_300), None);
    assert!(enabled(&playback, id), "the release is still fading");

    playback.tick(at(1_600), None);
    assert!(!enabled(&playback, id), "nothing is held any more");
}

#[test]
fn a_cuelist_that_never_held_anything_stays_on() {
    // An empty first Cue waiting for the next GO is not a released list.
    let list = cue_list(vec![Cue::new(1_u16.into()), Cue::new(2_u16.into())]);
    let id = list.id;
    let started = Utc::now();
    let mut playback = PlaybackEngine::default();
    playback.register(list).unwrap();

    playback.go_at(id, started).unwrap();
    playback.tick(started + ChronoDuration::seconds(10), None);

    assert!(enabled(&playback, id));
}

#[test]
fn a_cue_that_still_holds_something_keeps_the_list_on() {
    let fixture = FixtureId::new();
    let red = AttributeKey("color.red".into());
    let blue = AttributeKey("color.blue".into());
    let mut set = Cue::new(1_u16.into());
    for attribute in [&red, &blue] {
        set.changes.push(CueChange::set(
            fixture,
            attribute.clone(),
            AttributeValue::Normalized(1.0),
        ));
    }
    let mut partial = Cue::new(2_u16.into());
    partial.changes.push(CueChange {
        value: None,
        ..CueChange::set(fixture, red, AttributeValue::Normalized(0.0))
    });
    let list = cue_list(vec![set, partial]);
    let id = list.id;
    let started = Utc::now();
    let mut playback = PlaybackEngine::default();
    playback.register(list).unwrap();

    playback.go_at(id, started).unwrap();
    playback.go_at(id, started).unwrap();
    playback.tick(started + ChronoDuration::seconds(10), None);

    assert!(enabled(&playback, id), "blue is still held");
}
