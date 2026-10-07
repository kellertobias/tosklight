use super::*;

fn saved_dynamic(engine: &Engine) -> light_playback::ActiveDynamicPlayback {
    let mut row = engine.active_dynamic_playbacks()[0].clone();
    row.paused = true;
    row.size = 0.4;
    row.master = 0.5;
    row.fader_value = 0.3;
    row.master_transition = Some(light_playback::PlaybackMasterTransition {
        from: 0.5,
        to: 0.0,
        started_at: row.activated_at,
        duration_millis: 100,
        release_after: true,
    });
    row
}

#[test]
fn destination_restore_captures_exact_owners_before_install_for_same_and_changed_topology() {
    for changed in [false, true] {
        let (engine, clock, _, _) = setup();
        execute_pool(&engine, 1, PoolPlaybackAction::SetTempButton(true));
        let saved = saved_dynamic(&engine);
        let paused_since = Some(clock.now());
        engine.reserve_playback_source_occurrence_watermark(200);
        let live_generation = engine.generation.load_full();
        let live_playback = live_generation.playback_arc();
        let live_cues = engine.active_cue_dynamic_values();
        let live_dynamics = engine.active_dynamic_playbacks();
        let live_runtime = engine.playback_runtime();
        let mut candidate = (*engine.snapshot()).clone();
        candidate.revision += 1;
        let mut expected = saved.clone();
        if changed {
            Arc::make_mut(&mut candidate.playbacks)[1].number = 7;
            expected.playback_number = 7;
        }
        // The saved release transition is overdue. Finalization must not tick it.
        clock.advance_millis(500);
        let finalized = engine
            .finalize_snapshot_playback_restoring_dynamics(
                engine.prepare_snapshot(candidate).unwrap(),
                &[saved],
                paused_since,
            )
            .unwrap();
        assert!(finalized.cue_dynamic_values().is_empty());
        assert_eq!(finalized.dynamic_playbacks(), &[expected.clone()]);
        assert!(finalized.playback_dynamics_paused());
        assert!(Arc::ptr_eq(
            &live_generation,
            &engine.generation.load_full()
        ));
        assert_eq!(engine.playback_runtime(), live_runtime);
        assert_eq!(engine.active_cue_dynamic_values(), live_cues);
        assert_eq!(engine.active_dynamic_playbacks(), live_dynamics);
        assert!(!engine.playback_dynamics().paused);
        assert_eq!(engine.playback_source_occurrence_watermark(), 200);

        clock.advance_millis(500);
        engine.install_finalized_snapshot(finalized);
        let installed = engine.generation.load_full().playback_arc();
        assert!(!Arc::ptr_eq(&live_playback, &installed));
        assert!(Arc::ptr_eq(
            &live_playback.read().clock(),
            &installed.read().clock(),
        ));
        assert!(engine.playback_runtime().is_empty());
        assert!(engine.active_cue_dynamic_values().is_empty());
        assert_eq!(engine.active_dynamic_playbacks(), vec![expected]);
        assert_eq!(installed.read().dynamics_paused_since(), paused_since);
        assert_eq!(engine.playback_source_occurrence_watermark(), 200);
    }
}

#[test]
fn destination_restore_from_old_shared_preparation_uses_only_saved_rows_and_current_watermark() {
    let (engine, _, _, _) = setup();
    execute_pool(&engine, 1, PoolPlaybackAction::SetTempButton(true));
    let saved = saved_dynamic(&engine);
    let mut candidate = (*engine.snapshot()).clone();
    candidate.revision = 3;
    let prepared = engine.prepare_snapshot(candidate).unwrap();
    let mut intervening = (*engine.snapshot()).clone();
    intervening.revision = 2;
    Arc::make_mut(&mut intervening.cue_lists)[0].name = "Intervening compilation".into();
    engine.replace_snapshot(intervening).unwrap();
    execute_pool(&engine, 2, PoolPlaybackAction::Off);
    engine.reserve_playback_source_occurrence_watermark(500);
    engine
        .execute_playback(EnginePlaybackCommand::SetDynamicsPaused(true))
        .unwrap();
    let finalized = engine
        .finalize_snapshot_playback_restoring_dynamics(prepared, &[saved.clone()], None)
        .unwrap();
    assert!(finalized.cue_dynamic_values().is_empty());
    assert_eq!(finalized.dynamic_playbacks(), &[saved.clone()]);
    assert!(!finalized.playback_dynamics_paused());
    assert!(engine.playback_dynamics().paused);
    assert_eq!(engine.playback_source_occurrence_watermark(), 500);
    engine.install_finalized_snapshot(finalized);
    assert!(engine.playback_runtime().is_empty());
    assert_eq!(engine.active_dynamic_playbacks(), vec![saved]);
    assert!(!engine.playback_dynamics().paused);
    assert_eq!(engine.playback_source_occurrence_watermark(), 500);
}

#[test]
fn abandoned_destination_and_ordinary_release_are_independent() {
    let (engine, clock, _, _) = setup();
    let saved = saved_dynamic(&engine);
    let generation = engine.generation.load_full();
    let runtime = engine.playback_runtime();
    let dynamics = engine.active_dynamic_playbacks();
    let watermark = engine.playback_source_occurrence_watermark();
    let snapshot = (*engine.snapshot()).clone();
    let abandoned = engine
        .finalize_snapshot_playback_restoring_dynamics(
            engine.prepare_snapshot(snapshot.clone()).unwrap(),
            &[saved],
            Some(clock.now()),
        )
        .unwrap();
    assert_eq!(abandoned.dynamic_playbacks().len(), 1);
    drop(abandoned);
    assert!(Arc::ptr_eq(&generation, &engine.generation.load_full()));
    assert_eq!(engine.playback_runtime(), runtime);
    assert_eq!(engine.active_dynamic_playbacks(), dynamics);
    assert_eq!(engine.playback_source_occurrence_watermark(), watermark);
    assert!(!engine.playback_dynamics().paused);
    let released = finalize(&engine, snapshot, false);
    assert!(released.dynamic_playbacks().is_empty());
    assert!(!released.playback_dynamics_paused());
}

#[test]
fn destination_restore_keeps_existing_passive_target_and_flash_normalization_rules() {
    for missing in [false, true] {
        let (engine, _, _, _) = setup();
        let mut saved = saved_dynamic(&engine);
        saved.flash = true;
        saved.flash_restore_off = true;
        saved.fader_pickup_required = true;
        saved.fader_pickup_target = Some(0.8);
        saved.size = 1.5;
        let mut candidate = (*engine.snapshot()).clone();
        if missing {
            candidate.playbacks = Arc::default();
        }
        let finalized = engine
            .finalize_snapshot_playback_restoring_dynamics(
                engine.prepare_snapshot(candidate).unwrap(),
                &[saved.clone()],
                None,
            )
            .unwrap();
        if missing {
            assert!(finalized.dynamic_playbacks().is_empty());
        } else {
            saved.enabled = false;
            saved.flash = false;
            saved.flash_restore_off = false;
            saved.fader_pickup_required = false;
            saved.fader_pickup_target = None;
            saved.size = 1.0;
            assert_eq!(finalized.dynamic_playbacks(), &[saved]);
        }
    }
}

#[test]
fn malformed_destination_rows_fail_before_publication_or_live_changes() {
    let (engine, _, _, _) = setup();
    let good = saved_dynamic(&engine);
    let mut invalid = Vec::new();
    let mut row = good.clone();
    row.playback_number = 0;
    row.playback_identity = None;
    invalid.push(row);
    let mut row = good.clone();
    row.fader_value = f32::NAN;
    invalid.push(row);
    let mut row = good.clone();
    row.local_speed_multiplier.denominator = 0;
    invalid.push(row);
    let mut row = good.clone();
    row.master_transition.as_mut().unwrap().to = f32::INFINITY;
    invalid.push(row);

    let generation = engine.generation.load_full();
    let runtime = engine.playback_runtime();
    let dynamics = engine.active_dynamic_playbacks();
    let watermark = engine.playback_source_occurrence_watermark();
    for bad in invalid {
        let result = engine.finalize_snapshot_playback_restoring_dynamics(
            engine
                .prepare_snapshot((*engine.snapshot()).clone())
                .unwrap(),
            &[good.clone(), bad],
            None,
        );
        assert!(matches!(result, Err(EngineError::Invalid(_))));
        assert!(Arc::ptr_eq(&generation, &engine.generation.load_full()));
        assert_eq!(engine.playback_runtime(), runtime);
        assert_eq!(engine.active_dynamic_playbacks(), dynamics);
        assert_eq!(engine.playback_source_occurrence_watermark(), watermark);
    }
}

#[test]
fn destination_commit_reserves_later_live_and_checkpoint_watermarks_without_recapturing_owners() {
    for checkpoint_watermark in [100, 900, u64::MAX] {
        let (engine, clock, _, _) = setup();
        let saved = saved_dynamic(&engine);
        engine.reserve_playback_source_occurrence_watermark(200);
        let mut finalized = engine
            .finalize_snapshot_playback_restoring_dynamics(
                engine
                    .prepare_snapshot((*engine.snapshot()).clone())
                    .unwrap(),
                &[saved.clone()],
                Some(clock.now()),
            )
            .unwrap();
        // The old show emits a real temporary Cue occurrence while activation awaits commit.
        execute_pool(&engine, 1, PoolPlaybackAction::SetTempButton(true));
        let live_watermark = engine.playback_source_occurrence_watermark();
        assert!(live_watermark > 200);
        assert_eq!(engine.active_cue_dynamic_values().len(), 2);
        let required = live_watermark.max(checkpoint_watermark);
        finalized.reserve_playback_source_occurrence_watermark(required);
        finalized.reserve_playback_source_occurrence_watermark(100);
        assert_eq!(
            engine.playback_source_occurrence_watermark(),
            live_watermark
        );
        assert_eq!(finalized.dynamic_playbacks(), &[saved.clone()]);
        assert!(finalized.cue_dynamic_values().is_empty());
        assert!(finalized.playback_dynamics_paused());
        engine.install_finalized_snapshot(finalized);
        assert_eq!(engine.playback_source_occurrence_watermark(), required);
        assert_eq!(engine.active_dynamic_playbacks(), vec![saved]);
    }
}
