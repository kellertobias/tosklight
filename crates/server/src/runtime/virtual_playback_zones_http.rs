//! Explicitly scoped exclusion-zone snapshot and mutation routes.

use super::{
    ApiError, AppState, Session, authenticate, emit,
    playback_api::{
        VirtualPlaybackExclusionZone, VirtualPlaybackExclusionZoneInput,
        read_virtual_playback_exclusion_store, validate_virtual_playback_exclusion_zones,
        write_virtual_playback_exclusion_surface,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, put},
};
use light_core::ShowId;
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/shows/{show_id}/desks/{desk_id}/virtual-playback-exclusion-zones",
            get(snapshot),
        )
        .route(
            "/api/v2/shows/{show_id}/desks/{desk_id}/virtual-playback-exclusion-zones/{surface_id}",
            put(save_surface),
        )
}

#[derive(Serialize)]
struct SnapshotResponse {
    show_id: ShowId,
    desk_id: Uuid,
    surfaces: HashMap<String, Vec<VirtualPlaybackExclusionZone>>,
}

#[derive(Serialize)]
struct SaveResponse {
    show_id: ShowId,
    desk_id: Uuid,
    surface_id: String,
    zones: Vec<VirtualPlaybackExclusionZone>,
}

async fn snapshot(
    State(state): State<AppState>,
    Path((show_id, desk_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<SnapshotResponse>, ApiError> {
    let session = authenticated_desk(&state, &headers, desk_id)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let show_id = require_active_show(&state, show_id)?;
    let surfaces = read_virtual_playback_exclusion_store(&state.desk.lock(), show_id)
        .remove(&session.desk.id.to_string())
        .unwrap_or_default();
    Ok(Json(SnapshotResponse {
        show_id,
        desk_id: session.desk.id,
        surfaces,
    }))
}

async fn save_surface(
    State(state): State<AppState>,
    Path((show_id, desk_id, surface_id)): Path<(Uuid, Uuid, String)>,
    headers: HeaderMap,
    Json(input): Json<VirtualPlaybackExclusionZoneInput>,
) -> Result<Json<SaveResponse>, ApiError> {
    let session = authenticated_desk(&state, &headers, desk_id)?;
    validate_surface_id(&surface_id)?;
    let zones = validate_virtual_playback_exclusion_zones(input)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let show_id = require_active_show(&state, show_id)?;
    write_surface(&state, &session, show_id, &surface_id, &zones)?;
    Ok(Json(SaveResponse {
        show_id,
        desk_id: session.desk.id,
        surface_id,
        zones,
    }))
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

fn require_active_show(state: &AppState, requested: Uuid) -> Result<ShowId, ApiError> {
    let active = state
        .active_show
        .read()
        .as_ref()
        .map(|show| show.id)
        .ok_or_else(|| ApiError::conflict("no show is active"))?;
    if active.0 != requested {
        return Err(ApiError::conflict("requested show is no longer active"));
    }
    Ok(active)
}

fn validate_surface_id(surface_id: &str) -> Result<(), ApiError> {
    if surface_id.trim().is_empty() || surface_id.len() > 128 {
        return Err(ApiError::bad_request(
            "surface id must contain 1-128 characters",
        ));
    }
    Ok(())
}

fn write_surface(
    state: &AppState,
    session: &Session,
    show_id: ShowId,
    surface_id: &str,
    zones: &[VirtualPlaybackExclusionZone],
) -> Result<(), ApiError> {
    write_virtual_playback_exclusion_surface(
        &state.desk.lock(),
        show_id,
        session.desk.id,
        surface_id,
        zones,
    )?;
    emit(
        state,
        "virtual_playback_exclusion_zones_changed",
        serde_json::json!({
            "desk_id": session.desk.id,
            "show_id": show_id,
            "surface_id": surface_id,
            "zones": zones,
        }),
    );
    Ok(())
}
