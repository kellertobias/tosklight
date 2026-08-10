use super::*;

pub(super) async fn dmx_snapshot(
    State(state): State<AppState>,
    show: ShowContext,
) -> Result<Json<serde_json::Value>, ApiError> {
    show.verify(&state)?;
    Ok(Json(
        state.output.dmx_snapshot(state.output.snapshot().revision),
    ))
}
pub(super) async fn update_dmx_override(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<light_wire::v2::output_control::DmxOverrideRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    output_runtime_v2::validate_request_id(&input.request_id).map_err(ApiError::bad_request)?;
    apply_dmx_override(&state, &session, input)
}

pub(super) fn apply_dmx_override(
    state: &AppState,
    session: &Session,
    input: light_wire::v2::output_control::DmxOverrideRequest,
) -> Result<Json<serde_json::Value>, ApiError> {
    if input.universe == 0 || !(1..=512).contains(&input.address) {
        return Err(ApiError::bad_request(
            "universe and DMX address must be non-zero and address must be within 1-512",
        ));
    }
    state
        .output
        .set_dmx_override(input.universe, input.address, input.value);
    emit(
        state,
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
    state.lifecycle.request_shutdown();
    Ok(Json(serde_json::json!({"shutting_down":true})))
}
pub(super) async fn configuration(State(state): State<AppState>) -> Json<serde_json::Value> {
    let matter = refresh_matter_bridge(&state);
    let configuration = state.installation.configuration();
    let mut value =
        wire_configuration_value(&configuration).expect("configuration is serializable");
    value["highlight_look_feedback"] = serde_json::json!(
        state
            .output
            .highlight_look_warnings(&configuration.highlight_look)
    );
    Json(
        serde_json::json!({"configuration":value,"output_health":state.output.health_snapshot(),"matter":matter}),
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
    let enabled = state.installation.configuration().matter_enabled;
    let adapter = if !enabled {
        state
            .integrations
            .matter_bridge()
            .reconcile(false, &[], &[], &HashMap::new());
        state.integrations.matter_bridge().status()
    } else {
        let snapshot = state.output.snapshot();
        let values = matter_playback_values(state, &snapshot);
        state.integrations.matter_bridge().reconcile(
            true,
            &snapshot.playback_pages,
            &snapshot.playbacks,
            &values,
        )
    };
    let Some(transport) = state.integrations.matter_transport() else {
        return adapter;
    };
    let transport = transport.reconcile(enabled, &adapter.lights);
    state
        .integrations
        .matter_bridge()
        .apply_transport_snapshot(&transport)
}

pub(super) async fn matter_bridge_sync(
    state: AppState,
    cancellation: CancellationToken,
) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => break,
            _ = interval.tick() => {
                refresh_matter_bridge(&state);
                if let Some(transport) = state.integrations.matter_transport() {
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
    if let Some(transport) = state.integrations.matter_transport() {
        transport.stop();
    }
    Ok(())
}

pub(super) fn matter_playback_values(
    state: &AppState,
    snapshot: &EngineSnapshot,
) -> HashMap<u16, matter::PlaybackValue> {
    let runtime = state
        .output
        .playback_runtime_status()
        .into_iter()
        .filter_map(|status| {
            status
                .playback
                .playback_number
                .map(|number| (number, status))
        })
        .collect::<HashMap<_, _>>();
    snapshot
        .playbacks
        .iter()
        .filter(|definition| matter_exposes_target(&definition.target))
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
                PlaybackTarget::Dynamic { .. } => {
                    light_playback::PlaybackIdentity::physical(definition.number)
                        .ok()
                        .and_then(|identity| state.output.active_dynamic_playback_at(identity))
                        .map(|playback| {
                            matter::PlaybackValue::new(playback.fader_value, playback.enabled)
                        })
                        .unwrap_or_default()
                }
                PlaybackTarget::Group { group_id, .. } => state
                    .output
                    .group_master(group_id)
                    .map(|master| matter::PlaybackValue::new(master, master > 0.0))
                    .unwrap_or_default(),
                PlaybackTarget::SpeedGroup { .. }
                | PlaybackTarget::Macro { .. }
                | PlaybackTarget::Timecode { .. }
                | PlaybackTarget::ProgrammerFade
                | PlaybackTarget::CueFade
                | PlaybackTarget::GrandMaster => {
                    unreachable!("Matter target eligibility is filtered before value projection")
                }
            };
            (definition.number, value)
        })
        .collect()
}

fn matter_exposes_target(target: &light_playback::PlaybackTarget) -> bool {
    matches!(
        target,
        light_playback::PlaybackTarget::CueList { .. }
            | light_playback::PlaybackTarget::Dynamic { .. }
            | light_playback::PlaybackTarget::Group { .. }
    )
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
    let _activation = state
        .active_show
        .try_acquire()
        .map_err(|_| ApiError::conflict("active show transition is in progress"))?;
    refresh_matter_bridge(state);
    let resolved = state
        .integrations
        .matter_bridge()
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
