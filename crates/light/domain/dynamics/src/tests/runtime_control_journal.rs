use super::*;
use std::num::NonZeroUsize;
mod fallback;
mod history;
mod owner;
mod reconciliation;
mod recording;

fn installed(definition: &DynamicDefinition) -> DynamicRuntime {
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
}
fn journal() -> DynamicControlJournal {
    DynamicControlJournal::new(NonZeroUsize::new(64).unwrap())
}
fn timed(at_millis: u64, control: DynamicControl) -> TimedDynamicControl {
    TimedDynamicControl { at_millis, control }
}

#[test]
fn ordered_batch_replays_exact_start_and_controls_once_and_rejects_stale_history() {
    let definition = definition(lane());
    let target = FixtureId::new();
    let owner = controller(2001, 1, false);
    let mut live = installed(&definition);
    let mut pending = live.fork_for_pending_preview();
    let mut journal = journal();
    let mut cursor = journal.cursor();
    let mut scratch = DynamicOutputFrameScratch::default();
    let outcomes = journal
        .execute(
            &mut live,
            &mut scratch,
            vec![
                timed(
                    100,
                    DynamicControl::Start(Box::new(start_request(
                        definition.id,
                        owner.clone(),
                        target,
                        100,
                        false,
                    ))),
                ),
                timed(
                    100,
                    DynamicControl::Update {
                        controller: owner.id,
                        size: Some(0.4),
                        speed: Some(2.),
                        phase: Some(90.),
                    },
                ),
                timed(
                    100,
                    DynamicControl::Pause {
                        controller: owner.id,
                        paused: true,
                        resume: None,
                    },
                ),
                timed(
                    110,
                    DynamicControl::Pause {
                        controller: owner.id,
                        paused: false,
                        resume: Some(ActivationPolicy::StartNow),
                    },
                ),
                timed(
                    110,
                    DynamicControl::Rank {
                        controller: owner.id,
                        priority: 9,
                        authored_at: 99,
                    },
                ),
                timed(
                    110,
                    DynamicControl::OutputGate {
                        controller: owner.id,
                        enabled: false,
                        delay: 20,
                        duration: 100,
                    },
                ),
            ],
        )
        .unwrap();
    let id = outcomes[0].instance_id.unwrap();
    let batch = journal.read(cursor).unwrap();
    assert_eq!(batch.len(), 6);
    assert_eq!(
        batch.operation_times().collect::<Vec<_>>(),
        vec![100, 100, 100, 110, 110, 110]
    );
    assert!(batch.prefix(7).is_err());
    let prefix = batch.prefix(3).unwrap();
    replay_dynamic_controls(&mut pending, &mut scratch, &mut cursor, &prefix, None).unwrap();
    assert!(pending.snapshot().instances[0].controllers[0].paused);
    let remainder = journal.read(cursor).unwrap();
    assert_eq!(remainder.len(), 3);
    replay_dynamic_controls(&mut pending, &mut scratch, &mut cursor, &remainder, None).unwrap();
    assert_eq!(pending.controller(owner.id).unwrap().0, id);
    assert_eq!(pending.snapshot(), live.snapshot());
    let before = pending.snapshot();
    assert!(
        replay_dynamic_controls(&mut pending, &mut scratch, &mut cursor, &batch, None).is_err()
    );
    assert_eq!(pending.snapshot(), before);
    let empty = journal.read(cursor).unwrap();
    pending
        .sample(id, 125, 1000, 10, &Sources { current: 0. })
        .unwrap();
    assert!(
        replay_dynamic_controls(&mut pending, &mut scratch, &mut cursor, &empty, None).is_err()
    );
    let anchor = pending.committed_sample_boundary();
    replay_dynamic_controls(&mut pending, &mut scratch, &mut cursor, &empty, anchor).unwrap();
}

#[test]
fn rejected_batch_does_not_publish_controls_or_acknowledge_partial_replay() {
    let definition = definition(lane());
    let owner = controller(2002, 1, false);
    let target = FixtureId::new();
    let mut live = installed(&definition);
    let mut journal = journal();
    let start = journal.cursor();
    let before = live.snapshot();
    let mut scratch = DynamicOutputFrameScratch::default();
    assert!(
        journal
            .execute(
                &mut live,
                &mut scratch,
                vec![
                    timed(
                        0,
                        DynamicControl::Start(Box::new(start_request(
                            definition.id,
                            owner.clone(),
                            target,
                            0,
                            false
                        )))
                    ),
                    timed(
                        0,
                        DynamicControl::Update {
                            controller: owner.id,
                            size: Some(f32::NAN),
                            speed: None,
                            phase: None
                        }
                    ),
                ]
            )
            .is_err()
    );
    assert_eq!(live.snapshot(), before);
    assert_eq!(journal.cursor(), start);
    assert!(journal.read(start).unwrap().is_empty());

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
    journal
        .execute(
            &mut live,
            &mut scratch,
            vec![
                timed(1, DynamicControl::GlobalPause(true)),
                timed(
                    1,
                    DynamicControl::Update {
                        controller: owner.id,
                        size: Some(0.5),
                        speed: None,
                        phase: None,
                    },
                ),
            ],
        )
        .unwrap();
    pending.off_controller(id, owner.id, 0, 0, 0).unwrap();
    let before = pending.snapshot();
    let mut cursor = start;
    let batch = journal.read(start).unwrap();
    assert!(
        replay_dynamic_controls(&mut pending, &mut scratch, &mut cursor, &batch, None).is_err()
    );
    assert_eq!(cursor, start);
    assert_eq!(
        pending.snapshot(),
        before,
        "global pause preceding the failed update must roll back"
    );
}

#[test]
fn replayed_off_uses_pending_completion_and_timing_instead_of_live_deletion() {
    let mut definition = definition(lane());
    definition.run_mode = DynamicRunMode::OneShot;
    let owner = controller(2003, 1, false);
    let mut live = installed(&definition);
    let id = live
        .start(start_request(
            definition.id,
            owner.clone(),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let mut pending = live.fork_for_pending_preview();
    live.sample(id, 1100, 1000, 10, &Sources { current: 0. })
        .unwrap();
    assert!(live.snapshot().instances[0].completed);
    let mut journal = journal();
    let mut cursor = journal.cursor();
    let mut scratch = DynamicOutputFrameScratch::default();
    journal
        .execute(
            &mut live,
            &mut scratch,
            vec![timed(
                1200,
                DynamicControl::Off {
                    controller: owner.id,
                    delay: 100,
                    duration: 500,
                },
            )],
        )
        .unwrap();
    assert_eq!(live.instance_count(), 0);
    let batch = journal.read(cursor).unwrap();
    replay_dynamic_controls(&mut pending, &mut scratch, &mut cursor, &batch, None).unwrap();
    assert_eq!(
        pending.instance_count(),
        1,
        "pending evaluates its own release rather than copying Live removal"
    );
    let snapshot = pending.snapshot();
    let release = &snapshot.instances[0].controller_transitions[0];
    assert_eq!(release.release_started_at_millis, Some(1200));
    assert_eq!(release.release_delay_millis, 100);
    assert_eq!(release.release_duration_millis, 500);
}

#[test]
fn unchanged_controls_and_automatic_sampling_do_not_grow_authored_history() {
    let definition = definition(lane());
    let owner = controller(2004, 1, false);
    let mut live = installed(&definition);
    let id = live
        .start(start_request(
            definition.id,
            owner.clone(),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let mut journal = journal();
    let cursor = journal.cursor();
    let mut scratch = DynamicOutputFrameScratch::default();
    let outcomes = journal
        .execute(
            &mut live,
            &mut scratch,
            vec![
                timed(1, DynamicControl::GlobalPause(false)),
                timed(
                    1,
                    DynamicControl::Update {
                        controller: owner.id,
                        size: Some(1.),
                        speed: None,
                        phase: None,
                    },
                ),
                timed(
                    1,
                    DynamicControl::CancelRelease {
                        controller: owner.id,
                    },
                ),
                timed(
                    1,
                    DynamicControl::Lanes {
                        controller: owner.id,
                        selection: DynamicLaneSelection::All,
                    },
                ),
                timed(
                    1,
                    DynamicControl::OutputGate {
                        controller: owner.id,
                        enabled: true,
                        delay: 0,
                        duration: 0,
                    },
                ),
            ],
        )
        .unwrap();
    assert!(outcomes.iter().all(|outcome| !outcome.changed));
    assert_eq!(journal.cursor(), cursor);
    live.sample(id, 100, 1000, 10, &Sources { current: 0. })
        .unwrap();
    assert_eq!(journal.cursor(), cursor);
}
