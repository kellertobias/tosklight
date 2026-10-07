//! TL-548 C4: the Pending (Preload) episode executor.
//!
//! One dedicated worker thread owns the desk's single `FamilyPendingEpisode` (all-family
//! lanes, one retained evaluator state, one paired history, one gate lease). Lanes are `!Sync`,
//! so they never leave that thread; only `Send + Sync` handles cross into it:
//!
//! - [`PendingEpisodeSources`]: the desk `Arc<Engine>`, the Live `DynamicRuntime` mutex, the
//!   Live `DynamicSnapshotPublication` (retained inputs and cold generations), the Live source
//!   catalogue, the Programmer's Preload preview demand and the active show identity.
//! - Commands over an `mpsc` channel: `Wake` (output tick or retained input capture),
//!   lifecycle triggers and `Shutdown`. The channel is also the wake signal; without a command
//!   the worker polls at the retained-input sample interval.
//!
//! **Per step** the worker first reconciles the lifecycle, then evaluates:
//! 1. Lifecycle. Pending runs while Preload is engaged (armed, or Preload values active). A
//!    trigger (GO, clear/release, same-show reload/show activation), a lost history or a
//!    changed Programmer/show/activation mints a new [`PendingEpisodeIdentity`] and recreates
//!    the episode from new seeds. Disengaged Preload, a gate that turned off, or teardown ends
//!    it. A non-empty Preload Playback queue is passive: the episode ends and the status is
//!    [`PendingEpisodeStatus::QueuePreviewUnavailable`], with no Live fallback.
//! 2. Seeding (only on (re)creation), under the Live Dynamics lock: `begin_retained_history`,
//!    `input_capture_cursor`, the Live origins catalogue, `fork_for_pending_preview` twice and
//!    `committed_sample_boundary`. The seeds are moved into the episode on the worker.
//! 3. Evaluation: read `input_captures_since`, per-branch `cold_generations_since` (through the
//!    last input's cold cursor) and, under the Dynamics lock, per-branch `controls_since`; then
//!    `prepare_window` and `consume_window` with no lock held. One ticket per consumed window,
//!    strictly increasing and restarted per episode: success publishes the accepted pair,
//!    otherwise the ticket is recorded as a gap.
//!
//! **Lock order.** `dynamics` (seed, control reads, acknowledgement) is never held together
//! with the gate. The gate mutex is taken only for begin/end/publish/gap, never while
//! evaluating or preparing readouts; under it the C5 Position readouts are swapped in the same
//! critical section, so both carry the same ticket. Readers clone the latest `Arc` under the
//! gate lock and never block evaluation.
//!
//! **Gating.** Nothing starts unless [`family_adapters_gated`] holds: the engine supports
//! `PROGRAMMING_CONTRACT_VERSION` AND the explicit family-adapter opt-in is set. Production
//! never sets the opt-in, so production stays on the legacy path.
#![cfg_attr(not(test), allow(dead_code))]

use crate::runtime::capability_resources::OwnedWorkerThread;
use crate::runtime::dynamic_snapshot_publication::DynamicSnapshotPublication;
use crate::runtime::dynamic_source_origins::SharedDynamicSourceOrigins;
use crate::runtime::output_scheduler::dynamic_projection::pending_publication::{
    PendingAttemptTicket, PendingEpisodeIdentity, PendingNativeReadout, PendingPublicationGate,
};
use light_application::programming::PreloadPreviewDemand;
use light_core::programming::{PROGRAMMING_CONTRACT_VERSION, ProgrammingOwner};
use light_core::{AttributeValue, FixtureId, ShowId};
use light_dynamics::DynamicRuntime;
use light_engine::Engine;
use parking_lot::{Condvar, Mutex};
use std::sync::{Arc, mpsc};
use std::thread::ThreadId;

mod provider;
mod resource;
mod worker;

#[cfg(test)]
mod tests;

pub(in crate::runtime) use provider::PendingEpisodeReadouts;
pub(in crate::runtime) use resource::PendingEpisodeResource;

/// The one C3/C4 family-adapter gate: the engine supports the programming contract AND the
/// explicit family-adapter opt-in is set. See `resource::FamilyAdapterOptIn` for unifying the
/// opt-in with C3's Live flag.
pub(in crate::runtime) fn family_adapters_gated(engine: &Engine, opt_in: bool) -> bool {
    opt_in && engine.supported_programming_contract() >= PROGRAMMING_CONTRACT_VERSION
}

/// Every handle the worker reads. All `Send + Sync`; no physical lane crosses threads.
#[derive(Clone)]
pub(in crate::runtime) struct PendingEpisodeSources {
    pub engine: Arc<Engine>,
    pub dynamics: Arc<Mutex<DynamicRuntime>>,
    pub publication: Arc<DynamicSnapshotPublication>,
    pub origins: SharedDynamicSourceOrigins,
    /// The retained Programmer's Preload demand, read on every lifecycle step.
    pub preload: Arc<dyn Fn() -> PreloadPreviewDemand + Send + Sync>,
    /// The actual active show. None keeps Pending waiting.
    pub show: Arc<dyn Fn() -> Option<ShowId> + Send + Sync>,
}

/// A lifecycle event that invalidates the current episode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PendingTrigger {
    /// Preload GO committed.
    Go,
    /// Preload cleared or released.
    Clear,
    /// Show activation, including a same-show reload: a new activation nonce.
    Reload,
}

/// Passive, operator-visible state of the desk's Pending preview.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PendingEpisodeStatus {
    /// Preload is not engaged; no episode.
    Idle,
    /// Preload is engaged but no episode could be seeded yet (reason).
    Waiting(String),
    Running(PendingEpisodeIdentity),
    /// A non-empty Preload Playback queue cannot be previewed. No Live fallback is published.
    QueuePreviewUnavailable,
    /// The worker has shut down.
    Stopped,
}

/// One accepted and published pair, as observed by the worker. Diagnostics only.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct AcceptedPairSummary {
    pub identity: PendingEpisodeIdentity,
    pub ticket: PendingAttemptTicket,
    pub before: Vec<(FixtureId, ProgrammingOwner, AttributeValue)>,
    pub after: Vec<(FixtureId, ProgrammingOwner, AttributeValue)>,
    /// The evaluator state lineage of the After token; new for every episode.
    pub lineage: Arc<()>,
    /// Position fits and fit-cache hits of each branch lane since the episode began.
    pub position_fits: [(u64, u64); 2],
}

/// Monotonic worker counters, for observability and tests.
#[derive(Clone, Debug, Default)]
pub(in crate::runtime) struct PendingEpisodeProgress {
    pub steps: u64,
    pub episodes: u64,
    pub published: u64,
    pub gaps: u64,
    pub last: Option<AcceptedPairSummary>,
    pub last_error: Option<String>,
    pub worker: Option<ThreadId>,
}

enum Command {
    Wake,
    Trigger(PendingTrigger),
    Shutdown,
}

/// State shared between the worker and readers.
struct Shared {
    gate: Mutex<PendingPublicationGate>,
    status: Mutex<PendingEpisodeStatus>,
    readouts: Arc<PendingEpisodeReadouts>,
    progress: Mutex<PendingEpisodeProgress>,
    changed: Condvar,
    #[cfg(test)]
    fail_next_pair: std::sync::atomic::AtomicBool,
}

impl Shared {
    fn record(&self, update: impl FnOnce(&mut PendingEpisodeProgress)) {
        update(&mut self.progress.lock());
        self.changed.notify_all();
    }
}

/// Handle of the running worker. Dropping it shuts the worker down and ends its episode.
pub(in crate::runtime) struct PendingEpisodeExecutor {
    commands: mpsc::Sender<Command>,
    shared: Arc<Shared>,
    worker: OwnedWorkerThread,
}

impl PendingEpisodeExecutor {
    pub(in crate::runtime) fn spawn(sources: PendingEpisodeSources) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            gate: Mutex::default(),
            status: Mutex::new(PendingEpisodeStatus::Idle),
            readouts: Arc::new(PendingEpisodeReadouts::new(Arc::clone(&sources.engine))),
            progress: Mutex::default(),
            changed: Condvar::new(),
            #[cfg(test)]
            fail_next_pair: Default::default(),
        });
        let (commands, receiver) = mpsc::channel();
        let worker = worker::Worker::new(sources, Arc::clone(&shared), receiver);
        let worker = OwnedWorkerThread::spawn("tosklight-pending-episode", move || worker.run())?;
        Ok(Self {
            commands,
            shared,
            worker,
        })
    }

    /// The output tick or a retained input capture: evaluate what is pending.
    pub(in crate::runtime) fn wake(&self) {
        let _ = self.commands.send(Command::Wake);
    }

    pub(in crate::runtime) fn trigger(&self, trigger: PendingTrigger) {
        let _ = self.commands.send(Command::Trigger(trigger));
    }

    pub(in crate::runtime) fn status(&self) -> PendingEpisodeStatus {
        self.shared.status.lock().clone()
    }

    /// The current episode's last accepted readout. A cheap `Arc` clone under the gate lock.
    pub(in crate::runtime) fn latest(&self) -> Option<Arc<PendingNativeReadout>> {
        self.shared.gate.lock().latest().cloned()
    }

    /// The C5 Pending Position readout source of this executor.
    pub(in crate::runtime) fn readouts(&self) -> Arc<PendingEpisodeReadouts> {
        Arc::clone(&self.shared.readouts)
    }

    pub(in crate::runtime) fn progress(&self) -> PendingEpisodeProgress {
        self.shared.progress.lock().clone()
    }

    /// Wait until `done` holds for the worker's progress and status, or the timeout elapses.
    #[cfg(test)]
    pub(in crate::runtime) fn wait_for(
        &self,
        timeout: std::time::Duration,
        done: impl Fn(&PendingEpisodeProgress, &PendingEpisodeStatus) -> bool,
    ) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        let mut progress = self.shared.progress.lock();
        loop {
            if done(&progress, &self.shared.status.lock()) {
                return true;
            }
            if self
                .shared
                .changed
                .wait_until(&mut progress, deadline)
                .timed_out()
            {
                return done(&progress, &self.shared.status.lock());
            }
        }
    }

    /// TL-594 reader tests: begin an episode directly on this executor's gate and publish a
    /// readout under it, exactly as the worker would. Only valid while the worker is idle (no
    /// engaged Preload), so it never begins or ends an episode of its own.
    #[cfg(test)]
    pub(in crate::runtime) fn begin_test_episode(
        &self,
        identity: PendingEpisodeIdentity,
    ) -> crate::runtime::output_scheduler::dynamic_projection::pending_publication::PendingEpisodeLease
{
        self.shared.gate.lock().begin_episode(identity)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn publish_test_readout(
        &self,
        lease: &crate::runtime::output_scheduler::dynamic_projection::pending_publication::PendingEpisodeLease,
        readout: PendingNativeReadout,
    ) -> Arc<PendingNativeReadout> {
        self.shared
            .gate
            .lock()
            .publish(lease, readout)
            .expect("the test lease is current")
    }

    /// The next evaluated window fails in the real paired engine finalizer (swapped tokens).
    #[cfg(test)]
    pub(in crate::runtime) fn fail_next_pair(&self) {
        self.shared
            .fail_next_pair
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Drop for PendingEpisodeExecutor {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        self.worker.join();
    }
}
