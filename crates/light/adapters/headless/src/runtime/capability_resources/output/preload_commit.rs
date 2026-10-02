//! Atomic GO preparation under the existing ordered Playback operation.
use super::*;
use crate::runtime::dynamic_source_origins::DynamicSourceBinding;

impl OutputResource {
    /// Caller owns active-show, Programmer and ordered Playback serialization. Capture all
    /// Live inputs before the Engine takes its Playback write lock. The preparation callback
    /// may use only its supplied candidates and already captured ancillary data: calling a
    /// Live Playback/Dynamics getter inside it would self-deadlock.
    ///
    /// No publication lease is acquired here: that would reverse the outer ordered gate.
    /// All Result failures occur before installation. Both Playback and Dynamics remain
    /// guarded through the infallible install tail; no new sample or clock tick is performed.
    pub(in crate::runtime) fn install_preload_playback_batch<T>(
        &self,
        prepared: light_engine::PreparedPlaybackBatch,
        committed_at: chrono::DateTime<chrono::Utc>,
        prepare: impl FnOnce(
            &EngineSnapshot,
            &light_playback::PlaybackEngine,
            &light_dynamics::DynamicRuntimeSnapshot,
        ) -> Result<T, String>,
    ) -> Result<(T, bool), String> {
        let snapshot = self.engine.snapshot();
        let programmer_values = self.engine.dynamic_programmer_values();
        let captured_at_millis = u64::try_from(committed_at.timestamp_millis()).unwrap_or_default();
        self.engine.install_prepared_playback_batch_with(
            prepared,
            |playback| {
                let live = self.dynamics.lock();
                if !self.dynamic_snapshot.matches(&snapshot) {
                    return Err(
                        "Dynamic registry changed while Preload GO was being prepared".into(),
                    );
                }
                let before = live.snapshot();
                let mut candidate = live.fork_for_cold_install();
                candidate.set_definitions_pinned(false);
                let cue_values = playback.active_cue_dynamic_values();
                let playbacks = playback.active_dynamic_playbacks();
                output_scheduler::reconcile_cold_dynamic_candidate(
                    &mut candidate,
                    output_scheduler::ColdDynamicReconciliationInputs {
                        captured_at_millis,
                        snapshot: &snapshot,
                        programmer_values: &programmer_values,
                        extra_programmer_values: &[],
                        cue_values: &cue_values,
                        playbacks: &playbacks,
                        playback_paused: playback.dynamics_paused(),
                    },
                )
                .map_err(|error| error.to_string())?;
                output_scheduler::materialize_pending_preset_dependencies(
                    &snapshot,
                    &mut candidate,
                )
                .map_err(|error| error.to_string())?;
                let changed = before != candidate.snapshot();
                let projection = candidate.output_projection_snapshot();
                let active = projection
                    .instances
                    .iter()
                    .flat_map(|instance| {
                        instance
                            .controllers
                            .iter()
                            .map(move |controller| (instance.id, controller.id))
                    })
                    .collect::<std::collections::HashSet<_>>();
                let mut origins = (**self.dynamic_source_origins.load()).clone();
                origins.retain_bindings(|record| match record.binding {
                    DynamicSourceBinding::Authored {
                        instance_id,
                        controller_id,
                        ..
                    } => active.contains(&(instance_id, controller_id)),
                    DynamicSourceBinding::Fixed { .. }
                    | DynamicSourceBinding::StaticBaseline { .. } => true,
                });
                let origins = Arc::new(origins);
                let result = prepare(&snapshot, playback, &projection)?;
                Ok((live, candidate, origins, result, changed))
            },
            |(mut live, candidate, origins, result, changed)| {
                *live = candidate;
                self.dynamic_source_origins.store(origins);
                // GO ends the pending episode. Until pin changes have replay events, make this
                // an explicit lineage break while the exact installed runtime is still guarded.
                self.dynamic_snapshot.installed(snapshot.clone());
                (result, changed)
            },
        )
    }
}

#[cfg(test)]
mod tests;
