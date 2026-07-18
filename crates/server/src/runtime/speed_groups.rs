use super::*;

pub(super) fn speed_group_index(group: &str) -> Result<usize, ApiError> {
    match group.to_ascii_uppercase().as_str() {
        "A" | "1" => Ok(0),
        "B" | "2" => Ok(1),
        "C" | "3" => Ok(2),
        "D" | "4" => Ok(3),
        "E" | "5" => Ok(4),
        _ => Err(ApiError::bad_request("Speed Group must be A-E")),
    }
}

pub(super) fn speed_group_name(index: usize) -> String {
    char::from(b'A' + index as u8).to_string()
}

pub(super) fn linked_speed_group(
    controllers: &[SpeedGroupController; 5],
    index: usize,
) -> Option<usize> {
    controllers[index]
        .synchronized_with()
        .and_then(|group| usize::from(group).checked_sub(1))
        .filter(|peer| *peer < controllers.len() && *peer != index)
}

/// Detaches one group from its reciprocal phase link. The group which received the manual
/// action starts a new independent phase at `now_millis`; its untouched peer keeps its existing
/// phase origin and BPM.
pub(super) fn unlink_speed_group(
    controllers: &mut [SpeedGroupController; 5],
    index: usize,
    now_millis: u64,
) {
    let peer = linked_speed_group(controllers, index);
    controllers[index].break_synchronization(now_millis);
    if let Some(peer) = peer
        && controllers[peer].synchronized_with() == Some((index + 1) as u8)
    {
        controllers[peer].clear_synchronization();
    }
}

pub(super) fn synchronize_speed_groups(
    controllers: &mut [SpeedGroupController; 5],
    source: usize,
    target: usize,
    now_millis: u64,
) -> Result<(), ApiError> {
    if source == target {
        return Err(ApiError::bad_request(
            "source and target Speed Groups must be different",
        ));
    }

    let source_snapshot = controllers[source].snapshot(now_millis);
    let source_phase_reference = controllers[source].phase_reference_millis(now_millis);
    // Relinking does not itself count as the independent action that resets a beat. Preserve the
    // source origin, while removing any older links from both addressed groups.
    if let Some(peer) = linked_speed_group(controllers, source) {
        controllers[source].clear_synchronization();
        if controllers[peer].synchronized_with() == Some((source + 1) as u8) {
            controllers[peer].clear_synchronization();
        }
    }
    if let Some(peer) = linked_speed_group(controllers, target) {
        controllers[target].clear_synchronization();
        if controllers[peer].synchronized_with() == Some((target + 1) as u8) {
            controllers[peer].clear_synchronization();
        }
    }

    controllers[source]
        .set_manual_bpm(source_snapshot.manual_bpm)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    controllers[target]
        .set_manual_bpm(source_snapshot.manual_bpm)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    for index in [source, target] {
        controllers[index]
            .set_speed_master_scale(1.0)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        controllers[index].set_paused_at(source_snapshot.paused, now_millis);
    }
    controllers[source].synchronize_phase(
        (target + 1) as u8,
        source_snapshot.phase_origin_millis,
        source_phase_reference,
    );
    controllers[target].synchronize_phase(
        (source + 1) as u8,
        source_snapshot.phase_origin_millis,
        source_phase_reference,
    );
    Ok(())
}

pub(super) fn speed_group_action_indices(
    controllers: &[SpeedGroupController; 5],
    index: usize,
) -> Vec<usize> {
    let mut affected = vec![index];
    if let Some(peer) = linked_speed_group(controllers, index)
        && controllers[peer].synchronized_with() == Some((index + 1) as u8)
    {
        affected.push(peer);
    }
    affected
}

pub(super) fn copy_speed_group_runtime_to_configuration(
    state: &AppState,
    controllers: &[SpeedGroupController; 5],
    indices: &[usize],
) {
    let mut configuration = state.configuration.write();
    for &index in indices {
        configuration.speed_groups_bpm[index] = controllers[index].manual_bpm();
        configuration.speed_group_sound_to_light[index] = controllers[index].sound_config().clone();
    }
}

pub(super) fn application_millis(state: &AppState) -> u64 {
    state
        .engine
        .playback()
        .read()
        .clock()
        .now()
        .timestamp_millis()
        .max(0) as u64
}

/// Propagates the authoritative Speed Group controllers into both chaser scheduling and runtime
/// pause state. The controller retains the useful BPM while paused; the engine receives a
/// separate phase-advancing flag so resuming does not lose that rate.
pub(super) fn refresh_speed_group_engine(state: &AppState) -> [SpeedSnapshot; 5] {
    let now = application_millis(state);
    let snapshots = {
        let controllers = state.speed_groups.lock();
        std::array::from_fn(|index| controllers[index].snapshot(now))
    };
    let timing = state.configuration.read().clone();
    let effective_bpm = snapshots.map(|snapshot| snapshot.effective_bpm.clamp(0.1, 999.0));
    state.engine.set_control_timing(
        effective_bpm,
        timing.programmer_fade_millis,
        timing.sequence_master_fade_millis,
    );
    state
        .engine
        .set_speed_groups_paused(snapshots.map(|snapshot| !snapshot.phase_advancing));
    snapshots
}

pub(super) fn persist_server_configuration(state: &AppState) -> Result<(), ApiError> {
    let configuration = state.configuration.read().clone();
    let encoded = serde_json::to_string(&configuration)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .desk
        .lock()
        .set_setting("server_configuration", &encoded)
        .map_err(ApiError::store)
}

pub(super) fn speed_group_response(
    state: &AppState,
    index: usize,
    snapshots: [SpeedSnapshot; 5],
) -> SpeedGroupResponse {
    let configuration = state.speed_groups.lock()[index].sound_config().clone();
    SpeedGroupResponse {
        group: speed_group_name(index),
        configuration,
        snapshot: snapshots[index],
    }
}

pub(super) async fn speed_group(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let index = speed_group_index(&group)?;
    let snapshots = refresh_speed_group_engine(&state);
    Ok(Json(speed_group_response(&state, index, snapshots)))
}

pub(super) async fn update_speed_group(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
    Json(configuration): Json<SoundToLightConfig>,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let index = speed_group_index(&group)?;
    configuration
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    state.speed_groups.lock()[index]
        .set_sound_config(configuration.clone())
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    state.configuration.write().speed_group_sound_to_light[index] = configuration.clone();
    if !configuration.enabled {
        state.sound_capture_owners.lock()[index] = None;
    }
    persist_server_configuration(&state)?;
    let snapshots = refresh_speed_group_engine(&state);
    emit(
        &state,
        "speed_group_changed",
        serde_json::json!({"group":speed_group_name(index),"desk_id":session.desk.id,"configuration":configuration}),
    );
    Ok(Json(speed_group_response(&state, index, snapshots)))
}

pub(super) async fn observe_speed_group(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
    Json(mut observation): Json<SoundObservation>,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let index = speed_group_index(&group)?;
    let now = application_millis(&state);
    if !state.speed_groups.lock()[index].sound_config().enabled {
        return Err(ApiError::conflict(
            "enable Sound to Light before submitting observations",
        ));
    }
    {
        let mut owners = state.sound_capture_owners.lock();
        if owners[index].is_some_and(|owner| {
            owner.desk_id != session.desk.id && now.saturating_sub(owner.last_seen_millis) <= 3_000
        }) {
            return Err(ApiError::conflict(
                "this Speed Group is receiving audio from another desk",
            ));
        }
        owners[index] = Some(SoundCaptureOwner {
            desk_id: session.desk.id,
            last_seen_millis: now,
        });
    }
    // Browser clocks and capture callback timestamps are not comparable across desks. The server
    // stamps every accepted sample with the shared application clock used by playback.
    observation.captured_at_millis = now;
    state.speed_groups.lock()[index].observe_sound(observation);
    let snapshots = refresh_speed_group_engine(&state);
    emit(
        &state,
        "speed_group_sound_observed",
        serde_json::json!({"group":speed_group_name(index),"desk_id":session.desk.id,"snapshot":snapshots[index]}),
    );
    Ok(Json(speed_group_response(&state, index, snapshots)))
}

pub(super) async fn speed_group_action(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
    Json(input): Json<SpeedGroupActionInput>,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let index = speed_group_index(&group)?;
    let now = application_millis(&state);
    let mut controller = state.speed_groups.lock();
    let affected = match input.action.as_str() {
        "learn" => {
            // The optional browser timestamp is deliberately advisory only; all desk surfaces use
            // the same application clock so an attached OSC surface and the UI behave identically.
            let _browser_timestamp = input.captured_at_millis;
            unlink_speed_group(&mut controller, index, now);
            controller[index].tap_learn(now);
            vec![index]
        }
        "double" => {
            let affected = speed_group_action_indices(&controller, index);
            for &affected_index in &affected {
                controller[affected_index].double();
            }
            affected
        }
        "half" => {
            let affected = speed_group_action_indices(&controller, index);
            for &affected_index in &affected {
                controller[affected_index].half();
            }
            affected
        }
        "pause" => {
            let paused = !controller[index].snapshot(now).paused;
            let affected = speed_group_action_indices(&controller, index);
            for &affected_index in &affected {
                controller[affected_index].set_paused_at(paused, now);
            }
            affected
        }
        _ => {
            return Err(ApiError::bad_request(
                "Speed Group action must be learn, double, half, or pause",
            ));
        }
    };
    copy_speed_group_runtime_to_configuration(&state, &controller, &affected);
    drop(controller);
    if input.action == "learn" {
        state.sound_capture_owners.lock()[index] = None;
    }
    persist_server_configuration(&state)?;
    let snapshots = refresh_speed_group_engine(&state);
    emit(
        &state,
        "speed_group_action",
        serde_json::json!({"group":speed_group_name(index),"desk_id":session.desk.id,"action":input.action,"snapshot":snapshots[index]}),
    );
    Ok(Json(speed_group_response(&state, index, snapshots)))
}
