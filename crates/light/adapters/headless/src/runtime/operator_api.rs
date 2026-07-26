use super::*;
use light_wire::v2::runtime as wire;

pub(super) async fn operator_ui() -> Response {
    embedded_asset("index.html")
}
pub(super) async fn operator_asset(Path(path): Path<String>) -> Response {
    embedded_asset(&format!("assets/{path}"))
}
pub(super) fn embedded_asset(path: &str) -> Response {
    let Some(asset) = ControlUiAssets::get(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else {
        "application/octet-stream"
    };
    (
        [(header::CONTENT_TYPE, content_type)],
        asset.data.into_owned(),
    )
        .into_response()
}
pub(super) async fn readiness(
    State(state): State<AppState>,
) -> Result<Json<wire::RuntimeReadinessSnapshot>, ApiError> {
    let active_show_error = state.active_show.error();
    let recovery_mode = active_show_error.is_some();
    if !recovery_mode && let Some(show) = state.active_show.current().as_ref() {
        validate_show_file(&show.path).map_err(|error| ApiError::unavailable(error.to_string()))?;
    }
    Ok(Json(wire::RuntimeReadinessSnapshot {
        status: "ready".into(),
        active_show: state.active_show.current().as_ref().map(|show| show.id.0),
        active_show_error,
        recovery_mode,
        snapshot_revision: state.output.snapshot().revision,
    }))
}
pub(super) async fn diagnostics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::RuntimeDiagnosticsSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    // Refresh derived runtime state at the same application timestamp before exposing it. This is
    // especially important under the manually advanced Playwright clock, where no output frame is
    // guaranteed to have rendered between two exact MIB checkpoints.
    let _ = state.output.resolved_values();
    let route_send_errors = state.output.route_send_errors();
    let output_routes = NetworkOutput::route_diagnostics(&state.output.snapshot().routes);
    let output_bind_ip = state.installation.configuration().output_bind_ip;
    Ok(Json(wire::RuntimeDiagnosticsSnapshot {
        output: runtime_wire::output_health(state.output.health_snapshot()),
        output_bind_ip: output_bind_ip.to_string(),
        output_routes: serde_json::to_value(output_routes)
            .map_err(|error| ApiError::internal(error.to_string()))?,
        route_send_errors: serde_json::to_value(route_send_errors)
            .map_err(|error| ApiError::internal(error.to_string()))?,
        active_programmers: serde_json::to_value(state.programming.active())
            .map_err(|error| ApiError::internal(error.to_string()))?,
        active_playbacks: serde_json::to_value(state.output.active_playbacks())
            .map_err(|error| ApiError::internal(error.to_string()))?,
        move_in_black: serde_json::to_value(state.output.move_in_black_runtime())
            .map_err(|error| ApiError::internal(error.to_string()))?,
        timecode_source: state.output.timecode_status().0,
        media_servers: serde_json::to_value(state.media.statuses())
            .map_err(|error| ApiError::internal(error.to_string()))?,
        snapshot_revision: state.output.snapshot().revision,
    }))
}
pub(super) async fn bootstrap_v2(
    State(state): State<AppState>,
) -> Json<wire::RuntimeBootstrapSnapshot> {
    Json(bootstrap_snapshot(&state))
}

fn bootstrap_snapshot(state: &AppState) -> wire::RuntimeBootstrapSnapshot {
    let (users, desks, client_desks) = state.installation.bootstrap_desk_data();
    let mut clients = client_desks
        .into_iter()
        .map(|entry| {
            let client_id = entry.client_id.unwrap_or(entry.desk.id);
            let connected = state.sessions.client_connected(client_id);
            let desk_in_use = state.sessions.desk_in_use(entry.desk.id);
            wire::RuntimeClientSummary {
                client_id,
                name: entry.desk.name.clone(),
                connected,
                last_connected_at: entry.last_connected_at,
                desk: runtime_wire::desk(entry.desk),
                can_remove: !connected && !desk_in_use,
            }
        })
        .collect::<Vec<_>>();
    clients.sort_by(|left, right| {
        right
            .connected
            .cmp(&left.connected)
            .then_with(|| right.last_connected_at.cmp(&left.last_connected_at))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.client_id.cmp(&right.client_id))
    });
    let (active_timecode_source, active_timecode) = {
        let (source, current) = state.output.timecode_status();
        (
            source,
            current.map(|timecode| {
                format!(
                    "{:02}:{:02}:{:02}:{:02}",
                    timecode.hours, timecode.minutes, timecode.seconds, timecode.frames
                )
            }),
        )
    };
    let snapshot = state.output.snapshot();
    let highlight_fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let highlight_groups = highlight_groups(&snapshot);
    let highlight_states = state
        .sessions
        .sessions()
        .into_iter()
        .filter_map(|session| {
            let programmer = state.programming.get(session.id)?;
            let selection = state.programming.selection(session.id)?;
            let context =
                programming_context(&session, light_application::ActionSource::System, None);
            let ports = highlight_service_adapter::HeadlessHighlightPorts::with_environment(
                state,
                &session,
                light_application::HighlightEnvironment {
                    user_name: Some(session.user.name.clone()),
                    selection,
                    fixtures: highlight_fixtures.clone(),
                    groups: highlight_groups.clone(),
                    output_suppressed: programmer.blind || programmer.preview,
                },
            );
            let highlight = state.highlight.snapshot(&context, &ports).ok()?;
            Some(wire::RuntimeBootstrapHighlightState {
                session_id: session.id.0,
                desk_id: session.desk.id,
                user_id: session.user.id.0,
                state: runtime_wire::highlight(highlight),
            })
        })
        .collect();
    wire::RuntimeBootstrapSnapshot {
        api_version: "v2".into(),
        attribute_registry: ATTRIBUTE_REGISTRY
            .iter()
            .map(runtime_wire::attribute)
            .collect(),
        users: users.into_iter().map(runtime_wire::user).collect(),
        desks: desks.into_iter().map(runtime_wire::desk).collect(),
        clients,
        active_show: state.active_show.current().clone().map(runtime_wire::show),
        // Bootstrap is intentionally available before login so clients can discover enabled
        // users. Programmer state is authenticated separately through `/api/v2/programmers`.
        active_programmers: Vec::new(),
        highlight_states,
        frame_rate_hz: state.output.frame_rate_hz(),
        output_health: runtime_wire::output_health(state.output.health_snapshot()),
        active_timecode_source,
        active_timecode,
        active_show_error: state.active_show.error(),
        hardware_connected: state.integrations.hardware_connected(),
    }
}
pub(super) async fn visualization_snapshot(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
    Query(query): Query<VisualizationQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    let snapshot = state.output.snapshot();
    let options = state.output.render_options();
    let mut resolved = state.output.resolved_values();
    if query.preload
        && let Some(programmer) = state.programming.get(session.id)
    {
        for value in programmer
            .preload_active
            .iter()
            .chain(&programmer.preload_pending)
        {
            resolved.insert(
                (value.fixture_id, value.attribute.clone()),
                value.value.clone(),
            );
        }
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        for (group_id, attributes) in programmer
            .preload_group_active
            .iter()
            .chain(&programmer.preload_group_pending)
        {
            if let Ok(fixtures) = light_programmer::resolve_group(group_id, &groups) {
                for fixture in fixtures {
                    for (attribute, value) in attributes {
                        resolved.insert((fixture, attribute.clone()), value.value.clone());
                    }
                }
            }
        }
    }
    let profile_output_values = state
        .output
        .profile_visualization_values(&resolved, options)
        .map_err(|error| ApiError::internal(error.to_string()))?
        .into_iter()
        .map(|((fixture_id, attribute), value)| {
            serde_json::json!({
                "fixture_id": fixture_id,
                "attribute": attribute,
                "value": value,
            })
        })
        .collect::<Vec<_>>();
    let values = resolved
        .into_iter()
        .map(|((fixture_id, attribute), value)| {
            serde_json::json!({
                "fixture_id": fixture_id,
                "attribute": attribute,
                "value": value,
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({
        "revision": snapshot.revision,
        "generated_at": chrono::Utc::now(),
        "grand_master": options.grand_master,
        "blackout": options.blackout,
        "preload": query.preload,
        "values": values,
        "profile_output_values": profile_output_values,
    })))
}
