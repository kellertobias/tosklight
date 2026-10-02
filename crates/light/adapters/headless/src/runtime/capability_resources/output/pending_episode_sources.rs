//! TL-548 C4 hook: the Live handles the Pending episode worker reads. Kept in its own file so
//! `output.rs` (owned by the C3 Live-injection work) carries only the one `mod` line.
use super::*;

/// The desk engine and the Live Dynamics runtime, retained-input publication and source
/// catalogue, exactly as the Live output transaction uses them. All `Send + Sync`.
pub(in crate::runtime) struct OutputPendingEpisodeHandles {
    pub(in crate::runtime) engine: Arc<Engine>,
    pub(in crate::runtime) dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    pub(in crate::runtime) publication: Arc<DynamicSnapshotPublication>,
    pub(in crate::runtime) origins: SharedDynamicSourceOrigins,
}

impl OutputResource {
    pub(in crate::runtime) fn pending_episode_handles(&self) -> OutputPendingEpisodeHandles {
        OutputPendingEpisodeHandles {
            engine: Arc::clone(&self.engine),
            dynamics: Arc::clone(&self.dynamics),
            publication: Arc::clone(&self.dynamic_snapshot),
            origins: Arc::clone(&self.dynamic_source_origins),
        }
    }
}
