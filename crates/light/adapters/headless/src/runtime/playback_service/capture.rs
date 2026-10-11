//! Preload capture and accepted-pending persistence reporting.

use super::*;

impl ServerPlaybackPorts<'_> {
    pub(super) fn capture(
        &self,
        context: &ActionContext,
        address: light_playback::PlaybackAddress,
        definition: &light_playback::PlaybackDefinition,
        action_name: &str,
        input: &PoolPlaybackInput,
        control_source: &light_playback::PlaybackControlSource,
    ) -> Result<Option<PendingPlaybackAction>, ActionError> {
        let Some(session) = self.session else {
            return Ok(None);
        };
        if !self
            .state
            .programming
            .capture_mode(session.id)
            .is_some_and(|mode| mode.blind)
            || !capture_enabled(self.state, address)
        {
            return Ok(None);
        }
        let temp = predicted_preload_temp_state(self.state, session.id, address);
        let Some(pending) =
            preload_capture_action_with_temp_state(definition, action_name, input, temp)
                .map_err(api_action_error)?
                .map(light_programmer::PreloadPlaybackQueueAction::try_from)
                .transpose()
                .map_err(invalid)?
        else {
            return Ok(None);
        };
        control_source.validate().map_err(invalid)?;
        if !self
            .state
            .programming
            .queue_preload_playback_action_with_origin(
                session.id,
                address,
                pending,
                control_source.clone(),
                Some(context.desk_id),
            )
        {
            return Err(invalid("Preload capture state is unavailable"));
        }
        if let Err(error) = persist_programmer(self.state, session) {
            self.mark_persistence_pending(context, "programmer", error);
        }
        emit(
            self.state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id,"preload_playback_action":pending.legacy_name(),"address":address,"control_source":control_source,"changes":["preload_playback_queue"]}),
        );
        Ok(Some(parse_pending(pending)))
    }

    pub(super) fn mark_persistence_pending(
        &self,
        context: &ActionContext,
        domain: &str,
        error: ApiError,
    ) {
        self.persistence_pending
            .store(true, std::sync::atomic::Ordering::Relaxed);
        emit(
            self.state,
            "playback_persistence_pending",
            serde_json::json!({
                "desk_id": context.desk_id,
                "session_id": context.session_id,
                "correlation_id": context.correlation_id,
                "source": source_name(context.source),
                "failures": [{"domain":domain,"error":error.message}],
            }),
        );
    }
}
