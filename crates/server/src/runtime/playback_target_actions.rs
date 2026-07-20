use super::*;

use light_playback::{PlaybackButtonAction as Action, PlaybackTarget};

#[derive(Debug)]
pub(super) struct PlaybackTargetOutcome {
    pub(super) changed: bool,
    pub(super) released_playbacks: Vec<u16>,
}

impl PlaybackTargetOutcome {
    pub(super) fn changed(changed: bool) -> Self {
        Self {
            changed,
            released_playbacks: Vec::new(),
        }
    }
}

fn execute_pool_with_exclusions(
    state: &AppState,
    number: u16,
    action: PoolPlaybackAction,
    exclusion_zones: &[Vec<u16>],
) -> Result<PlaybackTargetOutcome, ApiError> {
    let transition = state
        .engine
        .execute_pool_playback_with_exclusions(number, action, exclusion_zones)
        .map_err(ApiError::bad_request)?;
    let EnginePlaybackOutcome::Changed(changed) = transition.outcome else {
        return Err(ApiError::internal("unexpected pool Playback outcome"));
    };
    Ok(PlaybackTargetOutcome {
        changed,
        released_playbacks: transition.released_playbacks,
    })
}

pub(super) fn apply_playback_master(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    input: &PoolPlaybackInput,
    source: &str,
    exclusion_zones: &[Vec<u16>],
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
    match &definition.target {
        PlaybackTarget::CueList { .. } => {
            return if virtual_fader {
                execute_pool_with_exclusions(
                    state,
                    definition.number,
                    PoolPlaybackAction::SetVirtualMaster(value),
                    exclusion_zones,
                )
            } else {
                execute_pool_with_exclusions(
                    state,
                    definition.number,
                    PoolPlaybackAction::SetMaster(value),
                    exclusion_zones,
                )
            };
        }
        PlaybackTarget::Group { group_id } => {
            return set_group_playback_master(state, group_id, value)
                .map(PlaybackTargetOutcome::changed);
        }
        PlaybackTarget::SpeedGroup { group } => {
            apply_speed_group_playback_action(state, group, "master", input, definition.fader)?
        }
        PlaybackTarget::GrandMaster => state.output_control.lock().options.grand_master = value,
        PlaybackTarget::ProgrammerFade | PlaybackTarget::CueFade => {
            let mut configuration = state.configuration.write();
            if matches!(definition.target, PlaybackTarget::ProgrammerFade) {
                configuration.programmer_fade_millis = (f64::from(value) * 20_000.0).round() as u64;
            } else {
                configuration.sequence_master_fade_millis =
                    (f64::from(value) * 60_000.0).round() as u64;
            }
            drop(configuration);
            persist_server_configuration(state)?;
            refresh_speed_group_engine(state);
        }
    }
    Ok(PlaybackTargetOutcome::changed(true))
}

pub(super) fn apply_direct_playback_action(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    action: &str,
    input: &PoolPlaybackInput,
    exclusion_zones: &[Vec<u16>],
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
        )?,
        "load" => execute_pool_with_exclusions(
            state,
            definition.number,
            PoolPlaybackAction::Load(cue()?),
            exclusion_zones,
        )?,
        "xfade-on" | "xfade-off" => execute_pool_with_exclusions(
            state,
            definition.number,
            PoolPlaybackAction::XFade(action == "xfade-on"),
            exclusion_zones,
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
) -> Result<(), ApiError> {
    if action != Action::Select
        || !matches!(
            definition.target,
            PlaybackTarget::CueList { .. } | PlaybackTarget::Group { .. }
        )
    {
        return Ok(());
    }
    let desk = desk.ok_or_else(|| ApiError::bad_request("playback selection needs a desk"))?;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    state
        .desk
        .lock()
        .set_selected_playback(desk.id, show.id, Some(definition.number))
        .map_err(ApiError::store)
}

fn apply_cuelist_action(
    state: &AppState,
    session: Option<&Session>,
    definition: &light_playback::PlaybackDefinition,
    cue_list_id: light_core::CueListId,
    action: Action,
    pressed: bool,
    exclusion_zones: &[Vec<u16>],
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
        return execute_pool_with_exclusions(state, definition.number, command, exclusion_zones);
    }
    Ok(PlaybackTargetOutcome::changed(true))
}

fn apply_group_action(
    state: &AppState,
    session: Option<&Session>,
    group_id: &str,
    action: Action,
    pressed: bool,
) -> Result<bool, ApiError> {
    match action {
        Action::Select | Action::SelectDereferenced => {
            let session =
                session.ok_or_else(|| ApiError::bad_request("selection needs a session"))?;
            select_group_playback(state, session, group_id, action == Action::Select)?;
        }
        Action::Flash => state
            .engine
            .set_group_master_flash(group_id.to_owned(), if pressed { 1.0 } else { 0.0 }),
        Action::None => return Ok(false),
        _ => {
            return Err(ApiError::bad_request(
                "action is incompatible with a Group Master playback",
            ));
        }
    }
    Ok(true)
}

fn apply_speed_action(
    state: &AppState,
    group: &str,
    action: Action,
    input: &PoolPlaybackInput,
    fader: light_playback::PlaybackFaderMode,
) -> Result<bool, ApiError> {
    let action = match action {
        Action::Learn => "learn",
        Action::Double => "double",
        Action::Half => "half",
        Action::Pause => "pause",
        Action::None => return Ok(false),
        _ => {
            return Err(ApiError::bad_request(
                "action is incompatible with a Speed Group playback",
            ));
        }
    };
    apply_speed_group_playback_action(state, group, action, input, fader)?;
    Ok(true)
}

fn apply_grand_master_action(
    state: &AppState,
    action: Action,
    pressed: bool,
) -> Result<bool, ApiError> {
    match action {
        Action::Blackout => {
            let current = state.output_control.lock().options.blackout;
            state.output_control.lock().options.blackout = !current;
        }
        Action::Flash => state.output_control.lock().grand_master_flash = pressed,
        Action::PauseDynamics => {
            state
                .engine
                .execute_playback(EnginePlaybackCommand::ToggleDynamicsPaused)
                .map_err(ApiError::bad_request)?;
        }
        Action::None => return Ok(false),
        _ => {
            return Err(ApiError::bad_request(
                "action is incompatible with a Grand Master playback",
            ));
        }
    }
    Ok(true)
}

fn apply_time_master_action(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    action: Action,
) -> Result<bool, ApiError> {
    let mut configuration = state.configuration.write();
    let (time, maximum) = if matches!(definition.target, PlaybackTarget::ProgrammerFade) {
        (&mut configuration.programmer_fade_millis, 20_000)
    } else {
        (&mut configuration.sequence_master_fade_millis, 60_000)
    };
    match action {
        Action::Double => *time = time.saturating_mul(2).min(maximum),
        Action::Half => *time /= 2,
        Action::Off => *time = 0,
        Action::None => return Ok(false),
        _ => {
            return Err(ApiError::bad_request(
                "action is incompatible with a time-master playback",
            ));
        }
    }
    drop(configuration);
    persist_server_configuration(state)?;
    refresh_speed_group_engine(state);
    Ok(true)
}

pub(super) fn apply_playback_target_action(
    state: &AppState,
    session: Option<&Session>,
    definition: &light_playback::PlaybackDefinition,
    action: Action,
    input: &PoolPlaybackInput,
    pressed: bool,
    exclusion_zones: &[Vec<u16>],
) -> Result<PlaybackTargetOutcome, ApiError> {
    match &definition.target {
        PlaybackTarget::CueList { cue_list_id } => apply_cuelist_action(
            state,
            session,
            definition,
            *cue_list_id,
            action,
            pressed,
            exclusion_zones,
        ),
        PlaybackTarget::Group { group_id } => {
            apply_group_action(state, session, group_id, action, pressed)
                .map(PlaybackTargetOutcome::changed)
        }
        PlaybackTarget::SpeedGroup { group } => {
            apply_speed_action(state, group, action, input, definition.fader)
                .map(PlaybackTargetOutcome::changed)
        }
        PlaybackTarget::GrandMaster => {
            apply_grand_master_action(state, action, pressed).map(PlaybackTargetOutcome::changed)
        }
        PlaybackTarget::ProgrammerFade | PlaybackTarget::CueFade => {
            apply_time_master_action(state, definition, action).map(PlaybackTargetOutcome::changed)
        }
    }
}
