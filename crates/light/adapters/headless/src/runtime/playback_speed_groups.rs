use super::*;

pub(super) fn apply_speed_group_playback_action(
    state: &AppState,
    group: &str,
    action: &str,
    input: &PoolPlaybackInput,
    fader: light_playback::PlaybackFaderMode,
) -> Result<bool, ApiError> {
    let index = speed_group_index(group)?;
    let now = application_millis(state);
    let configured_source = state.installation.configuration().speed_group_sources[index];
    let linked_fallback =
        if action != "pause" && matches!(configured_source, SpeedGroupSource::SpeedGroup { .. }) {
            Some(refresh_speed_group_engine(state)[index].effective_bpm)
        } else {
            None
        };
    let (changed, affected, takes_manual_control) = state.output.apply_speed_group_playback(
        index,
        now,
        action,
        input,
        fader,
        configured_source,
        linked_fallback,
    )?;
    if !changed {
        return Ok(false);
    }
    copy_speed_group_runtime_to_configuration(state, &affected);
    if takes_manual_control {
        state.installation.update_configuration(|configuration| {
            configuration.speed_group_sources[index] = SpeedGroupSource::Manual;
        });
    }
    persist_server_configuration(state)?;
    refresh_speed_group_engine(state);
    Ok(true)
}

pub(super) fn apply_speed_action(
    controllers: &mut [SpeedGroupController; 5],
    index: usize,
    now: u64,
    action: &str,
    input: &PoolPlaybackInput,
    fader: light_playback::PlaybackFaderMode,
    sound_owner_present: bool,
) -> Result<(Vec<usize>, bool), ApiError> {
    match action {
        "learn" => {
            unlink_speed_group(controllers, index, now);
            controllers[index].tap_learn(now);
            Ok((vec![index], true))
        }
        "double" => Ok((
            apply_to_speed_group(controllers, index, |controller| controller.double()),
            false,
        )),
        "half" => Ok((
            apply_to_speed_group(controllers, index, |controller| controller.half()),
            false,
        )),
        "pause" => Ok((set_speed_group_paused(controllers, index, now), false)),
        "master" => apply_speed_master(controllers, index, now, input, fader, sound_owner_present),
        _ => Err(ApiError::bad_request(
            "action is not available for a Speed Group playback",
        )),
    }
}

fn apply_speed_master(
    controllers: &mut [SpeedGroupController; 5],
    index: usize,
    now: u64,
    input: &PoolPlaybackInput,
    fader: light_playback::PlaybackFaderMode,
    sound_owner_present: bool,
) -> Result<(Vec<usize>, bool), ApiError> {
    let value = input
        .value
        .ok_or_else(|| ApiError::bad_request("master value is required"))?;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(ApiError::bad_request("playback master must be within 0-1"));
    }
    match fader {
        light_playback::PlaybackFaderMode::DirectBpm => {
            apply_direct_bpm_master(controllers, index, now, value, sound_owner_present)
        }
        light_playback::PlaybackFaderMode::CenteredRelative => {
            let scale = 4_f64.powf((f64::from(value) - 0.5) * 2.0);
            set_speed_group_scale(controllers, index, scale).map(|affected| (affected, false))
        }
        light_playback::PlaybackFaderMode::LearnedPercentage
        | light_playback::PlaybackFaderMode::Speed => {
            set_speed_group_percentage(controllers, index, now, value)
                .map(|affected| (affected, false))
        }
        _ => Err(ApiError::bad_request(
            "the configured fader mode is not available for a Speed Group",
        )),
    }
}

fn apply_direct_bpm_master(
    controllers: &mut [SpeedGroupController; 5],
    index: usize,
    now: u64,
    value: f32,
    sound_owner_present: bool,
) -> Result<(Vec<usize>, bool), ApiError> {
    if direct_bpm_is_unchanged(&controllers[index], now, value, sound_owner_present) {
        return Ok((Vec::new(), false));
    }
    unlink_speed_group(controllers, index, now);
    if value == 0.0 {
        controllers[index]
            .set_speed_master_scale(0.0)
            .map_err(speed_error)?;
        controllers[index].set_paused_at(true, now);
    } else {
        controllers[index]
            .set_manual_bpm(f64::from(value) * 300.0)
            .map_err(speed_error)?;
        controllers[index]
            .set_speed_master_scale(1.0)
            .map_err(speed_error)?;
        controllers[index].set_paused_at(false, now);
    }
    Ok((vec![index], value != 0.0))
}

fn direct_bpm_is_unchanged(
    controller: &SpeedGroupController,
    now: u64,
    value: f32,
    sound_owner_present: bool,
) -> bool {
    let snapshot = controller.snapshot(now);
    if controller.synchronized_with().is_some() {
        return false;
    }
    if value == 0.0 {
        return snapshot.speed_master_scale == 0.0 && snapshot.paused;
    }
    controller.manual_entry_is_current(f64::from(value) * 300.0)
        && snapshot.speed_master_scale == 1.0
        && !snapshot.paused
        && !sound_owner_present
}

fn set_speed_group_scale(
    controllers: &mut [SpeedGroupController; 5],
    index: usize,
    scale: f64,
) -> Result<Vec<usize>, ApiError> {
    let affected = speed_group_action_indices(controllers, index);
    for &affected_index in &affected {
        controllers[affected_index]
            .set_speed_master_scale(scale)
            .map_err(speed_error)?;
    }
    Ok(affected)
}

fn set_speed_group_percentage(
    controllers: &mut [SpeedGroupController; 5],
    index: usize,
    now: u64,
    value: f32,
) -> Result<Vec<usize>, ApiError> {
    let affected = set_speed_group_scale(controllers, index, f64::from(value))?;
    for &affected_index in &affected {
        controllers[affected_index].set_paused_at(value == 0.0, now);
    }
    Ok(affected)
}

fn set_speed_group_paused(
    controllers: &mut [SpeedGroupController; 5],
    index: usize,
    now: u64,
) -> Vec<usize> {
    let paused = !controllers[index].snapshot(now).paused;
    apply_to_speed_group(controllers, index, |controller| {
        controller.set_paused_at(paused, now)
    })
}

fn apply_to_speed_group(
    controllers: &mut [SpeedGroupController; 5],
    index: usize,
    mut apply: impl FnMut(&mut SpeedGroupController),
) -> Vec<usize> {
    let affected = speed_group_action_indices(controllers, index);
    for &affected_index in &affected {
        apply(&mut controllers[affected_index]);
    }
    affected
}

pub(super) fn controller_snapshots(
    controllers: &[SpeedGroupController; 5],
    now: u64,
) -> [SpeedSnapshot; 5] {
    std::array::from_fn(|index| controllers[index].snapshot(now))
}

pub(super) fn speed_group_changed(
    before: &[SpeedSnapshot; 5],
    controllers: &[SpeedGroupController; 5],
    affected: &[usize],
    now: u64,
) -> bool {
    affected
        .iter()
        .any(|&index| controllers[index].snapshot(now) != before[index])
}

fn speed_error(error: impl ToString) -> ApiError {
    ApiError::bad_request(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturated_double_is_an_exact_no_op() {
        let mut controllers = test_controllers();
        controllers[0].set_manual_bpm(999.0).unwrap();
        let before = controller_snapshots(&controllers, 100);

        controllers[0].double();

        assert!(!speed_group_changed(&before, &controllers, &[0], 100));
    }

    #[test]
    fn repeated_speed_scale_is_an_exact_no_op() {
        let mut controllers = test_controllers();
        let before = controller_snapshots(&controllers, 100);

        set_speed_group_scale(&mut controllers, 0, 1.0).unwrap();

        assert!(!speed_group_changed(&before, &controllers, &[0], 100));
    }

    #[test]
    fn changed_speed_value_is_detected() {
        let mut controllers = test_controllers();
        let before = controller_snapshots(&controllers, 100);

        controllers[0].half();

        assert!(speed_group_changed(&before, &controllers, &[0], 100));
    }

    fn test_controllers() -> [SpeedGroupController; 5] {
        std::array::from_fn(|index| {
            SpeedGroupController::new(default_speed_groups()[index], SoundToLightConfig::default())
                .unwrap()
        })
    }
}
