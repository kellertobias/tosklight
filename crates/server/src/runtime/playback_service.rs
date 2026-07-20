use super::{
    ApiError, AppState, ControlDesk, PlaybackDispatchContext, PoolPlaybackInput,
    ProgrammingLockPolicy, Session, cuelist_for_page_playback, dispatch_playback_action, emit,
    intercept_update_playback_target, persist_active_playbacks, persist_programmer,
    predicted_preload_temp_state, preload_capture_action_with_temp_state, programming_context,
    run_programming_interaction,
};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    PendingPlaybackAction, PlaybackAction, PlaybackAddress, PlaybackCommand, PlaybackDurability,
    PlaybackExecution, PlaybackPorts, PlaybackResult, PlaybackRuntimeIdentity, PlaybackSurface,
    ResolvedPlaybackAddress,
};
use light_engine::{CueListPlaybackAction, EnginePlaybackCommand, EnginePlaybackOutcome};

#[path = "playback_service/capture.rs"]
mod capture;
#[path = "playback_service/conversion.rs"]
mod conversion;
#[path = "playback_service/desk.rs"]
mod desk;
#[path = "playback_service/ports.rs"]
mod ports;
#[path = "playback_service/projection.rs"]
mod projection;
#[path = "playback_service/response.rs"]
mod response;
#[path = "playback_service/semantics.rs"]
mod semantics;
#[path = "playback_service/support.rs"]
mod support;

pub(super) use desk::ChangePage;
use ports::ServerPlaybackPorts;
pub(super) use projection::automatic_changes as automatic_projection_changes;
pub(super) use response::{cue_list_http_payload, pool_http_payload, websocket_payload};

use conversion::{
    action_touched, activation_surface, legacy_action, parse_action, parse_pending, parse_surface,
    source_name, surface_name,
};
use support::{
    action_error, api_action_error, capture_enabled, captures_preload, invalid, operator_context,
    playback_definition,
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
    let context = programming_context(session, ActionSource::Http, None);
    let playback_context = context.clone();
    run_programming_interaction(
        state,
        session,
        &context,
        "http",
        ProgrammingLockPolicy::RequireUnlocked,
        || {
            execute(
                state,
                Some(session),
                Some(&session.desk),
                playback_context,
                command,
            )
        },
    )?
    .output
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
    let Some(session) = session else {
        return execute(state, None, desk, context, command);
    };
    let playback_context = context.clone();
    run_programming_interaction(
        state,
        session,
        &context,
        "osc",
        ProgrammingLockPolicy::RequireUnlocked,
        || execute(state, Some(session), desk, playback_context, command),
    )?
    .output
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
    let ports = ServerPlaybackPorts::new(state, session, desk);
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
    let ports = ServerPlaybackPorts::new(state, Some(session), Some(&session.desk));
    state
        .playback_service
        .snapshot(&context, identities, &ports)
        .map_err(action_error)
}

pub(in crate::runtime) fn read_runtime_projections(
    state: &AppState,
    context: &ActionContext,
    identities: &[light_application::PlaybackRuntimeIdentity],
) -> Result<Vec<light_application::PlaybackRuntimeProjection>, ApiError> {
    let ports = ServerPlaybackPorts::new(state, None, None);
    PlaybackPorts::projections(&ports, context, identities).map_err(action_error)
}
