//! Detached, bounded Pending history consumption. This module acquires no application locks,
//! reads no Live state, and cannot install or publish a result. The caller supplies an already
//! detached episode seed and immutable histories captured in one authoritative lineage.
//!
//! Structural window validation is all-or-none. Replay is then accepted one attempt interval
//! at a time. A failed replay leaves that interval unchanged; a failed evaluation consumes its
//! attempt while keeping accepted controls and the previous successful history/result. Earlier
//! accepted attempts are not undone by a later runtime failure.
//!
//! Queue overlays and enriched queue-context capture are deliberately not implemented here:
//! a nonempty captured queue is a passive gap. Missing Live anchors are also gaps, never matched
//! by timestamp or repaired by copying Live history. Only the current anchor association is
//! retained: controls referring to older/unselected anchors require explicit upstream recovery.
use crate::runtime::dynamic_snapshot_publication::{
    ColdGenerationCursor, ColdGenerationEvent, InputCaptureCursor, RetainedInputCapture,
};
use crate::runtime::dynamic_source_origins::DynamicSourceOrigins;
use light_core::ProgrammerId;
use light_dynamics::{
    DynamicControlBatch, DynamicControlCursor, DynamicOutputFrameScratch, DynamicRuntime,
    DynamicSampleBoundary, DynamicSampleScope, replay_dynamic_controls,
};
use light_engine::{EngineSnapshot, PreloadBranch};
use std::{
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct PendingEpisodeKey {
    /// New for every show activation, including a same-show reload.
    pub activation: Uuid,
    pub programmer: ProgrammerId,
    pub branch: PreloadBranch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct PendingHistoryPosition {
    pub inputs: InputCaptureCursor,
    pub cold: ColdGenerationCursor,
    pub controls: DynamicControlCursor,
}

#[derive(Clone, Copy)]
pub(in crate::runtime) struct PendingHistoryLimits {
    pub attempts: NonZeroUsize,
    pub cold_changes: NonZeroUsize,
    pub controls: NonZeroUsize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PendingHistoryGap {
    InvalidSeed,
    LimitExceeded,
    WrongEpisode,
    StaleWindow,
    InputInterval,
    ColdInterval,
    ControlInterval,
    SnapshotMismatch,
    QueueContextUnavailable,
    MissingAnchor,
    InstanceAnchor,
    Replay(String),
}

pub(in crate::runtime) struct PendingHistorySeed {
    pub key: PendingEpisodeKey,
    /// Must be the unpinned runtime from `fork_for_pending_preview` at this episode's seed.
    /// The domain exposes no pin-state getter: unpinned policy and exact registry/catalogue
    /// correspondence are caller-proven preconditions, not facts checked by `new`.
    /// Recording runtimes and mismatched seed sample identities are rejected.
    pub runtime: DynamicRuntime,
    pub origins: DynamicSourceOrigins,
    pub snapshot: Arc<EngineSnapshot>,
    pub position: PendingHistoryPosition,
    /// Authoritative seed anchor corresponding to the detached runtime's own marker. The
    /// caller captures this association at episode creation, before subsequent Live sampling.
    pub live_sample: Option<DynamicSampleBoundary>,
}

pub(in crate::runtime) struct DetachedPendingHistory<T> {
    key: PendingEpisodeKey,
    identity: Arc<()>,
    revision: u64,
    runtime: DynamicRuntime,
    origins: DynamicSourceOrigins,
    snapshot: Arc<EngineSnapshot>,
    position: PendingHistoryPosition,
    live_sample: Option<DynamicSampleBoundary>,
    transaction: DynamicOutputFrameScratch,
    last_success: Option<PendingHistoryResult<T>>,
}

pub(in crate::runtime) struct PendingHistoryResult<T> {
    pub capture: Arc<RetainedInputCapture>,
    pub branch_sample: Option<DynamicSampleBoundary>,
    pub value: T,
}

/// The evaluator may sample/reconcile this private branch and return owned output/evidence.
/// It must include every fallible part of evaluation before returning Ok. It must not perform
/// cold installation, open a nested output transaction, mutate external continuity/state, or
/// publish. Catalogue writes use the supplied COW candidate and commit with the runtime.
/// Immutable compiler scratch may live in the evaluator; semantic state belongs in the branch.
/// On success the evaluator must either retain the prior sample marker (no sampling), or
/// complete a whole-runtime pass. Instance-only sampling is forbidden here. This is a trusted
/// internal callback precondition: the public committed-marker getter hides provisional state
/// during a transaction, so this coordinator cannot independently certify the callback's scope.
/// Success does not itself certify physical delivery or a fresh sample of every idle lane.
pub(in crate::runtime) trait PendingAttemptEvaluator<T> {
    fn evaluate(
        &mut self,
        key: PendingEpisodeKey,
        capture: &RetainedInputCapture,
        runtime: &mut DynamicRuntime,
        origins: &mut DynamicSourceOrigins,
    ) -> Result<T, String>;
}

#[derive(Debug)]
pub(in crate::runtime) struct PendingAttemptFailure {
    pub input: InputCaptureCursor,
    pub detail: String,
}

#[derive(Default, Debug)]
pub(in crate::runtime) struct PendingWindowOutcome {
    pub consumed_attempts: usize,
    pub successful_attempts: usize,
    pub failed_attempts: Vec<PendingAttemptFailure>,
    /// Earlier attempt intervals remain accepted. This interval and its attempt are untouched.
    pub stopped: Option<PendingHistoryGap>,
}

enum ReplayStep {
    Controls(DynamicControlBatch),
    Cold(Arc<ColdGenerationEvent>),
}
struct PreparedAttempt {
    replay: Vec<ReplayStep>,
    capture: Arc<RetainedInputCapture>,
}

/// Bound to one coordinator allocation and revision, not merely equal cursor values.
/// Fields are private; a prepared window cannot be moved to another branch/episode.
pub(in crate::runtime) struct PreparedPendingWindow {
    identity: Arc<()>,
    revision: u64,
    attempts: Vec<PreparedAttempt>,
}

fn whole(sample: Option<DynamicSampleBoundary>) -> Result<(), PendingHistoryGap> {
    if sample.is_some_and(|sample| sample.scope() != DynamicSampleScope::WholeRuntime) {
        Err(PendingHistoryGap::InstanceAnchor)
    } else {
        Ok(())
    }
}
fn anchors(
    batch: &DynamicControlBatch,
    known: Option<DynamicSampleBoundary>,
) -> Result<(), PendingHistoryGap> {
    for sample in batch.preceding_samples() {
        whole(sample)?;
        if sample != known {
            return Err(PendingHistoryGap::MissingAnchor);
        }
    }
    Ok(())
}

impl<T> DetachedPendingHistory<T> {
    pub(in crate::runtime) fn new(seed: PendingHistorySeed) -> Result<Self, PendingHistoryGap> {
        if seed.key.activation.is_nil()
            || seed.key.programmer.0.is_nil()
            || seed.runtime.control_cursor().is_some()
        {
            return Err(PendingHistoryGap::InvalidSeed);
        }
        whole(seed.live_sample)?;
        whole(seed.runtime.committed_sample_boundary())?;
        if seed.live_sample != seed.runtime.committed_sample_boundary() {
            return Err(PendingHistoryGap::InvalidSeed);
        }
        Ok(Self {
            key: seed.key,
            identity: Arc::new(()),
            revision: 0,
            runtime: seed.runtime,
            origins: seed.origins,
            snapshot: seed.snapshot,
            position: seed.position,
            live_sample: seed.live_sample,
            transaction: Default::default(),
            last_success: None,
        })
    }

    pub(in crate::runtime) fn position(&self) -> PendingHistoryPosition {
        self.position
    }
    pub(in crate::runtime) fn last_success(&self) -> Option<&PendingHistoryResult<T>> {
        self.last_success.as_ref()
    }

    /// Inputs and cold events are ordered contiguous prefixes; controls may contain a newer
    /// tail. Only records through the final input barrier are retained in the prepared window.
    /// Cold events beyond that barrier must be left for the next window by the caller.
    /// Limits cover supplied buffers too, before allocating or scanning their contents.
    pub(in crate::runtime) fn prepare_window(
        &self,
        key: PendingEpisodeKey,
        inputs: &[Arc<RetainedInputCapture>],
        cold: &[Arc<ColdGenerationEvent>],
        controls: &DynamicControlBatch,
        limits: PendingHistoryLimits,
    ) -> Result<PreparedPendingWindow, PendingHistoryGap> {
        if key != self.key {
            return Err(PendingHistoryGap::WrongEpisode);
        }
        if self.revision == u64::MAX {
            return Err(PendingHistoryGap::StaleWindow);
        }
        if inputs.len() > limits.attempts.get()
            || cold.len() > limits.cold_changes.get()
            || controls.len() > limits.controls.get()
        {
            return Err(PendingHistoryGap::LimitExceeded);
        }
        if controls.from() != self.position.controls {
            return Err(PendingHistoryGap::ControlInterval);
        }
        let mut position = self.position;
        let mut snapshot = Arc::clone(&self.snapshot);
        let mut known_anchor = self.live_sample;
        let mut cold_index = 0;
        let mut offset = 0;
        let mut attempts = Vec::with_capacity(inputs.len());
        for input in inputs {
            if input.from != position.inputs || !input.to.immediately_follows(input.from) {
                return Err(PendingHistoryGap::InputInterval);
            }
            if input.frame.programmer().identity != Some(self.key.programmer) {
                return Err(PendingHistoryGap::WrongEpisode);
            }
            if !input.frame.programmer().preload_playback_actions.is_empty() {
                return Err(PendingHistoryGap::QueueContextUnavailable);
            }
            whole(input.live_sample)?;
            let mut replay = Vec::new();
            while position.cold != input.cold {
                let event = cold
                    .get(cold_index)
                    .ok_or(PendingHistoryGap::ColdInterval)?;
                if event.from != position.cold {
                    return Err(PendingHistoryGap::ColdInterval);
                }
                let (previous, destination, batch) =
                    event.replay_inputs().map_err(PendingHistoryGap::Replay)?;
                if !Arc::ptr_eq(&snapshot, previous) {
                    return Err(PendingHistoryGap::SnapshotMismatch);
                }
                append_controls(
                    controls,
                    &mut offset,
                    batch.from(),
                    known_anchor,
                    &mut replay,
                )?;
                anchors(batch, known_anchor)?;
                let end = controls
                    .offset_of(batch.to())
                    .ok_or(PendingHistoryGap::ControlInterval)?;
                if end < offset || end - offset != batch.len() {
                    return Err(PendingHistoryGap::ControlInterval);
                }
                // This API requires the supplied interval to cover the entire cold batch.
                // Exact retained record identity must match: equal cursors/anchors alone can
                // belong to divergent cold forks. Pruned overlap is an explicit gap.
                let supplied = controls
                    .range(offset..end)
                    .map_err(|_| PendingHistoryGap::ControlInterval)?;
                if !batch.shares_records(&supplied) {
                    return Err(PendingHistoryGap::ControlInterval);
                }
                anchors(&supplied, known_anchor)?;
                offset = end;
                replay.push(ReplayStep::Cold(Arc::clone(event)));
                position.cold = event.to;
                snapshot = Arc::clone(destination);
                cold_index += 1;
            }
            append_controls(
                controls,
                &mut offset,
                input.controls,
                known_anchor,
                &mut replay,
            )?;
            if !Arc::ptr_eq(&snapshot, &input.frame.snapshot()) {
                return Err(PendingHistoryGap::SnapshotMismatch);
            }
            position.inputs = input.to;
            position.controls = input.controls;
            // None explicitly means no NEW Live marker. It does not erase a prior known one.
            // A failed Pending attempt will still acknowledge this authoritative identity and
            // associate it with Pending's unchanged last successful marker.
            if let Some(sample) = input.live_sample {
                known_anchor = Some(sample);
            }
            attempts.push(PreparedAttempt {
                replay,
                capture: Arc::clone(input),
            });
        }
        if cold_index != cold.len() {
            return Err(PendingHistoryGap::ColdInterval);
        }
        Ok(PreparedPendingWindow {
            identity: Arc::clone(&self.identity),
            revision: self.revision,
            attempts,
        })
    }

    pub(in crate::runtime) fn consume_window(
        &mut self,
        window: PreparedPendingWindow,
        evaluator: &mut impl PendingAttemptEvaluator<T>,
    ) -> PendingWindowOutcome {
        let mut outcome = PendingWindowOutcome::default();
        if !Arc::ptr_eq(&self.identity, &window.identity) || self.revision != window.revision {
            outcome.stopped = Some(PendingHistoryGap::StaleWindow);
            return outcome;
        }
        // Invalidate other prepared plans even if a later evaluator unwinds.
        self.revision += 1;
        for attempt in window.attempts {
            if let Err(gap) = self.replay_interval(&attempt.replay) {
                outcome.stopped = Some(gap);
                break;
            }
            let capture = attempt.capture;
            // This clone shares three immutable maps. Only an actual source assignment edit
            // copies them. Runtime sampling uses its existing undo journal, not a frame fork.
            let mut origins = self.origins.clone();
            let evaluated = catch_unwind(AssertUnwindSafe(|| {
                self.runtime
                    .with_output_frame_transaction(&mut self.transaction, |runtime| {
                        evaluator.evaluate(self.key, &capture, runtime, &mut origins)
                    })
            }));
            self.position.inputs = capture.to;
            if let Some(sample) = capture.live_sample {
                self.live_sample = Some(sample);
            }
            outcome.consumed_attempts += 1;
            match evaluated {
                Ok(Ok(value)) => {
                    self.origins = origins;
                    self.last_success = Some(PendingHistoryResult {
                        branch_sample: self.runtime.committed_sample_boundary(),
                        capture,
                        value,
                    });
                    outcome.successful_attempts += 1;
                }
                failed => outcome.failed_attempts.push(PendingAttemptFailure {
                    input: capture.to,
                    detail: match failed {
                        Ok(Err(detail)) => detail,
                        Err(_) => "Pending evaluator panicked; its attempt was rolled back".into(),
                        Ok(Ok(_)) => unreachable!(),
                    },
                }),
            }
        }
        outcome
    }

    fn replay_interval(&mut self, steps: &[ReplayStep]) -> Result<(), PendingHistoryGap> {
        if steps.iter().any(|step| matches!(step, ReplayStep::Cold(_))) {
            // Cold definition installation is outside the runtime sampling journal. Fork only
            // on these cold intervals so all their controls/compilation accept atomically.
            let mut runtime = self.runtime.fork_for_pending_preview();
            let mut snapshot = Arc::clone(&self.snapshot);
            let mut position = self.position;
            for step in steps {
                replay_step(
                    step,
                    &mut runtime,
                    &mut snapshot,
                    &mut position,
                    &mut self.transaction,
                )?;
            }
            self.runtime = runtime;
            self.snapshot = snapshot;
            self.position = position;
        } else {
            // Without a cold boundary the planner emits at most one contiguous batch. The
            // existing replay API provides rollback; no mutable-runtime copy is necessary.
            debug_assert!(steps.len() <= 1);
            for step in steps {
                replay_step(
                    step,
                    &mut self.runtime,
                    &mut self.snapshot,
                    &mut self.position,
                    &mut self.transaction,
                )?;
            }
        }
        Ok(())
    }
}

fn append_controls(
    controls: &DynamicControlBatch,
    offset: &mut usize,
    through: DynamicControlCursor,
    known_anchor: Option<DynamicSampleBoundary>,
    steps: &mut Vec<ReplayStep>,
) -> Result<(), PendingHistoryGap> {
    let end = controls
        .offset_of(through)
        .ok_or(PendingHistoryGap::ControlInterval)?;
    if end < *offset {
        return Err(PendingHistoryGap::ControlInterval);
    }
    if end != *offset {
        let batch = controls
            .range(*offset..end)
            .map_err(|_| PendingHistoryGap::ControlInterval)?;
        anchors(&batch, known_anchor)?;
        steps.push(ReplayStep::Controls(batch));
    }
    *offset = end;
    Ok(())
}

fn replay_step(
    step: &ReplayStep,
    runtime: &mut DynamicRuntime,
    snapshot: &mut Arc<EngineSnapshot>,
    position: &mut PendingHistoryPosition,
    scratch: &mut DynamicOutputFrameScratch,
) -> Result<(), PendingHistoryGap> {
    match step {
        ReplayStep::Controls(batch) => {
            let expected = runtime.committed_sample_boundary();
            replay_dynamic_controls(runtime, scratch, &mut position.controls, batch, expected)
                .map_err(|error| PendingHistoryGap::Replay(error.to_string()))
        }
        ReplayStep::Cold(event) => event
            .apply(
                runtime,
                snapshot,
                &mut position.controls,
                &mut position.cold,
                scratch,
            )
            .map_err(PendingHistoryGap::Replay),
    }
}

#[cfg(test)]
#[path = "retained_history/tests.rs"]
mod tests;

#[path = "retained_history/paired.rs"]
pub(in crate::runtime) mod paired;
