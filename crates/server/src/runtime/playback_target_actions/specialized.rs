use super::*;

pub(super) fn apply_specialized_master(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    input: &PoolPlaybackInput,
    value: f32,
) -> Result<PlaybackTargetOutcome, ApiError> {
    match &definition.target {
        PlaybackTarget::Group { group_id } => set_group_playback_master(state, group_id, value)
            .map(PlaybackTargetOutcome::output_runtime),
        PlaybackTarget::SpeedGroup { group } => {
            apply_speed_group_playback_action(state, group, "master", input, definition.fader)
                .map(PlaybackTargetOutcome::changed)
        }
        PlaybackTarget::GrandMaster => Ok(PlaybackTargetOutcome::output_runtime(set_grand_master(
            state, value,
        ))),
        PlaybackTarget::ProgrammerFade | PlaybackTarget::CueFade => {
            apply_time_master_fader(state, definition, value).map(PlaybackTargetOutcome::changed)
        }
        PlaybackTarget::CueList { .. } => unreachable!("Cuelist masters use the pool boundary"),
    }
}

pub(super) fn apply_specialized_target_action(
    state: &AppState,
    session: Option<&Session>,
    definition: &light_playback::PlaybackDefinition,
    action: Action,
    input: &PoolPlaybackInput,
    pressed: bool,
) -> Result<PlaybackTargetOutcome, ApiError> {
    match &definition.target {
        PlaybackTarget::Group { group_id } => {
            apply_group_action(state, session, group_id, action, pressed)
        }
        PlaybackTarget::SpeedGroup { group } => {
            apply_speed_action(state, group, action, input, definition.fader)
                .map(PlaybackTargetOutcome::changed)
        }
        PlaybackTarget::GrandMaster => apply_grand_master_action(state, action, pressed),
        PlaybackTarget::ProgrammerFade | PlaybackTarget::CueFade => {
            apply_time_master_action(state, definition, action).map(PlaybackTargetOutcome::changed)
        }
        PlaybackTarget::CueList { .. } => unreachable!("Cuelist actions use the pool boundary"),
    }
}

fn apply_group_action(
    state: &AppState,
    session: Option<&Session>,
    group_id: &str,
    action: Action,
    pressed: bool,
) -> Result<PlaybackTargetOutcome, ApiError> {
    match action {
        Action::Select | Action::SelectDereferenced => {
            let session =
                session.ok_or_else(|| ApiError::bad_request("selection needs a session"))?;
            select_group_playback(state, session, group_id, action == Action::Select)?;
            Ok(PlaybackTargetOutcome::changed(true))
        }
        Action::Flash => Ok(PlaybackTargetOutcome::changed(set_group_flash(
            state, group_id, pressed,
        ))),
        Action::None => Ok(PlaybackTargetOutcome::changed(false)),
        _ => Err(ApiError::bad_request(
            "action is incompatible with a Group Master playback",
        )),
    }
}

fn set_group_flash(state: &AppState, group_id: &str, pressed: bool) -> bool {
    let value = if pressed { 1.0 } else { 0.0 };
    if state.engine.group_master_flash(group_id) == value {
        return false;
    }
    state
        .engine
        .set_group_master_flash(group_id.to_owned(), value);
    true
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
    apply_speed_group_playback_action(state, group, action, input, fader)
}

fn apply_grand_master_action(
    state: &AppState,
    action: Action,
    pressed: bool,
) -> Result<PlaybackTargetOutcome, ApiError> {
    match action {
        Action::Blackout => toggle_blackout(state),
        Action::Flash => {
            let changed =
                set_if_changed(&mut state.output_control.lock().grand_master_flash, pressed);
            return Ok(PlaybackTargetOutcome::changed(changed));
        }
        Action::PauseDynamics => toggle_dynamics(state)?,
        Action::None => return Ok(PlaybackTargetOutcome::changed(false)),
        _ => {
            return Err(ApiError::bad_request(
                "action is incompatible with a Grand Master playback",
            ));
        }
    }
    Ok(PlaybackTargetOutcome::output_runtime(true))
}

fn toggle_blackout(state: &AppState) {
    let mut output = state.output_control.lock();
    output.options.blackout = !output.options.blackout;
}

fn toggle_dynamics(state: &AppState) -> Result<(), ApiError> {
    state
        .engine
        .execute_playback(EnginePlaybackCommand::ToggleDynamicsPaused)
        .map_err(ApiError::bad_request)?;
    Ok(())
}

fn apply_time_master_fader(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    value: f32,
) -> Result<bool, ApiError> {
    let maximum = time_master_maximum(definition);
    set_time_master(
        state,
        definition,
        (f64::from(value) * maximum as f64).round() as u64,
    )
}

fn apply_time_master_action(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    action: Action,
) -> Result<bool, ApiError> {
    let maximum = time_master_maximum(definition);
    let changed = {
        let mut configuration = state.configuration.write();
        let current = time_master_slot(&mut configuration, definition);
        match time_master_action_value(*current, maximum, action)? {
            Some(value) => set_if_changed(current, value),
            None => false,
        }
    };
    persist_time_master_change(state, changed)
}

fn time_master_action_value(
    current: u64,
    maximum: u64,
    action: Action,
) -> Result<Option<u64>, ApiError> {
    let value = match action {
        Action::Double => current.saturating_mul(2).min(maximum),
        Action::Half => current / 2,
        Action::Off => 0,
        Action::None => return Ok(None),
        _ => {
            return Err(ApiError::bad_request(
                "action is incompatible with a time-master playback",
            ));
        }
    };
    Ok((value != current).then_some(value))
}

fn time_master_slot<'a>(
    configuration: &'a mut DeskConfiguration,
    definition: &light_playback::PlaybackDefinition,
) -> &'a mut u64 {
    if matches!(definition.target, PlaybackTarget::ProgrammerFade) {
        &mut configuration.programmer_fade_millis
    } else {
        &mut configuration.sequence_master_fade_millis
    }
}

fn time_master_maximum(definition: &light_playback::PlaybackDefinition) -> u64 {
    if matches!(definition.target, PlaybackTarget::ProgrammerFade) {
        20_000
    } else {
        60_000
    }
}

fn set_time_master(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    value: u64,
) -> Result<bool, ApiError> {
    let changed = {
        let mut configuration = state.configuration.write();
        let current = time_master_slot(&mut configuration, definition);
        set_if_changed(current, value)
    };
    persist_time_master_change(state, changed)
}

fn persist_time_master_change(state: &AppState, changed: bool) -> Result<bool, ApiError> {
    if !changed {
        return Ok(false);
    }
    persist_server_configuration(state)?;
    refresh_speed_group_engine(state);
    Ok(true)
}

fn set_grand_master(state: &AppState, value: f32) -> bool {
    set_if_changed(&mut state.output_control.lock().options.grand_master, value)
}

fn set_if_changed<T: PartialEq>(current: &mut T, value: T) -> bool {
    if *current == value {
        return false;
    }
    *current = value;
    true
}

#[cfg(test)]
mod tests {
    use super::{Action, set_if_changed, time_master_action_value};

    #[test]
    fn exact_assignments_do_not_report_a_change() {
        let mut value = 3_000_u64;
        assert!(!set_if_changed(&mut value, 3_000));
        assert_eq!(value, 3_000);
    }

    #[test]
    fn different_assignments_report_one_change() {
        let mut value = false;
        assert!(set_if_changed(&mut value, true));
        assert!(value);
    }

    #[test]
    fn saturated_time_actions_are_exact_no_ops() {
        assert_eq!(
            time_master_action_value(20_000, 20_000, Action::Double).unwrap(),
            None
        );
        assert_eq!(
            time_master_action_value(0, 20_000, Action::Half).unwrap(),
            None
        );
        assert_eq!(
            time_master_action_value(0, 20_000, Action::Off).unwrap(),
            None
        );
    }

    #[test]
    fn time_actions_return_only_the_changed_value() {
        assert_eq!(
            time_master_action_value(3_000, 20_000, Action::Double).unwrap(),
            Some(6_000)
        );
        assert_eq!(
            time_master_action_value(3_000, 20_000, Action::Half).unwrap(),
            Some(1_500)
        );
        assert_eq!(
            time_master_action_value(3_000, 20_000, Action::Off).unwrap(),
            Some(0)
        );
    }
}
