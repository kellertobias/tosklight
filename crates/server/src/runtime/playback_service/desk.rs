//! Desk-local Playback view projection events for mutations outside Playback commands.

use light_application::{ActionContext, EventDraft, PlaybackDeskProjection, PlaybackPorts};

use super::{ApiError, AppState, ServerPlaybackPorts, action_error};

pub(in crate::runtime) fn projection(
    state: &AppState,
    context: &ActionContext,
) -> Result<PlaybackDeskProjection, ApiError> {
    let ports = ServerPlaybackPorts {
        state,
        session: None,
        desk: None,
        persistence_pending: std::sync::atomic::AtomicBool::new(false),
    };
    PlaybackPorts::desk_projection(&ports, context)
        .map_err(action_error)?
        .ok_or_else(|| ApiError::internal("playback desk projection unavailable"))
}

pub(in crate::runtime) fn publish_change(
    state: &AppState,
    context: &ActionContext,
    before: PlaybackDeskProjection,
) -> Result<Option<u64>, ApiError> {
    let after = projection(state, context)?;
    if after == before {
        return Ok(None);
    }
    Ok(Some(
        state
            .playback_service
            .events()
            .publish(EventDraft::playback_view_changed(context, after))
            .sequence,
    ))
}
