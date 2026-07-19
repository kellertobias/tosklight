use super::*;

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
pub(super) async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok", "service":"light-server", "api_version":"v1"}))
}
pub(super) async fn version() -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"service":"light-server","version":env!("CARGO_PKG_VERSION"),"api_version":"v1","show_schema":3,"desk_schema":6}),
    )
}
pub(super) async fn readiness(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let active_show_error = state.active_show_error.read().clone();
    let recovery_mode = active_show_error.is_some();
    if !recovery_mode && let Some(show) = state.active_show.read().as_ref() {
        validate_show_file(&show.path).map_err(|error| ApiError::unavailable(error.to_string()))?;
    }
    Ok(Json(
        serde_json::json!({"status":"ready","active_show":state.active_show.read().as_ref().map(|show|show.id),"active_show_error":active_show_error,"recovery_mode":recovery_mode,"snapshot_revision":state.engine.snapshot().revision}),
    ))
}
pub(super) async fn diagnostics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    // Refresh derived runtime state at the same application timestamp before exposing it. This is
    // especially important under the manually advanced Playwright clock, where no output frame is
    // guaranteed to have rendered between two exact MIB checkpoints.
    let _ = state.engine.resolved_values();
    let route_send_errors = state
        .network_output
        .as_ref()
        .map(|output| output.route_send_errors())
        .unwrap_or_default();
    let output_routes = NetworkOutput::route_diagnostics(&state.engine.snapshot().routes);
    let output_bind_ip = state.configuration.read().output_bind_ip;
    Ok(Json(
        serde_json::json!({"output":state.output_health.lock().expect("output health mutex poisoned").clone(),"output_bind_ip":output_bind_ip,"output_routes":output_routes,"route_send_errors":route_send_errors,"event_queue_pressure":state.events.len(),"active_programmers":state.programmers.active(),"active_playbacks":state.engine.active_playbacks(),"move_in_black":state.engine.move_in_black_runtime(),"timecode_source":state.timecode_router.lock().active_source(),"media_servers":state.media_status.read().clone(),"snapshot_revision":state.engine.snapshot().revision}),
    ))
}
pub(super) async fn bootstrap(State(state): State<AppState>) -> Json<Bootstrap> {
    let (users, desks, client_desks) = {
        let desk = state.desk.lock();
        (
            desk.users().unwrap_or_default(),
            desk.desks().unwrap_or_default(),
            desk.client_desks().unwrap_or_default(),
        )
    };
    let sessions = state.sessions.read();
    let session_clients = state.session_clients.read();
    let mut clients = client_desks
        .into_iter()
        .map(|entry| {
            let client_id = entry.client_id.unwrap_or(entry.desk.id);
            let connected = sessions
                .values()
                .any(|session| session_clients.get(&session.id) == Some(&client_id));
            let desk_in_use = sessions
                .values()
                .any(|session| session.desk.id == entry.desk.id);
            ClientSummary {
                client_id,
                name: entry.desk.name.clone(),
                connected,
                last_connected_at: entry.last_connected_at,
                desk: entry.desk,
                can_remove: !connected && !desk_in_use,
            }
        })
        .collect::<Vec<_>>();
    drop(sessions);
    drop(session_clients);
    clients.sort_by(|left, right| {
        right
            .connected
            .cmp(&left.connected)
            .then_with(|| right.last_connected_at.cmp(&left.last_connected_at))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.client_id.cmp(&right.client_id))
    });
    let (active_timecode_source, active_timecode) = {
        let router = state.timecode_router.lock();
        (
            router.active_source().map(str::to_owned),
            router.current().map(|timecode| {
                format!(
                    "{:02}:{:02}:{:02}:{:02}",
                    timecode.hours, timecode.minutes, timecode.seconds, timecode.frames
                )
            }),
        )
    };
    let snapshot = state.engine.snapshot();
    let highlight_fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let highlight_groups = highlight_groups(&snapshot);
    let highlight_states = state
        .sessions
        .read()
        .values()
        .filter_map(|session| {
            let programmer = state.programmers.get(session.id)?;
            let selection = state.programmers.selection(session.id)?;
            let transition = state.highlight.status(
                session.desk.id,
                session.user.id,
                Some(&session.user.name),
                &selection,
                &highlight_fixtures,
                &highlight_groups,
                programmer.blind || programmer.preview,
            );
            Some(BootstrapHighlightState {
                session_id: session.id,
                desk_id: session.desk.id,
                user_id: session.user.id,
                state: transition.state,
            })
        })
        .collect();
    Json(Bootstrap {
        api_version: "v1",
        attribute_registry: ATTRIBUTE_REGISTRY,
        users,
        desks,
        clients,
        active_show: state.active_show.read().clone(),
        active_programmers: state.programmers.active_for_sessions(),
        highlight_states,
        frame_rate_hz: state.output_rate.load(Ordering::Relaxed),
        output_health: state
            .output_health
            .lock()
            .expect("output health mutex poisoned")
            .clone(),
        active_timecode_source,
        active_timecode,
        active_show_error: state.active_show_error.read().clone(),
        hardware_connected: !state.osc_subscribers.lock().is_empty(),
    })
}
pub(super) async fn patch_snapshot(State(state): State<AppState>) -> Json<serde_json::Value> {
    let snapshot = state.engine.snapshot();
    Json(
        serde_json::json!({"revision":snapshot.revision,"fixtures":snapshot.fixtures,"routes":snapshot.routes}),
    )
}
pub(super) async fn visualization_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<VisualizationQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let snapshot = state.engine.snapshot();
    let options = state.output_control.lock().render_options();
    let mut resolved = state.engine.resolved_values();
    if query.preload
        && let Some(programmer) = state.programmers.get(session.id)
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
        .engine
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
