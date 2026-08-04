use super::*;

pub(super) fn apply_specialized_master(
    state: &AppState,
    context: &light_application::ActionContext,
    session: Option<&Session>,
    definition: &light_playback::PlaybackDefinition,
    input: &PoolPlaybackInput,
    value: f32,
    physical: bool,
) -> Result<PlaybackTargetOutcome, ApiError> {
    match &definition.target {
        PlaybackTarget::Group { group_id, .. } => {
            if !physical {
                return set_group_playback_master(state, group_id, value)
                    .map(PlaybackTargetOutcome::output_runtime);
            }
            let authoritative = state
                .output
                .group_master(group_id)
                .ok_or_else(|| ApiError::conflict("group has no assigned Group Master"))?;
            let outcome = state
                .output
                .execute_playback(EnginePlaybackCommand::Pool {
                    number: definition.number,
                    action: PoolPlaybackAction::SetGroupMasterFader {
                        value,
                        authoritative,
                    },
                })
                .map_err(ApiError::bad_request)?;
            let EnginePlaybackOutcome::Changed(effect) = outcome else {
                return Err(ApiError::internal("unexpected Group Master fader outcome"));
            };
            let identity = light_playback::PlaybackIdentity::physical(definition.number)
                .map_err(ApiError::bad_request)?;
            let control = state.output.playback_control_state_at(identity);
            let mut pickup = PlaybackTargetOutcome::changed(effect.changed());
            pickup.addressed_event_required = effect.addressed.changed();
            if control.fader_pickup_required {
                return Ok(pickup);
            }
            set_group_playback_master(state, group_id, value)
                .map(PlaybackTargetOutcome::output_runtime)
                .map(|output| pickup.combine(output))
        }
        PlaybackTarget::SpeedGroup { group } => {
            apply_speed_group_playback_action(state, group, "master", input, definition.fader)
                .map(PlaybackTargetOutcome::changed)
        }
        PlaybackTarget::GrandMaster => {
            apply_global_output(state, context, session, Some(value), None)
        }
        PlaybackTarget::ProgrammerFade | PlaybackTarget::CueFade => {
            apply_time_master_fader(state, definition, value).map(PlaybackTargetOutcome::changed)
        }
        PlaybackTarget::CueList { .. } | PlaybackTarget::Dynamic { .. } => {
            unreachable!("Cuelist and Dynamic masters use the pool boundary")
        }
    }
}

pub(super) fn apply_specialized_target_action(
    state: &AppState,
    context: &light_application::ActionContext,
    session: Option<&Session>,
    definition: &light_playback::PlaybackDefinition,
    action: Action,
    input: &PoolPlaybackInput,
    pressed: bool,
) -> Result<PlaybackTargetOutcome, ApiError> {
    match &definition.target {
        PlaybackTarget::Group { group_id, .. } => {
            apply_group_action(state, session, group_id, action, pressed)
        }
        PlaybackTarget::SpeedGroup { group } => {
            apply_speed_action(state, group, action, input, definition.fader)
                .map(PlaybackTargetOutcome::changed)
        }
        PlaybackTarget::GrandMaster => {
            apply_grand_master_action(state, context, session, action, pressed)
        }
        PlaybackTarget::ProgrammerFade | PlaybackTarget::CueFade => {
            apply_time_master_action(state, definition, action).map(PlaybackTargetOutcome::changed)
        }
        PlaybackTarget::CueList { .. } | PlaybackTarget::Dynamic { .. } => {
            unreachable!("Cuelist and Dynamic actions use the pool boundary")
        }
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
    if state.output.group_master_flash(group_id) == value {
        return false;
    }
    state
        .output
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
    context: &light_application::ActionContext,
    session: Option<&Session>,
    action: Action,
    pressed: bool,
) -> Result<PlaybackTargetOutcome, ApiError> {
    match action {
        Action::Blackout => {
            let blackout = !state.output.control_projection().blackout;
            apply_global_output(state, context, session, None, Some(blackout))
        }
        Action::Flash => {
            let changed = state.output.set_grand_master_flash(pressed);
            Ok(PlaybackTargetOutcome::changed(changed))
        }
        Action::PauseDynamics => {
            toggle_dynamics(state)?;
            Ok(PlaybackTargetOutcome::output_runtime(true))
        }
        Action::None => Ok(PlaybackTargetOutcome::changed(false)),
        _ => Err(ApiError::bad_request(
            "action is incompatible with a Grand Master playback",
        )),
    }
}

fn apply_global_output(
    state: &AppState,
    context: &light_application::ActionContext,
    session: Option<&Session>,
    grand_master: Option<f32>,
    blackout: Option<bool>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    let command = output_runtime_service::command(grand_master, blackout)?;
    let result = output_runtime_service::execute(state, session, context.clone(), command)?;
    Ok(PlaybackTargetOutcome::converged_output(&result))
}

fn toggle_dynamics(state: &AppState) -> Result<(), ApiError> {
    let outcome = state
        .output
        .execute_playback(EnginePlaybackCommand::ToggleDynamicsPaused)
        .map_err(ApiError::bad_request)?;
    let light_engine::EnginePlaybackOutcome::DynamicsPaused(paused) = outcome else {
        unreachable!("ToggleDynamicsPaused returns the authoritative pause state");
    };
    state.output.set_dynamic_runtime_paused(paused);
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
    let changed = state.installation.update_configuration(|configuration| {
        let current = time_master_slot(configuration, definition);
        match time_master_action_value(*current, maximum, action)? {
            Some(value) => Ok(set_if_changed(current, value)),
            None => Ok(false),
        }
    })?;
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
    let changed = state.installation.update_configuration(|configuration| {
        let current = time_master_slot(configuration, definition);
        set_if_changed(current, value)
    });
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
