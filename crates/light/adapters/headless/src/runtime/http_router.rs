//! HTTP route composition for the server runtime.

use super::*;

pub(super) fn build(state: AppState) -> Router {
    let test_bench = state.manual_clock.is_some();
    let router = Router::new()
        .merge(help::router::<AppState>())
        .merge(event_transport::router())
        .merge(output_runtime_v2::router())
        .merge(speed_group_v2::router())
        .merge(playback_v2::router())
        .merge(playback_topology_http::router())
        .merge(control_desk_configuration_v2::router())
        .merge(desk_management_v2::router())
        .merge(screen_configuration_v2::router())
        .merge(virtual_playback_zones_http::router())
        .merge(programming_update_http::router())
        .merge(show_patch_http::router())
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
        .route("/api/v2/bootstrap", get(bootstrap_v2))
}

fn media_and_output_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v2/output/visualization", get(visualization_snapshot))
        .route("/api/v2/media-servers", get(media_servers))
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
            desk_lock_boundary,
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
        ])
}
