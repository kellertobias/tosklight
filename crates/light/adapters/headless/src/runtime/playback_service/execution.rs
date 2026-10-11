//! Accepted canonical assignment and direct Cuelist execution.
use super::*;
impl ServerPlaybackPorts<'_> {
    pub(super) fn execute_cue_list(
        &self,
        context: &ActionContext,
        id: light_core::CueListId,
        action: PlaybackAction,
    ) -> Result<PlaybackExecution, ActionError> {
        if matches!(action, PlaybackAction::Select { pressed: true }) {
            if !self
                .state
                .output
                .snapshot()
                .cue_lists
                .iter()
                .any(|list| list.id == id)
            {
                return Err(invalid("Cuelist does not exist"));
            }
            let show = self
                .state
                .active_show
                .current()
                .clone()
                .ok_or_else(|| invalid("no show is open"))?;
            let before = self
                .state
                .installation
                .selected_cue_list(context.desk_id, show.id)
                .map_err(|error| invalid(error.to_string()))?;
            self.state
                .installation
                .set_selected_cue_list(context.desk_id, show.id, Some(id))
                .map_err(|error| invalid(error.to_string()))?;
            return Ok(PlaybackExecution::Target {
                changed: before != Some(id),
            });
        }
        let command = match action {
            PlaybackAction::GoTo(cue_number) => CueListPlaybackAction::Jump(cue_number),
            PlaybackAction::Go { pressed: true } => CueListPlaybackAction::Go,
            PlaybackAction::Back { pressed: true } => CueListPlaybackAction::Back,
            PlaybackAction::Pause { pressed: true } => CueListPlaybackAction::Pause,
            PlaybackAction::Release => CueListPlaybackAction::Release,
            _ => return Err(invalid("action is incompatible with a cue list")),
        };
        let outcome = super::super::playback_solo::execute_engine(
            self.state,
            EnginePlaybackCommand::CueList {
                id,
                action: command,
            },
        )
        .map_err(api_action_error)?;
        let (execution, durable) = match outcome {
            EnginePlaybackOutcome::Active(active) => (PlaybackExecution::Active(active), true),
            EnginePlaybackOutcome::ActiveList { active, effect } => (
                PlaybackExecution::ActiveList {
                    active,
                    changed: effect.changed(),
                },
                effect.durable(),
            ),
            EnginePlaybackOutcome::Changed(effect) => (
                PlaybackExecution::Released(effect.changed()),
                effect.durable(),
            ),
            _ => {
                return Err(ActionError::new(
                    ActionErrorKind::Internal,
                    "unexpected cue-list Playback outcome",
                ));
            }
        };
        if durable && let Err(error) = persist_active_playbacks(self.state) {
            self.mark_persistence_pending(context, "active_playbacks", error);
        }
        Ok(execution)
    }

    pub(super) fn execute_playback(
        &self,
        context: &ActionContext,
        address: light_playback::PlaybackAddress,
        action: PlaybackAction,
        control_source: &light_playback::PlaybackControlSource,
    ) -> Result<PlaybackExecution, ActionError> {
        let definition = playback_definition(self.state, address)?;
        let (definition, action) = configured_control_definition(&definition, action)?;
        if let PlaybackAction::MasterTransition {
            level,
            duration_millis,
        } = action
        {
            if let light_playback::PlaybackTarget::Group { group_id, .. } = &definition.target {
                let changed = self
                    .state
                    .output
                    .set_group_master_transition(
                        group_id,
                        level.value(),
                        u64::from(duration_millis),
                    )
                    .map_err(|error| invalid(error.to_string()))?;
                if changed && let Err(error) = persist_output_runtime(self.state) {
                    self.mark_persistence_pending(context, "output_runtime", error);
                }
                return Ok(PlaybackExecution::Playback {
                    changed,
                    pending: None,
                });
            }
            let outcome = super::super::playback_solo::execute_engine(
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
            return Ok(PlaybackExecution::Playback {
                changed: effect.changed(),
                pending: None,
            });
        }
        if context.source == ActionSource::Matter
            && let PlaybackAction::Master(level) = action
            && matches!(
                definition.target,
                light_playback::PlaybackTarget::CueList { .. }
                    | light_playback::PlaybackTarget::Dynamic { .. }
            )
        {
            let outcome = super::super::playback_solo::execute_engine(
                self.state,
                EnginePlaybackCommand::Playback {
                    address,
                    action: light_engine::PlaybackAction::SetMasterWithExplicitActivation(
                        level.value(),
                    ),
                    control_source: control_source
                        .scoped_to_desk(context.desk_id)
                        .map_err(invalid)?,
                    activation_origin: Some(light_playback::PlaybackActivationOrigin {
                        at: self.state.output.application_time(),
                        desk_id: Some(context.desk_id),
                        surface: activation_surface(context.source),
                        exclusion_scope: light_playback::PlaybackExclusionScope::Show,
                    }),
                },
            )
            .map_err(api_action_error)?;
            let EnginePlaybackOutcome::Changed(effect) = outcome else {
                return Err(ActionError::new(
                    ActionErrorKind::Internal,
                    "unexpected explicit automation fader outcome",
                ));
            };
            if effect.durable()
                && let Err(error) = persist_active_playbacks(self.state)
            {
                self.mark_persistence_pending(context, "active_playbacks", error);
            }
            return Ok(PlaybackExecution::Playback {
                changed: effect.changed(),
                pending: None,
            });
        }
        let (action_name, input) = dispatch_action_input(action.clone());
        if !matches!(action, PlaybackAction::Release)
            && let Some(pending) = self.capture(
                context,
                address,
                &definition,
                action_name,
                &input,
                control_source,
            )?
        {
            return Ok(PlaybackExecution::Playback {
                changed: false,
                pending: Some(pending),
            });
        }
        if context.source == ActionSource::Osc
            && self.session.is_some_and(|session| {
                intercept_update_playback_target(
                    self.state,
                    session,
                    address,
                    &definition,
                    action_touched(action.clone()),
                )
            })
        {
            return Ok(PlaybackExecution::Playback {
                changed: false,
                pending: None,
            });
        }
        let activation_origin = Some(light_playback::PlaybackActivationOrigin {
            at: self.state.output.application_time(),
            desk_id: Some(context.desk_id),
            surface: activation_surface(context.source),
            exclusion_scope: light_playback::PlaybackExclusionScope::Show,
        });
        let dispatch = dispatch_playback_action(
            self.state,
            address,
            &definition,
            action_name,
            &input,
            PlaybackDispatchContext {
                action: context,
                session: self.session,
                desk: self.desk,
                source: source_name(context.source),
                control_source,
                activation_origin,
            },
        )
        .map_err(api_action_error)?;
        if dispatch.persistence_pending {
            self.persistence_pending
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if dispatch.addressed_event_required {
            self.addressed_event_required
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(PlaybackExecution::Playback {
            changed: dispatch.changed,
            pending: None,
        })
    }
}
