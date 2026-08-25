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
    let visualization = runtime_visualization_diagnostics(&state);
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
        programmer_action_timing: serde_json::to_value(state.action_timing.snapshot())
            .map_err(|error| ApiError::internal(error.to_string()))?,
        visualization,
        extensions: serde_json::to_value(state.extensions.snapshot())
            .map_err(|error| ApiError::internal(error.to_string()))?,
        compatibility_reports: serde_json::json!({
            "installation": state
                .installation
                .setting("removed_midi_inputs_report")
                .map_err(ApiError::store)?
                .and_then(|report| serde_json::from_str::<serde_json::Value>(&report).ok()),
            "active_show": state
                .active_show
                .compatibility_reports()
                .map_err(ApiError::store)?,
        }),
    }))
}

pub(super) async fn performance_diagnostics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::RuntimePerformanceDiagnosticsSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let _ = state.output.resolved_values();
    Ok(Json(wire::RuntimePerformanceDiagnosticsSnapshot {
        output: runtime_wire::output_health(state.output.health_snapshot()),
        programmer_action_timing: serde_json::to_value(state.action_timing.snapshot())
            .map_err(|error| ApiError::internal(error.to_string()))?,
        visualization: runtime_visualization_diagnostics(&state),
    }))
}

fn runtime_visualization_diagnostics(state: &AppState) -> wire::RuntimeVisualizationDiagnostics {
    let visualization = state.output.visualization_metrics();
    wire::RuntimeVisualizationDiagnostics {
        normal_subscribers: visualization.normal_subscribers,
        preload_subscribers: visualization.preload_subscribers,
        projections: visualization.projections,
        projection_micros: visualization.projection_micros,
        payload_bytes: visualization.payload_bytes,
        source_age_millis: visualization.source_age_millis,
        skipped_source_frames: visualization.skipped_source_frames,
        snapshot_requests: visualization.snapshot_requests,
        snapshot_projection_micros: visualization.snapshot_projection_micros,
        snapshot_serialization_micros: visualization.snapshot_serialization_micros,
        snapshot_payload_bytes: visualization.snapshot_payload_bytes,
        snapshot_source_frame: visualization.snapshot_source_frame,
        snapshot_source_age_millis: visualization.snapshot_source_age_millis,
        stream_serializations: visualization.stream_serializations,
        stream_serialization_micros: visualization.stream_serialization_micros,
        stream_payload_bytes: visualization.stream_payload_bytes,
        stream_sends: visualization.stream_sends,
        stream_send_micros: visualization.stream_send_micros,
        stream_send_failures: visualization.stream_send_failures,
        stream_queue_depth: visualization.stream_queue_depth,
        stream_queue_drops: visualization.stream_queue_drops,
    }
}
pub(super) async fn bootstrap_v2(
    State(state): State<AppState>,
) -> Json<wire::RuntimeBootstrapSnapshot> {
    Json(bootstrap_snapshot(&state))
}

fn bootstrap_snapshot(state: &AppState) -> wire::RuntimeBootstrapSnapshot {
    let (desks, client_desks) = state.installation.bootstrap_desk_data();
    let mut clients = client_desks
        .into_iter()
        .map(|entry| {
            let client_id = entry.client_id.unwrap_or(entry.desk.id);
            let connected = state.sessions.client_connected(client_id);
            wire::RuntimeClientSummary {
                client_id,
                name: entry.desk.name.clone(),
                connected,
                last_connected_at: entry.last_connected_at,
                desk: runtime_wire::desk(entry.desk),
                // The desk outlives any one window, so somebody else standing at it does not
                // pin this record. A window is removable once it is no longer connected.
                can_remove: !connected,
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
                state: runtime_wire::highlight(highlight),
            })
        })
        .collect();
    wire::RuntimeBootstrapSnapshot {
        api_version: "v2".into(),
        attribute_registry: attribute_configuration::configured_descriptors(
            &state.attributes.snapshot().configuration,
        )
        .into_iter()
        .map(runtime_wire::attribute)
        .collect(),
        desks: desks.into_iter().map(runtime_wire::desk).collect(),
        clients,
        active_show: state.active_show.current().clone().map(runtime_wire::show),
        // Bootstrap is intentionally available before login so a client can discover the desk.
        // Programmer state is authenticated separately through `/api/v2/programmers`.
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
    let source = state.output.latest_visualization_frame();
    let projection_started = Instant::now();
    let mut snapshot = if query.dynamic_stack_only {
        let ordinary = fixture_sheet_ordinary_values(&state, &session, query.preload);
        let mut snapshot = visualization_snapshot_for_session_content_from_resolved(
            &state,
            &session,
            query.preload,
            true,
            true,
            None,
            None,
        )?;
        project_fixture_sheet_snapshot(
            &mut snapshot,
            ordinary.as_ref(),
            requested_visualization_fixture_ids(query.fixture_ids.as_deref()).as_ref(),
        );
        snapshot
    } else {
        let mut snapshot = visualization_snapshot_for_session(&state, &session, query.preload)?;
        if let Some(fixture_ids) = requested_visualization_fixture_ids(query.fixture_ids.as_deref())
        {
            retain_visualization_fixtures(&mut snapshot, &fixture_ids);
        }
        snapshot
    };
    let projection_duration = projection_started.elapsed();
    if let Some(source) = source.as_ref()
        && let Some(snapshot) = snapshot.as_object_mut()
    {
        snapshot.insert("source_frame".into(), source.sequence.into());
        snapshot.insert(
            "source_timestamp".into(),
            chrono::DateTime::<chrono::Utc>::from(source.generated_at)
                .to_rfc3339()
                .into(),
        );
    }
    let serialization_started = Instant::now();
    let payload_bytes = serde_json::to_vec(&snapshot)
        .map_err(|error| ApiError::internal(error.to_string()))?
        .len() as u64;
    let serialization_duration = serialization_started.elapsed();
    state.output.record_visualization_snapshot_route(
        projection_duration,
        serialization_duration,
        payload_bytes,
        source.as_deref(),
    );
    Ok(Json(snapshot))
}

/// Returns the Fixture Sheet's stable ordinary base, before Dynamic/FAT sampling.
///
/// Preload is a second programming lane, so its pending ordinary values replace the normal base in
/// that lane. Pending Dynamic values remain identities in `dynamic_stack`; they never replace this
/// map with a sampled value.
fn fixture_sheet_ordinary_values(
    state: &AppState,
    session: &Session,
    preload: bool,
) -> Arc<light_engine::ResolvedValues> {
    let ordinary = state.output.cached_visualization_ordinary_values();
    if !preload {
        return ordinary;
    }
    let Some(programmer) = state.programming.get(session.id) else {
        return ordinary;
    };
    let mut values = ordinary.as_ref().clone();
    for value in programmer
        .preload_active
        .iter()
        .chain(&programmer.preload_pending)
    {
        values.insert(
            (value.fixture_id, value.attribute.clone()),
            value.value.clone(),
        );
    }
    let snapshot = state.output.snapshot();
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
            let fixture_count = fixtures.len();
            for (index, fixture) in fixtures.into_iter().enumerate() {
                for (attribute, value) in attributes {
                    values.insert(
                        (fixture, attribute.clone()),
                        fixture_sheet_value_for_ordered_position(
                            &value.value,
                            index,
                            fixture_count,
                        ),
                    );
                }
            }
        }
    }
    Arc::new(values)
}

fn fixture_sheet_value_for_ordered_position(
    value: &light_core::AttributeValue,
    index: usize,
    count: usize,
) -> light_core::AttributeValue {
    let light_core::AttributeValue::Spread(points) = value else {
        return value.clone();
    };
    if points.is_empty() {
        return light_core::AttributeValue::Normalized(0.0);
    }
    light_core::AttributeValue::Normalized(light_core::spread_position(points, index, count))
}

fn project_fixture_sheet_snapshot(
    snapshot: &mut serde_json::Value,
    ordinary: &light_engine::ResolvedValues,
    fixture_ids: Option<&HashSet<Uuid>>,
) {
    let Some(snapshot_object) = snapshot.as_object_mut() else {
        return;
    };
    snapshot_object.insert("values".into(), visualization_wire_values(ordinary).into());
    snapshot_object.insert(
        "profile_output_values".into(),
        serde_json::Value::Array(Vec::new()),
    );
    if let Some(dynamic_stack) = snapshot_object
        .get_mut("dynamic_stack")
        .and_then(serde_json::Value::as_array_mut)
    {
        dynamic_stack.retain_mut(|entry| {
            if entry.get("entry_type").and_then(serde_json::Value::as_str)
                == Some("ordinary_static")
            {
                return false;
            }
            if let Some(entry) = entry.as_object_mut() {
                // The Fixture Sheet consumes stable identity/state only. Enforce that boundary even
                // if a future caller accidentally requests the verbose sampled projection.
                entry.remove("value");
                entry.remove("resolved_value");
                entry.remove("summary_count");
                entry.remove("summary_title");
            }
            true
        });
    }
    if let Some(fixture_ids) = fixture_ids {
        retain_visualization_fixtures(snapshot, fixture_ids);
    }
}

fn requested_visualization_fixture_ids(value: Option<&str>) -> Option<HashSet<Uuid>> {
    value.map(|fixture_ids| {
        fixture_ids
            .split(',')
            .take(10_000)
            .filter_map(|fixture_id| Uuid::parse_str(fixture_id).ok())
            .collect()
    })
}

/// Retains only the eventually-consistent display values requested by a scoped UI consumer.
///
/// Projection remains authoritative and complete before this cheap boundary filter. This keeps
/// Layout/Fixture Sheet transfer and JSON work proportional to visible fixtures without changing
/// output, Programmer, or Stage-render timing.
fn retain_visualization_fixtures(snapshot: &mut serde_json::Value, fixture_ids: &HashSet<Uuid>) {
    let Some(snapshot) = snapshot.as_object_mut() else {
        return;
    };
    for field in ["values", "profile_output_values", "dynamic_stack"] {
        let Some(entries) = snapshot
            .get_mut(field)
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        entries.retain(|entry| {
            entry
                .get("fixture_id")
                .and_then(serde_json::Value::as_str)
                .and_then(|fixture_id| Uuid::parse_str(fixture_id).ok())
                .is_some_and(|fixture_id| fixture_ids.contains(&fixture_id))
        });
    }
}

#[cfg(test)]
mod scoped_visualization_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normal_snapshot_filter_keeps_only_requested_fixture_values() {
        let included = Uuid::from_u128(1);
        let excluded = Uuid::from_u128(2);
        let mut snapshot = json!({
            "values": [
                {"fixture_id": included, "attribute": "intensity", "value": 0.5},
                {"fixture_id": excluded, "attribute": "intensity", "value": 1.0}
            ],
            "profile_output_values": [
                {"fixture_id": included, "attribute": "intensity", "value": 0.4},
                {"fixture_id": excluded, "attribute": "intensity", "value": 0.9}
            ],
            "dynamic_stack": [
                {"fixture_id": included, "attribute": "intensity"},
                {"fixture_id": excluded, "attribute": "intensity"}
            ],
            "revision": 7
        });
        retain_visualization_fixtures(&mut snapshot, &HashSet::from([included]));
        for field in ["values", "profile_output_values", "dynamic_stack"] {
            assert_eq!(snapshot[field].as_array().unwrap().len(), 1);
            assert_eq!(snapshot[field][0]["fixture_id"], included.to_string());
        }
        assert_eq!(snapshot["revision"], 7);
    }

    #[test]
    fn fixture_sheet_projection_keeps_stable_bases_and_individual_dynamic_identities() {
        let included = light_core::FixtureId(Uuid::from_u128(1));
        let excluded = light_core::FixtureId(Uuid::from_u128(2));
        let first_dynamic = Uuid::from_u128(10);
        let second_dynamic = Uuid::from_u128(11);
        let mut snapshot = json!({
            "values": [
                {"fixture_id": included.0, "attribute": "intensity", "value": 0.95}
            ],
            "profile_output_values": [
                {"fixture_id": included.0, "attribute": "intensity", "value": 0.9}
            ],
            "dynamic_stack": [
                {
                    "fixture_id": included.0,
                    "attribute": "intensity",
                    "entry_type": "ordinary_static",
                    "name": "Static base"
                },
                {
                    "fixture_id": included.0,
                    "attribute": "shaper_1_a",
                    "entry_type": "dynamic",
                    "dynamic_id": first_dynamic,
                    "pool_number": 7,
                    "name": "Fan",
                    "paused": false,
                    "pending": false,
                    "hidden": false,
                    "winning": true,
                    "value": 0.42,
                    "resolved_value": 0.73
                },
                {
                    "fixture_id": included.0,
                    "attribute": "shaper_1_a",
                    "entry_type": "dynamic",
                    "dynamic_id": second_dynamic,
                    "pool_number": 8,
                    "name": "Pulse",
                    "paused": true,
                    "pending": false,
                    "hidden": true,
                    "winning": false,
                    "value": 0.12,
                    "resolved_value": 0.73
                },
                {
                    "fixture_id": excluded.0,
                    "attribute": "media_file",
                    "entry_type": "dynamic",
                    "dynamic_id": Uuid::from_u128(12),
                    "pool_number": 9,
                    "name": "Excluded"
                }
            ]
        });
        let ordinary = light_engine::ResolvedValues::from_iter([
            (
                (included, light_core::AttributeKey("intensity".into())),
                light_core::AttributeValue::Normalized(0.25),
            ),
            (
                (included, light_core::AttributeKey("media.file".into())),
                light_core::AttributeValue::Discrete("4".into()),
            ),
            (
                (excluded, light_core::AttributeKey("intensity".into())),
                light_core::AttributeValue::Normalized(0.5),
            ),
        ]);

        project_fixture_sheet_snapshot(
            &mut snapshot,
            &ordinary,
            Some(&HashSet::from([included.0])),
        );

        let values = snapshot["values"].as_array().unwrap();
        assert_eq!(values.len(), 2);
        assert!(values.iter().any(|value| {
            value["attribute"] == "intensity"
                && value["value"] == serde_json::json!({"kind": "normalized", "value": 0.25})
        }));
        assert!(values.iter().any(|value| {
            value["attribute"] == "media.file"
                && value["value"] == serde_json::json!({"kind": "discrete", "value": "4"})
        }));
        assert_eq!(snapshot["profile_output_values"], json!([]));
        let dynamic_stack = snapshot["dynamic_stack"].as_array().unwrap();
        assert_eq!(dynamic_stack.len(), 2);
        assert_eq!(dynamic_stack[0]["dynamic_id"], first_dynamic.to_string());
        assert_eq!(dynamic_stack[1]["dynamic_id"], second_dynamic.to_string());
        assert_eq!(dynamic_stack[0]["attribute"], "shaper_1_a");
        assert_eq!(dynamic_stack[1]["attribute"], "shaper_1_a");
        assert!(dynamic_stack.iter().all(|entry| {
            entry.get("value").is_none()
                && entry.get("resolved_value").is_none()
                && entry.get("summary_count").is_none()
        }));
    }
}

pub(super) fn visualization_snapshot_for_session(
    state: &AppState,
    session: &Session,
    preload: bool,
) -> Result<serde_json::Value, ApiError> {
    visualization_snapshot_for_session_content(state, session, preload, true)
}

pub(super) fn visualization_snapshot_for_session_content(
    state: &AppState,
    session: &Session,
    preload: bool,
    include_dynamic_stack: bool,
) -> Result<serde_json::Value, ApiError> {
    visualization_snapshot_for_session_content_from_resolved(
        state,
        session,
        preload,
        include_dynamic_stack,
        false,
        None,
        None,
    )
}

pub(super) fn visualization_snapshot_for_session_content_from_resolved(
    state: &AppState,
    session: &Session,
    preload: bool,
    include_dynamic_stack: bool,
    summarize_dynamic_stack: bool,
    authoritative_resolved: Option<&light_engine::ResolvedValues>,
    authoritative_profile_output: Option<&light_engine::ResolvedValues>,
) -> Result<serde_json::Value, ApiError> {
    let snapshot = state.output.snapshot();
    let options = state.output.render_options();
    let programmer = preload.then(|| state.programming.get(session.id)).flatten();
    let extra_dynamic_values = programmer
        .as_ref()
        .map(|programmer| {
            programmer
                .preload_dynamic_pending
                .iter()
                .cloned()
                .map(|value| (programmer.id.0, programmer.priority, value))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let has_preload_overrides = programmer.as_ref().is_some_and(|programmer| {
        !programmer.preload_active.is_empty()
            || !programmer.preload_pending.is_empty()
            || !programmer.preload_group_active.is_empty()
            || !programmer.preload_group_pending.is_empty()
            || !extra_dynamic_values.is_empty()
    });
    let ordinary = include_dynamic_stack
        .then(|| state.output.cached_visualization_ordinary_values())
        .unwrap_or_default();
    let authoritative = authoritative_resolved.filter(|_| extra_dynamic_values.is_empty());
    let cached_dynamics = include_dynamic_stack
        .then(|| state.output.cached_visualization_dynamics())
        .flatten();
    let (mut resolved, dynamic_runtime, dynamic_samples) =
        match (authoritative, include_dynamic_stack, cached_dynamics) {
            (Some(resolved), false, _) => (
                std::borrow::Cow::Borrowed(resolved),
                light_dynamics::DynamicRuntimeSnapshot::default(),
                Vec::new(),
            ),
            (Some(resolved), true, Some(cached)) => (
                std::borrow::Cow::Borrowed(resolved),
                cached.runtime,
                cached.samples,
            ),
            _ => {
                let (resolved, runtime, samples) = state
                    .output
                    .visualization_dynamic_projection(&extra_dynamic_values, preload);
                (std::borrow::Cow::Owned(resolved), runtime, samples)
            }
        };
    if preload && let Some(programmer) = programmer {
        for value in programmer
            .preload_active
            .iter()
            .chain(&programmer.preload_pending)
        {
            resolved.to_mut().insert(
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
                let fixture_count = fixtures.len();
                for (index, fixture) in fixtures.into_iter().enumerate() {
                    for (attribute, value) in attributes {
                        resolved.to_mut().insert(
                            (fixture, attribute.clone()),
                            fixture_sheet_value_for_ordered_position(
                                &value.value,
                                index,
                                fixture_count,
                            ),
                        );
                    }
                }
            }
        }
    }
    let profile_output_values = if has_preload_overrides {
        let projected = state
            .output
            .profile_visualization_values(resolved.as_ref(), options)
            .map_err(|error| ApiError::internal(error.to_string()))?;
        visualization_wire_values(&projected)
    } else if let Some(authoritative) = authoritative_profile_output {
        visualization_wire_values(authoritative)
    } else {
        let projected = state
            .output
            .profile_visualization_values(resolved.as_ref(), options)
            .map_err(|error| ApiError::internal(error.to_string()))?;
        visualization_wire_values(&projected)
    };
    let values = visualization_wire_values(resolved.as_ref());
    let dynamic_stack = include_dynamic_stack
        .then(|| {
            dynamic_stack_projection(
                state,
                ordinary.as_ref(),
                resolved.as_ref(),
                &dynamic_runtime,
                &dynamic_samples,
                &extra_dynamic_values,
                summarize_dynamic_stack,
            )
        })
        .unwrap_or_default();
    let show_id = state.active_show.current().map(|show| show.id.0);
    Ok(serde_json::json!({
        "scope": {
            "show_id": show_id,
        },
        "revision": snapshot.revision,
        "generated_at": chrono::Utc::now(),
        "grand_master": options.grand_master,
        "blackout": options.blackout,
        "preload": preload,
        "values": values,
        "dynamic_stack": dynamic_stack,
        "profile_output_values": profile_output_values,
    }))
}

fn visualization_wire_values(values: &light_engine::ResolvedValues) -> Vec<serde_json::Value> {
    values
        .iter()
        .map(|((fixture_id, attribute), value)| {
            serde_json::json!({
                "fixture_id": fixture_id,
                "attribute": attribute,
                "value": value,
            })
        })
        .collect()
}

#[derive(Serialize)]
struct DynamicStackEntry {
    fixture_id: Uuid,
    attribute: String,
    entry_type: &'static str,
    priority: i16,
    changed_at_millis: u64,
    source: String,
    dynamic_id: Option<Uuid>,
    pool_number: Option<u16>,
    name: String,
    runtime_instance_id: Option<Uuid>,
    controller_id: Option<Uuid>,
    lane_id: Option<Uuid>,
    size: Option<f32>,
    activation_mix: Option<f32>,
    paused: bool,
    hidden: bool,
    pending: bool,
    winning: bool,
    value: Option<light_core::AttributeValue>,
    resolved_value: Option<light_core::AttributeValue>,
}

fn dynamic_stack_projection(
    state: &AppState,
    ordinary: &light_engine::ResolvedValues,
    resolved: &light_engine::ResolvedValues,
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    samples: &[light_dynamics::DynamicRuntimeSample],
    extra: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    summary: bool,
) -> Vec<DynamicStackEntry> {
    let now_millis =
        u64::try_from(state.output.application_time().timestamp_millis()).unwrap_or_default();
    let mut entries = Vec::new();
    push_runtime_stack_entries(
        &mut entries,
        ordinary,
        resolved,
        runtime,
        samples,
        now_millis,
        summary,
    );
    for (programmer_id, priority, stored) in state
        .output
        .dynamic_programmer_values()
        .iter()
        .cloned()
        .chain(extra.iter().cloned())
    {
        push_semantic_stack_entry(
            &mut entries,
            stored,
            priority,
            format!("Programmer {programmer_id}"),
            resolved,
            summary,
        );
    }
    for stored in state.output.active_cue_dynamic_values() {
        push_semantic_stack_entry(
            &mut entries,
            light_dynamics::DynamicAddressValue {
                fixture_id: stored.fixture_id,
                attribute: stored.attribute,
                value: stored.value,
                changed_at_millis: stored.changed_at_millis,
                programmer_order: 0,
            },
            stored.priority,
            format!("Cue {}", stored.current_cue_id),
            resolved,
            summary,
        );
    }
    entries.sort_by(|left, right| {
        left.fixture_id
            .cmp(&right.fixture_id)
            .then_with(|| left.attribute.cmp(&right.attribute))
            .then_with(|| left.priority.cmp(&right.priority))
            .then_with(|| left.changed_at_millis.cmp(&right.changed_at_millis))
            .then_with(|| left.controller_id.cmp(&right.controller_id))
    });
    entries
}

fn push_runtime_stack_entries(
    entries: &mut Vec<DynamicStackEntry>,
    ordinary: &light_engine::ResolvedValues,
    resolved: &light_engine::ResolvedValues,
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    samples: &[light_dynamics::DynamicRuntimeSample],
    now_millis: u64,
    summary: bool,
) {
    let mut ordinary_entries = HashSet::new();
    let dynamic_winners = samples
        .iter()
        .fold(
            HashMap::<
                (light_core::FixtureId, light_core::AttributeKey),
                &light_dynamics::DynamicRuntimeSample,
            >::new(),
            |mut winners, sample| {
                let key = (sample.target, sample.attribute.clone());
                let replace = winners.get(&key).is_none_or(|winner| {
                    (
                        sample.priority,
                        sample.activated_at_millis,
                        sample.controller_id,
                    ) > (
                        winner.priority,
                        winner.activated_at_millis,
                        winner.controller_id,
                    )
                });
                if replace {
                    winners.insert(key, sample);
                }
                winners
            },
        )
        .into_iter()
        .map(|(key, sample)| (key, sample.controller_id))
        .collect::<HashMap<_, _>>();
    let sample_values = (!summary).then(|| {
        samples
            .iter()
            .map(|sample| {
                (
                    (sample.controller_id, sample.target, sample.lane_id),
                    sample.value,
                )
            })
            .collect::<HashMap<_, _>>()
    });
    for instance in &runtime.instances {
        let transitions = instance
            .controller_transitions
            .iter()
            .map(|transition| (transition.controller_id, *transition))
            .collect::<HashMap<_, _>>();
        let pending = instance
            .pending_until_millis
            .is_some_and(|boundary| now_millis < boundary);
        for controller in &instance.controllers {
            let transition = transitions.get(&controller.id).copied().unwrap_or(
                light_dynamics::DynamicControllerTransitionSnapshot {
                    controller_id: controller.id,
                    activation_started_at_millis: controller.activated_at_millis,
                    ..Default::default()
                },
            );
            let activation_mix =
                super::dynamics_http::runtime_transition_mix(transition, now_millis);
            for target in &instance.targets {
                for lane in &instance.definition.lanes {
                    let key = (*target, lane.attribute.clone());
                    let winner = dynamic_winners.get(&key).copied();
                    entries.push(DynamicStackEntry {
                        fixture_id: target.0,
                        attribute: lane.attribute.0.to_string(),
                        entry_type: "dynamic",
                        priority: controller.priority,
                        changed_at_millis: controller.activated_at_millis,
                        source: super::dynamics_http::dynamic_source_label(&controller.source),
                        dynamic_id: Some(instance.definition.id),
                        pool_number: Some(instance.definition.pool_number),
                        name: instance.definition.name.clone(),
                        runtime_instance_id: Some(instance.id),
                        controller_id: Some(controller.id),
                        lane_id: Some(lane.id),
                        size: Some(controller.size),
                        activation_mix: Some(activation_mix),
                        paused: runtime.global_paused
                            || instance.paused_at_millis.is_some()
                            || controller.paused,
                        hidden: winner != Some(controller.id),
                        pending,
                        winning: winner == Some(controller.id),
                        value: sample_values
                            .as_ref()
                            .and_then(|samples| {
                                samples.get(&(controller.id, *target, lane.id)).copied()
                            })
                            .map(light_core::AttributeValue::Normalized),
                        resolved_value: (!summary).then(|| resolved.get(&key).cloned()).flatten(),
                    });
                    if !summary
                        && let Some(value) = ordinary.get(&key)
                        && ordinary_entries.insert(key.clone())
                    {
                        entries.push(DynamicStackEntry {
                            fixture_id: target.0,
                            attribute: lane.attribute.0.to_string(),
                            entry_type: "ordinary_static",
                            priority: i16::MIN,
                            changed_at_millis: 0,
                            source: "Resolved ordinary stack".into(),
                            dynamic_id: None,
                            pool_number: None,
                            name: "Static base".into(),
                            runtime_instance_id: None,
                            controller_id: None,
                            lane_id: None,
                            size: None,
                            activation_mix: None,
                            paused: false,
                            hidden: true,
                            pending: false,
                            winning: false,
                            value: Some(value.clone()),
                            resolved_value: resolved.get(&key).cloned(),
                        });
                    }
                }
            }
        }
    }
}

fn push_semantic_stack_entry(
    entries: &mut Vec<DynamicStackEntry>,
    stored: light_dynamics::DynamicAddressValue,
    priority: i16,
    source: String,
    resolved: &light_engine::ResolvedValues,
    summary: bool,
) {
    let key = (stored.fixture_id, stored.attribute.clone());
    let (entry_type, name, instance, value) = match stored.value {
        light_dynamics::DynamicSemanticValue::FixAt { value, .. } => (
            "fix_at",
            "FixAT".into(),
            None,
            Some(light_core::AttributeValue::Normalized(value)),
        ),
        light_dynamics::DynamicSemanticValue::DynamicOff { instance_link, .. } => (
            "dynamic_off",
            "Dynamic Off".into(),
            Some(instance_link),
            None,
        ),
        light_dynamics::DynamicSemanticValue::Static { value, .. } => {
            ("static", "Static".into(), None, Some(value))
        }
        light_dynamics::DynamicSemanticValue::DynamicOn { .. }
        | light_dynamics::DynamicSemanticValue::Release => return,
    };
    entries.push(DynamicStackEntry {
        fixture_id: stored.fixture_id.0,
        attribute: stored.attribute.0.to_string(),
        entry_type,
        priority,
        changed_at_millis: stored.changed_at_millis,
        source,
        dynamic_id: None,
        pool_number: None,
        name,
        runtime_instance_id: instance,
        controller_id: instance,
        lane_id: None,
        size: None,
        activation_mix: None,
        paused: false,
        hidden: false,
        pending: false,
        winning: true,
        value: (!summary).then_some(value).flatten(),
        resolved_value: (!summary).then(|| resolved.get(&key).cloned()).flatten(),
    });
}
