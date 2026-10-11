//! Engine-owned master transitions and their target-specific checkpoint boundary.
use super::*;
impl ServerPlaybackPorts<'_> {
    pub(super) fn execute_master_transition(
        &self,
        context: &ActionContext,
        address: light_playback::PlaybackAddress,
        definition: &light_playback::PlaybackDefinition,
        level: light_application::PlaybackLevel,
        duration_millis: u32,
        control_source: &light_playback::PlaybackControlSource,
    ) -> Result<PlaybackExecution, ActionError> {
        if let light_playback::PlaybackTarget::Group { group_id, .. } = &definition.target {
            let changed = self
                .state
                .output
                .set_group_master_transition(group_id, level.value(), u64::from(duration_millis))
                .map_err(|error| invalid(error.to_string()))?;
            if changed && let Err(error) = persist_output_runtime(self.state) {
                self.mark_persistence_pending(context, "output_runtime", error);
            }
            return Ok(PlaybackExecution::Playback {
                changed,
                pending: None,
            });
        }
        let outcome = crate::runtime::playback_solo::execute_engine(
            self.state,
            EnginePlaybackCommand::Playback {
                address,
                action: light_engine::PlaybackAction::SetMasterTransition {
                    value: level.value(),
                    duration_millis: u64::from(duration_millis),
                },
                activation_origin: None,
                control_source: control_source
                    .scoped_to_desk(context.desk_id)
                    .map_err(invalid)?,
            },
        )
        .map_err(api_action_error)?;
        let EnginePlaybackOutcome::Changed(effect) = outcome else {
            return Err(ActionError::new(
                ActionErrorKind::Internal,
                "unexpected master-transition outcome",
            ));
        };
        if effect.durable()
            && let Err(error) = persist_active_playbacks(self.state)
        {
            self.mark_persistence_pending(context, "active_playbacks", error);
        }
        Ok(PlaybackExecution::Playback {
            changed: effect.changed(),
            pending: None,
        })
    }
}
