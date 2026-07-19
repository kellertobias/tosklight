use super::*;

#[test]
fn prepared_batch_is_isolated_until_one_typed_install() {
    let engine = playback_engine();
    let started_at = chrono::Utc::now();
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                number: 1,
                action: PlaybackBatchAction::On,
            }],
            started_at,
            0,
            &[],
        )
        .unwrap();

    assert!(engine.playback_runtime().is_empty());
    assert_eq!(prepared.outcomes()[0].number, 1);
    engine.install_prepared_playback_batch(prepared).unwrap();
    assert!(
        engine
            .playback_runtime()
            .iter()
            .any(|playback| playback.playback_number == Some(1) && playback.enabled)
    );
}

#[test]
fn prepared_batch_cannot_overwrite_a_new_compiled_generation() {
    let engine = playback_engine();
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                number: 1,
                action: PlaybackBatchAction::Go,
            }],
            chrono::Utc::now(),
            0,
            &[],
        )
        .unwrap();
    let mut replacement = (*engine.snapshot()).clone();
    replacement.revision += 1;
    engine.replace_snapshot(replacement).unwrap();

    assert!(engine.install_prepared_playback_batch(prepared).is_err());
    assert!(engine.playback_runtime().is_empty());
}

fn playback_engine() -> Engine {
    let cue_list = test_cue_list("Typed boundary", Vec::new());
    let playback = test_playback(1, cue_list.id);
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list],
            playbacks: vec![playback],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine
}
