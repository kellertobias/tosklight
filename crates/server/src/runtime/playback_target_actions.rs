use super::*;

use light_playback::{PlaybackButtonAction as Action, PlaybackTarget};

pub(super) fn apply_playback_master(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    input: &PoolPlaybackInput,
    source: &str,
) -> Result<bool, ApiError> {
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
            let playback_runtime = state.engine.playback();
            let mut playback = playback_runtime.write();
            if virtual_fader {
                playback
                    .set_virtual_master(definition.number, value)
                    .map_err(ApiError::bad_request)?;
            } else {
                playback
                    .set_master(definition.number, value)
                    .map_err(ApiError::bad_request)?;
            }
        }
        PlaybackTarget::Group { group_id } => set_group_playback_master(state, group_id, value)?,
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
    Ok(true)
}

pub(super) fn apply_direct_playback_action(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    action: &str,
    input: &PoolPlaybackInput,
) -> Result<Option<bool>, ApiError> {
    let cue = || {
        input
            .cue_number
            .ok_or_else(|| ApiError::bad_request("cue_number is required"))
    };
    match action {
        "go-to" => {
            state
                .engine
                .playback()
                .write()
                .goto_playback(definition.number, cue()?)
                .map_err(ApiError::bad_request)?;
        }
        "load" => {
            state
                .engine
                .playback()
                .write()
                .load_playback(definition.number, cue()?)
                .map_err(ApiError::bad_request)?;
        }
        "xfade-on" | "xfade-off" => state
            .engine
            .playback()
            .write()
            .xfade(definition.number, action == "xfade-on")
            .map_err(ApiError::bad_request)?,
        "temp-on" | "temp-off" => {
            if !matches!(definition.target, PlaybackTarget::CueList { .. }) {
                return Err(ApiError::bad_request(
                    "Temp is available only for a Cuelist playback",
                ));
            }
            state
                .engine
                .playback()
                .write()
                .set_temp_button(definition.number, action == "temp-on")
                .map_err(ApiError::bad_request)?;
        }
        _ => return Ok(None),
    }
    Ok(Some(true))
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
) -> Result<bool, ApiError> {
    let playback_runtime = state.engine.playback();
    let mut playback = playback_runtime.write();
    match action {
        Action::On => playback
            .on(definition.number)
            .map_err(ApiError::bad_request)?,
        Action::Off => {
            playback
                .off(definition.number)
                .map_err(ApiError::bad_request)?;
        }
        Action::Toggle => {
            playback
                .toggle(definition.number)
                .map_err(ApiError::bad_request)?;
        }
        Action::Go => {
            playback
                .go_playback(definition.number)
                .map_err(ApiError::bad_request)?;
        }
        Action::GoMinus => {
            playback
                .back_playback(definition.number)
                .map_err(ApiError::bad_request)?;
        }
        Action::Pause => {
            let paused = playback.runtime().iter().any(|runtime| {
                runtime.playback_number == Some(definition.number) && runtime.paused
            });
            if paused {
                playback
                    .go_playback(definition.number)
                    .map_err(ApiError::bad_request)?;
            } else {
                playback
                    .pause_playback(definition.number)
                    .map_err(ApiError::bad_request)?;
            }
        }
        Action::FastForward => {
            playback
                .fast_forward_playback(definition.number)
                .map_err(ApiError::bad_request)?;
        }
        Action::FastRewind => {
            playback
                .fast_rewind_playback(definition.number)
                .map_err(ApiError::bad_request)?;
        }
        Action::Flash => playback
            .set_flash(definition.number, pressed)
            .map_err(ApiError::bad_request)?,
        Action::Temp => {
            playback
                .toggle_temp(definition.number)
                .map_err(ApiError::bad_request)?;
        }
        Action::Swap => playback
            .set_swap(definition.number, pressed)
            .map_err(ApiError::bad_request)?,
        Action::Select => {}
        Action::SelectContents => {
            drop(playback);
            let session =
                session.ok_or_else(|| ApiError::bad_request("selection needs a session"))?;
            select_cuelist_contents(state, session, cue_list_id)?;
        }
        Action::None => return Ok(false),
        _ => {
            return Err(ApiError::bad_request(
                "action is incompatible with a Cuelist playback",
            ));
        }
    }
    Ok(true)
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
            state.engine.playback().write().toggle_dynamics_paused();
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
) -> Result<bool, ApiError> {
    match &definition.target {
        PlaybackTarget::CueList { cue_list_id } => {
            apply_cuelist_action(state, session, definition, *cue_list_id, action, pressed)
        }
        PlaybackTarget::Group { group_id } => {
            apply_group_action(state, session, group_id, action, pressed)
        }
        PlaybackTarget::SpeedGroup { group } => {
            apply_speed_action(state, group, action, input, definition.fader)
        }
        PlaybackTarget::GrandMaster => apply_grand_master_action(state, action, pressed),
        PlaybackTarget::ProgrammerFade | PlaybackTarget::CueFade => {
            apply_time_master_action(state, definition, action)
        }
    }
}
