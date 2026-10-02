use super::*;
use light_playback::{CueAction, PlaybackRuntimeEffect};

#[test]
fn explicit_off_after_automatic_empty_release_resets_jump_history_for_every_alias() {
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let (fixture, logical) = fixture();
    let mut first = Cue::new(1_u16.into());
    first.changes.push(CueChange::set(
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    ));
    let mut jump = Cue::new(2_u16.into());
    jump.actions.push(CueAction::Jump {
        cue_id: first.id,
        count: 1,
    });
    let mut release = Cue::new(3_u16.into());
    release.changes.push(CueChange {
        fixture_id: logical,
        attribute: AttributeKey::intensity(),
        value: None,
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    });
    let mut cue_list = test_cue_list("Jump then release", Vec::new());
    cue_list.restart_mode = RestartMode::ContinueCurrentCue;
    cue_list.cues = vec![first, jump, release];
    let cue_list_id = cue_list.id;
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![cue_list].into(),
            playbacks: vec![test_playback(1, cue_list_id), test_playback(2, cue_list_id)].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();

    execute_pool(&engine, 1, PoolPlaybackAction::On);
    for expected in [2_u16, 1, 2, 3] {
        execute_pool(&engine, 1, PoolPlaybackAction::Go);
        assert_eq!(
            engine.playback_runtime()[0].current_cue_number,
            Some(expected.into())
        );
    }
    clock.advance_millis(1);
    let _ = engine.prepare_output_frame(RenderOptions::default());
    let released = engine.playback_runtime();
    assert_eq!(released.len(), 1);
    assert!(
        !released[0].enabled,
        "the empty final Cue automatically released"
    );

    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                number: 1,
                page: None,
                action: PlaybackBatchAction::Off,
                exclusion_zones: Arc::default(),
                activation_origin: None,
            }],
            clock.now(),
            0,
        )
        .unwrap();
    assert_eq!(prepared.effect(), PlaybackRuntimeEffect::Durable);
    assert_eq!(prepared.effect_for(1), PlaybackRuntimeEffect::Durable);
    assert_eq!(prepared.effect_for(2), PlaybackRuntimeEffect::Durable);
    assert_eq!(
        prepared.changed_playback_numbers().collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        engine.playback_runtime(),
        released,
        "preparation remains isolated"
    );
    engine.install_prepared_playback_batch(prepared).unwrap();

    // Loading the first Cue avoids the FirstCue restart policy, which would independently clear
    // Jump counters and hide a lost Off effect. The next pass must take the one allowed jump.
    execute_pool(&engine, 1, PoolPlaybackAction::Load(1_u16.into()));
    for expected in [1_u16, 2, 1] {
        execute_pool(&engine, 1, PoolPlaybackAction::Go);
        assert_eq!(
            engine.playback_runtime()[0].current_cue_number,
            Some(expected.into())
        );
    }
}
