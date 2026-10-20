use super::*;
use light_playback::{
    PlaybackActivationOrigin, PlaybackActivationSurface, PlaybackAddress, PlaybackExclusionScope,
    PlaybackRuntimeEffect, PlaybackSoloRegion,
};
use uuid::Uuid;

#[test]
fn prepared_batch_dependency_rejection_leaves_playback_unchanged_and_does_not_install() {
    let engine = playback_engine();
    let prepared = prepare_batch(&engine, PlaybackBatchAction::On);
    let installed = std::cell::Cell::new(false);
    let result = engine.install_prepared_playback_batch_with(
        prepared,
        |candidate| {
            assert!(
                candidate
                    .runtime_status_for_playback(playback_address(1, 1))
                    .is_some_and(|status| status.playback.enabled)
            );
            Err::<(), _>("dependent projection rejected".to_owned())
        },
        |()| installed.set(true),
    );
    assert_eq!(result.unwrap_err(), "dependent projection rejected");
    assert!(!installed.get());
    assert!(engine.playback_runtime().is_empty());

    let prepared = prepare_batch(&engine, PlaybackBatchAction::On);
    let result = engine
        .install_prepared_playback_batch_with(
            prepared,
            |candidate| {
                Ok(candidate
                    .runtime_status_for_playback(playback_address(1, 1))
                    .unwrap())
            },
            |projection| {
                installed.set(true);
                projection
            },
        )
        .unwrap();
    assert!(installed.get());
    assert_eq!(
        serde_json::to_value(
            engine
                .playback_runtime_status_at(playback_address(1, 1))
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(result).unwrap(),
    );
}

#[test]
fn stale_prepared_batch_skips_dependent_preparation_and_installation() {
    let engine = playback_engine();
    let prepared = prepare_batch(&engine, PlaybackBatchAction::On);
    let mut replacement = (*engine.snapshot()).clone();
    replacement.revision += 1;
    engine.replace_snapshot(replacement).unwrap();
    let result = engine.install_prepared_playback_batch_with(
        prepared,
        |_| -> Result<(), String> { panic!("stale generation cannot prepare dependencies") },
        |()| panic!("stale generation cannot install dependencies"),
    );
    assert!(result.is_err());
    assert!(engine.playback_runtime().is_empty());
}

#[test]
fn prepared_batch_is_isolated_until_one_typed_install() {
    let engine = playback_engine();
    let started_at = chrono::Utc::now();
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                address: playback_address(1, 1),
                control_source: test_control_source(),
                action: PlaybackBatchAction::On,
                activation_origin: None,
            }],
            started_at,
            0,
        )
        .unwrap();

    assert!(engine.playback_runtime().is_empty());
    assert_eq!(prepared.outcomes()[0].address, playback_address(1, 1));
    engine.install_prepared_playback_batch(prepared).unwrap();
    assert!(
        engine
            .playback_runtime()
            .iter()
            .any(
                |playback| playback.playback_address == Some(playback_address(1, 1))
                    && playback.enabled
            )
    );
}

#[test]
fn prepared_batch_cannot_overwrite_a_new_compiled_generation() {
    let engine = playback_engine();
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                address: playback_address(1, 1),
                control_source: test_control_source(),
                action: PlaybackBatchAction::Go,
                activation_origin: None,
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
fn atomic_playback_activation_releases_only_active_peers_in_number_order() {
    let engine = playback_engine_with_numbers(&[1, 2, 3, 4]);
    execute_address(&engine, playback_address(1, 2), PlaybackAction::On);
    execute_address(&engine, playback_address(1, 3), PlaybackAction::On);

    let transition = execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::On,
        &[
            vec![
                playback_address(1, 3),
                playback_address(1, 1),
                playback_address(1, 2),
            ],
            vec![
                playback_address(1, 4),
                playback_address(1, 3),
                playback_address(1, 1),
            ],
        ],
        None,
    )
    .unwrap();

    assert_changed_effect(transition, PlaybackRuntimeEffect::Durable);
    assert!(!playback_runtime(&engine, 2).enabled && !playback_runtime(&engine, 3).enabled);
    assert_eq!(enabled_playback_numbers(&engine), vec![1]);
}

#[test]
fn atomic_fader_activation_obeys_the_same_exclusion_boundary() {
    let engine = playback_engine_with_numbers(&[1, 2]);
    let mut snapshot = (*engine.snapshot()).clone();
    snapshot.revision += 1;
    Arc::make_mut(&mut snapshot.cue_lists)[0].auto_off_at_zero = true;
    engine.replace_snapshot(snapshot).unwrap();
    execute_address(&engine, playback_address(1, 2), PlaybackAction::On);
    // Observe and pick up the original full master before the zero-to-live gesture.
    execute_address(
        &engine,
        playback_address(1, 1),
        PlaybackAction::SetMaster(1.0),
    );

    execute_address(
        &engine,
        playback_address(1, 1),
        PlaybackAction::SetMaster(0.0),
    );
    assert!(
        !playback_runtime(&engine, 1).enabled,
        "zero must deactivate before the tested permanent start"
    );
    assert!(playback_runtime(&engine, 2).enabled);
    let transition = execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::SetMaster(0.5),
        &[vec![playback_address(1, 1), playback_address(1, 2)]],
        None,
    )
    .unwrap();

    assert_changed_effect(transition, PlaybackRuntimeEffect::Durable);
    assert!(!playback_runtime(&engine, 2).enabled);
    assert_eq!(enabled_playback_numbers(&engine), vec![1]);
}

#[test]
fn atomic_playback_noop_and_deactivation_do_not_release_peers() {
    let engine = playback_engine_with_numbers(&[1, 2]);
    execute_address(&engine, playback_address(1, 1), PlaybackAction::On);
    execute_address(&engine, playback_address(1, 2), PlaybackAction::On);

    let noop = execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::On,
        &[vec![playback_address(1, 1), playback_address(1, 2)]],
        None,
    )
    .unwrap();
    assert_changed_effect(noop, PlaybackRuntimeEffect::None);
    assert_eq!(enabled_playback_numbers(&engine), vec![1, 2]);

    let deactivation = execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::Off,
        &[vec![playback_address(1, 1), playback_address(1, 2)]],
        None,
    )
    .unwrap();
    assert_changed_effect(deactivation, PlaybackRuntimeEffect::Durable);
    assert_eq!(enabled_playback_numbers(&engine), vec![2]);
}

#[test]
fn activation_provenance_is_atomic_stable_on_noop_and_cleared_on_release() {
    let engine = playback_engine_with_numbers(&[1, 2]);
    let first_desk = Uuid::new_v4();
    let second_desk = Uuid::new_v4();
    let first = activation_origin(first_desk, PlaybackActivationSurface::Virtual);
    let second = activation_origin(second_desk, PlaybackActivationSurface::Physical);

    execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::On,
        &[],
        Some(first),
    )
    .unwrap();
    let recorded = playback_runtime(&engine, 1).activation.unwrap();
    assert_eq!(recorded.desk_id, Some(first_desk));
    assert_eq!(recorded.surface, PlaybackActivationSurface::Virtual);
    assert_eq!(recorded.ordinal, 1);

    let mut replacement = (*engine.snapshot()).clone();
    replacement.revision += 1;
    Arc::make_mut(&mut replacement.cue_lists)[0]
        .cues
        .push(Cue::new(2_u16.into()));
    engine.replace_snapshot(replacement).unwrap();
    execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::Go,
        &[],
        Some(second),
    )
    .unwrap();
    let advanced = playback_runtime(&engine, 1);
    assert_eq!(advanced.current_cue_number, Some(2_u16.into()));
    assert_eq!(advanced.activation.as_ref(), Some(&recorded));

    let repeated = execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::On,
        &[],
        Some(second),
    )
    .unwrap();
    assert_changed_effect(repeated, PlaybackRuntimeEffect::None);
    assert_eq!(
        playback_runtime(&engine, 1).activation,
        Some(recorded.clone())
    );

    execute_with_regions(
        &engine,
        playback_address(1, 2),
        PlaybackAction::On,
        &[],
        Some(second),
    )
    .unwrap();
    let transition = execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::Off,
        &[vec![playback_address(1, 1), playback_address(1, 2)]],
        Some(first),
    )
    .unwrap();
    assert_changed_effect(transition, PlaybackRuntimeEffect::Durable);
    assert!(playback_runtime(&engine, 2).enabled);
    assert!(playback_runtime(&engine, 1).activation.is_none());

    execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::On,
        &[vec![playback_address(1, 1), playback_address(1, 2)]],
        Some(first),
    )
    .unwrap();
    assert!(playback_runtime(&engine, 2).activation.is_none());
    assert_eq!(playback_runtime(&engine, 1).activation.unwrap().ordinal, 3);
}

#[test]
fn prepared_batch_assigns_distinct_activation_ordinals_in_queue_order() {
    let engine = playback_engine_with_numbers(&[1, 2]);
    let at = chrono::Utc::now();
    let desk_id = Uuid::new_v4();
    let origin = PlaybackActivationOrigin {
        at,
        desk_id: Some(desk_id),
        surface: PlaybackActivationSurface::Physical,
        exclusion_scope: PlaybackExclusionScope::OriginatingDesk,
    };
    let prepared = engine
        .prepare_playback_batch(
            &[
                PlaybackBatchCommand {
                    address: playback_address(1, 2),
                    control_source: test_control_source(),
                    action: PlaybackBatchAction::On,
                    activation_origin: Some(origin),
                },
                PlaybackBatchCommand {
                    address: playback_address(1, 1),
                    control_source: test_control_source(),
                    action: PlaybackBatchAction::On,
                    activation_origin: Some(origin),
                },
            ],
            at,
            0,
        )
        .unwrap();

    engine.install_prepared_playback_batch(prepared).unwrap();
    assert_eq!(playback_runtime(&engine, 2).activation.unwrap().ordinal, 1);
    assert_eq!(playback_runtime(&engine, 1).activation.unwrap().ordinal, 2);
}

#[test]
fn timed_preload_release_retains_activation_provenance_while_fade_is_active() {
    let engine = playback_engine();
    let origin = activation_origin(Uuid::new_v4(), PlaybackActivationSurface::Virtual);
    execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::On,
        &[],
        Some(origin),
    )
    .unwrap();
    let activation = playback_runtime(&engine, 1).activation.unwrap();
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                address: playback_address(1, 1),
                control_source: test_control_source(),
                action: PlaybackBatchAction::Off,
                activation_origin: Some(origin),
            }],
            origin.at + chrono::Duration::seconds(1),
            1_000,
        )
        .unwrap();

    engine.install_prepared_playback_batch(prepared).unwrap();
    let releasing = playback_runtime(&engine, 1);
    assert!(releasing.enabled);
    assert!(
        releasing
            .master_transition
            .as_ref()
            .is_some_and(|transition| transition.release_after)
    );
    assert_eq!(releasing.activation, Some(activation));
}

#[test]
fn toggle_off_reports_a_real_transition_without_releasing_peers() {
    let engine = playback_engine_with_numbers(&[1, 2]);
    execute_address(&engine, playback_address(1, 1), PlaybackAction::On);
    execute_address(&engine, playback_address(1, 2), PlaybackAction::On);

    let transition = execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::Toggle,
        &[vec![playback_address(1, 1), playback_address(1, 2)]],
        None,
    )
    .unwrap();

    assert_changed_effect(transition, PlaybackRuntimeEffect::Durable);
    assert_eq!(enabled_playback_numbers(&engine), vec![2]);
}

#[test]
fn failed_playback_activation_does_not_release_peers() {
    let engine = playback_engine_with_numbers(&[1, 2]);
    execute_address(&engine, playback_address(1, 2), PlaybackAction::On);

    let result = execute_with_regions(
        &engine,
        playback_address(1, 99),
        PlaybackAction::On,
        &[vec![playback_address(1, 99), playback_address(1, 2)]],
        None,
    );

    assert!(result.is_err());
    assert_eq!(enabled_playback_numbers(&engine), vec![2]);
}

#[test]
fn release_address_batch_reports_only_actual_changes_in_number_order() {
    let engine = playback_engine_with_numbers(&[1, 2, 3]);
    execute_address(&engine, playback_address(1, 2), PlaybackAction::On);

    let outcome = engine
        .execute_playback(EnginePlaybackCommand::ReleaseAddressBatch(vec![
            playback_address(1, 3),
            playback_address(1, 2),
            playback_address(1, 2),
            playback_address(1, 99),
        ]))
        .unwrap();

    assert!(matches!(
        outcome,
        EnginePlaybackOutcome::ChangedPlaybacks(ref changed) if changed == &vec![playback_address(1,2)]
    ));
    assert!(enabled_playback_numbers(&engine).is_empty());
}

#[test]
fn prepared_batch_reports_only_peers_it_actually_releases() {
    let engine = playback_engine_with_numbers(&[1, 2, 3]);
    execute_address(&engine, playback_address(1, 2), PlaybackAction::On);

    install_regions(
        &engine,
        &[vec![
            playback_address(1, 3),
            playback_address(1, 2),
            playback_address(1, 1),
        ]],
    );
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                address: playback_address(1, 1),
                control_source: test_control_source(),
                action: PlaybackBatchAction::On,
                activation_origin: None,
            }],
            chrono::Utc::now(),
            0,
        )
        .unwrap();
    assert_eq!(
        prepared.outcomes()[0].released_playbacks,
        vec![playback_address(1, 2)]
    );
    assert_eq!(enabled_playback_numbers(&engine), vec![2]);
    engine.install_prepared_playback_batch(prepared).unwrap();
    assert_eq!(enabled_playback_numbers(&engine), vec![1]);
}

#[test]
fn pool_mutations_report_durable_transient_and_exact_noop_effects() {
    let engine = playback_engine();
    assert_playback_effect(&engine, PlaybackAction::On, PlaybackRuntimeEffect::Durable);
    assert_playback_effect(&engine, PlaybackAction::On, PlaybackRuntimeEffect::None);
    assert_playback_effect(
        &engine,
        PlaybackAction::SetMaster(0.5),
        PlaybackRuntimeEffect::Transient,
    );
    assert_playback_effect(
        &engine,
        PlaybackAction::SetMaster(0.5),
        PlaybackRuntimeEffect::None,
    );
    assert_playback_effect(
        &engine,
        PlaybackAction::SetMasterWithExplicitActivation(0.5),
        PlaybackRuntimeEffect::Durable,
    );
    assert_playback_effect(
        &engine,
        PlaybackAction::SetMasterWithExplicitActivation(0.5),
        PlaybackRuntimeEffect::None,
    );
    assert_playback_effect(
        &engine,
        PlaybackAction::Load(1_u16.into()),
        PlaybackRuntimeEffect::Durable,
    );
    assert_playback_effect(
        &engine,
        PlaybackAction::Load(1_u16.into()),
        PlaybackRuntimeEffect::None,
    );
    assert_playback_effect(
        &engine,
        PlaybackAction::Pause,
        PlaybackRuntimeEffect::Durable,
    );
    assert_playback_effect(&engine, PlaybackAction::Pause, PlaybackRuntimeEffect::None);
    assert_playback_effect(
        &engine,
        PlaybackAction::SetTempButton(true),
        PlaybackRuntimeEffect::Transient,
    );
    assert_playback_effect(
        &engine,
        PlaybackAction::SetTempButton(true),
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
    let outcome = EnginePlaybackOutcome::Changed(
        EnginePlaybackEffect::from_addressed(PlaybackRuntimeEffect::Transient)
            .with_related(PlaybackRuntimeEffect::Durable),
    );

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
        .execute_playback(EnginePlaybackCommand::Playback {
            address: playback_address(1, 2),
            control_source: test_control_source(),
            activation_origin: None,
            action: PlaybackAction::On,
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
                address: playback_address(1, 2),
                control_source: test_control_source(),
                action: PlaybackBatchAction::On,
                activation_origin: None,
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
    execute_address(
        &engine,
        playback_address(1, 2),
        PlaybackAction::Load(1_u16.into()),
    );
    let loaded_id = playback_runtime(&engine, 2).loaded_cue_id;
    assert!(loaded_id.is_some());

    let transition = execute_with_regions(
        &engine,
        playback_address(1, 1),
        PlaybackAction::On,
        &[vec![playback_address(1, 1), playback_address(1, 2)]],
        None,
    )
    .unwrap();

    let peer = playback_runtime(&engine, 2);
    assert_changed_effect(transition, PlaybackRuntimeEffect::Durable);
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
fn prepared_batch_classifies_the_exact_final_state_after_transient_cancellation() {
    let engine = playback_engine();
    let prepared = engine
        .prepare_playback_batch(
            &[
                PlaybackBatchCommand {
                    address: playback_address(1, 1),
                    control_source: test_control_source(),
                    action: PlaybackBatchAction::SetTempButton(true),
                    activation_origin: None,
                },
                PlaybackBatchCommand {
                    address: playback_address(1, 1),
                    control_source: test_control_source(),
                    action: PlaybackBatchAction::SetTempButton(false),
                    activation_origin: None,
                },
            ],
            chrono::Utc::now(),
            0,
        )
        .unwrap();

    assert_eq!(prepared.outcomes().len(), 2);
    assert!(
        prepared
            .outcomes()
            .iter()
            .all(|outcome| outcome.effect == PlaybackRuntimeEffect::Transient)
    );
    assert_eq!(prepared.effect(), PlaybackRuntimeEffect::None);
    assert_eq!(
        prepared.effect_for(playback_address(1, 1)),
        PlaybackRuntimeEffect::None
    );
    assert_eq!(prepared.changed_playback_addresses().count(), 0);
    engine.install_prepared_playback_batch(prepared).unwrap();
    assert!(engine.playback_runtime().is_empty());
}

#[test]
fn repeated_on_batch_does_not_retrigger_timing_or_signal_persistence() {
    let engine = playback_engine();
    execute_address(&engine, playback_address(1, 1), PlaybackAction::On);
    let before = playback_runtime(&engine, 1);
    let started_at = before.activated_at + chrono::Duration::seconds(1);
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                address: playback_address(1, 1),
                control_source: test_control_source(),
                action: PlaybackBatchAction::On,
                activation_origin: None,
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

#[test]
fn solo_releases_only_exact_address_members_across_pages() {
    let engine = virtual_playback_engine();
    let activated = PlaybackAddress::new(3, 401).unwrap();
    let same_page_peer = PlaybackAddress::new(3, 402).unwrap();
    let other_page_first = PlaybackAddress::new(4, 401).unwrap();
    let other_page_peer = PlaybackAddress::new(4, 402).unwrap();
    let origin = activation_origin(Uuid::new_v4(), PlaybackActivationSurface::Osc);

    for address in [same_page_peer, other_page_first, other_page_peer] {
        engine
            .execute_playback(EnginePlaybackCommand::Playback {
                address,
                action: PlaybackAction::On,
                control_source: test_control_source(),
                activation_origin: None,
            })
            .unwrap();
    }
    execute_address(&engine, playback_address(1, 1), PlaybackAction::On);

    install_regions(&engine, &[vec![activated, same_page_peer]]);
    engine
        .execute_playback(EnginePlaybackCommand::Playback {
            address: activated,
            action: PlaybackAction::On,
            control_source: test_control_source(),
            activation_origin: Some(origin.clone()),
        })
        .unwrap();

    let runtime = engine.playback_runtime();
    let by_identity = |identity| {
        runtime
            .iter()
            .find(|playback| playback.playback_address == Some(identity))
            .unwrap()
    };
    assert!(by_identity(activated).enabled);
    assert!(!by_identity(same_page_peer).enabled);
    assert!(by_identity(other_page_first).enabled);
    assert!(by_identity(other_page_peer).enabled);
    assert!(runtime.iter().any(|playback| playback.playback_address
        == Some(playback_address(1, 1))
        && playback.playback_address == Some(playback_address(1, 1))
        && playback.enabled));
    let recorded = by_identity(activated).activation.as_ref().unwrap();
    assert_eq!(recorded.at, origin.at);
    assert_eq!(recorded.desk_id, origin.desk_id);
    assert_eq!(recorded.surface, origin.surface);
    assert_eq!(recorded.exclusion_scope, origin.exclusion_scope);
}

#[test]
fn prepared_address_batch_keeps_page_identity_and_solo_atomic() {
    let engine = virtual_playback_engine();
    let page_one_peer = PlaybackAddress::new(3, 402).unwrap();
    let page_two_peer = PlaybackAddress::new(4, 402).unwrap();
    for address in [page_one_peer, page_two_peer] {
        engine
            .execute_playback(EnginePlaybackCommand::Playback {
                address,
                action: PlaybackAction::On,
                control_source: test_control_source(),
                activation_origin: None,
            })
            .unwrap();
    }
    install_regions(
        &engine,
        &[
            vec![playback_address(3, 401), playback_address(3, 402)],
            vec![playback_address(4, 401), playback_address(4, 402)],
        ],
    );
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                address: playback_address(3, 401),
                control_source: test_control_source(),
                action: PlaybackBatchAction::On,
                activation_origin: None,
            }],
            chrono::Utc::now(),
            0,
        )
        .unwrap();

    assert_eq!(prepared.outcomes()[0].address, playback_address(3, 401));
    assert_eq!(
        prepared.outcomes()[0].released_playbacks,
        vec![playback_address(3, 402)]
    );
    assert!(
        engine
            .playback_runtime()
            .iter()
            .any(|playback| playback.playback_address == Some(page_one_peer) && playback.enabled)
    );
    engine.install_prepared_playback_batch(prepared).unwrap();
    let runtime = engine.playback_runtime();
    assert!(
        runtime
            .iter()
            .any(|playback| playback.playback_address == Some(page_one_peer) && !playback.enabled)
    );
    assert!(
        runtime
            .iter()
            .any(|playback| playback.playback_address == Some(page_two_peer) && playback.enabled)
    );
}

fn assert_playback_effect(
    engine: &Engine,
    action: PlaybackAction,
    expected: PlaybackRuntimeEffect,
) {
    let outcome = engine
        .execute_playback(EnginePlaybackCommand::Playback {
            address: playback_address(1, 1),
            action,
            control_source: test_control_source(),
            activation_origin: None,
        })
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
        .find(|playback| playback.playback_address == Some(playback_address(1, number)))
        .unwrap()
}

fn activation_origin(
    desk_id: Uuid,
    surface: PlaybackActivationSurface,
) -> PlaybackActivationOrigin {
    PlaybackActivationOrigin {
        at: chrono::Utc::now(),
        desk_id: Some(desk_id),
        surface,
        exclusion_scope: PlaybackExclusionScope::OriginatingDesk,
    }
}

fn prepare_batch(engine: &Engine, action: PlaybackBatchAction) -> PreparedPlaybackBatch {
    engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                address: playback_address(1, 1),
                control_source: test_control_source(),
                action,
                activation_origin: None,
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
            cue_lists: vec![low, high].into(),
            playback_pages: vec![playback_page(
                1,
                [(1, low_definition), (2, high_definition)],
            )]
            .into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    execute_address(&engine, playback_address(1, 1), PlaybackAction::On);
    clock.advance_millis(1);
    execute_address(&engine, playback_address(1, 2), PlaybackAction::On);
    let mut replacement = (*engine.snapshot()).clone();
    replacement.revision += 1;
    Arc::make_mut(&mut replacement.playback_pages)[0]
        .assignments
        .get_mut(&1)
        .unwrap()
        .auto_off = true;
    engine.replace_snapshot(replacement).unwrap();
    (engine, clock)
}

fn playback_engine() -> Engine {
    playback_engine_with_numbers(&[1])
}

fn playback_engine_with_numbers(numbers: &[u16]) -> Engine {
    let assignments = numbers
        .iter()
        .map(|number| {
            let cue_list = test_cue_list(&format!("Typed boundary {number}"), Vec::new());
            let playback = test_playback(*number, cue_list.id);
            (cue_list, playback)
        })
        .collect::<Vec<_>>();
    let (cue_lists, playbacks): (Vec<_>, Vec<_>) = assignments.into_iter().unzip();
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: cue_lists.into(),
            playback_pages: vec![playback_page(1, numbers.iter().copied().zip(playbacks))].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine
}

fn virtual_playback_engine() -> Engine {
    let mut cue_lists = Vec::new();
    let mut pages = Vec::new();
    for page_number in [3, 4] {
        let assignments = [401, 402].map(|position| {
            let cue_list = test_cue_list(&format!("Playback {page_number}.{position}"), Vec::new());
            let definition = test_playback(position, cue_list.id);
            cue_lists.push(cue_list);
            (position, definition)
        });
        pages.push(playback_page(page_number, assignments));
    }
    let list = test_cue_list("Main Playback", Vec::new());
    pages.push(playback_page(1, [(1, test_playback(1, list.id))]));
    cue_lists.push(list);
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: cue_lists.into(),
            playback_pages: pages.into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine
}

fn enabled_playback_numbers(engine: &Engine) -> Vec<u16> {
    engine
        .playback_runtime()
        .into_iter()
        .filter(|playback| playback.enabled)
        .filter_map(|playback| playback.playback_address.map(|address| address.position()))
        .collect()
}

fn install_regions(engine: &Engine, members: &[Vec<PlaybackAddress>]) {
    let mut snapshot = (*engine.snapshot()).clone();
    snapshot.revision += 1;
    snapshot.playback_solo_regions = members
        .iter()
        .enumerate()
        .map(|(index, members)| PlaybackSoloRegion {
            color: None,
            id: Uuid::new_v4(),
            name: format!("Solo {index}"),
            members: members.clone(),
        })
        .collect::<Vec<_>>()
        .into();
    engine.replace_snapshot(snapshot).unwrap();
}

fn execute_with_regions(
    engine: &Engine,
    address: PlaybackAddress,
    action: PlaybackAction,
    members: &[Vec<PlaybackAddress>],
    origin: Option<PlaybackActivationOrigin>,
) -> Result<EnginePlaybackOutcome, String> {
    install_regions(engine, members);
    engine.execute_playback(EnginePlaybackCommand::Playback {
        address,
        action,
        activation_origin: origin,
        control_source: test_control_source(),
    })
}
