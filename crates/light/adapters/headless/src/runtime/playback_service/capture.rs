//! Preload capture and accepted-pending persistence reporting.

use super::*;

impl ServerPlaybackPorts<'_> {
    pub(super) fn capture(
        &self,
        context: &ActionContext,
        definition: &light_playback::PlaybackDefinition,
        action_name: &str,
        input: &PoolPlaybackInput,
        surface: PlaybackSurface,
        page: Option<u8>,
    ) -> Result<Option<PendingPlaybackAction>, ActionError> {
        let Some(session) = self.session else {
            return Ok(None);
        };
        let pending = if matches!(action_name, "master" | "fader")
            && matches!(
                definition.target,
                light_playback::PlaybackTarget::Dynamic { .. }
            ) {
            input
                .value
                .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                .map(
                    |value| light_programmer::PreloadPlaybackQueueAction::Fader {
                        value_permyriad: (value * 10_000.0).round() as u16,
                    },
                )
        } else {
            let temp = predicted_preload_temp_state(self.state, session.id, definition.number);
            preload_capture_action_with_temp_state(definition, action_name, input, temp)
                .map_err(api_action_error)?
                .map(light_programmer::PreloadPlaybackQueueAction::try_from)
                .transpose()
                .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error))?
        };
        if !self.should_capture(session, pending.as_ref(), surface) {
            return Ok(None);
        }
        let pending = pending.expect("capture requires a pending action");
        self.queue_capture(context, session, definition.number, pending, surface, page)?;
        Ok(Some(parse_pending(pending)))
    }

    fn should_capture(
        &self,
        session: &Session,
        pending: Option<&light_programmer::PreloadPlaybackQueueAction>,
        surface: PlaybackSurface,
    ) -> bool {
        self.state
            .programming
            .capture_mode(session.id)
            .is_some_and(|mode| mode.blind)
            && pending.is_some()
            && capture_enabled(self.state, surface)
    }

    fn queue_capture(
        &self,
        context: &ActionContext,
        session: &Session,
        number: u16,
        pending: light_programmer::PreloadPlaybackQueueAction,
        surface: PlaybackSurface,
        page: Option<u8>,
    ) -> Result<(), ActionError> {
        let queue_surface =
            light_programmer::PreloadPlaybackQueueSurface::try_from(surface_name(surface))
                .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error))?;
        self.state
            .programming
            .queue_preload_playback_action_with_origin(
                session.id,
                number,
                page,
                pending,
                queue_surface,
                Some(session.desk.id),
            );
        if let Err(error) = persist_programmer(self.state, session) {
            self.mark_persistence_pending(context, "programmer", error);
        }
        emit(
            self.state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id,"preload_playback_action":pending.legacy_name(),"playback_number":number,"surface":surface_name(surface),"page":page,"changes":["preload_playback_queue"]}),
        );
        Ok(())
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
