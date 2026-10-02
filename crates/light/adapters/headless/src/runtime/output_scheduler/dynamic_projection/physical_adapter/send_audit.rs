//! TL-548: compile-time thread audit of every physical adapter, lane and episode type.
//!
//! Positive rows use the `state/tests/worker.rs` pattern (`assert_send::<T>()`); negative rows
//! use an ambiguity probe that stops compiling as soon as the type gains the trait. A change to
//! either side therefore fails this module at compile time and forces a recorded decision.
//!
//! Findings (TL-548 C0/C1 handoff, then the TL-548 Send prerequisite):
//! - Descriptor and instance fit scratch is a `parking_lot::Mutex` (was `RefCell`), so every
//!   descriptor, including `PositionDescriptor` and its shared `Arc<PositionInstance>`, is
//!   `Send + Sync`. A lane caches descriptors as `Arc<Descriptor>`, so every production lane
//!   and lane composite is now `Send`.
//! - Adapters are `Send`. Only `OpticsAdapter` is also `Sync` (`Arc<Mutex>` shared state); the
//!   others keep lane-local `RefCell`/`Cell` caches, counters and tracking.
//! - Lanes stay `!Sync` by design: `RefCell<LaneState>` serves one synchronous frame. The owner
//!   supplies synchronization: `Mutex<FamilyLanes>` is `Send + Sync`, and the Pending episode
//!   bundle (lanes, retained evaluator state, paired history) can move to a worker thread.
//! - Owned outputs (sidecars, continuity, released owners) are `Send + Sync`.
use super::color_router::{RoutedColorContinuity, RoutedColorDescriptor, RoutingColorAdapter};
use super::family_lanes::{FamilyLanes, FamilyPreloadLanes, FamilySidecar};
use super::position::{PositionAdapter, PositionContinuity, PositionDescriptor};
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::retained_preload_hybrid::position_episode::{
    PositionPendingEpisode, PositionPendingSidecar,
};
use crate::runtime::output_scheduler::dynamic_projection::retained_preload_hybrid::{
    PendingHybridResult, RetainedPreloadHybridState,
};
use crate::runtime::preload::retained_history::paired::PairedPendingHistory;

fn assert_send<T: Send>() {}
fn assert_send_sync<T: Send + Sync>() {}

/// Compiles only while `$ty` does NOT implement `$trait` (two applicable impls are ambiguous).
macro_rules! assert_not_impl {
    ($ty:ty, $trait:path) => {{
        trait AmbiguousIfImpl<A> {
            fn probe() {}
        }
        impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
        #[allow(dead_code)]
        struct Implemented;
        impl<T: ?Sized + $trait> AmbiguousIfImpl<Implemented> for T {}
        let _ = <$ty as AmbiguousIfImpl<_>>::probe;
    }};
}

#[test]
fn adapters_are_send_and_only_the_optics_adapter_is_also_sync() {
    assert_send_sync::<OpticsAdapter>();
    assert_send::<ColorAdapter>();
    assert_send::<MediaColorAdapter>();
    assert_send::<RoutingColorAdapter>();
    // Instance cache: Arc<PositionInstance { scratch: Mutex<..> }>.
    assert_send::<PositionAdapter>();
    assert_not_impl!(ColorAdapter, Sync);
    assert_not_impl!(MediaColorAdapter, Sync);
    assert_not_impl!(RoutingColorAdapter, Sync);
    assert_not_impl!(PositionAdapter, Sync);
}

#[test]
fn descriptors_guard_their_fit_scratch_and_are_send_and_sync() {
    assert_send_sync::<OpticsDescriptor>();
    assert_send_sync::<ColorDescriptor>();
    assert_send_sync::<RoutedColorDescriptor>();
    assert_send_sync::<MediaColorDescriptor>();
    assert_send_sync::<PositionDescriptor>();
}

#[test]
fn every_lane_is_send_and_needs_its_owner_for_sharing() {
    assert_send::<PhysicalAdapterLane<MediaColorAdapter>>();
    assert_send::<PhysicalAdapterLane<PositionAdapter>>();
    assert_send::<PhysicalAdapterLane<RoutingColorAdapter>>();
    assert_send::<PhysicalAdapterLane<OpticsAdapter>>();
    assert_send::<PhysicalPreloadLanes<PositionAdapter>>();
    assert_send::<PhysicalPreloadLanes<RoutingColorAdapter>>();
    assert_send::<OpticsLanes>();
    assert_send::<OpticsPreloadLanes>();
    assert_send::<FamilyLanes>();
    assert_send::<FamilyPreloadLanes>();
    // Shared runtime resources hold the Live set behind a lock.
    assert_send_sync::<parking_lot::Mutex<FamilyLanes>>();
    assert_send_sync::<parking_lot::Mutex<FamilyPreloadLanes>>();
    // RefCell<LaneState> is !Sync by design: one lane serves one synchronous frame.
    assert_not_impl!(PhysicalAdapterLane<MediaColorAdapter>, Sync);
    assert_not_impl!(PhysicalAdapterLane<PositionAdapter>, Sync);
    assert_not_impl!(PhysicalAdapterLane<OpticsAdapter>, Sync);
    assert_not_impl!(OpticsLanes, Sync);
    assert_not_impl!(FamilyLanes, Sync);
    assert_not_impl!(FamilyPreloadLanes, Sync);
}

#[test]
fn a_pending_worker_can_own_the_whole_episode_bundle() {
    assert_send::<PositionPendingEpisode>();
    assert_send::<RetainedPreloadHybridState>();
    assert_send::<PairedPendingHistory<PendingHybridResult<PositionPendingSidecar>>>();
    assert_not_impl!(PositionPendingEpisode, Sync);
    // HybridFrameScratch keeps a lane-local RefCell verification cache.
    assert_not_impl!(RetainedPreloadHybridState, Sync);
}

#[test]
fn owned_frame_outputs_cross_threads() {
    assert_send_sync::<PhysicalHeadResult<PositionAdapter>>();
    assert_send_sync::<PhysicalHeadResult<RoutingColorAdapter>>();
    assert_send_sync::<PhysicalHeadResult<OpticsAdapter>>();
    assert_send_sync::<FamilySidecar>();
    assert_send_sync::<ReleasedPhysicalOwner>();
    assert_send_sync::<PositionContinuity>();
    assert_send_sync::<RoutedColorContinuity>();
    assert_send_sync::<OpticsContinuity>();
}
