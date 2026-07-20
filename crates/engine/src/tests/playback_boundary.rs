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
                exclusion_zones: Vec::new(),
            }],
            started_at,
            0,
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
                exclusion_zones: Vec::new(),
            }],
            chrono::Utc::now(),
            0,
        )
        .unwrap();
    let mut replacement = (*engine.snapshot()).clone();
    replacement.revision += 1;
    engine.replace_snapshot(replacement).unwrap();

    assert!(engine.install_prepared_playback_batch(prepared).is_err());
    assert!(engine.playback_runtime().is_empty());
}

#[test]
fn atomic_pool_activation_releases_only_active_peers_in_number_order() {
    let engine = playback_engine_with_numbers(&[1, 2, 3, 4]);
    execute_pool(&engine, 2, PoolPlaybackAction::On);
    execute_pool(&engine, 3, PoolPlaybackAction::On);

    let transition = engine
        .execute_pool_playback_with_exclusions(
            1,
            PoolPlaybackAction::On,
            &[vec![3, 1, 2], vec![4, 3, 1]],
        )
        .unwrap();

    assert!(matches!(
        transition.outcome,
        EnginePlaybackOutcome::Changed(true)
    ));
    assert_eq!(transition.released_playbacks, vec![2, 3]);
    assert_eq!(enabled_playback_numbers(&engine), vec![1]);
}

#[test]
fn atomic_fader_activation_obeys_the_same_exclusion_boundary() {
    let engine = playback_engine_with_numbers(&[1, 2]);
    execute_pool(&engine, 2, PoolPlaybackAction::On);

    let transition = engine
        .execute_pool_playback_with_exclusions(1, PoolPlaybackAction::SetMaster(0.5), &[vec![1, 2]])
        .unwrap();

    assert_eq!(transition.released_playbacks, vec![2]);
    assert_eq!(enabled_playback_numbers(&engine), vec![1]);
}

#[test]
fn atomic_pool_noop_and_deactivation_do_not_release_peers() {
    let engine = playback_engine_with_numbers(&[1, 2]);
    execute_pool(&engine, 1, PoolPlaybackAction::On);
    execute_pool(&engine, 2, PoolPlaybackAction::On);

    let noop = engine
        .execute_pool_playback_with_exclusions(1, PoolPlaybackAction::On, &[vec![1, 2]])
        .unwrap();
    assert!(noop.released_playbacks.is_empty());
    assert_eq!(enabled_playback_numbers(&engine), vec![1, 2]);

    let deactivation = engine
        .execute_pool_playback_with_exclusions(1, PoolPlaybackAction::Off, &[vec![1, 2]])
        .unwrap();
    assert!(deactivation.released_playbacks.is_empty());
    assert_eq!(enabled_playback_numbers(&engine), vec![2]);
}

#[test]
fn toggle_off_reports_a_real_transition_without_releasing_peers() {
    let engine = playback_engine_with_numbers(&[1, 2]);
    execute_pool(&engine, 1, PoolPlaybackAction::On);
    execute_pool(&engine, 2, PoolPlaybackAction::On);

    let transition = engine
        .execute_pool_playback_with_exclusions(1, PoolPlaybackAction::Toggle, &[vec![1, 2]])
        .unwrap();

    assert!(matches!(
        transition.outcome,
        EnginePlaybackOutcome::Changed(true)
    ));
    assert!(transition.released_playbacks.is_empty());
    assert_eq!(enabled_playback_numbers(&engine), vec![2]);
}

#[test]
fn failed_pool_activation_does_not_release_peers() {
    let engine = playback_engine_with_numbers(&[1, 2]);
    execute_pool(&engine, 2, PoolPlaybackAction::On);

    let result =
        engine.execute_pool_playback_with_exclusions(99, PoolPlaybackAction::On, &[vec![99, 2]]);

    assert!(result.is_err());
    assert_eq!(enabled_playback_numbers(&engine), vec![2]);
}

#[test]
fn release_pool_batch_reports_only_actual_changes_in_number_order() {
    let engine = playback_engine_with_numbers(&[1, 2, 3]);
    execute_pool(&engine, 2, PoolPlaybackAction::On);

    let outcome = engine
        .execute_playback(EnginePlaybackCommand::ReleasePoolBatch(vec![3, 2, 2, 99]))
        .unwrap();

    assert!(matches!(
        outcome,
        EnginePlaybackOutcome::ChangedPlaybacks(ref changed) if changed == &vec![2]
    ));
    assert!(enabled_playback_numbers(&engine).is_empty());
}

#[test]
fn prepared_batch_reports_only_peers_it_actually_releases() {
    let engine = playback_engine_with_numbers(&[1, 2, 3]);
    execute_pool(&engine, 2, PoolPlaybackAction::On);

    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                number: 1,
                action: PlaybackBatchAction::On,
                exclusion_zones: vec![vec![3, 2, 1, 2]],
            }],
            chrono::Utc::now(),
            0,
        )
        .unwrap();

    assert_eq!(prepared.outcomes()[0].released_playbacks, vec![2]);
    assert_eq!(enabled_playback_numbers(&engine), vec![2]);
    engine.install_prepared_playback_batch(prepared).unwrap();
    assert_eq!(enabled_playback_numbers(&engine), vec![1]);
}

fn playback_engine() -> Engine {
    playback_engine_with_numbers(&[1])
}

fn playback_engine_with_numbers(numbers: &[u16]) -> Engine {
    let cue_list = test_cue_list("Typed boundary", Vec::new());
    let playbacks = numbers
        .iter()
        .map(|number| test_playback(*number, cue_list.id))
        .collect();
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list],
            playbacks,
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine
}

fn enabled_playback_numbers(engine: &Engine) -> Vec<u16> {
    engine
        .playback_runtime()
        .into_iter()
        .filter(|playback| playback.enabled)
        .filter_map(|playback| playback.playback_number)
        .collect()
}
