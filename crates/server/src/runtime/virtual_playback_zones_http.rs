//! Show-level snapshot and replay-safe exclusion-zone updates.

use super::{
    ApiError, AppState, authenticate, emit,
    playback_api::{
        VirtualPlaybackExclusionZone, read_virtual_playback_exclusion_store,
        validate_virtual_playback_exclusion_zones, write_virtual_playback_exclusion_surface,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::{get, post},
};
use light_application::{EventDraft, VirtualPlaybackExclusionZonesChange};
use light_core::ShowId;
use light_wire::v2::virtual_playback_zones::{
    VirtualPlaybackExclusionSnapshot, VirtualPlaybackExclusionUpdateOutcome,
    VirtualPlaybackExclusionUpdateRequest,
};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/shows/{show_id}/virtual-playback-exclusion-zones",
            get(snapshot),
        )
        .route(
            "/api/v2/shows/{show_id}/virtual-playback-exclusion-zones/{surface_id}/update",
            post(update_surface),
        )
}

async fn snapshot(
    State(state): State<AppState>,
    Path(show_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<VirtualPlaybackExclusionSnapshot>, ApiError> {
    authenticate(&state, &headers)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let show_id = require_active_show(&state, show_id)?;
    let desks = read_virtual_playback_exclusion_store(&state.desk.lock(), show_id);
    Ok(Json(VirtualPlaybackExclusionSnapshot {
        show_id: show_id.0,
        desks,
    }))
}

async fn update_surface(
    State(state): State<AppState>,
    Path((show_id, surface_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
    request: Result<Json<VirtualPlaybackExclusionUpdateRequest>, JsonRejection>,
) -> Result<Json<VirtualPlaybackExclusionUpdateOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_surface_id(&surface_id)?;
    let Json(request) = request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    validate_request_id(&request.request_id)?;
    let zones = validate_virtual_playback_exclusion_zones(request.zones)?;
    let action = ReplayAction {
        show_id,
        surface_id: surface_id.clone(),
        zones: zones.clone(),
    };
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };

    let _activation = state.activation_lock.clone().lock_owned().await;
    if let Some(outcome) = state
        .virtual_playback_zones_replay
        .lock()
        .get(&key, &action)?
    {
        return Ok(Json(outcome));
    }
    let show_id = require_active_show(&state, show_id)?;
    let desk_id = session.desk.id;
    let changed = {
        let desk = state.desk.lock();
        let stored = read_virtual_playback_exclusion_store(&desk, show_id);
        let previous = stored
            .get(&desk_id.to_string())
            .and_then(|surfaces| surfaces.get(&surface_id));
        if previous == Some(&zones) || (previous.is_none() && zones.is_empty()) {
            false
        } else {
            write_virtual_playback_exclusion_surface(&desk, show_id, desk_id, &surface_id, &zones)?;
            true
        }
    };
    let outcome = VirtualPlaybackExclusionUpdateOutcome {
        request_id: request.request_id,
        show_id: show_id.0,
        desk_id,
        surface_id: surface_id.clone(),
        zones: zones.clone(),
        replayed: false,
        changed,
    };
    state
        .virtual_playback_zones_replay
        .lock()
        .insert(key, action, outcome.clone());

    if changed {
        state
            .application_events
            .publish(EventDraft::virtual_playback_exclusion_zones_changed(
                VirtualPlaybackExclusionZonesChange {
                    show_id,
                    desk_id,
                    surface_id: surface_id.clone(),
                },
            ));
        // Retained for the compatibility event stream until its dedicated retirement chunk.
        emit(
            &state,
            "virtual_playback_exclusion_zones_changed",
            serde_json::json!({
                "desk_id": desk_id,
                "show_id": show_id,
                "surface_id": surface_id,
                "zones": zones,
            }),
        );
    }
    Ok(Json(outcome))
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

fn validate_request_id(request_id: &str) -> Result<(), ApiError> {
    if request_id.trim().is_empty()
        || request_id.len() > 128
        || request_id.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request(
            "request_id must contain 1-128 printable characters",
        ));
    }
    Ok(())
}

fn validate_surface_id(surface_id: &str) -> Result<(), ApiError> {
    if surface_id.is_empty()
        || surface_id.len() > 128
        || surface_id != surface_id.trim()
        || surface_id.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request(
            "surface_id must be a trimmed string containing 1-128 printable characters",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    session_id: Uuid,
    request_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReplayAction {
    show_id: Uuid,
    surface_id: String,
    zones: Vec<VirtualPlaybackExclusionZone>,
}

struct ReplayEntry {
    action: ReplayAction,
    outcome: VirtualPlaybackExclusionUpdateOutcome,
}

#[derive(Default)]
pub(super) struct VirtualPlaybackZonesReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl VirtualPlaybackZonesReplayCache {
    fn get(
        &self,
        key: &ReplayKey,
        action: &ReplayAction,
    ) -> Result<Option<VirtualPlaybackExclusionUpdateOutcome>, ApiError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if &entry.action != action {
            return Err(ApiError::conflict(
                "request_id was already used for a different exclusion-zone update",
            ));
        }
        let mut replay = entry.outcome.clone();
        replay.replayed = true;
        Ok(Some(replay))
    }

    fn insert(
        &mut self,
        key: ReplayKey,
        action: ReplayAction,
        outcome: VirtualPlaybackExclusionUpdateOutcome,
    ) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, ReplayEntry { action, outcome });
        while self.entries.len() > REQUEST_CACHE_ENTRY_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}
