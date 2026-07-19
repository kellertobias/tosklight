//! HTTP route composition for the server runtime.

use super::*;

pub(super) fn build(state: AppState) -> Router {
    let test_bench = state.manual_clock.is_some();
    let router = Router::new()
        .merge(help::router::<AppState>())
        .merge(event_transport::router())
        .merge(show_patch_http::router())
        .merge(operator_routes())
        .merge(fixture_routes())
        .merge(media_and_output_routes())
        .merge(session_routes())
        .merge(show_routes())
        .merge(show_object_routes())
        .merge(playback_routes())
        .merge(programmer_and_update_routes())
        .merge(file_manager::router());
    with_transport_layers(with_test_routes(router, test_bench), state)
}

fn operator_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(operator_ui))
        .route("/assets/{*path}", get(operator_asset))
        .route("/api/v1/health", get(health))
        .route("/api/v1/readiness", get(readiness))
        .route("/api/v1/version", get(version))
        .route("/api/v1/diagnostics", get(diagnostics))
        .route("/api/v1/bootstrap", get(bootstrap))
        .route("/api/v1/patch", get(patch_snapshot))
}

fn fixture_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/fixture-library",
            get(list_fixture_library).put(put_fixture_library),
        )
        .route(
            "/api/v1/fixture-library/{id}/{revision}",
            delete(delete_fixture_library),
        )
        .route(
            "/api/v1/fixture-profiles",
            get(list_fixture_profiles).put(put_fixture_profile),
        )
        .route(
            "/api/v1/fixture-profiles/warnings",
            get(list_fixture_profile_warnings),
        )
        .route(
            "/api/v1/fixture-profiles/{id}/revisions",
            get(list_fixture_profile_revisions),
        )
        .route(
            "/api/v1/fixture-profiles/{id}/{revision}",
            delete(delete_fixture_profile),
        )
        .route(
            "/api/v1/fixture-profiles/{id}/{revision}/package",
            get(export_fixture_package),
        )
        .route(
            "/api/v1/fixture-packages/import",
            post(import_fixture_package),
        )
        .route(
            "/api/v1/fixture-profiles/{id}/{revision}/source-gdtf",
            put(put_fixture_profile_source_gdtf),
        )
}

fn media_and_output_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/visualization", get(visualization_snapshot))
        .route("/api/v1/media", get(media_servers))
        .route(
            "/api/v1/media/{fixture_id}/thumbnails/refresh",
            post(refresh_media_thumbnails),
        )
        .route("/api/v1/media/{fixture_id}/thumbnail", get(media_thumbnail))
        .route(
            "/api/v1/media/{fixture_id}/preview/refresh",
            post(refresh_media_preview),
        )
        .route(
            "/api/v1/media/{fixture_id}/preview/{source}",
            get(media_preview),
        )
        .route("/api/v1/dmx", get(dmx_snapshot))
        .route("/api/v1/dmx/override", put(update_dmx_override))
        .route("/api/v1/shutdown", post(shutdown_server))
        .route(
            "/api/v1/configuration",
            get(configuration).put(update_configuration),
        )
        .route("/api/v1/matter/status", get(matter_bridge_status))
        .route(
            "/api/v1/speed-groups/{group}",
            get(speed_group).put(update_speed_group),
        )
        .route(
            "/api/v1/speed-groups/{group}/observation",
            post(observe_speed_group),
        )
        .route(
            "/api/v1/speed-groups/{group}/action",
            post(speed_group_action),
        )
}

fn session_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sessions", post(create_session))
        .route("/api/v1/sessions/{id}", delete(close_session))
        .route("/api/v1/clients/{id}", delete(remove_client))
        .route("/api/v1/desk-lock", get(desk_lock).put(update_desk_lock))
        .route("/api/v1/desk-lock/lock", post(lock_desk))
        .route("/api/v1/desk-lock/unlock", post(unlock_desk))
        .route("/api/v1/desk-lock/force-unlock", post(force_unlock_desk))
        .route("/api/v1/users", post(create_user))
        .route("/api/v1/users/{id}", put(update_user).delete(delete_user))
}

fn show_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/shows", get(list_shows).post(upload_show))
        .route("/api/v1/shows/default/open", post(open_clean_default_show))
        .route("/api/v1/shows/rollback", post(rollback_show))
        .route("/api/v1/shows/{id}", delete(delete_show))
        .route("/api/v1/shows/{id}/open", post(open_show))
        .route("/api/v1/shows/{id}/rename", put(rename_show))
        .route("/api/v1/shows/{id}/download", get(download_show))
        .route(
            "/api/v1/shows/{source_id}/overwrite/{destination_id}",
            post(overwrite_show),
        )
        .route(
            "/api/v1/shows/{id}/revisions",
            get(list_show_revisions).post(save_show_revision),
        )
        .route(
            "/api/v1/shows/{id}/revisions/{revision}/open",
            post(open_show_revision),
        )
        .route("/api/v1/mvr/imports/preview", post(preview_mvr_import))
        .route("/api/v1/mvr/imports/{token}/apply", post(apply_mvr_import))
        .route("/api/v1/shows/{id}/mvr/preview", get(preview_mvr_export))
        .route("/api/v1/shows/{id}/mvr", get(export_mvr))
}

fn show_object_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/shows/{id}/objects/{kind}", get(list_objects))
        .route(
            "/api/v1/shows/{id}/objects/{kind}/{object_id}",
            get(get_object).put(put_object).delete(delete_object),
        )
        .route(
            "/api/v1/shows/{id}/objects/{kind}/{object_id}/undo",
            post(undo_object),
        )
        .route(
            "/api/v1/shows/{id}/presets/{preset_id}/store",
            post(store_preset),
        )
        .route("/api/v1/shows/{id}/preload/store", post(store_preload))
}

fn playback_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/playbacks/{id}/{action}", post(playback_action))
        .route("/api/v1/cuelists/{number}", get(pool_playback_state))
        .route(
            "/api/v1/cuelists/{number}/{action}",
            post(pool_playback_action).put(pool_playback_action),
        )
        .route("/api/v1/qlists/{number}", get(pool_playback_state))
        .route(
            "/api/v1/qlists/{number}/{action}",
            post(pool_playback_action).put(pool_playback_action),
        )
        .route("/api/v1/playback-pool/{number}", get(pool_playback_state))
        .route(
            "/api/v1/playback-pool/{number}/{action}",
            post(pool_playback_action).put(pool_playback_action),
        )
        .route("/api/v1/control-desks/{id}/page", put(update_desk_page))
        .route("/api/v1/control-desks/{id}", put(update_control_desk))
        .route(
            "/api/v1/control-desks/{id}/page-playbacks/{slot}/{action}",
            post(paged_playback_action).put(paged_playback_action),
        )
        .route(
            "/api/v1/control-desks/{id}/paged-playbacks/{slot}/{action}",
            post(paged_playback_action).put(paged_playback_action),
        )
        .route("/api/v1/screens", get(list_screens))
        .route(
            "/api/v1/screens/{id}",
            put(put_screen).delete(delete_screen),
        )
        .route("/api/v1/screens/{id}/page", put(update_screen_page))
        .route("/api/v1/playbacks", get(playbacks))
        .route(
            "/api/v1/virtual-playback-exclusion-zones",
            get(virtual_playback_exclusion_zones),
        )
        .route(
            "/api/v1/virtual-playback-exclusion-zones/{surface_id}",
            put(put_virtual_playback_exclusion_zones),
        )
        .route(
            "/api/v1/playback-pages/{page}/slots/{slot}",
            put(upsert_playback_slot).delete(clear_playback_slot),
        )
}

fn programmer_and_update_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/programmers", get(list_programmers))
        .route("/api/v1/programmers/{id}/clear", post(clear_programmer))
        .route("/api/v1/programmer/set", post(set_programmer))
        .merge(command_http::router())
        .route(
            "/api/v1/update/settings",
            get(update_settings).put(put_update_settings),
        )
        .route("/api/v1/update/preview", post(preview_update))
        .route("/api/v1/update/apply", post(apply_update))
        .route("/api/v1/update/targets", get(update_targets))
        .route("/api/v1/highlight", get(highlight_status))
        .route("/api/v1/highlight/action", post(highlight_action))
        .route(
            "/api/v1/patch-preview-highlight",
            put(patch_preview_highlight),
        )
        .route("/api/v1/master", put(update_master))
        .route("/api/v1/midi/inputs", get(midi_inputs))
        .route("/api/v1/events", get(ws_events))
        .route("/api/v1/command-history", get(command_history))
        .route("/api/v1/audit", get(audit_events))
}

fn with_test_routes(router: Router<AppState>, enabled: bool) -> Router<AppState> {
    if !enabled {
        return router;
    }
    router
        .route("/api/v1/test/clock/reset", post(reset_test_clock))
        .route("/api/v1/test/clock/advance", post(advance_test_clock))
        .route("/api/v1/test/output/failure", post(set_test_output_failure))
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
        ])
        .expose_headers([
            header::ETAG,
            header::ACCEPT_RANGES,
            header::CONTENT_RANGE,
            header::CONTENT_LENGTH,
        ])
}
