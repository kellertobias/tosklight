//! Final ownership validation after startup has normalized restored Playback rows.
use super::*;
use crate::runtime::dynamic_source_origins::DynamicSourceBinding;
use light_application::PlaybackOperation;

impl OutputResource {
    /// Reconcile the restored checkpoint against the final desk owners without advancing time.
    ///
    /// Startup calls this before workers begin. Any later caller must also serialize Programmer
    /// edits. A rejected candidate leaves runtime, origins and retained-history cursors intact;
    /// the caller owns preserved-data recovery before output is allowed to begin.
    pub(in crate::runtime) fn finalize_restored_owners(
        &self,
        playback: &PlaybackRenderCapability,
    ) -> Result<(), IntentError> {
        let _publication = self.dynamic_snapshot.begin_install();
        output_scheduler::ordered_output_operation(playback, || {
            PlaybackOperation::new(self.finalize_restored_owners_ordered())
        })
    }

    fn finalize_restored_owners_ordered(&self) -> Result<(), IntentError> {
        let snapshot = self.engine.snapshot();
        let captured_at_millis =
            u64::try_from(self.engine.application_time().timestamp_millis()).unwrap_or_default();
        let programmer_values = self.engine.retained_dynamic_programmer_values();
        let cue_values = self.engine.active_cue_dynamic_values();
        let playbacks = self.engine.active_dynamic_playbacks();
        let playback_paused = self.engine.playback_dynamics().paused;
        let mut live = self.dynamics.lock();
        if !self.dynamic_snapshot.matches(&snapshot) {
            return Err(IntentError(
                "restored Dynamic registry does not match the final Engine snapshot".into(),
            ));
        }
        let mut candidate = live.fork_for_cold_install();
        output_scheduler::reconcile_cold_dynamic_candidate(
            &mut candidate,
            output_scheduler::ColdDynamicReconciliationInputs {
                captured_at_millis,
                snapshot: &snapshot,
                programmer_values: &programmer_values,
                extra_programmer_values: &[],
                cue_values: &cue_values,
                playbacks: &playbacks,
                playback_paused,
            },
        )
        .map_err(|error| IntentError(error.to_string()))?;
        output_scheduler::materialize_pending_preset_dependencies(&snapshot, &mut candidate)
            .map_err(|error| IntentError(error.to_string()))?;

        // Retire active lookups only. Immutable records may still be referenced by surviving
        // held expressions, and fixed/static assignments have no Dynamic controller owner.
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
            DynamicSourceBinding::Fixed { .. } | DynamicSourceBinding::StaticBaseline { .. } => {
                true
            }
        });
        *live = candidate;
        self.dynamic_source_origins.store(Arc::new(origins));
        self.dynamic_snapshot.installed(snapshot);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
