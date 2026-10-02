use super::*;

#[derive(Default, serde::Deserialize)]
pub(super) struct DmxSnapshotQuery {
    #[serde(default)]
    include_preload: bool,
}

pub(super) async fn dmx_snapshot(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
    Query(query): Query<DmxSnapshotQuery>,
) -> Result<Json<light_wire::v2::output_control::OutputDmxSnapshot>, ApiError> {
    show.verify(&state)?;
    if query.include_preload {
        authenticate(&state, &headers)?;
    }
    let (mut output, source) = state.output.dmx_snapshot();
    // TL-594: with the family adapters engaged, Preload is the accepted Pending publication
    // with its own identity, independent of the Live source; never a fresh projection.
    let published = query
        .include_preload
        .then(|| super::pending_preload_readers::published_preload(&state))
        .flatten();
    if let Some(published) = &published {
        super::pending_preload_readers::attach_dmx_preload(&mut output, published);
    }
    let Some(source) = source else {
        return Ok(Json(output));
    };
    output.frame = Some(source.identity());
    let snapshot = state.output.snapshot();
    if source.show_revision != snapshot.revision
        || source.scope.show_id != state.active_show.current().map(|s| s.id.0)
    {
        return Ok(Json(output));
    }
    // The output publication already contains these poses. A read must never resample
    // tracking, rebuild the whole value map, or combine newer geometry with held DMX.
    let points = native_points(&source.points);
    output.points = points.clone();
    let mut live = native_lane(&source, &source.physical, points);
    live.frame = Some(source.identity());
    output.native = Some(live);
    if query.include_preload && published.is_none() {
        let snapshot = Arc::clone(&source.source_snapshot);
        // A read-only Stage observes the desk's existing programmer; it never creates one.
        let programmer = state
            .programming
            .desk_interaction_context()
            .and_then(|id| state.programming.get(id));
        let extra = programmer
            .as_ref()
            .map(|p| {
                p.preload_dynamic_pending
                    .iter()
                    .cloned()
                    .map(|v| (p.id.0, p.priority, v))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut resolved = if extra.is_empty() {
            source.values.values().clone()
        } else {
            state
                .output
                .visualization_dynamic_projection(&extra, true)
                .0
        };
        let mut affected =
            operator_api::apply_preload_values(&snapshot, programmer.as_ref(), &mut resolved);
        let mut previewed = affected.clone();
        // DynamicAddressValue already carries each authoritative target and attribute. Never
        // infer ownership by comparing samples taken at different instants.
        for (_, _, value) in &extra {
            previewed.insert((value.fixture_id, value.attribute.clone()));
            if !matches!(
                value.value,
                light_dynamics::DynamicSemanticValue::DynamicOff { .. }
                    | light_dynamics::DynamicSemanticValue::Release
            ) {
                affected.insert((value.fixture_id, value.attribute.clone()));
            }
        }
        if let Ok(projected) = state.output.engine().profile_preload_projection_at(
            &resolved,
            source.options,
            Some(&snapshot),
            &affected,
            &previewed,
        ) && Arc::ptr_eq(&snapshot, &state.output.snapshot())
        {
            let mut lane = native_lane(
                &source,
                &projected.physical,
                native_points(&projected.points),
            );
            lane.instances.retain_mut(|i| {
                let Some(mask) = projected
                    .native_ownership
                    .get(&light_core::FixtureId(i.fixture_id))
                else {
                    return false;
                };
                i.owned_channels = Some(mask.to_vec());
                true
            });
            output.preload = Some(lane);
        }
    }
    Ok(Json(output))
}

fn native_points(
    points: &[light_engine::ResolvedPointPose],
) -> Vec<light_wire::v2::output_control::OutputPointPose> {
    points
        .iter()
        .map(|p| light_wire::v2::output_control::OutputPointPose {
            fixture_id: p.fixture_id.0,
            offset_metres: p.offset_metres,
            rotation_degrees: p.rotation_degrees,
        })
        .collect()
}
fn native_lane(
    source: &super::visualization_frame::PublishedVisualizationFrame,
    physical: &light_engine::PhysicalForwardFrame,
    points: Vec<light_wire::v2::output_control::OutputPointPose>,
) -> light_wire::v2::output_control::OutputNativeLane {
    use light_wire::v2::output_control::*;
    OutputNativeLane {
        show_id: source.scope.show_id,
        revision: source.show_revision,
        // Only the published Live lane has this identity. The pending preview adapter
        // must provide its own coherent stamp when it moves to retained publication.
        frame: None,
        points,
        instances: physical
            .instances
            .iter()
            .filter(|i| i.complete)
            .map(|i| OutputNativeInstance {
                fixture_id: i.fixture_id.0,
                instance_id: i.instance_id,
                native_identity: i.native_identity.to_string(),
                raw: i.native_raw.to_vec(),
                owned_channels: None,
            })
            .collect(),
    }
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
                    .map(|master| {
                        matter::PlaybackValue::new(master, master > 0.0)
                            .with_color(state.output.group_color(group_id))
                    })
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
    if let Some(color) = resolved.color {
        let group_id = state
            .output
            .snapshot()
            .playbacks
            .iter()
            .find(|definition| definition.number == resolved.playback_number)
            .and_then(|definition| match &definition.target {
                light_playback::PlaybackTarget::Group { group_id, .. } => Some(group_id.clone()),
                _ => None,
            })
            .ok_or_else(|| ApiError::bad_request("Matter color requires a Group Master"))?;
        let color_changed = state
            .output
            .set_group_color(&group_id, Some(color.xyz()))
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if color_changed {
            emit(
                state,
                "playback_changed",
                serde_json::json!({
                    "page":resolved.page,
                    "playback":resolved.playback,
                    "playback_number":resolved.playback_number,
                    "action":"color",
                    "source":"matter"
                }),
            );
        }
        state
            .integrations
            .matter_bridge()
            .apply_color_write(resolved.endpoint_id, color);
    }
    Ok(refresh_matter_bridge(state))
}
