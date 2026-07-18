use super::{
    ApiError, AppState, ControlDesk, PoolPlaybackInput, Session, cuelist_for_page_playback,
    dispatch_playback_action, emit, intercept_update_playback_target, persist_active_playbacks,
    persist_programmer, predicted_preload_temp_state, preload_capture_action_with_temp_state,
};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    PendingPlaybackAction, PlaybackAction, PlaybackAddress, PlaybackCommand, PlaybackExecution,
    PlaybackPorts, PlaybackResult, PlaybackSurface, ResolvedPlaybackAddress,
};

#[path = "playback_service/conversion.rs"]
mod conversion;
#[path = "playback_service/response.rs"]
mod response;

pub(super) use response::{cue_list_http_payload, pool_http_payload, websocket_payload};

use conversion::{
    action_touched, legacy_action, parse_action, parse_pending, parse_surface, source_name,
    surface_name,
};

pub(super) fn http_action(
    state: &AppState,
    session: &Session,
    address: PlaybackAddress,
    action_name: &str,
    input: &PoolPlaybackInput,
) -> Result<PlaybackResult, ApiError> {
    let command = PlaybackCommand {
        address,
        action: parse_action(action_name, input)?,
        surface: parse_surface(input.surface.as_deref()),
    };
    let context = operator_context(session, session.desk.id, ActionSource::Http, None);
    execute(state, Some(session), Some(&session.desk), context, command)
}

pub(super) fn osc_action(
    state: &AppState,
    session: Option<&Session>,
    desk: Option<&ControlDesk>,
    address: PlaybackAddress,
    action_name: &str,
    input: &PoolPlaybackInput,
) -> Result<PlaybackResult, ApiError> {
    let command = PlaybackCommand {
        address,
        action: parse_action(action_name, input)?,
        surface: PlaybackSurface::Osc,
    };
    let desk_id = desk
        .map(|desk| desk.id)
        .or_else(|| session.map(|session| session.desk.id))
        .unwrap_or_default();
    let context = session.map_or_else(
        || ActionContext::system(desk_id, ActionSource::Osc),
        |session| operator_context(session, desk_id, ActionSource::Osc, None),
    );
    execute(state, session, desk, context, command)
}

pub(super) fn websocket_action(
    state: &AppState,
    session: &Session,
    address: PlaybackAddress,
    action: PlaybackAction,
    request_id: &str,
) -> Result<PlaybackResult, ApiError> {
    let context = operator_context(
        session,
        session.desk.id,
        ActionSource::UserInterface,
        Some(request_id),
    );
    let command = PlaybackCommand {
        address,
        action,
        surface: PlaybackSurface::Virtual,
    };
    execute(state, Some(session), Some(&session.desk), context, command)
}

fn execute(
    state: &AppState,
    session: Option<&Session>,
    desk: Option<&ControlDesk>,
    context: ActionContext,
    command: PlaybackCommand,
) -> Result<PlaybackResult, ApiError> {
    let ports = ServerPlaybackPorts {
        state,
        session,
        desk,
    };
    state
        .playback_service
        .handle(ActionEnvelope { context, command }, &ports)
        .map_err(action_error)
}

struct ServerPlaybackPorts<'a> {
    pub(super) state: &'a AppState,
    pub(super) session: Option<&'a Session>,
    pub(super) desk: Option<&'a ControlDesk>,
}

impl PlaybackPorts for ServerPlaybackPorts<'_> {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        let Some(session_id) = context.session_id else {
            return Ok(());
        };
        self.session
            .filter(|session| session.id.0 == session_id)
            .map(|_| ())
            .ok_or_else(|| ActionError::new(ActionErrorKind::Unauthorized, "invalid session"))
    }

    fn current_page(&self, context: &ActionContext) -> Result<u8, ActionError> {
        let show = self
            .state
            .active_show
            .read()
            .clone()
            .ok_or_else(|| invalid("no show is open"))?;
        self.state
            .desk
            .lock()
            .desk_page(context.desk_id, show.id)
            .map_err(|error| invalid(error.to_string()))
    }

    fn playback_at(&self, page: u8, slot: u8) -> Result<Option<u16>, ActionError> {
        Ok(cuelist_for_page_playback(
            &self.state.engine.snapshot(),
            page,
            slot,
        ))
    }

    fn execute(
        &self,
        context: &ActionContext,
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
        surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        match address {
            ResolvedPlaybackAddress::CueList(id) => self.execute_cue_list(id, action),
            ResolvedPlaybackAddress::Pool { number, .. } => {
                self.execute_pool(context, number, action, surface)
            }
        }
    }
}

impl ServerPlaybackPorts<'_> {
    fn execute_cue_list(
        &self,
        id: light_core::CueListId,
        action: PlaybackAction,
    ) -> Result<PlaybackExecution, ActionError> {
        let playback = self.state.engine.playback();
        let mut playback = playback.write();
        let execution = match action {
            PlaybackAction::Go { pressed: true } => {
                PlaybackExecution::Active(Box::new(playback.go(id).map_err(invalid)?.clone()))
            }
            PlaybackAction::Back { pressed: true } => {
                PlaybackExecution::Active(Box::new(playback.back(id).map_err(invalid)?.clone()))
            }
            PlaybackAction::Pause { pressed: true } => {
                playback.pause(id).map_err(invalid)?;
                PlaybackExecution::ActiveList(playback.active())
            }
            PlaybackAction::Release => PlaybackExecution::Released(playback.release(id)),
            _ => return Err(invalid("action is incompatible with a cue list")),
        };
        drop(playback);
        persist_active_playbacks(self.state).map_err(api_action_error)?;
        Ok(execution)
    }

    fn execute_pool(
        &self,
        context: &ActionContext,
        number: u16,
        action: PlaybackAction,
        surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        let definition = playback_definition(self.state, number)?;
        let (action_name, input) = legacy_action(action, surface);
        if captures_preload(context.source)
            && let Some(pending) = self.capture(&definition, action_name, &input, surface)?
        {
            return Ok(PlaybackExecution::Pool {
                changed: false,
                pending: Some(pending),
            });
        }
        if self.intercept_update(context, &definition, action) {
            return Ok(PlaybackExecution::Pool {
                changed: false,
                pending: None,
            });
        }
        let changed = dispatch_playback_action(
            self.state,
            self.session,
            self.desk,
            &definition,
            action_name,
            &input,
            source_name(context.source),
        )
        .map_err(api_action_error)?;
        Ok(PlaybackExecution::Pool {
            changed,
            pending: None,
        })
    }

    fn intercept_update(
        &self,
        context: &ActionContext,
        definition: &light_playback::PlaybackDefinition,
        action: PlaybackAction,
    ) -> bool {
        context.source == ActionSource::Osc
            && self.session.is_some_and(|session| {
                intercept_update_playback_target(
                    self.state,
                    session,
                    definition,
                    action_touched(action),
                )
            })
    }

    fn capture(
        &self,
        definition: &light_playback::PlaybackDefinition,
        action_name: &str,
        input: &PoolPlaybackInput,
        surface: PlaybackSurface,
    ) -> Result<Option<PendingPlaybackAction>, ActionError> {
        let Some(session) = self.session else {
            return Ok(None);
        };
        let temp = predicted_preload_temp_state(self.state, session.id, definition.number);
        let pending = preload_capture_action_with_temp_state(definition, action_name, input, temp)
            .map_err(api_action_error)?;
        if !self.should_capture(session, pending, surface) {
            return Ok(None);
        }
        let pending = pending.expect("capture requires a pending action");
        self.queue_capture(session, definition.number, pending, surface)?;
        Ok(Some(parse_pending(pending)))
    }

    fn should_capture(
        &self,
        session: &Session,
        pending: Option<&str>,
        surface: PlaybackSurface,
    ) -> bool {
        self.state
            .programmers
            .get(session.id)
            .is_some_and(|programmer| programmer.blind)
            && pending.is_some()
            && capture_enabled(self.state, surface)
    }

    fn queue_capture(
        &self,
        session: &Session,
        number: u16,
        pending: &str,
        surface: PlaybackSurface,
    ) -> Result<(), ActionError> {
        self.state.programmers.queue_preload_playback_action(
            session.id,
            number,
            pending.to_owned(),
            surface_name(surface).to_owned(),
        );
        persist_programmer(self.state, session).map_err(api_action_error)?;
        emit(
            self.state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id,"preload_playback_action":pending,"playback_number":number,"surface":surface_name(surface)}),
        );
        Ok(())
    }
}

fn playback_definition(
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

fn operator_context(
    session: &Session,
    desk_id: uuid::Uuid,
    source: ActionSource,
    request_id: Option<&str>,
) -> ActionContext {
    let context = ActionContext::operator(desk_id, session.user.id.0, session.id.0, source);
    request_id.map_or(context.clone(), |id| context.with_request_id(id))
}

fn capture_enabled(state: &AppState, surface: PlaybackSurface) -> bool {
    let configuration = state.configuration.read();
    if surface == PlaybackSurface::Virtual {
        configuration.preload_virtual_playback_actions
    } else {
        configuration.preload_physical_playback_actions
    }
}

fn captures_preload(source: ActionSource) -> bool {
    matches!(
        source,
        ActionSource::UserInterface | ActionSource::Keyboard | ActionSource::Http
    )
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn api_action_error(error: ApiError) -> ActionError {
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

fn action_error(error: ActionError) -> ApiError {
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
