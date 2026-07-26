//! Typed v2 routes for installation settings and desk-owned operator state.

use super::*;
use crate::tolerant_json::TolerantJson;
use axum::extract::rejection::JsonRejection;
use light_wire::v2::desk_management as wire;

const REPLAY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/configuration", get(configuration_snapshot))
        .route("/api/v2/configuration/update", post(update_configuration))
        .route("/api/v2/matter/status", get(matter_bridge_status))
        .route("/api/v2/speed-groups/{group}", get(speed_group))
        .route(
            "/api/v2/speed-groups/{group}/settings/update",
            post(update_speed_group_settings),
        )
        .route(
            "/api/v2/speed-groups/{group}/actions",
            post(speed_group_action),
        )
        .route(
            "/api/v2/speed-groups/{group}/observations",
            post(observe_speed_group),
        )
        .route("/api/v2/shutdown", post(shutdown))
        .route(
            "/api/v2/output-runtime/global-master/actions",
            post(output_master_action),
        )
        .route("/api/v2/control-desks/{desk_id}/desk-lock", get(desk_lock))
        .route(
            "/api/v2/control-desks/{desk_id}/desk-lock/update",
            post(update_desk_lock),
        )
        .route(
            "/api/v2/control-desks/{desk_id}/desk-lock/lock",
            get(lock_desk),
        )
        .route(
            "/api/v2/control-desks/{desk_id}/desk-lock/unlock",
            post(unlock_desk),
        )
        .route("/api/v2/users/create", post(create_user))
        .route("/api/v2/command-history", get(command_history))
        .route("/api/v2/audit", get(audit_events))
        .route("/api/v2/programmers", get(list_programmers))
        .route("/api/v2/programmers/{id}/clear", post(clear_programmer))
}

async fn configuration_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    authenticate(&state, &headers)?;
    Ok(output_api::configuration(State(state)).await)
}

async fn update_configuration(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<TolerantJson<wire::ConfigurationUpdateRequest>, JsonRejection>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    show_objects_v2::validate_request_id(&request.request_id)?;
    let key = ReplayKey::new(session.id, "configuration", &request.request_id);
    let fingerprint =
        serde_json::to_value(&request).map_err(|error| ApiError::internal(error.to_string()))?;
    let mut replay = state.desk_management_replay.lock().await;
    if let Some(value) = replay.get(&key, &fingerprint)? {
        return Ok(Json(value));
    }
    let configuration = patched_configuration(state.configuration.read().clone(), request.patch)?;
    let Json(mut value) =
        sessions::update_configuration(State(state.clone()), headers, Json(configuration)).await?;
    attach_intent_metadata(&mut value, &request.request_id, false);
    replay.insert(key, fingerprint, value.clone());
    Ok(Json(value))
}

async fn matter_bridge_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<matter::MatterBridgeStatus>, ApiError> {
    output_api::matter_bridge_status(State(state), headers).await
}

async fn speed_group(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    speed_groups::speed_group(State(state), Path(group), headers).await
}

async fn update_speed_group_settings(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
    request: Result<TolerantJson<wire::SpeedGroupSettingsUpdateRequest>, JsonRejection>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    show_objects_v2::validate_request_id(&request.request_id)?;
    let index = speed_groups::speed_group_index(&group)?;
    let key = ReplayKey::new(
        session.id,
        &format!("speed-group-{index}-settings"),
        &request.request_id,
    );
    let fingerprint =
        serde_json::to_value(&request).map_err(|error| ApiError::internal(error.to_string()))?;
    let mut replay = state.desk_management_replay.lock().await;
    if let Some(value) = replay.get(&key, &fingerprint)? {
        return Ok(Json(value));
    }
    let source = application_source(request.source);
    let mut sources = state.configuration.read().speed_group_sources;
    sources[index] = source;
    configuration::validate_speed_group_source_graph(&sources)?;
    let configuration = application_sound_configuration(request.configuration, source);
    configuration
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    state.speed_groups.lock()[index]
        .set_sound_config(configuration.clone())
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    {
        let mut desk_configuration = state.configuration.write();
        desk_configuration.speed_group_sound_to_light[index] = configuration.clone();
        desk_configuration.speed_group_sources = sources;
    }
    if source != SpeedGroupSource::SoundToLight {
        state.sound_capture_owners.lock()[index] = None;
    }
    speed_groups::persist_server_configuration(&state)?;
    let snapshots = speed_groups::refresh_speed_group_engine(&state);
    let response = speed_groups::speed_group_response(&state, index, snapshots);
    emit(
        &state,
        "speed_group_changed",
        serde_json::json!({
            "group": speed_groups::speed_group_name(index),
            "desk_id": session.desk.id,
            "configuration": configuration,
            "source": source,
        }),
    );
    let mut value =
        serde_json::to_value(response).map_err(|error| ApiError::internal(error.to_string()))?;
    attach_intent_metadata(&mut value, &request.request_id, false);
    replay.insert(key, fingerprint, value.clone());
    Ok(Json(value))
}

async fn speed_group_action(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
    request: Result<TolerantJson<wire::SpeedGroupLiveActionRequest>, JsonRejection>,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    let action = request.action;
    let index = speed_groups::speed_group_index(&group)?;
    let source = state.configuration.read().speed_group_sources[index];
    if matches!(action, wire::SpeedGroupLiveAction::SetBpm) {
        let bpm = request
            .bpm
            .ok_or_else(|| ApiError::bad_request("set_bpm requires bpm"))?;
        let now = speed_groups::application_millis(&state);
        let mut controllers = state.speed_groups.lock();
        speed_groups::unlink_speed_group(&mut controllers, index, now);
        controllers[index]
            .set_manual_bpm(bpm)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let mut sound_configuration = controllers[index].sound_config().clone();
        sound_configuration.enabled = false;
        controllers[index]
            .set_sound_config(sound_configuration)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        speed_groups::copy_speed_group_runtime_to_configuration(&state, &controllers, &[index]);
        drop(controllers);
        state.configuration.write().speed_group_sources[index] = SpeedGroupSource::Manual;
        state.sound_capture_owners.lock()[index] = None;
        speed_groups::persist_server_configuration(&state)?;
        let snapshots = speed_groups::refresh_speed_group_engine(&state);
        return Ok(Json(speed_groups::speed_group_response(
            &state, index, snapshots,
        )));
    }
    if source != SpeedGroupSource::SoundToLight
        && !matches!(action, wire::SpeedGroupLiveAction::Pause)
    {
        if matches!(source, SpeedGroupSource::SpeedGroup { .. }) {
            let effective_bpm =
                speed_groups::refresh_speed_group_engine(&state)[index].effective_bpm;
            state.speed_groups.lock()[index]
                .set_manual_fallback_bpm(effective_bpm)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        let mut controllers = state.speed_groups.lock();
        let mut sound_configuration = controllers[index].sound_config().clone();
        sound_configuration.enabled = false;
        controllers[index]
            .set_sound_config(sound_configuration.clone())
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        drop(controllers);
        let mut configuration = state.configuration.write();
        configuration.speed_group_sources[index] = SpeedGroupSource::Manual;
        configuration.speed_group_sound_to_light[index] = sound_configuration;
        drop(configuration);
        state.sound_capture_owners.lock()[index] = None;
    }
    let input = SpeedGroupActionInput {
        action: match action {
            wire::SpeedGroupLiveAction::SetBpm => unreachable!("handled above"),
            wire::SpeedGroupLiveAction::Learn => "learn",
            wire::SpeedGroupLiveAction::Double => "double",
            wire::SpeedGroupLiveAction::Half => "half",
            wire::SpeedGroupLiveAction::Pause => "pause",
        }
        .into(),
        captured_at_millis: request.captured_at_millis,
    };
    let response =
        speed_groups::speed_group_action(State(state.clone()), Path(group), headers, Json(input))
            .await?;
    speed_groups::persist_server_configuration(&state)?;
    Ok(response)
}

async fn observe_speed_group(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
    request: Result<TolerantJson<wire::SoundObservation>, JsonRejection>,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    speed_groups::observe_speed_group(
        State(state),
        Path(group),
        headers,
        Json(SoundObservation {
            captured_at_millis: request.captured_at_millis,
            source_available: request.source_available,
            usable_signal: request.usable_signal,
            level: request.level,
            selected_band_level: request.selected_band_level,
            detected_bpm: request.detected_bpm,
            confidence: request.confidence,
        }),
    )
    .await
}

async fn shutdown(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    output_api::shutdown_server(State(state), headers).await
}

async fn output_master_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<TolerantJson<wire::OutputMasterActionRequest>, JsonRejection>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    event_ws::update_master(
        State(state),
        headers,
        Json(MasterInput {
            grand_master: request.grand_master,
            blackout: request.blackout,
        }),
    )
    .await
}

async fn desk_lock(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DeskLockResponse>, ApiError> {
    authenticated_desk(&state, &headers, desk_id)?;
    boundaries::desk_lock(State(state), headers).await
}

async fn update_desk_lock(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
    request: Result<TolerantJson<wire::DeskLockConfigurationUpdateRequest>, JsonRejection>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticated_desk(&state, &headers, desk_id)?;
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    show_objects_v2::validate_request_id(&request.request_id)?;
    let key = ReplayKey::new(session.id, "desk-lock-configuration", &request.request_id);
    let fingerprint =
        serde_json::to_value(&request).map_err(|error| ApiError::internal(error.to_string()))?;
    let mut replay = state.desk_management_replay.lock().await;
    if let Some(value) = replay.get(&key, &fingerprint)? {
        return Ok(Json(value));
    }
    let Json(response) = boundaries::update_desk_lock(
        State(state.clone()),
        headers,
        Json(DeskLockUpdate {
            message: request.message,
            wallpaper: request.wallpaper,
            unlock_mode: match request.unlock_mode {
                wire::DeskUnlockMode::Button => "button",
                wire::DeskUnlockMode::Pin => "pin",
            }
            .into(),
            pin: request.pin,
        }),
    )
    .await?;
    let mut value =
        serde_json::to_value(response).map_err(|error| ApiError::internal(error.to_string()))?;
    attach_intent_metadata(&mut value, &request.request_id, false);
    replay.insert(key, fingerprint, value.clone());
    Ok(Json(value))
}

async fn lock_desk(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DeskLockResponse>, ApiError> {
    authenticated_desk(&state, &headers, desk_id)?;
    boundaries::lock_desk(State(state), headers).await
}

async fn unlock_desk(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
    request: Result<TolerantJson<wire::DeskUnlockRequest>, JsonRejection>,
) -> Result<Json<DeskLockResponse>, ApiError> {
    authenticated_desk(&state, &headers, desk_id)?;
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    boundaries::unlock_desk(
        State(state),
        headers,
        Json(DeskUnlockInput { pin: request.pin }),
    )
    .await
}

async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<TolerantJson<wire::UserCreateRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let session = authenticate(&state, &headers)?;
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    show_objects_v2::validate_request_id(&request.request_id)?;
    let key = ReplayKey::new(session.id, "user-create", &request.request_id);
    let fingerprint =
        serde_json::to_value(&request).map_err(|error| ApiError::internal(error.to_string()))?;
    let mut replay = state.desk_management_replay.lock().await;
    if let Some(value) = replay.get(&key, &fingerprint)? {
        return Ok((StatusCode::OK, Json(value)));
    }
    let (_, Json(user)) = sessions::create_user(
        State(state.clone()),
        headers,
        Json(UserInput {
            name: request.name,
            enabled: request.enabled,
        }),
    )
    .await?;
    let mut value = serde_json::json!({"user":user});
    attach_intent_metadata(&mut value, &request.request_id, false);
    replay.insert(key, fingerprint, value.clone());
    Ok((StatusCode::CREATED, Json(value)))
}

async fn command_history(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CommandHistoryEntry>>, ApiError> {
    event_ws::command_history(State(state), headers).await
}

async fn audit_events(
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<Event>>, ApiError> {
    event_ws::audit_events(State(state), Query(query), headers).await
}

async fn list_programmers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<light_programmer::ProgrammerState>>, ApiError> {
    update_api::list_programmers(State(state), headers).await
}

async fn clear_programmer(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    event_ws::clear_programmer(State(state), Path(id), headers).await
}

fn authenticated_desk(
    state: &AppState,
    headers: &HeaderMap,
    desk_id: Uuid,
) -> Result<Session, ApiError> {
    let session = authenticate(state, headers)?;
    if session.desk.id != desk_id {
        return Err(ApiError::forbidden(
            "session is not authorized for this desk",
        ));
    }
    Ok(session)
}

fn patched_configuration(
    mut configuration: DeskConfiguration,
    patch: wire::ConfigurationPatch,
) -> Result<DeskConfiguration, ApiError> {
    if let Some(value) = patch.frame_rate_hz {
        configuration.frame_rate_hz = value;
    }
    if let Some(value) = patch.output_bind_ip {
        configuration.output_bind_ip = value
            .parse()
            .map_err(|_| ApiError::bad_request("output_bind_ip must be an IP address"))?;
    }
    if let Some(value) = patch.osc_bind {
        configuration.osc_bind = parse_socket(value, "osc_bind")?;
    }
    if let Some(value) = patch.art_timecode_bind {
        configuration.art_timecode_bind = parse_socket(value, "art_timecode_bind")?;
    }
    if let Some(value) = patch.midi_inputs {
        configuration.midi_inputs = value;
    }
    if let Some(value) = patch.rtp_midi_bind {
        configuration.rtp_midi_bind = parse_socket(value, "rtp_midi_bind")?;
    }
    if let Some(value) = patch.timecode_sources {
        configuration.timecode_sources = value
            .into_iter()
            .map(|source| TimecodeSourceConfig {
                source_prefix: source.source_prefix,
                priority: source.priority,
                fallback: source.fallback,
                loss_timeout_millis: source.loss_timeout_millis,
            })
            .collect();
    }
    if let Some(value) = patch.osc_timecode {
        configuration.osc_timecode = value
            .map(|timecode| {
                Ok(OscTimecodeConfig {
                    address: timecode.address,
                    rate: match timecode.rate.as_str() {
                        "fps24" => FrameRate::Fps24,
                        "fps25" => FrameRate::Fps25,
                        "fps2997_drop" => FrameRate::Fps2997Drop,
                        "fps30" => FrameRate::Fps30,
                        _ => {
                            return Err(ApiError::bad_request("osc_timecode.rate is invalid"));
                        }
                    },
                })
            })
            .transpose()?;
    }
    if let Some(value) = patch.backup_retention {
        configuration.backup_retention = value;
    }
    if let Some(value) = patch.autosave_interval_seconds {
        configuration.autosave_interval_seconds = value;
    }
    if let Some(value) = patch.programmer_fade_millis {
        configuration.programmer_fade_millis = value;
    }
    if let Some(value) = patch.command_line_at_uses_programmer_fade {
        configuration.command_line_at_uses_programmer_fade = value;
    }
    if let Some(value) = patch.sequence_master_fade_millis {
        configuration.sequence_master_fade_millis = value;
    }
    if let Some(value) = patch.preload_programmer_changes {
        configuration.preload_programmer_changes = value;
    }
    if let Some(value) = patch.preload_physical_playback_actions {
        configuration.preload_physical_playback_actions = value;
    }
    if let Some(value) = patch.preload_virtual_playback_actions {
        configuration.preload_virtual_playback_actions = value;
    }
    if let Some(value) = patch.patch_preview_highlight_dmx {
        configuration.patch_preview_highlight_dmx = value;
    }
    if let Some(value) = patch.matter_enabled {
        configuration.matter_enabled = value;
    }
    if let Some(value) = patch.pool_presentation {
        configuration.pool_presentation = application_pool_presentation(value);
    }
    if let Some(value) = patch.file_manager_system_picker_fallback {
        configuration.file_manager_system_picker_fallback = value;
    }
    if let Some(value) = patch.file_manager_roots {
        configuration.file_manager_roots = value
            .into_iter()
            .map(|root| file_manager::ConfiguredRoot {
                id: root.id,
                label: root.label,
                path: PathBuf::from(root.path),
                icon: root.icon,
            })
            .collect();
    }
    configuration.validate()?;
    Ok(configuration)
}

fn application_pool_presentation(
    value: wire::PoolPresentationConfiguration,
) -> PoolPresentationConfiguration {
    PoolPresentationConfiguration {
        palette: PoolColorPalette {
            group: value.palette.group,
            macro_color: value.palette.macro_color,
            dynamic: value.palette.dynamic,
            cuelist: value.palette.cuelist,
            sequence: value.palette.sequence,
            preset: PresetPoolColorPalette {
                mixed: value.palette.preset.mixed,
                intensity: value.palette.preset.intensity,
                color: value.palette.preset.color,
                position: value.palette.preset.position,
                beam: value.palette.preset.beam,
            },
        },
        modes: value
            .modes
            .into_iter()
            .map(|(key, mode)| {
                (
                    key,
                    match mode {
                        wire::PoolColorMode::Type => PoolColorMode::Type,
                        wire::PoolColorMode::Individual => PoolColorMode::Individual,
                    },
                )
            })
            .collect(),
        items: value
            .items
            .into_iter()
            .map(|(key, item)| {
                (
                    key,
                    PoolItemPresentation {
                        title: item.title,
                        icon: item.icon,
                        color: item.color,
                    },
                )
            })
            .collect(),
    }
}

fn parse_socket(value: Option<String>, field: &str) -> Result<Option<SocketAddr>, ApiError> {
    value
        .map(|value| {
            value
                .parse()
                .map_err(|_| ApiError::bad_request(format!("{field} must be a socket address")))
        })
        .transpose()
}

fn application_source(source: wire::SpeedGroupSource) -> SpeedGroupSource {
    match source {
        wire::SpeedGroupSource::Manual => SpeedGroupSource::Manual,
        wire::SpeedGroupSource::SoundToLight => SpeedGroupSource::SoundToLight,
        wire::SpeedGroupSource::SpeedGroup { group } => SpeedGroupSource::SpeedGroup {
            group: match group {
                light_wire::v2::speed_group::SpeedGroupId::A => 1,
                light_wire::v2::speed_group::SpeedGroupId::B => 2,
                light_wire::v2::speed_group::SpeedGroupId::C => 3,
                light_wire::v2::speed_group::SpeedGroupId::D => 4,
                light_wire::v2::speed_group::SpeedGroupId::E => 5,
            },
        },
    }
}

fn application_sound_configuration(
    configuration: wire::SoundToLightConfiguration,
    source: SpeedGroupSource,
) -> SoundToLightConfig {
    use light_control::speed::{
        FrequencyPreset as ApplicationFrequencyPreset,
        FrequencySelection as ApplicationFrequencySelection,
        SoundAnalysisMode as ApplicationSoundAnalysisMode,
    };
    SoundToLightConfig {
        enabled: source == SpeedGroupSource::SoundToLight,
        analysis_mode: match configuration.analysis_mode {
            wire::SoundAnalysisMode::TempoBpm => ApplicationSoundAnalysisMode::TempoBpm,
        },
        frequency: match configuration.frequency {
            wire::FrequencySelection::Preset { preset } => ApplicationFrequencySelection::Preset {
                preset: match preset {
                    wire::FrequencyPreset::Sub => ApplicationFrequencyPreset::Sub,
                    wire::FrequencyPreset::Low => ApplicationFrequencyPreset::Low,
                    wire::FrequencyPreset::Mid => ApplicationFrequencyPreset::Mid,
                    wire::FrequencyPreset::High => ApplicationFrequencyPreset::High,
                    wire::FrequencyPreset::FullRange => ApplicationFrequencyPreset::FullRange,
                },
            },
            wire::FrequencySelection::Custom { low_hz, high_hz } => {
                ApplicationFrequencySelection::Custom { low_hz, high_hz }
            }
        },
        input_gain_db: configuration.input_gain_db,
        confidence_threshold: configuration.confidence_threshold,
        smoothing: configuration.smoothing,
        minimum_bpm: configuration.minimum_bpm,
        maximum_bpm: configuration.maximum_bpm,
        signal_hold_millis: configuration.signal_hold_millis,
        multiplier: configuration.multiplier,
    }
}

fn attach_intent_metadata(value: &mut serde_json::Value, request_id: &str, replayed: bool) {
    if let Some(object) = value.as_object_mut() {
        object.insert("request_id".into(), request_id.into());
        object.insert("replayed".into(), replayed.into());
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    session_id: SessionId,
    operation: String,
    request_id: String,
}

impl ReplayKey {
    pub(super) fn new(session_id: SessionId, operation: &str, request_id: &str) -> Self {
        Self {
            session_id,
            operation: operation.into(),
            request_id: request_id.into(),
        }
    }
}

#[derive(Clone)]
struct ReplayEntry {
    fingerprint: serde_json::Value,
    outcome: serde_json::Value,
}

#[derive(Default)]
pub(super) struct DeskManagementReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl DeskManagementReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        fingerprint: &serde_json::Value,
    ) -> Result<Option<serde_json::Value>, ApiError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if &entry.fingerprint != fingerprint {
            return Err(ApiError::conflict(
                "request_id was already used for a different edit",
            ));
        }
        let mut outcome = entry.outcome.clone();
        if let Some(object) = outcome.as_object_mut() {
            object.insert("replayed".into(), true.into());
        }
        Ok(Some(outcome))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        fingerprint: serde_json::Value,
        outcome: serde_json::Value,
    ) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(
            key,
            ReplayEntry {
                fingerprint,
                outcome,
            },
        );
        while self.order.len() > REPLAY_LIMIT {
            if let Some(expired) = self.order.pop_front() {
                self.entries.remove(&expired);
            }
        }
    }
}
