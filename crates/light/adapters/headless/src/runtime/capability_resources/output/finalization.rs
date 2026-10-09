use super::*;
use light_application::{ActionError, ActionErrorKind, PlaybackOperation};

impl OutputResource {
    /// Finalize native dependencies against the current running state, before persistence.
    ///
    /// The caller already owns activation and Programmer serialization. Publication, ordered
    /// Playback and Dynamics remain held from capture through commit and installation; a cold
    /// fork can therefore never overwrite a phase/history change made after its capture. The
    /// persistence callback must not call back into these runtime boundaries. Selection and
    /// Highlight callbacks belong after this method returns.
    ///
    /// Native sources, retained history, destination controllers/targets and one atomic Preset
    /// dependency batch are validated before persistence. Failed candidates never reach Live.
    pub(in crate::runtime) fn finalize_snapshot<T>(
        &self,
        playback: &PlaybackRenderCapability,
        prepared: PreparedOutputSnapshot,
        persist: impl FnOnce() -> Result<T, ActionError>,
    ) -> Result<T, ActionError> {
        let _publication = self.dynamic_snapshot.begin_install();
        output_scheduler::ordered_output_operation(playback, || {
            PlaybackOperation::new(self.finalize_snapshot_ordered(prepared, persist))
        })
    }

    fn finalize_snapshot_ordered<T>(
        &self,
        prepared: PreparedOutputSnapshot,
        persist: impl FnOnce() -> Result<T, ActionError>,
    ) -> Result<T, ActionError> {
        let revision = prepared.snapshot().revision;
        let invalid = |message: String| {
            ActionError::new(ActionErrorKind::Invalid, message).at_revision(revision)
        };
        let previous = self.engine.snapshot();
        let engine = self
            .engine
            .finalize_snapshot_playback_with_replacement_migrations(
                prepared.engine,
                &prepared.replacement_migrations,
            )
            .map_err(|error| invalid(error.to_string()))?;
        let snapshot = engine.snapshot_arc();
        let programmer_values = self.engine.dynamic_programmer_values();
        let mut live = self.dynamics.lock();
        let cold_boundary = self.dynamic_snapshot.cold_boundary(&live);
        let mut candidate = live.fork_for_cold_install();
        // Deleted unused definitions must not veto destination validation. Running/pinned
        // instances retained by installation still validate against the new provider below.
        candidate.install_prepared_definitions(prepared.definitions);
        candidate
            .refresh_native_color_models(snapshot.native_color_sources.clone())
            .map_err(|error| invalid(error.to_string()))?;
        output_scheduler::reconcile_cold_dynamic_candidate(
            &mut candidate,
            output_scheduler::ColdDynamicReconciliationInputs {
                captured_at_millis: u64::try_from(engine.sampled_at().timestamp_millis())
                    .unwrap_or_default(),
                snapshot: &snapshot,
                programmer_values: &programmer_values,
                extra_programmer_values: &[],
                cue_values: engine.cue_dynamic_values(),
                playbacks: engine.dynamic_playbacks(),
                playback_paused: engine.playback_dynamics_paused(),
            },
        )
        .map_err(|error| invalid(error.to_string()))?;
        output_scheduler::materialize_cold_preset_dependencies(
            &previous,
            &snapshot,
            &mut candidate,
        )
        .map_err(|error| invalid(error.to_string()))?;
        let cold_event = cold_boundary
            .map(|boundary| boundary.prepare(previous.clone(), snapshot.clone(), &candidate));
        let committed = persist()?;
        self.engine.install_finalized_snapshot(engine);
        *live = candidate;
        self.dynamic_snapshot
            .installed_with_cold_event(snapshot, cold_event);
        Ok(committed)
    }
}
