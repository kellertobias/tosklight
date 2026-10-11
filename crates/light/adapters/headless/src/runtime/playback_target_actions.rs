use super::playback_persistence::PlaybackPersistencePlan;
use super::*;

use light_playback::{PlaybackButtonAction as Action, PlaybackTarget};

mod specialized;

use specialized::{apply_specialized_master, apply_specialized_target_action};

#[derive(Debug)]
pub(super) struct PlaybackTargetOutcome {
    pub(super) changed: bool,
    pub(super) addressed_event_required: bool,
    pub(super) persistence_pending: bool,
    pub(super) released_playbacks: Vec<light_playback::PlaybackAddress>,
    pub(super) persistence: PlaybackPersistencePlan,
}

impl PlaybackTargetOutcome {
    pub(super) fn changed(changed: bool) -> Self {
        Self {
            changed,
            addressed_event_required: false,
            persistence_pending: false,
            released_playbacks: Vec::new(),
            persistence: PlaybackPersistencePlan::none(),
        }
    }

    fn output_runtime(changed: bool) -> Self {
        Self {
            changed,
            addressed_event_required: false,
            persistence_pending: false,
            released_playbacks: Vec::new(),
            persistence: if changed {
                PlaybackPersistencePlan::output_runtime()
            } else {
                PlaybackPersistencePlan::none()
            },
        }
    }

    fn converged_output(result: &light_application::OutputRuntimeResult) -> Self {
        Self {
            changed: result.outcome == light_application::OutputRuntimeOutcome::Applied,
            addressed_event_required: false,
            persistence_pending: result.durability
                == light_application::OutputRuntimeDurability::PersistencePending,
            released_playbacks: Vec::new(),
            persistence: PlaybackPersistencePlan::none(),
        }
    }

    pub(super) fn combine(mut self, other: Self) -> Self {
        self.changed |= other.changed;
        self.addressed_event_required |= other.addressed_event_required;
        self.persistence_pending |= other.persistence_pending;
        self.released_playbacks.extend(other.released_playbacks);
        self.persistence = self.persistence.combine(other.persistence);
        self
    }
}

fn execute_addressed(
    state: &AppState,
    address: light_playback::PlaybackAddress,
    action: light_engine::PlaybackAction,
    control_source: &light_playback::PlaybackControlSource,
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    let outcome = super::playback_solo::execute_engine(
        state,
        EnginePlaybackCommand::Playback {
            address,
            action: action.clone(),
            activation_origin,
            control_source: control_source.clone(),
        },
    )?;
    let EnginePlaybackOutcome::Changed(effect) = outcome else {
        return Err(ApiError::internal("unexpected Playback outcome"));
    };
    Ok(PlaybackTargetOutcome {
        changed: effect.changed(),
        addressed_event_required: effect.addressed.changed()
            && may_change_unprojected_runtime(action),
        persistence_pending: false,
        released_playbacks: Vec::new(),
        persistence: PlaybackPersistencePlan::for_cuelist(effect.aggregate),
    })
}

fn may_change_unprojected_runtime(action: light_engine::PlaybackAction) -> bool {
    matches!(
        action,
        light_engine::PlaybackAction::On
            | light_engine::PlaybackAction::Load(_)
            | light_engine::PlaybackAction::SetMaster(_)
            | light_engine::PlaybackAction::SetConfiguredFader { .. }
            | light_engine::PlaybackAction::SetConfiguredFaderAbsolute { .. }
            | light_engine::PlaybackAction::XFade(_)
    )
}

pub(super) fn apply_playback_master(
    state: &AppState,
    context: &light_application::ActionContext,
    session: Option<&Session>,
    address: light_playback::PlaybackAddress,
    definition: &light_playback::PlaybackDefinition,
    input: &PoolPlaybackInput,
    absolute: bool,
    control_source: &light_playback::PlaybackControlSource,
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    if !definition.has_fader {
        return Err(ApiError::bad_request("playback does not have a fader"));
    }
    let value = input
        .value
        .ok_or_else(|| ApiError::bad_request("master value is required"))?;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(ApiError::bad_request("playback master must be within 0-1"));
    }
    if matches!(
        definition.target,
        PlaybackTarget::CueList { .. } | PlaybackTarget::Dynamic { .. }
    ) {
        let mut outcome = execute_addressed(
            state,
            address,
            if absolute {
                light_engine::PlaybackAction::SetConfiguredFaderAbsolute {
                    mode: definition.fader,
                    value,
                }
            } else {
                light_engine::PlaybackAction::SetConfiguredFader {
                    mode: definition.fader,
                    value,
                }
            },
            control_source,
            activation_origin,
        )?;
        if matches!(definition.target, PlaybackTarget::Dynamic { .. }) {
            outcome.persistence = outcome
                .persistence
                .combine(PlaybackPersistencePlan::output_runtime());
        }
        return Ok(outcome);
    }
    apply_specialized_master(
        state,
        context,
        session,
        address,
        definition,
        input,
        value,
        absolute,
        control_source,
    )
}

pub(super) fn apply_direct_playback_action(
    state: &AppState,
    address: light_playback::PlaybackAddress,
    definition: &light_playback::PlaybackDefinition,
    action: &str,
    input: &PoolPlaybackInput,
    control_source: &light_playback::PlaybackControlSource,
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<Option<PlaybackTargetOutcome>, ApiError> {
    let cue = || {
        input
            .cue_number
            .as_deref()
            .ok_or_else(|| ApiError::bad_request("cue_number is required"))
            .and_then(|number| {
                number
                    .parse::<light_playback::CueNumber>()
                    .map_err(ApiError::bad_request)
            })
    };
    let outcome = match action {
        "go-to" => execute_addressed(
            state,
            address,
            light_engine::PlaybackAction::GoTo(cue()?),
            control_source,
            activation_origin,
        )?,
        "load" => execute_addressed(
            state,
            address,
            light_engine::PlaybackAction::Load(cue()?),
            control_source,
            activation_origin,
        )?,
        "xfade-on" | "xfade-off" => execute_addressed(
            state,
            address,
            light_engine::PlaybackAction::XFade(action == "xfade-on"),
            control_source,
            activation_origin,
        )?,
        "temp-on" | "temp-off" => {
            if !matches!(definition.target, PlaybackTarget::CueList { .. }) {
                return Err(ApiError::bad_request(
                    "Temp is available only for a Cuelist playback",
                ));
            }
            execute_addressed(
                state,
                address,
                light_engine::PlaybackAction::SetTempButton(action == "temp-on"),
                control_source,
                activation_origin,
            )?
        }
        "dynamic-restart" => execute_addressed(
            state,
            address,
            light_engine::PlaybackAction::DynamicRestart,
            control_source,
            activation_origin,
        )?,
        "dynamic-double-speed" => execute_addressed(
            state,
            address,
            light_engine::PlaybackAction::DynamicDoubleSpeed,
            control_source,
            activation_origin,
        )?,
        "dynamic-half-speed" => execute_addressed(
            state,
            address,
            light_engine::PlaybackAction::DynamicHalfSpeed,
            control_source,
            activation_origin,
        )?,
        _ => return Ok(None),
    };
    Ok(Some(outcome))
}

pub(super) fn select_playback_target(
    state: &AppState,
    desk: Option<&ControlDesk>,
    address: light_playback::PlaybackAddress,
    definition: &light_playback::PlaybackDefinition,
    action: Action,
) -> Result<bool, ApiError> {
    if action != Action::Select
        || !matches!(
            definition.target,
            PlaybackTarget::CueList { .. } | PlaybackTarget::Group { .. }
        )
    {
        return Ok(false);
    }
    let desk = desk.ok_or_else(|| ApiError::bad_request("playback selection needs a desk"))?;
    let show = state
        .active_show
        .current()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let selected = state
        .installation
        .selected_playback(desk.id, show.id)
        .map_err(ApiError::store)?;
    let selected_cue_list = state
        .installation
        .selected_cue_list(desk.id, show.id)
        .map_err(ApiError::store)?;
    state
        .installation
        .set_selected_cue_list(desk.id, show.id, None)
        .map_err(ApiError::store)?;
    if selected
        == Some(light_show::PlaybackSelection {
            page: address.page(),
            position: address.position(),
        })
    {
        return Ok(selected_cue_list.is_some());
    }
    state
        .installation
        .set_selected_playback(
            desk.id,
            show.id,
            Some(light_show::PlaybackSelection {
                page: address.page(),
                position: address.position(),
            }),
        )
        .map_err(ApiError::store)?;
    Ok(true)
}

fn apply_cuelist_action(
    state: &AppState,
    session: Option<&Session>,
    address: light_playback::PlaybackAddress,
    definition: &light_playback::PlaybackDefinition,
    cue_list_id: light_core::CueListId,
    action: Action,
    pressed: bool,
    control_source: &light_playback::PlaybackControlSource,
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    let command = match action {
        Action::On => Some(light_engine::PlaybackAction::On),
        Action::Off => Some(light_engine::PlaybackAction::Off),
        Action::Toggle => Some(light_engine::PlaybackAction::Toggle),
        Action::Go => Some(light_engine::PlaybackAction::Go),
        Action::GoMinus => Some(light_engine::PlaybackAction::Back),
        Action::Pause => Some(light_engine::PlaybackAction::TogglePause),
        Action::FastForward => Some(light_engine::PlaybackAction::FastForward),
        Action::FastRewind => Some(light_engine::PlaybackAction::FastRewind),
        Action::Flash => Some(light_engine::PlaybackAction::SetFlash(pressed)),
        Action::Temp => Some(light_engine::PlaybackAction::ToggleTemp),
        Action::Swap => Some(light_engine::PlaybackAction::SetSwap(pressed)),
        Action::Select => None,
        Action::SelectContents => {
            let session =
                session.ok_or_else(|| ApiError::bad_request("selection needs a session"))?;
            select_cuelist_contents(state, session, cue_list_id)?;
            None
        }
        Action::None => return Ok(PlaybackTargetOutcome::changed(false)),
        _ => {
            return Err(ApiError::bad_request(
                "action is incompatible with a Cuelist playback",
            ));
        }
    };
    if let Some(command) = command {
        return execute_addressed(state, address, command, control_source, activation_origin);
    }
    Ok(PlaybackTargetOutcome::changed(action != Action::Select))
}

fn apply_dynamic_action(
    state: &AppState,
    address: light_playback::PlaybackAddress,
    definition: &light_playback::PlaybackDefinition,
    action: Action,
    pressed: bool,
    control_source: &light_playback::PlaybackControlSource,
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    let command = match action {
        Action::On => light_engine::PlaybackAction::On,
        Action::Off => light_engine::PlaybackAction::Off,
        Action::Toggle => light_engine::PlaybackAction::Toggle,
        Action::Pause => light_engine::PlaybackAction::TogglePause,
        Action::Flash => light_engine::PlaybackAction::SetFlash(pressed),
        Action::DynamicRestart => light_engine::PlaybackAction::DynamicRestart,
        Action::DynamicDoubleSpeed => light_engine::PlaybackAction::DynamicDoubleSpeed,
        Action::DynamicHalfSpeed => light_engine::PlaybackAction::DynamicHalfSpeed,
        Action::DynamicLearnSpeed => light_engine::PlaybackAction::DynamicLearnSpeed,
        Action::None => return Ok(PlaybackTargetOutcome::changed(false)),
        _ => {
            return Err(ApiError::bad_request(
                "action is incompatible with a Dynamic playback",
            ));
        }
    };
    let mut outcome =
        execute_addressed(state, address, command, control_source, activation_origin)?;
    outcome.persistence = outcome
        .persistence
        .combine(PlaybackPersistencePlan::output_runtime());
    Ok(outcome)
}

pub(super) fn apply_playback_target_action(
    state: &AppState,
    context: &light_application::ActionContext,
    session: Option<&Session>,
    address: light_playback::PlaybackAddress,
    definition: &light_playback::PlaybackDefinition,
    action: Action,
    input: &PoolPlaybackInput,
    pressed: bool,
    control_source: &light_playback::PlaybackControlSource,
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    if let PlaybackTarget::CueList { cue_list_id } = &definition.target {
        return apply_cuelist_action(
            state,
            session,
            address,
            definition,
            *cue_list_id,
            action,
            pressed,
            control_source,
            activation_origin,
        );
    }
    if matches!(definition.target, PlaybackTarget::Dynamic { .. }) {
        return apply_dynamic_action(
            state,
            address,
            definition,
            action,
            pressed,
            control_source,
            activation_origin,
        );
    }
    apply_specialized_target_action(
        state,
        context,
        session,
        address,
        definition,
        action,
        input,
        pressed,
        control_source,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_outcomes_schedule_only_real_output_changes() {
        assert_eq!(
            PlaybackTargetOutcome::output_runtime(true).persistence,
            PlaybackPersistencePlan::output_runtime()
        );
        assert_eq!(
            PlaybackTargetOutcome::output_runtime(false).persistence,
            PlaybackPersistencePlan::none()
        );
    }

    #[test]
    fn interaction_changes_never_gain_runtime_persistence() {
        let outcome =
            PlaybackTargetOutcome::changed(false).combine(PlaybackTargetOutcome::changed(true));

        assert!(outcome.changed);
        assert_eq!(outcome.persistence, PlaybackPersistencePlan::none());
    }
}
