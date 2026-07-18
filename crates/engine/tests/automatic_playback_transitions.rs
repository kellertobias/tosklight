use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};
use light_core::{CueListId, ManualClock};
use light_engine::{Engine, EngineSnapshot, RenderOptions};
use light_playback::{
    AutomaticPlaybackTransitionCause, Cue, CueList, CueListMode, CueTrigger, IntensityPriorityMode,
    RestartMode, WrapMode,
};
use light_programmer::ProgrammerRegistry;

#[test]
fn render_returns_automatic_transitions_after_releasing_playback_state() {
    let started = Utc::now();
    let clock = Arc::new(ManualClock::new(started));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let mut next = Cue::new(2.0);
    next.trigger = CueTrigger::Follow { delay_millis: 100 };
    let cue_list = cue_list(vec![Cue::new(1.0), next]);
    let id = cue_list.id;
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list],
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.playback().write().go_at(id, started).unwrap();
    clock.set(started + ChronoDuration::milliseconds(100));
    // Read-only projections cannot consume the transition before the output scheduler observes it.
    engine.resolved_values();

    let result = engine.render(RenderOptions::default()).unwrap();

    assert_eq!(result.automatic_playback_transitions.len(), 1);
    assert_eq!(
        result.automatic_playback_transitions[0].cause,
        AutomaticPlaybackTransitionCause::Follow
    );
    assert!(engine.playback().try_write().is_some());
}

fn cue_list(cues: Vec<Cue>) -> CueList {
    CueList {
        id: CueListId::new(),
        name: "Render transition test".into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
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
