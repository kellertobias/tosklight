use super::*;

fn assert_held_current(runtime: &DynamicRuntime, instance_id: Uuid, expected: f32) {
    let snapshot = runtime.snapshot();
    let instance = snapshot
        .instances
        .iter()
        .find(|instance| instance.id == instance_id)
        .unwrap();
    assert!(!instance.synchronized_hold_values.is_empty());
    for held in &instance.synchronized_hold_values {
        let DynamicHeldPayload::TapeRoot { tape_root } = held.payload else {
            panic!("paused Current must retain its source graph");
        };
        let expression = DynamicSampleExpression::Retained {
            tape: Arc::clone(instance.expression_tape.as_ref().unwrap()),
            root: tape_root,
        };
        let mut values = Vec::new();
        expression.visit_legacy_contributions(|_, value, _| values.push(value));
        assert!(!values.is_empty());
        assert!(
            values.iter().all(|value| (*value - expected).abs() < 1e-6),
            "held Current must remain local to its branch: {values:?}"
        );
    }
}

#[test]
fn journal_pause_retains_branch_current_and_failed_sampling_keeps_accepted_controls() {
    let target = FixtureId::new();
    let owner = controller(2101, 1, false);
    let mut current = lane();
    for point in &mut current.legacy_mut().unwrap().keyframes.points {
        point.source = ScalarSource::Current;
    }
    let mut definition = definition(current);
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    definition.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![target],
    };
    let mut live = installed(&definition);
    let id = live
        .start(start_request(
            definition.id,
            owner.clone(),
            target,
            0,
            false,
        ))
        .unwrap();
    let mut pending = live.fork_for_pending_preview();
    let transports = [DynamicSpeedTransport {
        effective_bpm: 60.,
        phase_origin_millis: 0,
        phase_reference_millis: 100,
        beat_phase: 0.1,
        phase_advancing: true,
    }; 5];
    live.sample_all_addressed(100, 10, &transports, &Sources { current: 0.2 }, None);
    pending.sample_all_addressed(100, 10, &transports, &Sources { current: 0.8 }, None);
    let live_anchor = live.committed_sample_boundary();
    let pending_anchor = pending.committed_sample_boundary();
    assert!(live_anchor.is_some());
    assert!(pending_anchor.is_some());
    assert_ne!(live_anchor, pending_anchor);

    let mut journal = journal();
    let mut cursor = journal.cursor();
    let mut scratch = DynamicOutputFrameScratch::default();
    journal
        .execute(
            &mut live,
            &mut scratch,
            vec![timed(150, DynamicControl::GlobalPause(true))],
        )
        .unwrap();
    let batch = journal.read(cursor).unwrap();
    assert_eq!(
        batch.preceding_samples().collect::<Vec<_>>(),
        vec![live_anchor]
    );
    replay_dynamic_controls(
        &mut pending,
        &mut scratch,
        &mut cursor,
        &batch,
        pending_anchor,
    )
    .unwrap();
    assert_eq!(cursor, batch.to());
    assert_eq!(live.controller(owner.id).unwrap().0, id);
    assert_eq!(pending.controller(owner.id).unwrap().0, id);
    assert!(live.snapshot().global_paused);
    assert!(pending.snapshot().global_paused);
    assert_held_current(&live, id, 0.2);
    assert_held_current(&pending, id, 0.8);

    // Sampling is a later transaction: rejecting its output must not unaccept the controls
    // already replayed above or rewind their consumer cursor.
    let accepted = pending.snapshot();
    let accepted_cursor = cursor;
    let accepted_anchor = pending.committed_sample_boundary();
    let rejected: Result<(), &str> =
        pending.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime.sample_all_addressed(200, 10, &transports, &Sources { current: 0.95 }, None);
            Err("output rejected after sampling")
        });
    assert_eq!(rejected, Err("output rejected after sampling"));
    assert_eq!(pending.snapshot(), accepted);
    assert_eq!(pending.committed_sample_boundary(), accepted_anchor);
    assert_eq!(cursor, accepted_cursor);
    assert!(journal.read(cursor).unwrap().is_empty());
    assert_held_current(&pending, id, 0.8);
    assert_held_current(&live, id, 0.2);

    let accepted_retry: Result<(), &str> =
        pending.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime.sample_all_addressed(200, 10, &transports, &Sources { current: 0.95 }, None);
            Ok(())
        });
    accepted_retry.unwrap();
    assert_ne!(pending.committed_sample_boundary(), accepted_anchor);
    assert_eq!(cursor, accepted_cursor);
    assert!(pending.snapshot().global_paused);
    assert_held_current(&pending, id, 0.8);
}
