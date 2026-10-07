//! TL-548 C4: one Pending (Preload) episode bundle, generic over the physical lanes it owns.
//!
//! One [`PendingEpisode`] is everything one [`PendingEpisodeIdentity`] owns: fresh Before/After
//! physical lanes (continuity, tracking, fit memo, descriptor and instance caches, counters),
//! ONE `RetainedPreloadHybridState` (engine token lineage and compiler scratch, shared by every
//! family the lanes route), the paired detached history built from the caller's episode seeds,
//! and the gate lease begun for this identity. Nothing is shared with, copied from or reset in
//! place from an earlier episode: clear, GO and same-show reload build a new bundle through
//! [`PendingEpisode::begin`] or [`PendingEpisode::recreate`].
//!
//! The lanes decide which families are evaluated: `PhysicalPreloadLanes<PositionAdapter>` is
//! the TL-556 Position bundle (`position_episode`), `FamilyPreloadLanes` the all-family bundle
//! (`family_episode`). Both are driven identically, so the Position regression suite covers the
//! shared lifecycle.
//!
//! This is not the executor. It acquires no lock, owns no schedule or trigger, reads no Live
//! state, samples no producer and publishes nothing on its own (see `pending_executor`).
//! Accepted-continuity semantics are exactly those of the retained evaluator and the physical
//! lanes; this bundle only scopes their lifetime to one episode.
#![cfg_attr(not(test), allow(dead_code))]

use super::{
    HybridFrameResolver, PendingHybridResult, RetainedHybridFrameObserver,
    RetainedPreloadHybridEvaluator, RetainedPreloadHybridState,
};
use crate::runtime::dynamic_snapshot_publication::{ColdGenerationEvent, RetainedInputCapture};
use crate::runtime::output_scheduler::dynamic_projection::pending_publication::{
    PendingAttemptTicket, PendingEpisodeIdentity, PendingEpisodeLease, PendingNativeReadout,
    PendingPublicationGate, PendingPublicationRejection, PendingReadoutError,
};
use crate::runtime::preload::retained_history::paired::{
    PairedPendingHistory, PendingPairResult, PendingPairWindowOutcome, PreparedPendingPairWindow,
};
use crate::runtime::preload::retained_history::{
    PendingHistoryGap, PendingHistoryLimits, PendingHistoryPosition, PendingHistorySeed,
};
use light_dynamics::DynamicControlBatch;
use light_engine::Engine;
use std::sync::Arc;

/// The Before/After physical lanes one episode owns, and the observer that drives them.
pub(in crate::runtime) trait PendingEpisodeLanes: HybridFrameResolver {
    /// One branch's owned per-head result.
    type Sidecar;
    /// The paired observer, borrowing these lanes for one synchronous evaluation.
    type Observer<'a>: RetainedHybridFrameObserver<Self::Sidecar>
    where
        Self: 'a;

    /// Entirely fresh lanes: no continuity, tracking, caches or counters.
    fn fresh() -> Self;

    fn observer(&self) -> Self::Observer<'_>;
}

/// The accepted paired result of one episode.
pub(in crate::runtime) type PendingEpisodePair<S> = PendingPairResult<PendingHybridResult<S>>;

/// Why an episode could not begin. No lease was taken and the gate is unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PendingEpisodeError {
    /// A seed's activation or Programmer is not the episode identity's.
    SeedIdentity,
    /// The paired history rejected the seeds.
    History(PendingHistoryGap),
}

/// Why an accepted pair was not published. The gate keeps its last accepted readout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PendingEpisodePublishError {
    /// This episode has no accepted pair yet (for example only failed attempts).
    NothingAccepted,
    Readout(PendingReadoutError),
    Rejected(PendingPublicationRejection),
}

/// One Pending episode. Plain owned data: the owner supplies any synchronization. Lanes use
/// interior mutability for one synchronous frame and are therefore not `Sync`, but the bundle
/// is `Send`: a dedicated Pending worker thread owns it.
pub(in crate::runtime) struct PendingEpisode<L: PendingEpisodeLanes> {
    identity: PendingEpisodeIdentity,
    lanes: L,
    state: RetainedPreloadHybridState,
    pair: PairedPendingHistory<PendingHybridResult<L::Sidecar>>,
    lease: PendingEpisodeLease,
}

impl<L: PendingEpisodeLanes> PendingEpisode<L> {
    /// Begin a new episode with entirely fresh lanes, evaluator state and paired history. Both
    /// seeds must be this identity's activation and Programmer; validation precedes the gate,
    /// so a rejected begin leaves the gate (and any current episode) untouched.
    pub(in crate::runtime) fn begin(
        gate: &mut PendingPublicationGate,
        identity: PendingEpisodeIdentity,
        before: PendingHistorySeed,
        after: PendingHistorySeed,
    ) -> Result<Self, PendingEpisodeError> {
        if [&before, &after].into_iter().any(|seed| {
            seed.key.activation != identity.activation || seed.key.programmer != identity.programmer
        }) {
            return Err(PendingEpisodeError::SeedIdentity);
        }
        let pair =
            PairedPendingHistory::new(before, after).map_err(PendingEpisodeError::History)?;
        Ok(Self {
            identity,
            lanes: L::fresh(),
            state: RetainedPreloadHybridState::default(),
            pair,
            lease: gate.begin_episode(identity),
        })
    }

    /// Clear, GO or same-show reload: end this episode and begin the next from new seeds.
    /// Nothing of this bundle survives, including on error, where no episode remains current.
    pub(in crate::runtime) fn recreate(
        self,
        gate: &mut PendingPublicationGate,
        identity: PendingEpisodeIdentity,
        before: PendingHistorySeed,
        after: PendingHistorySeed,
    ) -> Result<Self, PendingEpisodeError> {
        self.end(gate);
        Self::begin(gate, identity, before, after)
    }

    /// End the episode. Returns whether its lease was still the gate's current episode; a
    /// superseded bundle never ends its successor.
    pub(in crate::runtime) fn end(self, gate: &mut PendingPublicationGate) -> bool {
        gate.end_episode(&self.lease)
    }

    pub(in crate::runtime) fn identity(&self) -> PendingEpisodeIdentity {
        self.identity
    }

    pub(in crate::runtime) fn lease(&self) -> &PendingEpisodeLease {
        &self.lease
    }

    /// This episode's Before/After lanes. Diagnostics and readouts only.
    pub(in crate::runtime) fn lanes(&self) -> &L {
        &self.lanes
    }

    /// Each branch's own accepted history cursor; prepare the next window from these.
    pub(in crate::runtime) fn positions(&self) -> (PendingHistoryPosition, PendingHistoryPosition) {
        self.pair.positions()
    }

    /// The last pair accepted in this episode. Never an earlier episode's pair.
    pub(in crate::runtime) fn accepted(&self) -> Option<&PendingEpisodePair<L::Sidecar>> {
        self.pair.last_success()
    }

    /// Validate one bounded window against this episode's history; see `PairedPendingHistory`.
    pub(in crate::runtime) fn prepare_window(
        &self,
        inputs: &[Arc<RetainedInputCapture>],
        before_cold: &[Arc<ColdGenerationEvent>],
        before_controls: &DynamicControlBatch,
        after_cold: &[Arc<ColdGenerationEvent>],
        after_controls: &DynamicControlBatch,
        limits: PendingHistoryLimits,
    ) -> Result<PreparedPendingPairWindow, PendingHistoryGap> {
        self.pair.prepare_window(
            inputs,
            before_cold,
            before_controls,
            after_cold,
            after_controls,
            limits,
        )
    }

    /// Evaluate a prepared window with a fresh evaluator that borrows this episode's ONE state
    /// and its lanes for the synchronous call.
    pub(in crate::runtime) fn consume_window(
        &mut self,
        engine: &Engine,
        window: PreparedPendingPairWindow,
    ) -> PendingPairWindowOutcome {
        self.consume(engine, window, false)
    }

    /// A real failed pair for tests: the engine finalizer rejects exchanged branch tokens.
    #[cfg(test)]
    pub(in crate::runtime) fn consume_window_with_swapped_finalization(
        &mut self,
        engine: &Engine,
        window: PreparedPendingPairWindow,
    ) -> PendingPairWindowOutcome {
        self.consume(engine, window, true)
    }

    fn consume(
        &mut self,
        engine: &Engine,
        window: PreparedPendingPairWindow,
        swap: bool,
    ) -> PendingPairWindowOutcome {
        #[cfg_attr(not(test), allow(unused_mut))]
        let mut evaluator = RetainedPreloadHybridEvaluator::with_state_and_observer(
            engine,
            self.identity.programmer,
            &mut self.state,
            &self.lanes,
            self.lanes.observer(),
        );
        #[cfg(test)]
        {
            evaluator.swap_finalization_tokens = swap;
        }
        #[cfg(not(test))]
        let _ = swap;
        self.pair.consume_window(window, &mut evaluator)
    }

    /// Prepare the readout of this episode's accepted pair from the exact retained capture the
    /// caller fed in. Pure: no gate is touched, so an owner can prepare outside its gate lock.
    pub(in crate::runtime) fn prepare_readout(
        &self,
        ticket: PendingAttemptTicket,
        exact: &Arc<RetainedInputCapture>,
    ) -> Result<PendingNativeReadout, PendingEpisodePublishError> {
        let accepted = self
            .accepted()
            .ok_or(PendingEpisodePublishError::NothingAccepted)?;
        PendingNativeReadout::prepare(self.identity, ticket, accepted, exact)
            .map_err(PendingEpisodePublishError::Readout)
    }

    /// Publish a readout prepared by [`Self::prepare_readout`] under this episode's lease.
    pub(in crate::runtime) fn publish_readout(
        &self,
        gate: &mut PendingPublicationGate,
        readout: PendingNativeReadout,
    ) -> Result<Arc<PendingNativeReadout>, PendingEpisodePublishError> {
        gate.publish(&self.lease, readout)
            .map_err(PendingEpisodePublishError::Rejected)
    }

    /// Prepare a readout from this episode's accepted pair and the exact retained capture the
    /// caller fed in, then publish it under this episode's lease. The caller owns the ticket.
    pub(in crate::runtime) fn publish_accepted(
        &self,
        gate: &mut PendingPublicationGate,
        ticket: PendingAttemptTicket,
        exact: &Arc<RetainedInputCapture>,
    ) -> Result<Arc<PendingNativeReadout>, PendingEpisodePublishError> {
        let readout = self.prepare_readout(ticket, exact)?;
        self.publish_readout(gate, readout)
    }

    /// Consume a failed or stopped attempt's ticket under this episode's lease.
    pub(in crate::runtime) fn record_gap(
        &self,
        gate: &mut PendingPublicationGate,
        ticket: PendingAttemptTicket,
    ) -> Result<(), PendingPublicationRejection> {
        gate.record_gap(&self.lease, ticket)
    }
}
