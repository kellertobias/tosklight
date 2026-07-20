use super::*;
use crate::playback::combine_release_effect;
use light_playback::PlaybackRuntimeEffect;

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

    assert_changed_effect(transition.outcome, PlaybackRuntimeEffect::Durable);
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

    assert_changed_effect(transition.outcome, PlaybackRuntimeEffect::Durable);
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

#[test]
fn pool_mutations_report_durable_transient_and_exact_noop_effects() {
    let engine = playback_engine();
    assert_pool_effect(
        &engine,
        PoolPlaybackAction::On,
        PlaybackRuntimeEffect::Durable,
    );
    assert_pool_effect(&engine, PoolPlaybackAction::On, PlaybackRuntimeEffect::None);
    assert_pool_effect(
        &engine,
        PoolPlaybackAction::SetMaster(0.5),
        PlaybackRuntimeEffect::Durable,
    );
    assert_pool_effect(
        &engine,
        PoolPlaybackAction::SetMaster(0.5),
        PlaybackRuntimeEffect::None,
    );
    assert_pool_effect(
        &engine,
        PoolPlaybackAction::Load(1.0),
        PlaybackRuntimeEffect::Durable,
    );
    assert_pool_effect(
        &engine,
        PoolPlaybackAction::Load(1.0),
        PlaybackRuntimeEffect::None,
    );
    assert_pool_effect(
        &engine,
        PoolPlaybackAction::Pause,
        PlaybackRuntimeEffect::Durable,
    );
    assert_pool_effect(
        &engine,
        PoolPlaybackAction::Pause,
        PlaybackRuntimeEffect::None,
    );
    assert_pool_effect(
        &engine,
        PoolPlaybackAction::SetTempButton(true),
        PlaybackRuntimeEffect::Transient,
    );
    assert_pool_effect(
        &engine,
        PoolPlaybackAction::SetTempButton(true),
        PlaybackRuntimeEffect::None,
    );
}

#[test]
fn cue_list_pause_retains_active_projection_with_exact_effect() {
    let engine = playback_engine();
    let cue_list_id = engine.snapshot().cue_lists[0].id;
    execute_cue_list(&engine, cue_list_id, CueListPlaybackAction::Go);

    let first = engine
        .execute_playback(EnginePlaybackCommand::CueList {
            id: cue_list_id,
            action: CueListPlaybackAction::Pause,
        })
        .unwrap();
    assert_active_list_effect(first, PlaybackRuntimeEffect::Durable);

    let repeated = engine
        .execute_playback(EnginePlaybackCommand::CueList {
            id: cue_list_id,
            action: CueListPlaybackAction::Pause,
        })
        .unwrap();
    assert_active_list_effect(repeated, PlaybackRuntimeEffect::None);
}

#[test]
fn exclusion_release_upgrades_only_the_aggregate_effect() {
    let outcome = combine_release_effect(
        EnginePlaybackOutcome::Changed(EnginePlaybackEffect::from_addressed(
            PlaybackRuntimeEffect::Transient,
        )),
        &[2],
    )
    .unwrap();

    assert_changed_effects(
        outcome,
        PlaybackRuntimeEffect::Transient,
        PlaybackRuntimeEffect::Durable,
    );
}

#[test]
fn peer_only_auto_off_keeps_the_addressed_effect_unchanged() {
    let (engine, _) = peer_only_auto_off_engine();

    let outcome = engine
        .execute_playback(EnginePlaybackCommand::Pool {
            number: 2,
            action: PoolPlaybackAction::On,
        })
        .unwrap();

    assert_changed_effects(
        outcome,
        PlaybackRuntimeEffect::None,
        PlaybackRuntimeEffect::Durable,
    );
    assert_eq!(enabled_playback_numbers(&engine), vec![2]);
}

#[test]
fn peer_only_auto_off_batch_does_not_retime_the_addressed_playback() {
    let (engine, clock) = peer_only_auto_off_engine();
    let before = playback_runtime(&engine, 2);
    let started_at = clock.now() + chrono::Duration::seconds(1);
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                number: 2,
                action: PlaybackBatchAction::On,
                exclusion_zones: Vec::new(),
            }],
            started_at,
            1_000,
        )
        .unwrap();

    assert_eq!(
        prepared.outcomes()[0].addressed_effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(
        prepared.outcomes()[0].effect,
        PlaybackRuntimeEffect::Durable
    );
    engine.install_prepared_playback_batch(prepared).unwrap();
    let after = playback_runtime(&engine, 2);
    assert_eq!(after.activated_at, before.activated_at);
    assert_eq!(after.transition_fade_fallback_millis, None);
    assert!(after.master_transition.is_none());
    assert_eq!(enabled_playback_numbers(&engine), vec![2]);
}

#[test]
fn exclusion_does_not_clean_up_an_inactive_loaded_peer() {
    let engine = playback_engine_with_numbers(&[1, 2]);
    execute_pool(&engine, 2, PoolPlaybackAction::Load(1.0));
    let loaded_id = playback_runtime(&engine, 2).loaded_cue_id;
    assert!(loaded_id.is_some());

    let transition = engine
        .execute_pool_playback_with_exclusions(1, PoolPlaybackAction::On, &[vec![1, 2]])
        .unwrap();

    assert!(transition.released_playbacks.is_empty());
    let peer = playback_runtime(&engine, 2);
    assert!(!peer.enabled);
    assert_eq!(peer.loaded_cue_id, loaded_id);
}

#[test]
fn prepared_batch_aggregates_none_transient_and_durable_effects() {
    let engine = playback_engine();
    let no_change = prepare_batch(&engine, PlaybackBatchAction::Off);
    assert_eq!(no_change.effect(), PlaybackRuntimeEffect::None);
    assert_eq!(
        no_change.outcomes()[0].addressed_effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(no_change.outcomes()[0].effect, PlaybackRuntimeEffect::None);

    let transient = prepare_batch(&engine, PlaybackBatchAction::SetTempButton(true));
    assert_eq!(transient.effect(), PlaybackRuntimeEffect::Transient);
    assert_eq!(
        transient.outcomes()[0].addressed_effect,
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        transient.outcomes()[0].effect,
        PlaybackRuntimeEffect::Transient
    );

    let durable = prepare_batch(&engine, PlaybackBatchAction::On);
    assert_eq!(durable.effect(), PlaybackRuntimeEffect::Durable);
    assert_eq!(
        durable.outcomes()[0].addressed_effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(durable.outcomes()[0].effect, PlaybackRuntimeEffect::Durable);
}

#[test]
fn repeated_on_batch_does_not_retrigger_timing_or_signal_persistence() {
    let engine = playback_engine();
    execute_pool(&engine, 1, PoolPlaybackAction::On);
    let before = playback_runtime(&engine, 1);
    let started_at = before.activated_at + chrono::Duration::seconds(1);
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                number: 1,
                action: PlaybackBatchAction::On,
                exclusion_zones: Vec::new(),
            }],
            started_at,
            1_000,
        )
        .unwrap();

    assert_eq!(prepared.effect(), PlaybackRuntimeEffect::None);
    assert_eq!(prepared.outcomes()[0].effect, PlaybackRuntimeEffect::None);
    engine.install_prepared_playback_batch(prepared).unwrap();
    let after = playback_runtime(&engine, 1);
    assert_eq!(after.activated_at, before.activated_at);
    assert_eq!(after.transition_fade_fallback_millis, None);
    assert!(after.master_transition.is_none());
}

fn assert_pool_effect(
    engine: &Engine,
    action: PoolPlaybackAction,
    expected: PlaybackRuntimeEffect,
) {
    let outcome = engine
        .execute_playback(EnginePlaybackCommand::Pool { number: 1, action })
        .unwrap();
    assert_changed_effect(outcome, expected);
}

fn assert_changed_effect(outcome: EnginePlaybackOutcome, expected: PlaybackRuntimeEffect) {
    assert_changed_effects(outcome, expected, expected);
}

fn assert_changed_effects(
    outcome: EnginePlaybackOutcome,
    addressed: PlaybackRuntimeEffect,
    aggregate: PlaybackRuntimeEffect,
) {
    let EnginePlaybackOutcome::Changed(effect) = outcome else {
        panic!("expected a changed Playback outcome");
    };
    assert_eq!(effect.addressed, addressed);
    assert_eq!(effect.aggregate, aggregate);
}

fn assert_active_list_effect(outcome: EnginePlaybackOutcome, expected: PlaybackRuntimeEffect) {
    let EnginePlaybackOutcome::ActiveList { active, effect } = outcome else {
        panic!("expected an active-list Playback outcome");
    };
    assert_eq!(active.len(), 1);
    assert_eq!(effect, expected);
}

fn playback_runtime(engine: &Engine, number: u16) -> light_playback::ActivePlayback {
    engine
        .playback_runtime()
        .into_iter()
        .find(|playback| playback.playback_number == Some(number))
        .unwrap()
}

fn prepare_batch(engine: &Engine, action: PlaybackBatchAction) -> PreparedPlaybackBatch {
    engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                number: 1,
                action,
                exclusion_zones: Vec::new(),
            }],
            chrono::Utc::now(),
            0,
        )
        .unwrap()
}

fn peer_only_auto_off_engine() -> (Engine, Arc<ManualClock>) {
    let started = chrono::Utc::now();
    let clock = Arc::new(ManualClock::new(started));
    let fixture = FixtureId::new();
    let low = test_cue_list(
        "Auto-off source",
        vec![CueChange::set(
            fixture,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.2),
        )],
    );
    let high = test_cue_list(
        "Covering Playback",
        vec![CueChange::set(
            fixture,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.8),
        )],
    );
    let mut low_definition = test_playback(1, low.id);
    low_definition.auto_off = false;
    let mut high_definition = test_playback(2, high.id);
    high_definition.auto_off = false;
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![low, high],
            playbacks: vec![low_definition, high_definition],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::On);
    clock.advance_millis(1);
    execute_pool(&engine, 2, PoolPlaybackAction::On);
    let mut replacement = (*engine.snapshot()).clone();
    replacement.revision += 1;
    replacement.playbacks[0].auto_off = true;
    engine.replace_snapshot(replacement).unwrap();
    (engine, clock)
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
