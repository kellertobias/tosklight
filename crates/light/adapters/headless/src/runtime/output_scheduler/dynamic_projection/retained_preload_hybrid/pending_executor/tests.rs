//! TL-548 C4: the Pending executor on its dedicated worker thread, over the C1 mixed rig (a
//! calibrated mover, an RGB wash, a shipped Media Server layer and a Focus/Zoom wash). The test
//! plays the Live output tick: it retains one captured input per tick through the real
//! `DynamicSnapshotPublication` and wakes the worker. Profiles are synthetic family test
//! profiles: these tests prove lifecycle, threading and publication, not lamp accuracy.
use super::*;
use crate::runtime::dynamic_snapshot_publication::RetainedFrameCapture;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::family_lanes::tests::Rig;
use crate::runtime::position_readout::PendingPositionReadoutSource;
use light_core::programming::{PositionIntent, ProgrammingOwner};
use light_dynamics::DynamicSpeedTransport;
use light_programmer::{PreloadPlaybackQueueAction, PreloadPlaybackQueueSurface};
use std::cell::Cell;
use std::time::{Duration, Instant};

mod gate;

const PATIENCE: Duration = Duration::from_secs(30);

fn transports() -> [DynamicSpeedTransport; 5] {
    [DynamicSpeedTransport {
        effective_bpm: 120.,
        phase_origin_millis: 0,
        phase_reference_millis: 0,
        beat_phase: 0.,
        phase_advancing: true,
    }; 5]
}

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}

/// The mixed rig armed in Preload with every family pending, the Live publication and runtime
/// the worker reads, and one running executor.
struct Bench {
    rig: Rig,
    dynamics: Arc<Mutex<DynamicRuntime>>,
    publication: Arc<DynamicSnapshotPublication>,
    started: Instant,
    selected: Cell<u64>,
    executor: PendingEpisodeExecutor,
}

impl Bench {
    fn new() -> Self {
        let rig = Rig::new();
        rig.programmers.arm_preload(rig.session, true);
        rig.program_all();
        let dynamics = Arc::new(Mutex::new(
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION),
        ));
        let publication = Arc::new(DynamicSnapshotPublication::new(rig.engine.snapshot()));
        let show = ShowId::new();
        let executor = PendingEpisodeExecutor::spawn(PendingEpisodeSources {
            engine: Arc::clone(&rig.engine),
            dynamics: Arc::clone(&dynamics),
            publication: Arc::clone(&publication),
            origins: Default::default(),
            preload: {
                let programmers = rig.programmers.clone();
                Arc::new(move || {
                    light_application::programming::preload_preview_demand(&programmers)
                })
            },
            show: Arc::new(move || Some(show)),
        })
        .unwrap();
        Self {
            rig,
            dynamics,
            publication,
            started: Instant::now(),
            selected: Cell::new(0),
            executor,
        }
    }

    /// One Live output tick: capture, retain the selected input under Dynamics, wake Pending.
    fn tick(&self) {
        self.rig.clock.advance_millis(40);
        let selected = self.selected.get();
        self.selected.set(selected + 1);
        let frame = RetainedFrameCapture::select(
            self.rig.engine.prepare_output_frame(Default::default()),
            &self.publication,
            self.started + Duration::from_millis(selected * 40),
        );
        if let Some(input) = frame.retained() {
            self.publication.retain_accepted_input(
                &self.dynamics.lock(),
                input,
                &[],
                &transports(),
                37,
                None,
            );
        }
        self.executor.wake();
    }

    fn describe(&self) -> String {
        format!(
            "{:?} / {:?}",
            self.executor.status(),
            self.executor.progress()
        )
    }

    /// The running episode, once it is not `previous`.
    fn running_after(&self, previous: Option<PendingEpisodeIdentity>) -> PendingEpisodeIdentity {
        let fresh = |status: &PendingEpisodeStatus| {
            matches!(status, PendingEpisodeStatus::Running(identity)
                if Some(identity.episode) != previous.map(|old| old.episode))
        };
        assert!(
            self.executor.wait_for(PATIENCE, |_, status| fresh(status)),
            "no new running episode: {}",
            self.describe()
        );
        match self.executor.status() {
            PendingEpisodeStatus::Running(identity) => identity,
            other => panic!("expected a running episode, got {other:?}"),
        }
    }

    fn status_becomes(&self, expected: PendingEpisodeStatus) {
        assert!(
            self.executor
                .wait_for(PATIENCE, |_, status| *status == expected),
            "expected {expected:?}: {}",
            self.describe()
        );
    }

    /// Tick until the worker publishes one more accepted pair, and return it.
    fn publish_next(&self) -> AcceptedPairSummary {
        let published = self.executor.progress().published;
        for _ in 0..40 {
            self.tick();
            if self
                .executor
                .wait_for(Duration::from_millis(500), |progress, _| {
                    progress.published > published
                })
            {
                return self.executor.progress().last.unwrap();
            }
        }
        panic!("no publication: {}", self.describe());
    }

    fn value(
        rows: &[(FixtureId, ProgrammingOwner, AttributeValue)],
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Option<AttributeValue> {
        rows.iter()
            .find(|row| row.0 == target && row.1 == owner)
            .map(|row| row.2.clone())
    }
}

/// Every programmed family appears in both branches of an accepted pair.
fn assert_every_family(summary: &AcceptedPairSummary, rig: &Rig) {
    for (target, owner) in rig.show.owners() {
        for rows in [&summary.before, &summary.after] {
            assert!(
                Bench::value(rows, target, owner).is_some(),
                "{owner:?} of {target:?} evaluated"
            );
        }
    }
}

/// A new episode's first accepted pair: its own identity and ticket 1, a new evaluator token
/// lineage, and Position fitted in both branches without any reused fit memo.
fn assert_fresh_first_pair(
    summary: &AcceptedPairSummary,
    identity: PendingEpisodeIdentity,
    previous: &AcceptedPairSummary,
    baseline: [(u64, u64); 2],
) {
    assert_eq!(summary.identity, identity);
    assert_eq!(
        summary.position_fits, baseline,
        "lane counters restart: exactly a fresh episode's first-pair work"
    );
    assert_eq!(
        summary.ticket,
        PendingAttemptTicket::new(1),
        "tickets restart"
    );
    assert!(!Arc::ptr_eq(&summary.lineage, &previous.lineage));
    for (fits, cache_hits) in summary.position_fits {
        assert!(fits > 0);
        assert_eq!(cache_hits, 0, "no fit memo survives the episode");
    }
}

#[test]
fn the_worker_thread_publishes_accepted_all_family_pairs_and_readers_keep_their_arc() {
    let bench = Bench::new();
    let identity = bench.running_after(None);
    assert!(bench.executor.latest().is_none(), "nothing before a pair");
    assert_eq!(
        identity.programmer,
        bench.rig.programmers.programmer_id().unwrap()
    );

    let first = bench.publish_next();
    assert_eq!(first.identity, identity);
    assert_eq!(first.ticket, PendingAttemptTicket::new(1));
    assert_every_family(&first, &bench.rig);
    let progress = bench.executor.progress();
    let worker = progress.worker.expect("the worker reported its thread");
    assert_ne!(
        worker,
        std::thread::current().id(),
        "evaluated off the caller"
    );
    let held = bench.executor.latest().expect("a published readout");
    assert_eq!(held.identity(), identity);
    assert_eq!(held.ticket(), first.ticket);
    assert_eq!(held.lane().frame.as_ref().unwrap().sequence, 1);

    let show = bench.rig.show;
    bench.rig.fix(
        show.mover,
        ProgrammingOwner::Position,
        angles(10., 20.),
        angles(80., 10.),
    );
    let second = bench.publish_next();
    assert_eq!(second.identity, identity);
    assert!(second.ticket > first.ticket);
    assert!(
        Arc::ptr_eq(&second.lineage, &first.lineage),
        "one evaluator state across the episode"
    );
    assert_eq!(
        Bench::value(&second.after, show.mover, ProgrammingOwner::Position),
        Some(angles(80., 10.))
    );
    let latest = bench.executor.latest().unwrap();
    assert_eq!(latest.ticket(), second.ticket);
    assert_eq!(held.ticket(), first.ticket, "a reader's Arc is immutable");
    assert_eq!(bench.executor.progress().gaps, 0);
}

#[test]
fn a_failed_first_pair_exposes_nothing_and_consumes_its_ticket() {
    let bench = Bench::new();
    let identity = bench.running_after(None);
    bench.executor.fail_next_pair();
    bench.tick();
    assert!(
        bench
            .executor
            .wait_for(PATIENCE, |progress, _| progress.gaps == 1),
        "the failed pair is a gap: {}",
        bench.describe()
    );
    let progress = bench.executor.progress();
    assert_eq!(progress.published, 0);
    assert!(progress.last.is_none());
    assert!(bench.executor.latest().is_none(), "nothing exposed");
    assert!(
        bench
            .executor
            .readouts()
            .capture(identity.programmer, &[bench.rig.show.mover])
            .is_none()
    );
    assert_eq!(
        bench.executor.status(),
        PendingEpisodeStatus::Running(identity)
    );
    let first = bench.publish_next();
    assert_eq!(first.identity, identity);
    assert_eq!(
        first.ticket,
        PendingAttemptTicket::new(2),
        "the gap consumed ticket 1"
    );
}

#[test]
fn go_clear_and_reload_each_recreate_the_episode_fresh_with_every_family() {
    let bench = Bench::new();
    let rig = &bench.rig;
    let mut identity = bench.running_after(None);
    let mut previous = bench.publish_next();
    let baseline = previous.position_fits;
    assert_every_family(&previous, rig);

    // GO: the Preload values become active; a new episode of the same activation.
    assert!(rig.programmers.activate_preload(rig.session));
    bench.executor.trigger(PendingTrigger::Go);
    let go = bench.running_after(Some(identity));
    assert_eq!(go.activation, identity.activation);
    let first = bench.publish_next();
    assert_fresh_first_pair(&first, go, &previous, baseline);
    (identity, previous) = (go, first);

    // Clear: release ends the episode (nothing stays readable); re-arming begins a new one.
    assert!(rig.programmers.release_preload(rig.session));
    bench.executor.trigger(PendingTrigger::Clear);
    bench.status_becomes(PendingEpisodeStatus::Idle);
    assert!(
        bench.executor.latest().is_none(),
        "a cleared episode exposes nothing"
    );
    rig.programmers.arm_preload(rig.session, true);
    rig.program_all();
    let cleared = bench.running_after(Some(identity));
    assert_eq!(cleared.activation, identity.activation);
    let first = bench.publish_next();
    assert_fresh_first_pair(&first, cleared, &previous, baseline);
    assert_every_family(&first, rig);
    (identity, previous) = (cleared, first);

    // Same-show reload: the installation resets retained history; a new activation nonce.
    {
        let _dynamics = bench.dynamics.lock();
        bench.publication.installed(rig.engine.snapshot());
    }
    bench.executor.trigger(PendingTrigger::Reload);
    let reloaded = bench.running_after(Some(identity));
    assert_ne!(reloaded.activation, identity.activation);
    assert_eq!(reloaded.show_id, identity.show_id);
    let first = bench.publish_next();
    assert_fresh_first_pair(&first, reloaded, &previous, baseline);
    assert_every_family(&first, rig);
    assert_eq!(bench.executor.latest().unwrap().identity(), reloaded);
    assert_eq!(bench.executor.progress().episodes, 4);
}

#[test]
fn a_queued_preload_playback_is_a_passive_status_without_live_fallback() {
    let bench = Bench::new();
    let rig = &bench.rig;
    let identity = bench.running_after(None);
    bench.publish_next();
    assert!(rig.programmers.queue_preload_playback_action(
        rig.session,
        1,
        None,
        PreloadPlaybackQueueAction::Go,
        PreloadPlaybackQueueSurface::Physical,
    ));
    bench.status_becomes(PendingEpisodeStatus::QueuePreviewUnavailable);
    assert!(
        bench.executor.latest().is_none(),
        "no stale or Live readout"
    );
    assert!(
        bench
            .executor
            .readouts()
            .capture(identity.programmer, &[rig.show.mover])
            .is_none()
    );
    let published = bench.executor.progress().published;
    for _ in 0..3 {
        bench.tick();
    }
    let steps = bench.executor.progress().steps;
    assert!(
        bench
            .executor
            .wait_for(PATIENCE, |progress, _| progress.steps > steps + 2)
    );
    assert_eq!(bench.executor.progress().published, published);
    assert_eq!(
        bench.executor.status(),
        PendingEpisodeStatus::QueuePreviewUnavailable
    );

    assert_eq!(
        rig.programmers
            .take_preload_playback_actions(rig.session)
            .len(),
        1
    );
    let resumed = bench.running_after(Some(identity));
    assert_eq!(bench.publish_next().identity, resumed);
}

#[test]
fn the_position_readout_provider_adopts_the_after_branch_of_the_published_pair_only() {
    let bench = Bench::new();
    let rig = &bench.rig;
    let mover = rig.show.mover;
    let position = ProgrammingOwner::Position;
    let identity = bench.running_after(None);
    let source = bench.executor.readouts();
    let first = bench.publish_next();
    let captured = source.capture(identity.programmer, &[mover]).unwrap();
    let latest = bench.executor.latest().unwrap();
    assert_eq!(Some(&captured.identity), latest.lane().frame.as_ref());
    assert_eq!(captured.identity.sequence, first.ticket.sequence());
    assert_eq!(captured.scope.show_id, Some(identity.show_id.0));
    assert_eq!(captured.owners.len(), 1);
    assert_eq!(
        captured.owners[0].requested,
        Bench::value(&first.after, mover, position)
    );
    assert!(captured.owners[0].common_angles().is_some());

    // A pending Position Release: the branches diverge and only After is adopted.
    rig.release(mover, position);
    let released = bench.publish_next();
    let before = Bench::value(&released.before, mover, position);
    let after = Bench::value(&released.after, mover, position);
    assert!(before.is_some(), "Before keeps a Position request");
    assert_eq!(after, None, "After releases the pending Position");
    let captured = source.capture(identity.programmer, &[mover]).unwrap();
    assert_eq!(captured.identity.sequence, released.ticket.sequence());
    assert_eq!(captured.owners[0].requested, after, "After only");

    // Another Programmer, or another engine snapshot, reads nothing.
    assert!(
        source
            .capture(light_core::ProgrammerId::new(), &[mover])
            .is_none()
    );
    let mut snapshot = rig.engine.snapshot().as_ref().clone();
    snapshot.revision += 1;
    rig.engine.replace_snapshot(snapshot).unwrap();
    assert!(source.capture(identity.programmer, &[mover]).is_none());
}
