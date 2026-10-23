use super::*;

fn rig() -> (PlaybackEngine, CueList, DateTime<Utc>) {
    let cues = (1..=3)
        .map(|number| Cue::new(cue_number(f64::from(number))))
        .collect();
    let list = list(cues);
    let mut engine = PlaybackEngine::default();
    engine.register(list.clone()).unwrap();
    let now = Utc::now();
    engine
        .reconstruct_timeline_cues(list.id, &[(list.cues[0].id, 0)])
        .unwrap();
    (engine, list, now)
}

#[test]
fn exact_and_elapsed_timecode_crossings_publish_one_final_capture_transition() {
    for elapsed in [false, true] {
        let (mut engine, list, now) = rig();
        if elapsed {
            engine
                .execute_timeline_cues(list.id, &[(list.cues[0].id, 1750), (list.cues[1].id, 750)])
                .unwrap();
        } else {
            engine
                .execute_timeline_cue_id(list.id, list.cues[1].id)
                .unwrap();
        }
        let captured = engine.tick(now, None).transitions;
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].previous.id, list.cues[0].id);
        assert_eq!(captured[0].current.id, list.cues[1].id);
        assert_eq!(
            captured[0].cause,
            AutomaticPlaybackTransitionCause::Timecode
        );
        assert!(engine.tick(now, None).transitions.is_empty());
    }
}

#[test]
fn seek_release_and_superseding_operator_jump_discard_uncaptured_timecode_actions() {
    for cancel in ["seek", "release", "jump", "checkpoint", "clock restore"] {
        let (mut engine, list, now) = rig();
        engine
            .execute_timeline_cue_id(list.id, list.cues[1].id)
            .unwrap();
        match cancel {
            "seek" => {
                engine
                    .reconstruct_timeline_cues(list.id, &[(list.cues[0].id, 100)])
                    .unwrap();
            }
            "release" => {
                engine.release(list.id);
            }
            "jump" => {
                engine.jump_to_cue_id(list.id, list.cues[2].id).unwrap();
            }
            "checkpoint" => {
                engine.restore_permanent_runtime(engine.runtime(), std::iter::empty());
            }
            "clock restore" => {
                engine.restore_timeline_clock_state(engine.capture_timeline_clock_state());
            }
            _ => unreachable!(),
        }
        assert!(
            engine.tick(now, None).transitions.is_empty(),
            "{cancel} must supersede the pending live Cue action"
        );
    }
}

#[test]
fn only_the_latest_uncaptured_timecode_crossing_keeps_its_final_identity() {
    let (mut engine, list, now) = rig();
    engine
        .execute_timeline_cue_id(list.id, list.cues[1].id)
        .unwrap();
    engine
        .execute_timeline_cue_id(list.id, list.cues[2].id)
        .unwrap();
    let captured = engine.tick(now, None).transitions;
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].current.id, list.cues[2].id);
    let active = engine.active.get(&PlaybackKey::CueList(list.id)).unwrap();
    assert_eq!(captured[0].transition_ordinal, active.transition_ordinal);
}

#[test]
fn multiqueue_history_retains_timecode_ownership_and_does_not_auto_follow() {
    let (mut engine, mut list, now) = rig();
    list.cues[2].trigger = CueTrigger::Follow { delay_millis: 0 };
    engine.register(list.clone()).unwrap();
    engine
        .reconstruct_timeline_cues(list.id, &[(list.cues[0].id, 2000), (list.cues[1].id, 1000)])
        .unwrap();
    assert!(engine.timeline_controlled.contains_key(&list.id));
    assert!(
        engine
            .tick(now + chrono::Duration::seconds(10), None)
            .transitions
            .is_empty()
    );
    assert_eq!(
        engine
            .active
            .get(&PlaybackKey::CueList(list.id))
            .unwrap()
            .current_cue_id,
        Some(list.cues[1].id)
    );
}
