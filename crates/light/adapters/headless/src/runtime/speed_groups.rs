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

pub(super) fn copy_speed_group_runtime_to_configuration(state: &AppState, indices: &[usize]) {
    state.installation.update_configuration(|configuration| {
        for &index in indices {
            configuration.speed_groups_bpm[index] = state.output.speed_group_manual_bpm(index);
            configuration.speed_group_sound_to_light[index] =
                state.output.speed_group_sound_config(index);
        }
    });
}

pub(super) fn application_millis(state: &AppState) -> u64 {
    state.output.application_time().timestamp_millis().max(0) as u64
}

/// Propagates the authoritative Speed Group controllers into both chaser scheduling and runtime
/// pause state. The controller retains the useful BPM while paused; the engine receives a
/// separate phase-advancing flag so resuming does not lose that rate.
pub(super) fn refresh_speed_group_engine(state: &AppState) -> [SpeedSnapshot; 5] {
    let now = application_millis(state);
    let mut snapshots = state.output.speed_group_snapshots(now);
    let timing = state.installation.configuration();
    let base = snapshots;
    for index in 0..snapshots.len() {
        let mut source = index;
        while let SpeedGroupSource::SpeedGroup { group } = timing.speed_group_sources[source] {
            let next = usize::from(group).saturating_sub(1);
            if next >= snapshots.len() || next == source {
                break;
            }
            source = next;
        }
        if source != index {
            snapshots[index].effective_bpm = base[source].effective_bpm;
            snapshots[index].phase_advancing =
                base[source].phase_advancing && !snapshots[index].paused;
            snapshots[index].phase_origin_millis = base[source].phase_origin_millis;
            snapshots[index].beat_phase = base[source].beat_phase;
        }
    }
    let effective_bpm = snapshots.map(|snapshot| snapshot.effective_bpm.clamp(0.1, 999.0));
    state.output.set_control_timing(
        effective_bpm,
        timing.programmer_fade_millis,
        timing.sequence_master_fade_millis,
    );
    state
        .output
        .set_speed_groups_paused(snapshots.map(|snapshot| !snapshot.phase_advancing));
    snapshots
}

pub(super) fn persist_server_configuration(state: &AppState) -> Result<(), ApiError> {
    let configuration = state.installation.configuration();
    let encoded = serde_json::to_string(&configuration)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .installation
        .set_setting("server_configuration", &encoded)
        .map_err(ApiError::store)
}

pub(super) fn speed_group_response(
    state: &AppState,
    index: usize,
    snapshots: [SpeedSnapshot; 5],
) -> SpeedGroupResponse {
    let configuration = state.output.speed_group_sound_config(index);
    SpeedGroupResponse {
        group: speed_group_name(index),
        source: wire_speed_group_source(
            state.installation.configuration().speed_group_sources[index],
        ),
        configuration,
        snapshot: snapshots[index],
    }
}

fn wire_speed_group_source(
    source: SpeedGroupSource,
) -> light_wire::v2::desk_management::SpeedGroupSource {
    use light_wire::v2::{
        desk_management::SpeedGroupSource as WireSource, speed_group::SpeedGroupId as WireGroup,
    };
    match source {
        SpeedGroupSource::Manual => WireSource::Manual,
        SpeedGroupSource::SoundToLight => WireSource::SoundToLight,
        SpeedGroupSource::SpeedGroup { group } => WireSource::SpeedGroup {
            group: match group {
                1 => WireGroup::A,
                2 => WireGroup::B,
                3 => WireGroup::C,
                4 => WireGroup::D,
                5 => WireGroup::E,
                _ => unreachable!("validated Speed Group sources stay within A-E"),
            },
        },
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

pub(super) async fn observe_speed_group(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
    Json(observation): Json<SoundObservation>,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let index = speed_group_index(&group)?;
    let now = application_millis(&state);
    state
        .output
        .observe_speed_group_sound(index, session.desk.id, now, observation)?;
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
    let _browser_timestamp = input.captured_at_millis;
    let affected = state
        .output
        .apply_speed_group_action(index, now, &input.action)?;
    copy_speed_group_runtime_to_configuration(&state, &affected);
    persist_server_configuration(&state)?;
    let snapshots = refresh_speed_group_engine(&state);
    emit(
        &state,
        "speed_group_action",
        serde_json::json!({"group":speed_group_name(index),"desk_id":session.desk.id,"action":input.action,"snapshot":snapshots[index]}),
    );
    super::speed_group_service::record_external_change(&state, &session, &affected);
    Ok(Json(speed_group_response(&state, index, snapshots)))
}
