//! Pair the prepared Engine snapshot with its installed Dynamic definition registry.
//! Publication may briefly expose the Engine before its registry; a frame encountering that
//! gap keeps the previous output and retries before advancing any Dynamic state.

use light_engine::EngineSnapshot;
use parking_lot::{Mutex, MutexGuard};
use std::sync::Arc;

mod cold_history;
use cold_history::ColdGenerationLog;
pub(in crate::runtime) use cold_history::{
    ColdGenerationCursor, ColdGenerationEvent, InputCaptureCursor, RetainedFrameCapture,
    RetainedInputBorrow, RetainedInputCapture,
};

pub(super) struct DynamicSnapshotPublication {
    installation: Mutex<()>,
    installed: arc_swap::ArcSwap<EngineSnapshot>,
    cold_history: Mutex<Option<ColdGenerationLog>>,
    retaining: std::sync::atomic::AtomicBool,
}

impl DynamicSnapshotPublication {
    /// Startup has already prepared both objects from this exact snapshot.
    pub(super) fn new(snapshot: Arc<EngineSnapshot>) -> Self {
        Self {
            installation: Mutex::new(()),
            installed: arc_swap::ArcSwap::from(snapshot),
            cold_history: Mutex::new(None),
            retaining: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Serialize publishers from before the Engine swap through the Dynamic registry swap.
    /// Take this before either engine/playback or Dynamics guards; never from the output loop.
    pub(super) fn begin_install(&self) -> MutexGuard<'_, ()> {
        self.installation.lock()
    }

    /// Call while holding the Dynamics lock, after installing the matching prepared registry.
    pub(super) fn installed(&self, snapshot: Arc<EngineSnapshot>) {
        // An install without a retained cold event cannot be replayed by existing episodes.
        // Invalidate their epoch instead of silently applying a partial history.
        if let Some(history) = self.cold_history.lock().as_mut() {
            history.reset();
        }
        self.installed.store(snapshot);
    }

    /// Read under the same Dynamics lock before beginning the frame transaction. Group-master
    /// generation changes retain the snapshot Arc and therefore need no registry republish.
    pub(super) fn matches(&self, snapshot: &Arc<EngineSnapshot>) -> bool {
        Arc::ptr_eq(&self.installed.load(), snapshot)
    }
}
