//! Authenticated snapshot of Internal Audio Player and local-library availability.

use super::*;
use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v2/internal-audio/status", get(status))
}

async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<light_wire::v2::internal_audio::InternalAudioStatus>, ApiError> {
    authenticate(&state, &headers)?;
    Ok(Json(state.internal_audio.status()))
}
