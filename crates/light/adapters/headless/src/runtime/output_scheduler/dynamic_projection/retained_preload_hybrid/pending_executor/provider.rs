//! TL-548 C5 provider: Pending Position readouts from the executor's accepted pairs.
//!
//! Readouts are computed at publish time (owner decision (ii)) with
//! `capture_pending_position_readouts`, for every fixture of the exact retained capture's show,
//! from the SAME accepted pair and ticket as the published `PendingNativeReadout`. They are
//! swapped in under the gate lock in the same critical section, so both bind one frame
//! identity. Requested intent and commands are the After branch's; Before and the Live
//! publication are never read. A reader filters the precomputed owners; the result is passive
//! (None) for another Programmer, after the episode ended, or once the engine runs another
//! snapshot than the pair's.
//!
//! G5: the same accepted pair's lamp Color outputs (After branch) are kept beside the Position
//! readouts, so the first Direct edit in Preload seeds from the Pending output the operator saw.
use crate::runtime::dynamic_snapshot_publication::RetainedInputCapture;
use crate::runtime::output_scheduler::dynamic_projection::pending_publication::{
    PendingAttemptTicket, PendingEpisodeIdentity,
};
use crate::runtime::output_scheduler::dynamic_projection::retained_preload_hybrid::pending_episode::PendingEpisodePair;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::color_router::RoutedColorQuality;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::family_lanes::FamilySidecar;
use crate::runtime::position_readout::{
    CapturedPositionReadouts, PendingColorOutput, PendingPositionReadoutSource,
    capture_pending_position_readouts,
};
use light_core::{FixtureId, ProgrammerId};
use light_engine::{Engine, EngineSnapshot};
use parking_lot::Mutex;
use std::sync::Arc;

/// One accepted pair's Position readouts, bound to its Programmer and engine snapshot.
pub(in crate::runtime) struct AcceptedPositionReadouts {
    programmer: ProgrammerId,
    snapshot: Arc<EngineSnapshot>,
    readouts: CapturedPositionReadouts,
    /// G5: the pair's After-branch lamp Color outputs, one per programming target.
    colors: Vec<PendingColorOutput>,
}

/// G5: the lamp Color outputs of one accepted pair's After branch (Before is never read).
pub(super) fn pending_colors(sidecars: &[FamilySidecar]) -> Vec<PendingColorOutput> {
    sidecars
        .iter()
        .filter_map(FamilySidecar::color)
        .filter(|color| matches!(color.quality, RoutedColorQuality::Lamp(_)))
        .map(|color| PendingColorOutput {
            token: color.token.clone(),
            target: color.target,
            value: color.value.clone(),
            writes: color.writes.clone(),
        })
        .collect()
}

/// The executor's `PendingPositionReadoutSource`. Readers take only this mutex.
pub(in crate::runtime) struct PendingEpisodeReadouts {
    engine: Arc<Engine>,
    current: Mutex<Option<Arc<AcceptedPositionReadouts>>>,
}

impl PendingEpisodeReadouts {
    pub(super) fn new(engine: Arc<Engine>) -> Self {
        Self {
            engine,
            current: Mutex::new(None),
        }
    }

    /// Compute every owner's readout of one accepted pair. Pure; call outside the gate lock.
    pub(super) fn capture_accepted<S>(
        &self,
        identity: PendingEpisodeIdentity,
        ticket: PendingAttemptTicket,
        accepted: &PendingEpisodePair<S>,
        exact: &Arc<RetainedInputCapture>,
        colors: Vec<PendingColorOutput>,
    ) -> Option<Arc<AcceptedPositionReadouts>> {
        let snapshot = exact.frame.snapshot();
        let owners: Vec<FixtureId> = snapshot
            .fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect();
        let readouts = capture_pending_position_readouts(
            &self.engine,
            identity,
            ticket,
            accepted,
            exact,
            &owners,
        )
        .ok()?;
        Some(Arc::new(AcceptedPositionReadouts {
            programmer: identity.programmer,
            snapshot,
            readouts,
            colors,
        }))
    }

    /// Swap in the readouts of the pair just published (under the gate lock).
    pub(super) fn publish(&self, readouts: Option<Arc<AcceptedPositionReadouts>>) {
        *self.current.lock() = readouts;
    }

    /// The episode ended or was replaced: nothing of it stays readable.
    pub(super) fn clear(&self) {
        *self.current.lock() = None;
    }
}

impl PendingPositionReadoutSource for PendingEpisodeReadouts {
    fn color_output(
        &self,
        programmer: ProgrammerId,
        target: FixtureId,
    ) -> Option<PendingColorOutput> {
        let current = self.current.lock().clone()?;
        if current.programmer != programmer
            || !Arc::ptr_eq(&self.engine.snapshot(), &current.snapshot)
        {
            return None;
        }
        current
            .colors
            .iter()
            .find(|output| output.target == target)
            .cloned()
    }

    fn capture(
        &self,
        programmer: ProgrammerId,
        owners: &[FixtureId],
    ) -> Option<CapturedPositionReadouts> {
        let current = self.current.lock().clone()?;
        if current.programmer != programmer
            || !Arc::ptr_eq(&self.engine.snapshot(), &current.snapshot)
        {
            return None;
        }
        let readouts = &current.readouts;
        Some(CapturedPositionReadouts {
            identity: readouts.identity.clone(),
            scope: readouts.scope,
            show_revision: readouts.show_revision,
            owners: owners
                .iter()
                .filter_map(|owner| {
                    readouts
                        .owners
                        .iter()
                        .find(|entry| entry.owner == *owner)
                        .cloned()
                })
                .collect(),
        })
    }
}
