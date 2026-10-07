//! TL-659: every Cue transition is owed to the first output frame sampled at or after it.
use super::*;

fn two_cues() -> (CueList, FixtureId) {
    let fixture = FixtureId::new();
    let mut one = Cue::new(cue_number(1.0));
    one.changes.push(value(fixture, "intensity", 0.5));
    let mut two = Cue::new(cue_number(2.0));
    two.changes.push(value(fixture, "intensity", 1.0));
    (list(vec![one, two]), fixture)
}

#[test]
fn a_go_is_claimed_once_by_the_first_frame_sampled_after_it() {
    let (cue_list, _) = two_cues();
    let id = cue_list.id;
    let mut playback = PlaybackEngine::default();
    playback.register(cue_list).unwrap();
    let go = Utc::now();

    playback.go_at(id, go).unwrap();

    let frame = go + chrono::Duration::milliseconds(12);
    assert_eq!(
        playback.claim_change_lead_start(frame),
        Some(go.timestamp_micros())
    );
    assert_eq!(
        playback.claim_change_lead_start(frame),
        None,
        "claimed once"
    );
}

#[test]
fn every_transition_kind_marks_its_instant_and_a_frame_measures_the_earliest() {
    let (cue_list, _) = two_cues();
    let id = cue_list.id;
    let mut playback = PlaybackEngine::default();
    playback.register(cue_list).unwrap();
    let first = Utc::now();
    let second = first + chrono::Duration::milliseconds(4);
    let third = first + chrono::Duration::milliseconds(9);

    playback.go_at(id, first).unwrap();
    playback.go_at(id, second).unwrap();
    playback.back_at(id, third).unwrap();

    assert_eq!(
        playback.claim_change_lead_start(third),
        Some(first.timestamp_micros()),
        "one frame carries all three; its lead runs from the earliest"
    );
}

#[test]
fn an_automatic_follow_is_marked_at_the_tick_that_takes_it() {
    let (mut cue_list, _) = two_cues();
    cue_list.cues[1].trigger = CueTrigger::Follow { delay_millis: 0 };
    let id = cue_list.id;
    let mut playback = PlaybackEngine::default();
    playback.register(cue_list).unwrap();
    let go = Utc::now();
    playback.go_at(id, go).unwrap();
    assert!(playback.claim_change_lead_start(go).is_some());

    let tick = go + chrono::Duration::milliseconds(25);
    let result = playback.tick(tick, None);

    assert!(!result.transitions.is_empty(), "the Follow fired");
    assert_eq!(
        playback.claim_change_lead_start(tick),
        Some(tick.timestamp_micros())
    );
}
