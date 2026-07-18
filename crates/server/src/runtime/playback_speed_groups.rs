use super::*;

pub(super) fn apply_speed_group_playback_action(
    state: &AppState,
    group: &str,
    action: &str,
    input: &PoolPlaybackInput,
    fader: light_playback::PlaybackFaderMode,
) -> Result<(), ApiError> {
    let index = speed_group_index(group)?;
    let now = application_millis(state);
    let mut controllers = state.speed_groups.lock();
    let affected = match action {
        "learn" => {
            unlink_speed_group(&mut controllers, index, now);
            controllers[index].tap_learn(now);
            state.sound_capture_owners.lock()[index] = None;
            vec![index]
        }
        "double" => {
            let affected = speed_group_action_indices(&controllers, index);
            for &affected_index in &affected {
                controllers[affected_index].double();
            }
            affected
        }
        "half" => {
            let affected = speed_group_action_indices(&controllers, index);
            for &affected_index in &affected {
                controllers[affected_index].half();
            }
            affected
        }
        "pause" => {
            let paused = !controllers[index].snapshot(now).paused;
            let affected = speed_group_action_indices(&controllers, index);
            for &affected_index in &affected {
                controllers[affected_index].set_paused_at(paused, now);
            }
            affected
        }
        "master" => {
            let value = input
                .value
                .ok_or_else(|| ApiError::bad_request("master value is required"))?;
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(ApiError::bad_request("playback master must be within 0-1"));
            }
            match fader {
                light_playback::PlaybackFaderMode::DirectBpm => {
                    unlink_speed_group(&mut controllers, index, now);
                    if value == 0.0 {
                        controllers[index]
                            .set_speed_master_scale(0.0)
                            .map_err(|error| ApiError::bad_request(error.to_string()))?;
                        controllers[index].set_paused_at(true, now);
                    } else {
                        controllers[index]
                            .set_manual_bpm(f64::from(value) * 300.0)
                            .map_err(|error| ApiError::bad_request(error.to_string()))?;
                        controllers[index]
                            .set_speed_master_scale(1.0)
                            .map_err(|error| ApiError::bad_request(error.to_string()))?;
                        controllers[index].set_paused_at(false, now);
                        state.sound_capture_owners.lock()[index] = None;
                    }
                    vec![index]
                }
                light_playback::PlaybackFaderMode::CenteredRelative => {
                    let scale = 4_f64.powf((f64::from(value) - 0.5) * 2.0);
                    let affected = speed_group_action_indices(&controllers, index);
                    for &affected_index in &affected {
                        controllers[affected_index]
                            .set_speed_master_scale(scale)
                            .map_err(|error| ApiError::bad_request(error.to_string()))?;
                    }
                    affected
                }
                light_playback::PlaybackFaderMode::LearnedPercentage
                | light_playback::PlaybackFaderMode::Speed => {
                    let affected = speed_group_action_indices(&controllers, index);
                    for &affected_index in &affected {
                        controllers[affected_index]
                            .set_speed_master_scale(f64::from(value))
                            .map_err(|error| ApiError::bad_request(error.to_string()))?;
                        controllers[affected_index].set_paused_at(value == 0.0, now);
                    }
                    affected
                }
                _ => {
                    return Err(ApiError::bad_request(
                        "the configured fader mode is not available for a Speed Group",
                    ));
                }
            }
        }
        _ => {
            return Err(ApiError::bad_request(
                "action is not available for a Speed Group playback",
            ));
        }
    };
    copy_speed_group_runtime_to_configuration(state, &controllers, &affected);
    drop(controllers);
    persist_server_configuration(state)?;
    refresh_speed_group_engine(state);
    Ok(())
}
