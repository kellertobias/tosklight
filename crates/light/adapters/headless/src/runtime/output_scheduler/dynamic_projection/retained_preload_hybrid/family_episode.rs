//! TL-548 C4: the all-family Pending (Preload) episode bundle.
//!
//! [`FamilyPendingEpisode`] owns one `FamilyPreloadLanes` set (Position, routed Color and
//! Focus/Zoom lanes of each branch), ONE `RetainedPreloadHybridState` shared by every family,
//! one `PairedPendingHistory<PendingHybridResult<FamilySidecar>>` and one gate lease. One state
//! is forced by the evaluator: each branch is prepared once with a single resolver/observer and
//! `render_prepared_preload_families` consumes one Before/After token pair per attempt, so
//! per-family states would yield separate token lineages and incoherent frames.
#![cfg_attr(not(test), allow(dead_code))]

use super::pending_episode::{PendingEpisode, PendingEpisodeLanes};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::family_lanes::{
    FamilyPreloadLanes, FamilyPreloadObserver, FamilySidecar,
};

/// The all-family bundle of one Pending episode.
pub(in crate::runtime) type FamilyPendingEpisode = PendingEpisode<FamilyPreloadLanes>;

impl PendingEpisodeLanes for FamilyPreloadLanes {
    type Sidecar = FamilySidecar;
    type Observer<'a> = FamilyPreloadObserver<'a>;

    fn fresh() -> Self {
        FamilyPreloadLanes::default()
    }

    fn observer(&self) -> FamilyPreloadObserver<'_> {
        FamilyPreloadObserver::new(self)
    }
}
