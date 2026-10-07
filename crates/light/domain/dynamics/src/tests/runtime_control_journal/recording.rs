use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn running() -> (DynamicRuntime, Uuid) {
    let definition = definition(lane());
    let mut runtime = installed(&definition);
    let id = runtime
        .start(start_request(
            definition.id,
            controller(2201, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    (runtime, id)
}

#[test]
fn beginning_recording_again_preserves_history_and_original_capacity() {
    let (mut runtime, _) = running();
    let start = runtime.begin_control_recording(NonZeroUsize::new(1).unwrap());
    assert_eq!(
        runtime.begin_control_recording(NonZeroUsize::new(64).unwrap()),
        start
    );
    runtime
        .apply_recorded_control(timed(1, DynamicControl::GlobalPause(true)))
        .unwrap();
    let paused = runtime.control_cursor().unwrap();
    assert_eq!(
        runtime.begin_control_recording(NonZeroUsize::new(64).unwrap()),
        paused
    );
    assert_eq!(runtime.controls_since(start).unwrap().unwrap().len(), 1);
    runtime
        .apply_recorded_control(timed(2, DynamicControl::GlobalPause(false)))
        .unwrap();
    assert!(matches!(
        runtime.controls_since(start),
        Some(Err(DynamicControlLogError::HistoryLost))
    ));
    assert_eq!(runtime.controls_since(paused).unwrap().unwrap().len(), 1);
}

#[test]
fn controls_inside_an_output_transaction_publish_only_on_commit_without_nesting() {
    let (mut runtime, _) = running();
    let start = runtime.begin_control_recording(NonZeroUsize::new(8).unwrap());
    let mut scratch = DynamicOutputFrameScratch::default();
    let result: Result<(), DynamicRuntimeError> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime.apply_recorded_control(timed(1, DynamicControl::GlobalPause(true)))?;
            assert!(runtime.snapshot().global_paused);
            assert_eq!(runtime.control_cursor(), Some(start));
            assert!(runtime.controls_since(start).unwrap().unwrap().is_empty());
            runtime.apply_recorded_control(timed(2, DynamicControl::GlobalPause(false)))?;
            assert_eq!(runtime.control_cursor(), Some(start));
            Ok(())
        });
    result.unwrap();
    let batch = runtime.controls_since(start).unwrap().unwrap();
    assert_eq!(batch.operation_times().collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(runtime.control_cursor(), Some(batch.to()));
    assert!(!runtime.snapshot().global_paused);
}

#[test]
fn errors_and_unwinds_discard_provisional_controls_before_scratch_reuse() {
    let (mut runtime, _) = running();
    let start = runtime.begin_control_recording(NonZeroUsize::new(8).unwrap());
    let before = runtime.snapshot();
    let mut scratch = DynamicOutputFrameScratch::default();
    let rejected: Result<(), &str> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime
                .apply_recorded_control(timed(1, DynamicControl::GlobalPause(true)))
                .unwrap();
            Err("frame rejected")
        });
    assert_eq!(rejected, Err("frame rejected"));
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(runtime.control_cursor(), Some(start));
    assert!(runtime.controls_since(start).unwrap().unwrap().is_empty());

    let unwound = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), &str> = runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime
                .apply_recorded_control(timed(2, DynamicControl::GlobalPause(true)))
                .unwrap();
            panic!("synthetic output unwind");
        });
    }));
    assert!(unwound.is_err());
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(runtime.control_cursor(), Some(start));
    assert!(runtime.controls_since(start).unwrap().unwrap().is_empty());

    let accepted: Result<(), DynamicRuntimeError> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime.apply_recorded_control(timed(3, DynamicControl::GlobalPause(true)))?;
            Ok(())
        });
    accepted.unwrap();
    assert_eq!(
        runtime
            .controls_since(start)
            .unwrap()
            .unwrap()
            .operation_times()
            .collect::<Vec<_>>(),
        vec![3]
    );
}

#[test]
fn preview_forks_disable_recording_and_cold_candidates_have_independent_logs() {
    let (mut live, _) = running();
    let start = live.begin_control_recording(NonZeroUsize::new(8).unwrap());
    live.apply_recorded_control(timed(1, DynamicControl::GlobalPause(true)))
        .unwrap();
    let live_cursor = live.control_cursor().unwrap();
    for mut preview in [live.fork_for_preview(), live.fork_for_pending_preview()] {
        assert_eq!(preview.control_cursor(), None);
        assert!(preview.controls_since(start).is_none());
        preview
            .apply_recorded_control(timed(2, DynamicControl::GlobalPause(false)))
            .unwrap();
        assert_eq!(preview.control_cursor(), None);
    }
    let mut candidate = live.fork_for_cold_install();
    assert_eq!(candidate.control_cursor(), Some(live_cursor));
    candidate
        .apply_recorded_control(timed(2, DynamicControl::GlobalPause(false)))
        .unwrap();
    assert_eq!(candidate.controls_since(start).unwrap().unwrap().len(), 2);
    candidate
        .acknowledge_controls(candidate.control_cursor().unwrap())
        .unwrap()
        .unwrap();
    assert!(matches!(
        candidate.controls_since(start),
        Some(Err(DynamicControlLogError::HistoryLost))
    ));
    assert_eq!(live.control_cursor(), Some(live_cursor));
    assert_eq!(live.controls_since(start).unwrap().unwrap().len(), 1);
    assert!(live.snapshot().global_paused);
}

#[test]
fn cold_fork_of_provisional_state_has_no_authoritative_history_lineage() {
    let (mut runtime, id) = running();
    runtime
        .sample(id, 10, 1000, 10, &Sources { current: 0.2 })
        .unwrap();
    let anchor = runtime.committed_sample_boundary();
    assert!(anchor.is_some());
    let start = runtime.begin_control_recording(NonZeroUsize::new(8).unwrap());
    let mut scratch = DynamicOutputFrameScratch::default();
    let rejected: Result<(), &str> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime
                .apply_recorded_control(timed(20, DynamicControl::GlobalPause(true)))
                .unwrap();
            let candidate = runtime.fork_for_cold_install();
            assert!(candidate.snapshot().global_paused);
            assert_eq!(candidate.control_cursor(), None);
            assert_eq!(candidate.committed_sample_boundary(), None);
            Err("do not publish candidate")
        });
    assert!(rejected.is_err());
    assert_eq!(runtime.control_cursor(), Some(start));
    assert_eq!(runtime.committed_sample_boundary(), anchor);
    assert!(!runtime.snapshot().global_paused);
}

#[test]
fn failed_restore_keeps_epoch_and_successful_restore_invalidates_old_cursors() {
    let (mut runtime, _) = running();
    let start = runtime.begin_control_recording(NonZeroUsize::new(8).unwrap());
    runtime
        .apply_recorded_control(timed(1, DynamicControl::GlobalPause(true)))
        .unwrap();
    let cursor = runtime.control_cursor().unwrap();
    let before = runtime.snapshot();
    let mut invalid = before.clone();
    invalid.instances[0].targets.clear();
    assert!(runtime.restore_snapshot(invalid).is_err());
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(runtime.control_cursor(), Some(cursor));
    assert_eq!(runtime.controls_since(start).unwrap().unwrap().len(), 1);

    runtime.restore_snapshot(before).unwrap();
    assert_ne!(runtime.control_cursor(), Some(cursor));
    assert!(matches!(
        runtime.controls_since(cursor),
        Some(Err(DynamicControlLogError::WrongEpoch))
    ));
    let restored = runtime.control_cursor().unwrap();
    assert!(
        runtime
            .controls_since(restored)
            .unwrap()
            .unwrap()
            .is_empty()
    );
    runtime
        .apply_recorded_control(timed(2, DynamicControl::GlobalPause(false)))
        .unwrap();
    assert_eq!(runtime.controls_since(restored).unwrap().unwrap().len(), 1);
}

#[test]
fn recorded_control_after_sampling_rejects_and_rolls_back_the_whole_frame() {
    let (mut runtime, id) = running();
    let start = runtime.begin_control_recording(NonZeroUsize::new(8).unwrap());
    let before = runtime.snapshot();
    let mut scratch = DynamicOutputFrameScratch::default();
    let result: Result<(), DynamicRuntimeError> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime.apply_recorded_control(timed(1, DynamicControl::GlobalPause(true)))?;
            runtime.sample(id, 10, 1000, 10, &Sources { current: 0.2 })?;
            runtime.apply_recorded_control(timed(11, DynamicControl::GlobalPause(false)))?;
            Ok(())
        });
    assert!(matches!(result, Err(DynamicRuntimeError::InvalidReplay(_))));
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(runtime.committed_sample_boundary(), None);
    assert_eq!(runtime.control_cursor(), Some(start));
    assert!(runtime.controls_since(start).unwrap().unwrap().is_empty());
}

#[test]
fn external_journal_and_replay_cannot_create_a_second_stream_on_recording_runtime() {
    let (mut runtime, _) = running();
    let recording_cursor = runtime.begin_control_recording(NonZeroUsize::new(8).unwrap());
    let before = runtime.snapshot();
    let mut external = journal();
    let mut cursor = external.cursor();
    let mut scratch = DynamicOutputFrameScratch::default();
    assert!(matches!(
        external.execute(
            &mut runtime,
            &mut scratch,
            vec![timed(1, DynamicControl::GlobalPause(true))]
        ),
        Err(DynamicRuntimeError::InvalidReplay(_))
    ));
    assert!(external.read(cursor).unwrap().is_empty());

    let mut unrecorded = runtime.fork_for_preview();
    external
        .execute(
            &mut unrecorded,
            &mut scratch,
            vec![timed(1, DynamicControl::GlobalPause(true))],
        )
        .unwrap();
    let batch = external.read(cursor).unwrap();
    let initial_cursor = cursor;
    assert!(matches!(
        replay_dynamic_controls(&mut runtime, &mut scratch, &mut cursor, &batch, None),
        Err(DynamicRuntimeError::InvalidReplay(_))
    ));
    assert_eq!(cursor, initial_cursor);
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(runtime.control_cursor(), Some(recording_cursor));
    assert!(
        runtime
            .controls_since(recording_cursor)
            .unwrap()
            .unwrap()
            .is_empty()
    );
}
