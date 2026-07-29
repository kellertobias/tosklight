//! Show-level snapshot and replay-safe exclusion-zone updates.

use super::{
    ApiError, AppState, ShowContext, TolerantJson, authenticate, emit,
    playback_api::{VirtualPlaybackExclusionZone, validate_virtual_playback_exclusion_zones},
};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::HeaderMap,
    routing::{get, post},
};
use light_application::{EventDraft, VirtualPlaybackExclusionZonesChange};
use light_wire::v2::virtual_playback_zones::{
    VirtualPlaybackExclusionSnapshot, VirtualPlaybackExclusionUpdateOutcome,
    VirtualPlaybackExclusionUpdateRequest,
};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/virtual-playback-exclusion-zones", get(snapshot))
        .route(
            "/api/v2/virtual-playback-exclusion-zones/update",
            post(update_zones),
        )
}

async fn snapshot(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Json<VirtualPlaybackExclusionSnapshot>, ApiError> {
    authenticate(&state, &headers)?;
    let _activation = state.active_show.acquire().await;
    let show_id = show.resolve(&state)?;
    let stored = super::playback_api::read_virtual_playback_exclusions(&state, show_id)?;
    Ok(Json(VirtualPlaybackExclusionSnapshot {
        show_id: show_id.0,
        revision: stored.revision,
        zones: stored.zones,
    }))
}

async fn update_zones(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
    request: Result<TolerantJson<VirtualPlaybackExclusionUpdateRequest>, JsonRejection>,
) -> Result<Json<VirtualPlaybackExclusionUpdateOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    validate_request_id(&request.request_id)?;
    let zones = validate_virtual_playback_exclusion_zones(request.zones)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };

    let _activation = state.active_show.acquire().await;
    let show_id = show.resolve(&state)?;
    let action = ReplayAction {
        show_id: show_id.0,
        expected_revision: request.expected_revision,
        zones: zones.clone(),
    };
    if let Some(outcome) = state
        .replay
        .lookup_virtual_playback_zones(&key, &action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let (changed, stored) = super::playback_api::update_virtual_playback_exclusions(
        &state,
        show_id,
        request.expected_revision,
        &zones,
        &request.request_id,
    )?;
    let outcome = VirtualPlaybackExclusionUpdateOutcome {
        request_id: request.request_id,
        show_id: show_id.0,
        revision: stored.revision,
        zones: stored.zones,
        replayed: false,
        changed,
    };
    state
        .replay
        .insert_virtual_playback_zones(key, action, outcome.clone())
        .await;

    if changed {
        state
            .events
            .publish(EventDraft::virtual_playback_exclusion_zones_changed(
                VirtualPlaybackExclusionZonesChange {
                    show_id,
                    revision: outcome.revision,
                },
            ));
        // Retained for the compatibility event stream until its dedicated retirement chunk.
        emit(
            &state,
            "virtual_playback_exclusion_zones_changed",
            serde_json::json!({
                "show_id": show_id,
                "revision": outcome.revision,
                "zones": zones,
            }),
        );
    }
    Ok(Json(outcome))
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    session_id: Uuid,
    request_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReplayAction {
    show_id: Uuid,
    expected_revision: u64,
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
    pub(super) fn get(
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

    pub(super) fn insert(
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
