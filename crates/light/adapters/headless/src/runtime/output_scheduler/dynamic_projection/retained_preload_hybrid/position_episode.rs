//! TL-556: the family-local Position Pending (Preload) episode bundle.
//!
//! Since TL-548 C4 this is the generic [`PendingEpisode`] over Position-only lanes: fresh
//! Before/After `PositionAdapter` lanes, one `RetainedPreloadHybridState`, the paired detached
//! history and the gate lease of one Pending episode identity. Its API and recreation suite
//! (`tests/position/episode_recreation.rs`) are unchanged and remain the regression net of the
//! shared lifecycle; the all-family bundle is `family_episode::FamilyPendingEpisode`.
#![cfg_attr(not(test), allow(dead_code))]

use super::pending_episode::{
    PendingEpisode, PendingEpisodeError, PendingEpisodeLanes, PendingEpisodePair,
    PendingEpisodePublishError,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::position::{
    PositionAdapter, PositionPreloadObserver,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::{
    PhysicalHeadResult, PhysicalPreloadLanes,
};

/// One branch's Position sidecar as produced by the physical lanes.
pub(in crate::runtime) type PositionPendingSidecar = PhysicalHeadResult<PositionAdapter>;
/// The accepted paired Position result of one episode.
pub(in crate::runtime) type PositionPendingPair = PendingEpisodePair<PositionPendingSidecar>;
/// Why a Position episode could not begin. No lease was taken and the gate is unchanged.
pub(in crate::runtime) type PositionPendingEpisodeError = PendingEpisodeError;
/// Why an accepted Position pair was not published.
pub(in crate::runtime) type PositionPendingPublishError = PendingEpisodePublishError;
/// Family-local Position bundle of one Pending episode.
pub(in crate::runtime) type PositionPendingEpisode =
    PendingEpisode<PhysicalPreloadLanes<PositionAdapter>>;

impl PendingEpisodeLanes for PhysicalPreloadLanes<PositionAdapter> {
    type Sidecar = PositionPendingSidecar;
    type Observer<'a> = PositionPreloadObserver<'a>;

    fn fresh() -> Self {
        PhysicalPreloadLanes::new(PositionAdapter::default(), PositionAdapter::default())
    }

    fn observer(&self) -> PositionPreloadObserver<'_> {
        PositionPreloadObserver::new(self)
    }
}
