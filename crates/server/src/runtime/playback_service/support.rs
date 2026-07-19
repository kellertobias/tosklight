use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActionSource, PlaybackSurface,
};

use super::{ApiError, AppState, Session};

pub(super) fn playback_definition(
    state: &AppState,
    number: u16,
) -> Result<light_playback::PlaybackDefinition, ActionError> {
    state
        .engine
        .snapshot()
        .playbacks
        .iter()
        .find(|playback| playback.number == number)
        .cloned()
        .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "playback"))
}

pub(super) fn operator_context(
    session: &Session,
    desk_id: uuid::Uuid,
    source: ActionSource,
    request_id: Option<&str>,
) -> ActionContext {
    let context = ActionContext::operator(desk_id, session.user.id.0, session.id.0, source);
    request_id.map_or(context.clone(), |id| context.with_request_id(id))
}

pub(super) fn capture_enabled(state: &AppState, surface: PlaybackSurface) -> bool {
    let configuration = state.configuration.read();
    if surface == PlaybackSurface::Virtual {
        configuration.preload_virtual_playback_actions
    } else {
        configuration.preload_physical_playback_actions
    }
}

pub(super) fn captures_preload(source: ActionSource) -> bool {
    matches!(
        source,
        ActionSource::UserInterface | ActionSource::Keyboard | ActionSource::Http
    )
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
