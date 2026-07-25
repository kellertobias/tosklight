use chrono::{Duration as ChronoDuration, Utc};
use light_core::CueListId;
use light_playback::{
    AutomaticPlaybackTransition, AutomaticPlaybackTransitionCause, Cue, CueList, CueListMode,
    CueTrigger, IntensityPriorityMode, PlaybackEngine, RestartMode, WrapMode,
};

fn cue_list(cues: Vec<Cue>) -> CueList {
    CueList {
        id: CueListId::new(),
        name: "Automatic transition test".into(),
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
        chaser_xfade_millis: 0,
        chaser_xfade_percent: None,
        speed_multiplier: 1.0,
        cues,
    }
}

fn only_transition(transitions: Vec<AutomaticPlaybackTransition>) -> AutomaticPlaybackTransition {
    assert_eq!(transitions.len(), 1);
    transitions.into_iter().next().unwrap()
}

#[test]
fn delayed_chaser_tick_emits_one_final_state_transition() {
    let mut list = cue_list((1..=4).map(|number| Cue::new(f64::from(number))).collect());
    list.mode = CueListMode::Chaser;
    list.wrap_mode = Some(WrapMode::Reset);
    let id = list.id;
    let started = Utc::now();
    let mut playback = PlaybackEngine::default();
    playback.register(list).unwrap();
    playback.go_at(id, started).unwrap();

    let event = only_transition(
        playback
            .tick(started + ChronoDuration::milliseconds(700), None)
            .transitions,
    );

    assert_eq!(event.cause, AutomaticPlaybackTransitionCause::Chaser);
    assert_eq!(event.previous.number, 1.0);
    assert_eq!(event.current.number, 4.0);
    assert_eq!(event.advanced_steps, 7);
}

#[test]
fn follow_trigger_emits_its_cause_after_completion_and_delay() {
    let mut next = Cue::new(2.0);
    next.trigger = CueTrigger::Follow { delay_millis: 100 };
    let list = cue_list(vec![Cue::new(1.0), next]);
    let id = list.id;
    let started = Utc::now();
    let mut playback = PlaybackEngine::default();
    playback.register(list).unwrap();
    playback.go_at(id, started).unwrap();

    assert!(
        playback
            .tick(started + ChronoDuration::milliseconds(99), None)
            .transitions
            .is_empty()
    );
    let event = only_transition(
        playback
            .tick(started + ChronoDuration::milliseconds(100), None)
            .transitions,
    );
    assert_eq!(event.cause, AutomaticPlaybackTransitionCause::Follow);
    assert_eq!(event.advanced_steps, 1);
}

#[test]
fn wait_trigger_emits_its_distinct_cause() {
    let mut next = Cue::new(2.0);
    next.trigger = CueTrigger::Wait { delay_millis: 25 };
    let list = cue_list(vec![Cue::new(1.0), next]);
    let id = list.id;
    let started = Utc::now();
    let mut playback = PlaybackEngine::default();
    playback.register(list).unwrap();
    playback.go_at(id, started).unwrap();

    let event = only_transition(
        playback
            .tick(started + ChronoDuration::milliseconds(25), None)
            .transitions,
    );
    assert_eq!(event.cause, AutomaticPlaybackTransitionCause::Wait);
    assert_eq!(event.current.number, 2.0);
}

#[test]
fn timecode_jump_reports_the_number_of_crossed_cues() {
    let mut second = Cue::new(2.0);
    second.trigger = CueTrigger::Timecode { frame: 100 };
    let mut third = Cue::new(3.0);
    third.trigger = CueTrigger::Timecode { frame: 200 };
    let list = cue_list(vec![Cue::new(1.0), second, third]);
    let id = list.id;
    let started = Utc::now();
    let mut playback = PlaybackEngine::default();
    playback.register(list).unwrap();
    playback.go_at(id, started).unwrap();

    let event = only_transition(playback.tick(started, Some(200)).transitions);
    assert_eq!(event.cause, AutomaticPlaybackTransitionCause::Timecode);
    assert_eq!(event.previous.number, 1.0);
    assert_eq!(event.current.number, 3.0);
    assert_eq!(event.advanced_steps, 2);
}
