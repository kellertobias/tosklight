//! `GET /api/v2/output/readouts`: authenticated whole-snapshot read of typed readouts from one
//! accepted source, leased to the calling session (TL-594 C2).
//!
//! API rules: a read (rule 1) answered with a complete typed snapshot (rule 5); the optional
//! `X-Tosk-Show` guard rejects a show-switch race with 409 and a malformed id with 400 (rule 6);
//! there is no show or desk segment in the path. Unknown query parameters are ignored. The
//! response is `no-store` because it renews the session's lease of the source it delivered.

use super::{ApiError, AppState, ShowContext, authenticate};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::get,
};
use light_core::FixtureId;
use light_wire::v2::visualization::VisualizationLane;
use serde::Deserialize;
use uuid::Uuid;

/// Owners per readout request; larger selections read in pages.
const MAX_READOUT_OWNERS: usize = 512;

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v2/output/readouts", get(readouts))
}

#[derive(Deserialize)]
struct ReadoutQuery {
    /// `normal` (default) or `preload`. Preload never falls back to Live.
    #[serde(default)]
    lane: Option<VisualizationLane>,
    /// Comma-separated fixture ids in display order; duplicates are kept.
    #[serde(default)]
    fixture_ids: Option<String>,
}

async fn readouts(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
    Query(query): Query<ReadoutQuery>,
) -> Result<Response, ApiError> {
    let session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    let owners = requested_owners(query.fixture_ids.as_deref())?;
    let lane = query.lane.unwrap_or(VisualizationLane::Normal);
    let snapshot = tokio::task::spawn_blocking(move || {
        super::output_readouts::read_readouts(&state, session.id, lane, &owners)
    })
    .await
    .map_err(|error| ApiError::internal(format!("readout task failed: {error}")))?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(snapshot)).into_response())
}

fn requested_owners(list: Option<&str>) -> Result<Vec<FixtureId>, ApiError> {
    let owners = list
        .filter(|list| !list.trim().is_empty())
        .map(|list| {
            list.split(',')
                .map(|id| {
                    Uuid::parse_str(id.trim()).map(FixtureId).map_err(|_| {
                        ApiError::bad_request(format!("fixture_ids: invalid fixture id `{id}`"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if owners.len() > MAX_READOUT_OWNERS {
        return Err(ApiError::bad_request(format!(
            "fixture_ids: at most {MAX_READOUT_OWNERS} fixtures per readout"
        )));
    }
    Ok(owners)
}
