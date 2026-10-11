use light_application::{ActionContext, ActionError, ActionErrorKind, ActionSource};

use super::{ApiError, AppState, Session};

pub(super) fn playback_definition(
    state: &AppState,
    address: light_playback::PlaybackAddress,
) -> Result<light_playback::PlaybackDefinition, ActionError> {
    state
        .output
        .snapshot()
        .playback_pages
        .iter()
        .find(|page| page.number == address.page())
        .and_then(|page| page.assignments.get(&address.position()))
        .cloned()
        .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "Playback not found"))
}
pub(super) fn resolve_group_playback(
    snapshot: &light_engine::EngineSnapshot,
    group_id: &str,
) -> Result<Option<light_playback::PlaybackAddress>, ActionError> {
    if !snapshot.groups.iter().any(|group| group.id == group_id) {
        return Err(ActionError::new(
            ActionErrorKind::NotFound,
            "Group not found",
        ));
    }
    Ok(snapshot.playback_pages.iter().flat_map(|page| page.assignments.iter().filter_map(move |(position,definition)| {
      matches!(&definition.target,light_playback::PlaybackTarget::Group{group_id:assigned,..} if assigned==group_id)
        .then(||light_playback::PlaybackAddress::new(page.number,*position).ok()).flatten()
    })).min())
}

pub(super) fn operator_context(
    session: &Session,
    desk_id: uuid::Uuid,
    source: ActionSource,
    request_id: Option<&str>,
) -> ActionContext {
    let context = ActionContext::operator(desk_id, session.id.0, source);
    request_id.map_or(context.clone(), |id| context.with_request_id(id))
}

pub(super) fn capture_enabled(state: &AppState, address: light_playback::PlaybackAddress) -> bool {
    state
        .output
        .snapshot()
        .playback_preload_scope
        .contains(address)
}

pub(super) fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

pub(super) fn api_action_error(error: ApiError) -> ActionError {
    let kind = match error.status {
        axum::http::StatusCode::UNAUTHORIZED => ActionErrorKind::Unauthorized,
        axum::http::StatusCode::FORBIDDEN => ActionErrorKind::Forbidden,
        axum::http::StatusCode::NOT_FOUND => ActionErrorKind::NotFound,
        axum::http::StatusCode::CONFLICT => ActionErrorKind::Conflict,
        axum::http::StatusCode::SERVICE_UNAVAILABLE => ActionErrorKind::Unavailable,
        status if status.is_server_error() => ActionErrorKind::Internal,
        _ => ActionErrorKind::Invalid,
    };
    ActionError::new(kind, error.message)
}

pub(super) fn action_error(error: ActionError) -> ApiError {
    match error.kind {
        ActionErrorKind::Invalid => ApiError::bad_request(error.message),
        ActionErrorKind::Unauthorized => ApiError::unauthorized(error.message),
        ActionErrorKind::Forbidden => ApiError::forbidden(error.message),
        ActionErrorKind::NotFound if error.message.ends_with("not found") => ApiError {
            status: axum::http::StatusCode::NOT_FOUND,
            message: error.message,
        },
        ActionErrorKind::NotFound => ApiError::not_found(error.message),
        ActionErrorKind::Conflict | ActionErrorKind::Busy => ApiError::conflict(error.message),
        ActionErrorKind::Unavailable => ApiError::unavailable(error.message),
        ActionErrorKind::Internal => ApiError::internal(error.message),
    }
}
