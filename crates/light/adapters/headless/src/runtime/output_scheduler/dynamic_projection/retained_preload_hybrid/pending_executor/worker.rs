//! The dedicated Pending worker: lifecycle reconciliation, seeding and paired evaluation.
//! Everything here runs on the worker thread; see the module root for locks and triggers.
use super::{
    AcceptedPairSummary, Command, PendingEpisodeSources, PendingEpisodeStatus, PendingTrigger,
    Shared, family_adapters_gated,
};
use crate::runtime::dynamic_snapshot_publication::{
    ColdGenerationCursor, ColdGenerationEvent, RetainedInputCapture,
};
use crate::runtime::dynamic_source_origins::DynamicSourceOrigins;
use crate::runtime::output_scheduler::dynamic_projection::pending_publication::{
    PendingAttemptTicket, PendingEpisodeIdentity,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::family_lanes::FamilySidecar;
use crate::runtime::output_scheduler::dynamic_projection::retained_preload_hybrid::family_episode::FamilyPendingEpisode;
use crate::runtime::preload::retained_history::{
    PendingEpisodeKey, PendingHistoryGap, PendingHistoryLimits, PendingHistoryPosition,
    PendingHistorySeed,
};
use light_core::{FixtureId, ProgrammerId, SessionId, ShowId};
use light_engine::{CapturedFrameLane, PreloadBranch};
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, mpsc};
use uuid::Uuid;

/// Without a wake command the worker polls at the retained-input sample interval.
const POLL: std::time::Duration =
    crate::runtime::visualization_frame::VISUALIZATION_SOURCE_SAMPLE_INTERVAL;
const HISTORY: NonZeroUsize = NonZeroUsize::new(256).unwrap();
const LIMITS: PendingHistoryLimits = PendingHistoryLimits {
    attempts: NonZeroUsize::new(16).unwrap(),
    cold_changes: NonZeroUsize::new(16).unwrap(),
    controls: HISTORY,
};

/// Why the current episode must be replaced before the next evaluation.
#[derive(Clone, Copy, Debug)]
enum Recreate {
    /// GO, clear/release or show activation.
    Trigger,
    /// The retained input, cold or control history can no longer be followed.
    HistoryLost,
}

/// What the lifecycle wants right now.
enum Desired {
    Off,
    Queue,
    Waiting(String),
    On {
        programmer: ProgrammerId,
        show: ShowId,
    },
}

pub(super) struct Worker {
    sources: PendingEpisodeSources,
    shared: Arc<Shared>,
    commands: mpsc::Receiver<Command>,
    episode: Option<FamilyPendingEpisode>,
    /// Last ticket of the current episode; restarted for every episode.
    ticket: u64,
    /// The show activation nonce; new on every activation, including a same-show reload.
    activation: Uuid,
    recreate: Option<Recreate>,
}

impl Worker {
    pub(super) fn new(
        sources: PendingEpisodeSources,
        shared: Arc<Shared>,
        commands: mpsc::Receiver<Command>,
    ) -> Self {
        Self {
            sources,
            shared,
            commands,
            episode: None,
            ticket: 0,
            activation: Uuid::new_v4(),
            recreate: None,
        }
    }

    pub(super) fn run(mut self) {
        let thread = std::thread::current().id();
        self.shared
            .record(|progress| progress.worker = Some(thread));
        'run: loop {
            match self.commands.recv_timeout(POLL) {
                Ok(command) => {
                    if !self.apply(command) {
                        break 'run;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break 'run,
            }
            while let Ok(command) = self.commands.try_recv() {
                if !self.apply(command) {
                    break 'run;
                }
            }
            if catch_unwind(AssertUnwindSafe(|| self.step())).is_err() {
                // A panicking evaluation leaves no trustworthy episode state behind.
                self.episode = None;
                self.recreate = Some(Recreate::HistoryLost);
                self.set_status(PendingEpisodeStatus::Waiting("evaluation panicked".into()));
            }
            self.shared.record(|progress| progress.steps += 1);
        }
        self.end(PendingEpisodeStatus::Stopped);
        self.shared.record(|_| {});
    }

    /// Returns false on shutdown.
    fn apply(&mut self, command: Command) -> bool {
        match command {
            Command::Wake => true,
            Command::Trigger(trigger) => {
                if trigger == PendingTrigger::Reload {
                    self.activation = Uuid::new_v4();
                }
                self.recreate = Some(Recreate::Trigger);
                true
            }
            Command::Shutdown => false,
        }
    }

    fn step(&mut self) {
        match self.desired() {
            Desired::Off => self.end(PendingEpisodeStatus::Idle),
            Desired::Queue => self.end(PendingEpisodeStatus::QueuePreviewUnavailable),
            Desired::Waiting(reason) => self.end(PendingEpisodeStatus::Waiting(reason)),
            Desired::On { programmer, show } => {
                let current = self.episode.as_ref().map(FamilyPendingEpisode::identity);
                let stale = current.is_none_or(|identity| {
                    identity.programmer != programmer
                        || identity.show_id != show
                        || identity.activation != self.activation
                });
                if (stale || self.recreate.is_some()) && !self.begin(programmer, show) {
                    return;
                }
                self.attempt();
            }
        }
    }

    /// Preload engaged = armed (blind capture) or retained active Preload values. The
    /// registry is desk-scoped: its Preload readers ignore the session argument.
    fn desired(&self) -> Desired {
        let sources = &self.sources;
        if !family_adapters_gated(&sources.engine, true) {
            return Desired::Off;
        }
        let programmers = &sources.programmers;
        let desk = SessionId(Uuid::nil());
        let Some(programmer) = programmers.programmer_id() else {
            return Desired::Off;
        };
        let armed = programmers
            .capture_mode(desk)
            .is_some_and(|mode| mode.blind);
        if !armed && programmers.has_active_preload(desk) != Some(true) {
            return Desired::Off;
        }
        if programmers
            .preload_playback_actions(desk)
            .is_some_and(|queue| !queue.is_empty())
        {
            return Desired::Queue;
        }
        match (sources.show)() {
            Some(show) => Desired::On { programmer, show },
            None => Desired::Waiting("no active show".into()),
        }
    }

    /// Mint a new identity, seed both branches and (re)create the episode. Returns whether an
    /// episode is now current. A requested recreation never keeps the previous episode.
    fn begin(&mut self, programmer: ProgrammerId, show: ShowId) -> bool {
        let identity = PendingEpisodeIdentity {
            show_id: show,
            activation: self.activation,
            programmer,
            episode: Uuid::new_v4(),
        };
        let (before, after) = match seed(&self.sources, identity) {
            Ok(seeds) => seeds,
            Err(reason) => {
                self.end(PendingEpisodeStatus::Waiting(reason));
                return false;
            }
        };
        let result = {
            let mut gate = self.shared.gate.lock();
            self.shared.readouts.clear();
            match self.episode.take() {
                Some(old) => old.recreate(&mut gate, identity, before, after),
                None => FamilyPendingEpisode::begin(&mut gate, identity, before, after),
            }
        };
        match result {
            Ok(episode) => {
                self.episode = Some(episode);
                self.ticket = 0;
                self.recreate = None;
                self.set_status(PendingEpisodeStatus::Running(identity));
                self.shared.record(|progress| progress.episodes += 1);
                true
            }
            Err(error) => {
                self.set_status(PendingEpisodeStatus::Waiting(format!(
                    "episode refused its seeds: {error:?}"
                )));
                false
            }
        }
    }

    /// End the current episode (its lease leaves the gate, hiding its readout) and report.
    fn end(&mut self, status: PendingEpisodeStatus) {
        if let Some(episode) = self.episode.take() {
            let mut gate = self.shared.gate.lock();
            episode.end(&mut gate);
            self.shared.readouts.clear();
        }
        self.set_status(status);
    }

    fn set_status(&self, status: PendingEpisodeStatus) {
        let mut current = self.shared.status.lock();
        if *current != status {
            *current = status;
        }
    }

    fn lost(&mut self, reason: String) {
        self.recreate = Some(Recreate::HistoryLost);
        self.shared
            .record(|progress| progress.last_error = Some(reason));
    }

    /// Evaluate every retained input since the episode's cursor in one bounded window.
    fn attempt(&mut self) {
        let Some(episode) = self.episode.as_ref() else {
            return;
        };
        let window = match window(&self.sources, episode) {
            Ok(Some(window)) => window,
            Ok(None) => return,
            Err(WindowError::Queue) => {
                return self.end(PendingEpisodeStatus::QueuePreviewUnavailable);
            }
            Err(WindowError::Lost(reason)) => return self.lost(reason),
        };
        let engine = Arc::clone(&self.sources.engine);
        let episode = self.episode.as_mut().expect("checked above");
        #[cfg(test)]
        let outcome = if self
            .shared
            .fail_next_pair
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            episode.consume_window_with_swapped_finalization(&engine, window)
        } else {
            episode.consume_window(&engine, window)
        };
        #[cfg(not(test))]
        let outcome = episode.consume_window(&engine, window);
        if outcome.consumed_attempts > 0 {
            self.ticket += 1;
            let ticket = PendingAttemptTicket::new(self.ticket);
            if outcome.successful_attempts > 0 {
                self.publish(ticket);
            } else {
                let detail = outcome
                    .failed_attempts
                    .last()
                    .map(|failure| failure.detail.clone());
                self.gap(ticket, detail);
            }
        }
        if let Some(stop) = outcome.stopped {
            self.lost(format!("Pending window stopped: {stop:?}"));
        }
        self.acknowledge_controls();
    }

    /// Prepare both readouts outside the gate lock; publish them in one critical section.
    fn publish(&self, ticket: PendingAttemptTicket) {
        let episode = self.episode.as_ref().expect("an evaluated episode");
        let accepted = episode.accepted().expect("a successful attempt");
        let exact = Arc::clone(&accepted.capture);
        let identity = episode.identity();
        let readout = match episode.prepare_readout(ticket, &exact) {
            Ok(readout) => readout,
            Err(error) => return self.gap(ticket, Some(format!("{error:?}"))),
        };
        let positions = self
            .shared
            .readouts
            .capture_accepted(identity, ticket, accepted, &exact);
        let summary = summarize(episode, ticket);
        let published = {
            let mut gate = self.shared.gate.lock();
            let published = episode.publish_readout(&mut gate, readout);
            if published.is_ok() {
                self.shared.readouts.publish(positions);
            }
            published
        };
        self.shared.record(|progress| match published {
            Ok(_) => {
                progress.published += 1;
                progress.last = Some(summary);
            }
            Err(error) => progress.last_error = Some(format!("publication refused: {error:?}")),
        });
    }

    fn gap(&self, ticket: PendingAttemptTicket, detail: Option<String>) {
        let Some(episode) = self.episode.as_ref() else {
            return;
        };
        let recorded = episode.record_gap(&mut self.shared.gate.lock(), ticket);
        self.shared.record(|progress| {
            progress.gaps += 1;
            progress.last_error = detail.or(recorded.err().map(|error| format!("{error:?}")));
        });
    }

    /// Release Live control records both branches have consumed. Only this episode reads the
    /// journal, so its common cursor is the minimum acknowledged position.
    fn acknowledge_controls(&self) {
        let Some(episode) = self.episode.as_ref() else {
            return;
        };
        let (before, after) = episode.positions();
        if before.controls == after.controls {
            let _ = self
                .sources
                .dynamics
                .lock()
                .acknowledge_controls(before.controls);
        }
    }
}

/// Both branch seeds of one episode, forked under ONE Live Dynamics guard from the same
/// authoritative recording boundary (the `seeds()` recipe of the retained Preload tests).
fn seed(
    sources: &PendingEpisodeSources,
    identity: PendingEpisodeIdentity,
) -> Result<(PendingHistorySeed, PendingHistorySeed), String> {
    let mut live = sources.dynamics.lock();
    let snapshot = sources.engine.snapshot();
    let (cold, controls) = sources
        .publication
        .begin_retained_history(&mut live, &snapshot, HISTORY)
        .map_err(str::to_owned)?;
    let inputs = sources
        .publication
        .input_capture_cursor()
        .ok_or("no retained input history")?;
    let origins = DynamicSourceOrigins::clone(&sources.origins.load());
    let live_sample = live.committed_sample_boundary();
    let key = PendingEpisodeKey {
        activation: identity.activation,
        programmer: identity.programmer,
        branch: PreloadBranch::BeforeRelease,
    };
    let seed = |branch| PendingHistorySeed {
        key: PendingEpisodeKey { branch, ..key },
        runtime: live.fork_for_pending_preview(),
        origins: origins.clone(),
        snapshot: Arc::clone(&snapshot),
        position: PendingHistoryPosition {
            inputs,
            cold,
            controls,
        },
        live_sample,
    };
    Ok((
        seed(PreloadBranch::BeforeRelease),
        seed(PreloadBranch::AfterRelease),
    ))
}

enum WindowError {
    Queue,
    Lost(String),
}

/// The next bounded window of retained inputs, or None when nothing new was retained.
fn window(
    sources: &PendingEpisodeSources,
    episode: &FamilyPendingEpisode,
) -> Result<
    Option<crate::runtime::preload::retained_history::paired::PreparedPendingPairWindow>,
    WindowError,
> {
    let lost = |what: &str, error: &dyn std::fmt::Debug| {
        WindowError::Lost(format!("{what} history lost: {error:?}"))
    };
    let (before, after) = episode.positions();
    let mut inputs = sources
        .publication
        .input_captures_since(before.inputs)
        .map_err(|error| lost("input", &error))?;
    inputs.truncate(LIMITS.attempts.get());
    let Some(last) = inputs.last() else {
        return Ok(None);
    };
    if inputs.iter().any(queued) {
        return Err(WindowError::Queue);
    }
    let through = last.cold;
    let before_cold = cold_through(sources, before.cold, through).map_err(|e| lost("cold", &e))?;
    let after_cold = cold_through(sources, after.cold, through).map_err(|e| lost("cold", &e))?;
    let (before_controls, after_controls) = {
        let live = sources.dynamics.lock();
        let read = |cursor| match live.controls_since(cursor) {
            Some(Ok(batch)) => Ok(batch),
            Some(Err(error)) => Err(lost("control", &error)),
            None => Err(lost("control", &"recording ended")),
        };
        (read(before.controls)?, read(after.controls)?)
    };
    episode
        .prepare_window(
            &inputs,
            &before_cold,
            &before_controls,
            &after_cold,
            &after_controls,
            LIMITS,
        )
        .map(Some)
        .map_err(|gap| match gap {
            PendingHistoryGap::QueueContextUnavailable => WindowError::Queue,
            gap => lost("window", &gap),
        })
}

fn queued(input: &Arc<RetainedInputCapture>) -> bool {
    !input.frame.programmer().preload_playback_actions.is_empty()
}

/// Cold events from `from` through the last input's cold cursor; later events stay for the
/// next window, as `prepare_window` requires.
fn cold_through(
    sources: &PendingEpisodeSources,
    from: ColdGenerationCursor,
    through: ColdGenerationCursor,
) -> Result<Vec<Arc<ColdGenerationEvent>>, String> {
    if from == through {
        return Ok(Vec::new());
    }
    let mut position = from;
    let mut events = Vec::new();
    for event in sources
        .publication
        .cold_generations_since(from)
        .map_err(|error| format!("{error:?}"))?
    {
        if position == through {
            break;
        }
        position = event.to;
        events.push(event);
    }
    Ok(events)
}

fn rows(
    sidecars: &[FamilySidecar],
) -> Vec<(
    FixtureId,
    light_core::programming::ProgrammingOwner,
    light_core::AttributeValue,
)> {
    sidecars
        .iter()
        .map(|row| (row.target(), row.owner(), row.value().clone()))
        .collect()
}

fn summarize(episode: &FamilyPendingEpisode, ticket: PendingAttemptTicket) -> AcceptedPairSummary {
    let pair = episode.accepted().expect("a published pair");
    let lineage = match pair.value.after.frame_token.lane() {
        CapturedFrameLane::Preload { state, .. } => Arc::clone(state),
        CapturedFrameLane::Live => Arc::new(()),
    };
    let fits = |branch| {
        let counters = episode
            .lanes()
            .lanes(branch)
            .position()
            .adapter()
            .counters();
        (counters.fits, counters.fit_cache_hits)
    };
    AcceptedPairSummary {
        identity: episode.identity(),
        ticket,
        before: rows(&pair.value.before.sidecars),
        after: rows(&pair.value.after.sidecars),
        lineage,
        position_fits: [
            fits(PreloadBranch::BeforeRelease),
            fits(PreloadBranch::AfterRelease),
        ],
    }
}
