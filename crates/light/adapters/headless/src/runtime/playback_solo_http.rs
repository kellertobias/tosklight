//! Show-level snapshot and replay-safe exclusion-zone updates.

use super::{
    ApiError, AppState, ShowContext, TolerantJson, authenticate, emit,
    playback_api::{PlaybackSoloRegion, validate_playback_solo_regions},
};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::HeaderMap,
    routing::{get, post},
};
use light_application::{EventDraft, PlaybackSoloRegionsChange};
use light_wire::v2::playback_solo::{
    PlaybackSoloSnapshot, PlaybackSoloUpdateOutcome, PlaybackSoloUpdateRequest,
};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/playback-solo-regions", get(snapshot))
        .route("/api/v2/playback-solo-regions/update", post(update_regions))
}

async fn snapshot(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Json<PlaybackSoloSnapshot>, ApiError> {
    authenticate(&state, &headers)?;
    let _activation = state.active_show.acquire().await;
    let show_id = show.resolve(&state)?;
    if state.active_show.in_recovery() {
        return Ok(Json(PlaybackSoloSnapshot {
            show_id: show_id.0,
            revision: 0,
            regions: Vec::new(),
        }));
    }
    let stored = super::playback_api::read_playback_solo(&state, show_id)?;
    Ok(Json(PlaybackSoloSnapshot {
        show_id: show_id.0,
        revision: stored.revision,
        regions: stored.regions,
    }))
}

async fn update_regions(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
    request: Result<TolerantJson<PlaybackSoloUpdateRequest>, JsonRejection>,
) -> Result<Json<PlaybackSoloUpdateOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    validate_request_id(&request.request_id)?;
    let regions = validate_playback_solo_regions(request.regions)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };

    let _activation = state.active_show.acquire().await;
    let show_id = show.resolve_writable(&state)?;
    let action = ReplayAction {
        show_id: show_id.0,
        expected_revision: request.expected_revision,
        regions: regions.clone(),
    };
    if let Some(outcome) = state.replay.lookup_playback_solo(&key, &action).await? {
        return Ok(Json(outcome));
    }
    let (changed, stored) = super::playback_api::update_playback_solo(
        &state,
        &light_application::ActionContext::operator(
            session.desk.id,
            session.id.0,
            light_application::ActionSource::Http,
        ),
        show_id,
        request.expected_revision,
        &regions,
        &request.request_id,
    )?;
    let outcome = PlaybackSoloUpdateOutcome {
        request_id: request.request_id,
        show_id: show_id.0,
        revision: stored.revision,
        regions: stored.regions,
        replayed: false,
        changed,
    };
    state
        .replay
        .insert_playback_solo(key, action, outcome.clone())
        .await;

    if changed {
        state
            .events
            .publish(EventDraft::playback_solo_regions_changed(
                PlaybackSoloRegionsChange {
                    show_id,
                    revision: outcome.revision,
                },
            ));
        // Retained for the compatibility event stream until its dedicated retirement chunk.
        emit(
            &state,
            "playback_solo_regions_changed",
            serde_json::json!({
                "show_id": show_id,
                "revision": outcome.revision,
                "regions": regions,
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
    regions: Vec<PlaybackSoloRegion>,
}

struct ReplayEntry {
    action: ReplayAction,
    outcome: PlaybackSoloUpdateOutcome,
}

#[derive(Default)]
pub(super) struct PlaybackSoloReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl PlaybackSoloReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        action: &ReplayAction,
    ) -> Result<Option<PlaybackSoloUpdateOutcome>, ApiError> {
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
        outcome: PlaybackSoloUpdateOutcome,
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
