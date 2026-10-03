//! The desk-facing owner of the Pending executor and its lifecycle hooks.
//!
//! It lives on `ProgrammingResource` (shared by every staged/detached copy), so the Preload
//! release path can signal it without application state. Every hook is a no-op until
//! [`PendingEpisodeResource::start_if_gated`] started a worker, which happens only when the
//! family-adapter gate holds. The gate is C3's: `OutputResource::live_family_adapters()`
//! carries the explicit opt-in and checks the engine contract (`LiveFamilyAdapters::engaged`),
//! so Live injection and the Pending executor can never be enabled independently.
use super::{
    PendingEpisodeExecutor, PendingEpisodeSources, PendingEpisodeStatus, PendingTrigger,
    family_adapters_gated,
};
use crate::runtime::AppState;
use crate::runtime::position_readout::PendingPositionReadoutSource;
use light_application::programming::preload_preview_demand;
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone, Default)]
pub(in crate::runtime) struct PendingEpisodeResource {
    executor: Arc<Mutex<Option<Arc<PendingEpisodeExecutor>>>>,
}

impl PendingEpisodeResource {
    /// The C3/C4 gate of this desk: contract support AND the explicit family-adapter opt-in.
    pub(in crate::runtime) fn gated(state: &AppState) -> bool {
        let output = &state.output;
        let opted_in = output.live_family_adapters().engaged(output.engine());
        family_adapters_gated(output.engine(), opted_in)
    }

    /// Start the worker and install its Pending Position readout source when gated. Idempotent;
    /// production never opts in, so it returns without starting anything.
    pub(in crate::runtime) fn start_if_gated(&self, state: &AppState) -> bool {
        if !Self::gated(state) {
            return false;
        }
        let mut executor = self.executor.lock();
        if executor.is_some() {
            return true;
        }
        let handles = state.output.pending_episode_handles();
        let active = state.active_show.output_projection();
        let sources = PendingEpisodeSources {
            engine: handles.engine,
            dynamics: handles.dynamics,
            publication: handles.publication,
            origins: handles.origins,
            preload: {
                let programmers = state.programming.programmers();
                Arc::new(move || preload_preview_demand(&programmers))
            },
            show: Arc::new(move || active.current().map(|show| show.id)),
        };
        match PendingEpisodeExecutor::spawn(sources) {
            Ok(started) => {
                let started = Arc::new(started);
                let source: Arc<dyn PendingPositionReadoutSource> = started.readouts();
                state.output.pending_position_readouts().install(source);
                *executor = Some(started);
                true
            }
            Err(error) => {
                tracing::warn!(%error, "Pending episode worker could not start");
                false
            }
        }
    }

    /// Teardown: clear the readout source, shut the worker down and end its episode.
    pub(in crate::runtime) fn stop(&self, state: &AppState) {
        if let Some(executor) = self.executor.lock().take() {
            state.output.pending_position_readouts().clear();
            drop(executor);
        }
    }

    /// Lifecycle hook (GO, clear/release, show activation). A no-op without a worker.
    pub(in crate::runtime) fn trigger(&self, trigger: PendingTrigger) {
        if let Some(executor) = self.executor.lock().as_ref() {
            executor.trigger(trigger);
        }
    }

    /// Output tick or retained input capture. A no-op without a worker.
    pub(in crate::runtime) fn wake(&self) {
        if let Some(executor) = self.executor.lock().as_ref() {
            executor.wake();
        }
    }

    pub(in crate::runtime) fn status(&self) -> Option<PendingEpisodeStatus> {
        self.executor
            .lock()
            .as_ref()
            .map(|executor| executor.status())
    }

    pub(in crate::runtime) fn executor(&self) -> Option<Arc<PendingEpisodeExecutor>> {
        self.executor.lock().clone()
    }
}
