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
    pub(super) released_playbacks: Vec<u16>,
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

fn execute_pool_with_exclusions(
    state: &AppState,
    number: u16,
    action: PoolPlaybackAction,
    exclusion_zones: &[Vec<u16>],
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    let transition = state
        .engine
        .execute_pool_playback_with_activation(number, action, exclusion_zones, activation_origin)
        .map_err(ApiError::bad_request)?;
    let EnginePlaybackOutcome::Changed(effect) = transition.outcome else {
        return Err(ApiError::internal("unexpected pool Playback outcome"));
    };
    Ok(PlaybackTargetOutcome {
        changed: effect.changed(),
        addressed_event_required: effect.addressed.changed()
            && may_change_unprojected_runtime(action),
        persistence_pending: false,
        released_playbacks: transition.released_playbacks,
        persistence: PlaybackPersistencePlan::for_cuelist(effect.aggregate),
    })
}

const fn may_change_unprojected_runtime(action: PoolPlaybackAction) -> bool {
    matches!(
        action,
        PoolPlaybackAction::On
            | PoolPlaybackAction::Load(_)
            | PoolPlaybackAction::SetMaster(_)
            | PoolPlaybackAction::SetVirtualMaster(_)
            | PoolPlaybackAction::XFade(_)
    )
}

pub(super) fn apply_playback_master(
    state: &AppState,
    context: &light_application::ActionContext,
    session: Option<&Session>,
    definition: &light_playback::PlaybackDefinition,
    input: &PoolPlaybackInput,
    source: &str,
    exclusion_zones: &[Vec<u16>],
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    let virtual_fader = source == "matter" && !definition.has_fader;
    if !definition.has_fader && !virtual_fader {
        return Err(ApiError::bad_request("playback does not have a fader"));
    }
    let value = input
        .value
        .ok_or_else(|| ApiError::bad_request("master value is required"))?;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(ApiError::bad_request("playback master must be within 0-1"));
    }
    if matches!(definition.target, PlaybackTarget::CueList { .. }) {
        return if virtual_fader {
            execute_pool_with_exclusions(
                state,
                definition.number,
                PoolPlaybackAction::SetVirtualMaster(value),
                exclusion_zones,
                activation_origin,
            )
        } else {
            execute_pool_with_exclusions(
                state,
                definition.number,
                PoolPlaybackAction::SetMaster(value),
                exclusion_zones,
                activation_origin,
            )
        };
    }
    apply_specialized_master(state, context, session, definition, input, value)
}

pub(super) fn apply_direct_playback_action(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    action: &str,
    input: &PoolPlaybackInput,
    exclusion_zones: &[Vec<u16>],
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<Option<PlaybackTargetOutcome>, ApiError> {
    let cue = || {
        input
            .cue_number
            .ok_or_else(|| ApiError::bad_request("cue_number is required"))
    };
    let outcome = match action {
        "go-to" => execute_pool_with_exclusions(
            state,
            definition.number,
            PoolPlaybackAction::GoTo(cue()?),
            exclusion_zones,
            activation_origin,
        )?,
        "load" => execute_pool_with_exclusions(
            state,
            definition.number,
            PoolPlaybackAction::Load(cue()?),
            exclusion_zones,
            activation_origin,
        )?,
        "xfade-on" | "xfade-off" => execute_pool_with_exclusions(
            state,
            definition.number,
            PoolPlaybackAction::XFade(action == "xfade-on"),
            exclusion_zones,
            activation_origin,
        )?,
        "temp-on" | "temp-off" => {
            if !matches!(definition.target, PlaybackTarget::CueList { .. }) {
                return Err(ApiError::bad_request(
                    "Temp is available only for a Cuelist playback",
                ));
            }
            execute_pool_with_exclusions(
                state,
                definition.number,
                PoolPlaybackAction::SetTempButton(action == "temp-on"),
                exclusion_zones,
                activation_origin,
            )?
        }
        _ => return Ok(None),
    };
    Ok(Some(outcome))
}

pub(super) fn select_playback_target(
    state: &AppState,
    desk: Option<&ControlDesk>,
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
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let store = state.desk.lock();
    let selected = store
        .selected_playback(desk.id, show.id)
        .map_err(ApiError::store)?;
    if selected == Some(definition.number) {
        return Ok(false);
    }
    store
        .set_selected_playback(desk.id, show.id, Some(definition.number))
        .map_err(ApiError::store)?;
    Ok(true)
}

fn apply_cuelist_action(
    state: &AppState,
    session: Option<&Session>,
    definition: &light_playback::PlaybackDefinition,
    cue_list_id: light_core::CueListId,
    action: Action,
    pressed: bool,
    exclusion_zones: &[Vec<u16>],
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    let command = match action {
        Action::On => Some(PoolPlaybackAction::On),
        Action::Off => Some(PoolPlaybackAction::Off),
        Action::Toggle => Some(PoolPlaybackAction::Toggle),
        Action::Go => Some(PoolPlaybackAction::Go),
        Action::GoMinus => Some(PoolPlaybackAction::Back),
        Action::Pause => Some(PoolPlaybackAction::TogglePause),
        Action::FastForward => Some(PoolPlaybackAction::FastForward),
        Action::FastRewind => Some(PoolPlaybackAction::FastRewind),
        Action::Flash => Some(PoolPlaybackAction::SetFlash(pressed)),
        Action::Temp => Some(PoolPlaybackAction::ToggleTemp),
        Action::Swap => Some(PoolPlaybackAction::SetSwap(pressed)),
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
        return execute_pool_with_exclusions(
            state,
            definition.number,
            command,
            exclusion_zones,
            activation_origin,
        );
    }
    Ok(PlaybackTargetOutcome::changed(action != Action::Select))
}

pub(super) fn apply_playback_target_action(
    state: &AppState,
    context: &light_application::ActionContext,
    session: Option<&Session>,
    definition: &light_playback::PlaybackDefinition,
    action: Action,
    input: &PoolPlaybackInput,
    pressed: bool,
    exclusion_zones: &[Vec<u16>],
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    if let PlaybackTarget::CueList { cue_list_id } = &definition.target {
        return apply_cuelist_action(
            state,
            session,
            definition,
            *cue_list_id,
            action,
            pressed,
            exclusion_zones,
            activation_origin,
        );
    }
    apply_specialized_target_action(state, context, session, definition, action, input, pressed)
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
