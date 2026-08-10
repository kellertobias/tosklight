//! HTTP route composition for the server runtime.

use super::*;

pub(super) fn build(state: AppState) -> Router {
    let test_bench = state.output.has_test_clock();
    let router = Router::new()
        .merge(help::router::<AppState>())
        .merge(event_transport::router())
        .merge(extensions_runtime::router())
        .merge(usb_output::router())
        .merge(visualization_transport::router())
        .merge(live_action_http::router())
        .merge(output_runtime_v2::router())
        .merge(speed_group_v2::router())
        .merge(playback_v2::router())
        .merge(dynamics_http::router())
        .merge(macros_v2::router())
        .merge(timecode_v2::router())
        .merge(playback_topology_http::router())
        .merge(attribute_configuration::router())
        .merge(control_desk_configuration_v2::router())
        .merge(desk_management_v2::router())
        .merge(screen_configuration_v2::router())
        .merge(schedules_v2::router())
        .merge(virtual_playback_zones_http::router())
        .merge(visualizer_view_http::router())
        .merge(discovery_http::router())
        .merge(programming_update_http::router())
        .merge(show_patch_http::router())
        .merge(cue_thumbnails_http::router())
        .merge(stage_layout_http::router())
        .merge(selective_import_http::router())
        .merge(operator_routes())
        .merge(fixture_api::router())
        .merge(media_and_output_routes())
        .merge(session_routes())
        .merge(show_routes())
        .merge(show_object_routes())
        .merge(programmer_and_update_routes())
        .merge(file_manager::router());
    with_transport_layers(with_test_routes(router, test_bench), state)
}

fn operator_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(operator_ui))
        .route("/assets/{*path}", get(operator_asset))
        .route("/api/v2/readiness", get(readiness))
        .route("/api/v2/diagnostics", get(diagnostics))
        .route(
            "/api/v2/diagnostics/performance",
            get(performance_diagnostics),
        )
        .route("/api/v2/bootstrap", get(bootstrap_v2))
}

fn media_and_output_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v2/output/visualization", get(visualization_snapshot))
        .route("/api/v2/media-servers", get(media_servers))
        .route(
            "/api/v2/media-servers/{fixture_id}/inspect",
            get(inspect_media_server),
        )
        .route(
            "/api/v2/media-servers/{fixture_id}/thumbnails/refresh",
            post(refresh_media_thumbnails),
        )
        .route(
            "/api/v2/media-servers/{fixture_id}/preview/refresh",
            post(refresh_media_preview),
        )
        .route(
            "/api/v2/media-servers/{fixture_id}/preview/{source}",
            get(media_preview),
        )
        .route(
            "/api/v2/media-servers/{fixture_id}/thumbnails/{folder}/{element}",
            get(media_thumbnail),
        )
        .route("/api/v2/output/dmx", get(dmx_snapshot))
        .route("/api/v2/output/dmx-overrides", post(update_dmx_override))
        .route("/api/v2/output/highlight", get(highlight_status))
        .route("/api/v2/output/highlight/actions", post(highlight_action))
        .route(
            "/api/v2/output/patch-preview-highlight",
            post(patch_preview_highlight),
        )
}

fn session_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v2/sessions", post(create_session))
        .route("/api/v2/sessions/{id}", delete(close_session))
}

fn show_routes() -> Router<AppState> {
    Router::new().merge(show_library_v2::router())
}

fn show_object_routes() -> Router<AppState> {
    Router::new()
        .merge(show_object_intents_v2::router())
        .merge(show_objects_v2::router())
}

fn programmer_and_update_routes() -> Router<AppState> {
    Router::new().merge(command_http::router())
}

fn with_test_routes(router: Router<AppState>, enabled: bool) -> Router<AppState> {
    if !enabled {
        return router;
    }
    router
        .route("/api/v2/test/clock/reset", post(reset_test_clock))
        .route("/api/v2/test/clock/advance", post(advance_test_clock))
        .route("/api/v2/test/clock/free-run", post(free_run_test_clock))
        .route(
            "/api/v2/test/output/frame-rate",
            post(set_test_output_frame_rate),
        )
        .route("/api/v2/test/output/failure", post(set_test_output_failure))
        .route(
            "/api/v2/test/shows/{show_id}/objects/{kind}/{object_id}",
            post(seed_test_show_object),
        )
}

fn with_transport_layers(router: Router<AppState>, state: AppState) -> Router {
    router
        .layer(middleware::from_fn_with_state(
            state.clone(),
            action_timing_boundary,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            desk_lock_boundary,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            read_only_session_boundary,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), desk_boundary))
        .with_state(state)
        .layer(DefaultBodyLimit::max(256 * 1024 * 1024))
        .layer(cors_layer())
        .layer(TraceLayer::new_for_http())
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::IF_MATCH,
            header::RANGE,
            header::HeaderName::from_static("x-light-desk-token"),
            header::HeaderName::from_static("x-tosk-desk"),
            header::HeaderName::from_static("x-tosk-show"),
        ])
        .expose_headers([
            header::ETAG,
            header::ACCEPT_RANGES,
            header::CONTENT_RANGE,
            header::CONTENT_LENGTH,
            header::HeaderName::from_static("x-tosk-action-id"),
            header::HeaderName::from_static("x-tosk-received-output-tick"),
            header::HeaderName::from_static("x-tosk-ack-output-tick"),
            header::HeaderName::from_static("x-tosk-action-wall-micros"),
            header::HeaderName::from_static("x-tosk-output-frame-hz"),
            header::HeaderName::from_static("x-tosk-action-budget-ticks"),
            header::HeaderName::from_static("x-tosk-action-within-budget"),
        ])
}

async fn action_timing_boundary(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let Some((action, may_change_output)) =
        timed_http_action(request.method(), request.uri().path())
    else {
        return next.run(request).await;
    };
    let Some(session) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|token| authenticate_token(&state, token).ok())
    else {
        return next.run(request).await;
    };
    let request_id = format!("http-{}-{}", session.id.0, Uuid::new_v4());
    let timing = state.action_timing.begin(
        "http",
        action,
        request_id,
        state.output.frame_rate_hz(),
        may_change_output,
    );
    let mut response = next.run(request).await;
    let timing = timing.acknowledge(response.status().is_success());
    for (name, value) in [
        ("x-tosk-action-id", timing.action_id.to_string()),
        (
            "x-tosk-received-output-tick",
            timing.received_output_tick.to_string(),
        ),
        (
            "x-tosk-ack-output-tick",
            timing.acknowledged_output_tick.to_string(),
        ),
        (
            "x-tosk-action-wall-micros",
            timing.acknowledgement_wall_micros.to_string(),
        ),
        ("x-tosk-output-frame-hz", timing.output_frame_hz.to_string()),
        (
            "x-tosk-action-budget-ticks",
            timing.budget_ticks.to_string(),
        ),
        (
            "x-tosk-action-within-budget",
            timing.acknowledgement_within_budget.to_string(),
        ),
    ] {
        if let Ok(value) = value.parse::<header::HeaderValue>() {
            response
                .headers_mut()
                .insert(header::HeaderName::from_static(name), value);
        }
    }
    response
}

fn timed_http_action(method: &Method, path: &str) -> Option<(&'static str, bool)> {
    let mutation = matches!(*method, Method::POST | Method::PUT | Method::DELETE)
        || (*method == Method::GET
            && (path == "/api/v2/programmer-undo/actions"
                || path.starts_with("/api/v2/playbacks/")));
    if !mutation {
        return None;
    }
    match path {
        "/api/v2/command-line" => Some(("command_line_edit", false)),
        "/api/v2/command-line/keys" => Some(("command_key", true)),
        "/api/v2/command-line/execute" => Some(("command_execute", true)),
        "/api/v2/programmer-undo/actions" => Some(("undo", true)),
        "/api/v2/playback-actions" => Some(("playback_action", true)),
        "/api/v2/programmer-capture-mode/actions" => Some(("capture_mode", true)),
        "/api/v2/programming-align/actions" => Some(("align", true)),
        "/api/v2/fixture-controls/actions" => Some(("fixture_control", true)),
        "/api/v2/presets/recall" => Some(("preset_recall", true)),
        _ if path.contains("programming-selection") => Some(("selection", false)),
        _ if path.contains("programmer-preload-values") => Some(("preload_values", false)),
        _ if path.contains("programmer-values") => Some(("values", true)),
        _ if path.contains("programmer-preload") => Some(("preload_lifecycle", true)),
        _ if path.contains("programmer-priority") => Some(("priority", true)),
        _ if path.contains("/dynamics/") => Some(("dynamic", true)),
        _ if path.starts_with("/api/v2/playbacks/") => Some(("playback_action", true)),
        _ => None,
    }
}

#[cfg(test)]
mod action_timing_route_tests {
    use super::*;

    #[test]
    fn typed_programmer_value_routes_match_their_actual_v2_paths() {
        assert_eq!(
            timed_http_action(
                &Method::POST,
                "/api/v2/users/operator/programmer-values/actions"
            ),
            Some(("values", true))
        );
        assert_eq!(
            timed_http_action(
                &Method::POST,
                "/api/v2/users/operator/programmer-preload-values/actions"
            ),
            Some(("preload_values", false))
        );
        assert_eq!(
            timed_http_action(&Method::POST, "/api/v2/playback-actions"),
            Some(("playback_action", true))
        );
        assert_eq!(
            timed_http_action(&Method::GET, "/api/v2/playbacks/1/go"),
            Some(("playback_action", true))
        );
    }
}
