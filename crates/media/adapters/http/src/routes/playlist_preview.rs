//! HTTP equivalents for the ordered, non-replayed playlist preview actions.
use super::ApiState;
use crate::{
    ApiError, TolerantJson,
    wire::{PlaylistPreviewResult, PreviewPlaylistStep, StopPlaylistPreview},
};
use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};

pub(super) fn show_action(
    state: &ApiState,
    request: &PreviewPlaylistStep,
) -> Result<PlaylistPreviewResult, ApiError> {
    let now = (state.now)();
    let (active, output) = state
        .diagnostics
        .automation
        .previews
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .show(
            &state.state,
            &state.configuration.load(),
            &state.catalog.load(),
            now,
            request,
        )?;
    Ok(PlaylistPreviewResult {
        active: Some(active),
        output: super::outputs::view_of(state, &output, now),
    })
}
pub(super) fn stop_action(
    state: &ApiState,
    request: &StopPlaylistPreview,
) -> Result<PlaylistPreviewResult, ApiError> {
    let now = (state.now)();
    let output = state
        .diagnostics
        .automation
        .previews
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .stop(&state.state, now, request)?;
    Ok(PlaylistPreviewResult {
        active: None,
        output: super::outputs::view_of(state, &output, now),
    })
}
pub(super) async fn show(
    State(state): State<ApiState>,
    TolerantJson(request): TolerantJson<PreviewPlaylistStep>,
) -> Result<Response, ApiError> {
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(show_action(&state, &request)?),
    )
        .into_response())
}
pub(super) async fn stop(
    State(state): State<ApiState>,
    TolerantJson(request): TolerantJson<StopPlaylistPreview>,
) -> Result<Response, ApiError> {
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(stop_action(&state, &request)?),
    )
        .into_response())
}
