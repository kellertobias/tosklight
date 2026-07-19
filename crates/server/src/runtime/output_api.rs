use super::*;

pub(super) async fn dmx_snapshot(State(state): State<AppState>) -> Json<serde_json::Value> {
    let control = state.output_control.lock();
    let mut universes = control
        .last_frames
        .iter()
        .map(|(&universe, frame)| serde_json::json!({"universe":universe,"slots":frame.to_vec()}))
        .collect::<Vec<_>>();
    universes.sort_by_key(|universe| universe["universe"].as_u64().unwrap_or_default());
    Json(serde_json::json!({
        "revision":state.engine.snapshot().revision,
        "universes":universes,
        "overrides":control.raw_overrides.iter().map(|(&(universe,address),&value)| serde_json::json!({"universe":universe,"address":address,"value":value})).collect::<Vec<_>>()
    }))
}
pub(super) async fn update_dmx_override(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RawDmxOverrideInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if input.universe == 0 || !(1..=512).contains(&input.address) {
        return Err(ApiError::bad_request(
            "universe and DMX address must be non-zero and address must be within 1-512",
        ));
    }
    let mut control = state.output_control.lock();
    match input.value {
        Some(value) => {
            control
                .raw_overrides
                .insert((input.universe, input.address), value);
        }
        None => {
            control
                .raw_overrides
                .remove(&(input.universe, input.address));
        }
    }
    drop(control);
    emit(
        &state,
        "dmx_override_changed",
        serde_json::json!({"session_id":session.id,"universe":input.universe,"address":input.address,"value":input.value}),
    );
    Ok(Json(serde_json::json!({"updated":true})))
}
pub(super) async fn shutdown_server(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    emit(
        &state,
        "server_shutdown_requested",
        serde_json::json!({"session_id":session.id}),
    );
    state.shutdown.cancel();
    Ok(Json(serde_json::json!({"shutting_down":true})))
}
pub(super) async fn configuration(State(state): State<AppState>) -> Json<serde_json::Value> {
    let matter = refresh_matter_bridge(&state);
    Json(
        serde_json::json!({"configuration":state.configuration.read().clone(),"output_health":state.output_health.lock().expect("output health mutex poisoned").clone(),"matter":matter}),
    )
}

pub(super) async fn matter_bridge_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<matter::MatterBridgeStatus>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(refresh_matter_bridge(&state)))
}

pub(super) fn refresh_matter_bridge(state: &AppState) -> matter::MatterBridgeStatus {
    let enabled = state.configuration.read().matter_enabled;
    let adapter = if !enabled {
        state
            .matter_bridge
            .reconcile(false, &[], &[], &HashMap::new());
        state.matter_bridge.status()
    } else {
        let snapshot = state.engine.snapshot();
        let values = matter_playback_values(state, &snapshot);
        state
            .matter_bridge
            .reconcile(true, &snapshot.playback_pages, &snapshot.playbacks, &values)
    };
    let Some(transport) = &state.matter_transport else {
        return adapter;
    };
    let transport = transport.reconcile(enabled, &adapter.lights);
    state.matter_bridge.apply_transport_snapshot(&transport)
}

pub(super) fn spawn_matter_bridge_sync(
    state: AppState,
    cancellation: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => break,
                _ = interval.tick() => {
                    refresh_matter_bridge(&state);
                    if let Some(transport) = &state.matter_transport {
                        let writes = transport.drain_remote_writes();
                        for remote in &writes {
                            if let Err(error) = apply_matter_playback_write(
                                &state,
                                remote.endpoint_id,
                                remote.write,
                            ) {
                                emit(
                                    &state,
                                    "matter_write_rejected",
                                    serde_json::json!({"endpoint_id":remote.endpoint_id,"error":error.message}),
                                );
                            }
                        }
                        if !writes.is_empty() {
                            refresh_matter_bridge(&state);
                        }
                    }
                }
            }
        }
        if let Some(transport) = &state.matter_transport {
            transport.stop();
        }
    })
}

pub(super) fn matter_playback_values(
    state: &AppState,
    snapshot: &EngineSnapshot,
) -> HashMap<u16, matter::PlaybackValue> {
    let runtime = state
        .engine
        .playback_runtime_status()
        .into_iter()
        .filter_map(|status| {
            status
                .playback
                .playback_number
                .map(|number| (number, status))
        })
        .collect::<HashMap<_, _>>();
    let now = application_millis(state);
    let speeds = {
        let controllers = state.speed_groups.lock();
        std::array::from_fn::<_, 5, _>(|index| controllers[index].snapshot(now))
    };
    let configuration = state.configuration.read().clone();
    let grand_master = state.output_control.lock().options.grand_master;
    snapshot
        .playbacks
        .iter()
        .map(|definition| {
            use light_playback::PlaybackTarget;
            let value = match &definition.target {
                PlaybackTarget::CueList { .. } => runtime
                    .get(&definition.number)
                    .map(|status| match definition.fader {
                        light_playback::PlaybackFaderMode::Temp => matter::PlaybackValue::new(
                            status.temporary_master,
                            status.temporary_active,
                        ),
                        light_playback::PlaybackFaderMode::XFade => matter::PlaybackValue::new(
                            status.playback.manual_xfade_position,
                            status.playback.enabled,
                        ),
                        _ => matter::PlaybackValue::new(
                            status.playback.master,
                            status.playback.enabled,
                        ),
                    })
                    .unwrap_or_default(),
                PlaybackTarget::Group { group_id } => snapshot
                    .groups
                    .iter()
                    .find(|group| group.id == *group_id)
                    .map(|group| matter::PlaybackValue::new(group.master, group.master > 0.0))
                    .unwrap_or_default(),
                PlaybackTarget::SpeedGroup { group } => speed_group_index(group)
                    .ok()
                    .map(|index| {
                        let level = matter_speed_fader_level(speeds[index], definition.fader);
                        matter::PlaybackValue::new(level, level > 0.0)
                    })
                    .unwrap_or_default(),
                PlaybackTarget::ProgrammerFade => {
                    let level =
                        (configuration.programmer_fade_millis as f32 / 20_000.0).clamp(0.0, 1.0);
                    matter::PlaybackValue::new(level, level > 0.0)
                }
                PlaybackTarget::CueFade => {
                    let level = (configuration.sequence_master_fade_millis as f32 / 60_000.0)
                        .clamp(0.0, 1.0);
                    matter::PlaybackValue::new(level, level > 0.0)
                }
                PlaybackTarget::GrandMaster => {
                    matter::PlaybackValue::new(grand_master, grand_master > 0.0)
                }
            };
            (definition.number, value)
        })
        .collect()
}

pub(super) fn matter_speed_fader_level(
    snapshot: SpeedSnapshot,
    fader: light_playback::PlaybackFaderMode,
) -> f32 {
    use light_playback::PlaybackFaderMode;
    let level = match fader {
        PlaybackFaderMode::DirectBpm => {
            if snapshot.speed_master_scale == 0.0 {
                0.0
            } else {
                snapshot.manual_bpm / 300.0
            }
        }
        PlaybackFaderMode::CenteredRelative => {
            snapshot.speed_master_scale.max(f64::MIN_POSITIVE).log(4.0) / 2.0 + 0.5
        }
        PlaybackFaderMode::LearnedPercentage | PlaybackFaderMode::Speed => {
            snapshot.speed_master_scale
        }
        _ => 0.0,
    };
    level.clamp(0.0, 1.0) as f32
}

/// Apply the protocol-independent result of a Matter On/Off or Level Control write through the
/// same global playback dispatcher used by attached desk surfaces. A protocol transport can call
/// this seam after commissioning without acquiring a desk-local current-page context.
#[allow(dead_code)]
pub(super) fn apply_matter_playback_write(
    state: &AppState,
    endpoint_id: u16,
    write: matter::MatterPlaybackWrite,
) -> Result<matter::MatterBridgeStatus, ApiError> {
    refresh_matter_bridge(state);
    let resolved = state
        .matter_bridge
        .resolve_write(endpoint_id, write)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let result = playback_service::execute(
        state,
        None,
        None,
        light_application::ActionContext::system(
            Uuid::nil(),
            light_application::ActionSource::Matter,
        ),
        light_application::PlaybackCommand {
            address: light_application::PlaybackAddress::Pool(resolved.playback_number),
            action: light_application::PlaybackAction::Master(
                light_application::PlaybackLevel::new(resolved.level),
            ),
            surface: light_application::PlaybackSurface::Matter,
        },
    )?;
    let changed = matches!(
        result.execution,
        light_application::PlaybackExecution::Pool { changed: true, .. }
    );
    if changed {
        emit(
            state,
            "playback_changed",
            serde_json::json!({
                "page":resolved.page,
                "playback":resolved.playback,
                "playback_number":resolved.playback_number,
                "action":"fader",
                "source":"matter"
            }),
        );
    }
    Ok(refresh_matter_bridge(state))
}
