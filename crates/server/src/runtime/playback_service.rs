use super::{
    ApiError, AppState, ControlDesk, PoolPlaybackInput, Session, cuelist_for_page_playback,
    dispatch_playback_action, emit, intercept_update_playback_target, persist_active_playbacks,
    persist_programmer, predicted_preload_temp_state, preload_capture_action_with_temp_state,
};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    PendingPlaybackAction, PlaybackAction, PlaybackAddress, PlaybackCommand, PlaybackDurability,
    PlaybackExecution, PlaybackPorts, PlaybackResult, PlaybackSurface, ResolvedPlaybackAddress,
};

#[path = "playback_service/capture.rs"]
mod capture;
#[path = "playback_service/conversion.rs"]
mod conversion;
#[path = "playback_service/desk.rs"]
mod desk;
#[path = "playback_service/projection.rs"]
mod projection;
#[path = "playback_service/response.rs"]
mod response;
#[path = "playback_service/semantics.rs"]
mod semantics;

pub(super) use desk::{projection as desk_projection, publish_change as publish_desk_change};
pub(super) use projection::automatic_changes as automatic_projection_changes;
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

pub(super) fn execute(
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
        persistence_pending: std::sync::atomic::AtomicBool::new(false),
    };
    state
        .playback_service
        .handle(ActionEnvelope { context, command }, &ports)
        .map_err(action_error)
}

pub(super) fn snapshot(
    state: &AppState,
    session: &Session,
    context: ActionContext,
    identities: &[light_application::PlaybackRuntimeIdentity],
) -> Result<light_application::PlaybackRuntimeSnapshot, ApiError> {
    let ports = ServerPlaybackPorts {
        state,
        session: Some(session),
        desk: Some(&session.desk),
        persistence_pending: std::sync::atomic::AtomicBool::new(false),
    };
    state
        .playback_service
        .snapshot(&context, identities, &ports)
        .map_err(action_error)
}

pub(in crate::runtime) fn read_runtime_projection(
    state: &AppState,
    context: &ActionContext,
    identity: light_application::PlaybackRuntimeIdentity,
) -> Result<light_application::PlaybackRuntimeProjection, ApiError> {
    let ports = ServerPlaybackPorts {
        state,
        session: None,
        desk: None,
        persistence_pending: std::sync::atomic::AtomicBool::new(false),
    };
    PlaybackPorts::projection(&ports, context, identity).map_err(action_error)
}

struct ServerPlaybackPorts<'a> {
    pub(super) state: &'a AppState,
    pub(super) session: Option<&'a Session>,
    pub(super) desk: Option<&'a ControlDesk>,
    persistence_pending: std::sync::atomic::AtomicBool,
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
            ResolvedPlaybackAddress::CueList(id) => self.execute_cue_list(context, id, action),
            ResolvedPlaybackAddress::Pool { number, .. } => {
                self.execute_pool(context, number, action, surface)
            }
        }
    }

    fn durability(&self) -> PlaybackDurability {
        if self
            .persistence_pending
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            PlaybackDurability::PersistencePending
        } else {
            PlaybackDurability::Durable
        }
    }

    fn transition_cause(
        &self,
        context: &ActionContext,
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
    ) -> Result<Option<light_application::PlaybackTransitionCause>, ActionError> {
        semantics::transition_cause(self, context, address, action)
    }

    fn projection(
        &self,
        context: &ActionContext,
        identity: light_application::PlaybackRuntimeIdentity,
    ) -> Result<light_application::PlaybackRuntimeProjection, ActionError> {
        projection::projection(self, context, identity)
    }

    fn projections(
        &self,
        context: &ActionContext,
        identities: &[light_application::PlaybackRuntimeIdentity],
    ) -> Result<Vec<light_application::PlaybackRuntimeProjection>, ActionError> {
        projection::projections(self, context, identities)
    }

    fn desk_projection(
        &self,
        context: &ActionContext,
    ) -> Result<Option<light_application::PlaybackDeskProjection>, ActionError> {
        projection::desk_projection(self, context)
    }
}

impl ServerPlaybackPorts<'_> {
    fn execute_cue_list(
        &self,
        context: &ActionContext,
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
        if let Err(error) = persist_active_playbacks(self.state) {
            self.mark_persistence_pending(context, "active_playbacks", error);
        }
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
            && let Some(pending) =
                self.capture(context, &definition, action_name, &input, surface)?
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
        let dispatch = dispatch_playback_action(
            self.state,
            self.session,
            self.desk,
            &definition,
            action_name,
            &input,
            source_name(context.source),
        )
        .map_err(api_action_error)?;
        if dispatch.persistence_pending {
            self.persistence_pending
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(PlaybackExecution::Pool {
            changed: dispatch.changed,
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
