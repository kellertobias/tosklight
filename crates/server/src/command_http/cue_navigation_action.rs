//! Shared CUE navigation execution over the typed Playback application service.
//!
//! Every surface — v2 command-line HTTP, the compatibility WebSocket, and OSC keys — resolves the
//! same parsed grammar into the same `PlaybackAction::GoTo`/`PlaybackAction::Load` and executes it
//! through the same application service. Only the temporary v1 `playback_changed` notification is
//! surface-specific, and it stays out of the v2 path.

use super::cue_navigation_command::{CueNavigationCommand, CueNavigationTarget};
use light_application::{
    ActionContext, ActionSource, CueNumber, PlaybackAction, PlaybackAddress, PlaybackCommand,
    PlaybackOutcome, PlaybackSurface,
};

use super::super::{AppState, Session};

pub(super) struct CueNavigationTransition {
    pub playback: u16,
    /// One real semantic transition; `false` for an exact no-change action.
    pub applied: bool,
    pub replayed: bool,
}

pub(super) fn execute(
    state: &AppState,
    session: &Session,
    context: &ActionContext,
    parsed: CueNavigationCommand,
) -> Result<CueNavigationTransition, String> {
    let address = resolve_address(state, session, parsed.target)?;
    let result = super::super::playback_service::execute(
        state,
        Some(session),
        Some(&session.desk),
        context.clone(),
        PlaybackCommand {
            address,
            action: action(parsed),
            surface: surface(context.source),
        },
    )
    .map_err(|error| error.message)?;
    let playback = result
        .resolved
        .playback_number()
        .ok_or("Cue command resolved to a Cuelist without a playback")?;
    Ok(CueNavigationTransition {
        playback,
        applied: result.outcome == PlaybackOutcome::Applied,
        replayed: result.replayed,
    })
}

/// Resolves desk selection and page topology; the Cue itself is resolved by the Playback service.
fn resolve_address(
    state: &AppState,
    session: &Session,
    target: CueNavigationTarget,
) -> Result<PlaybackAddress, String> {
    let snapshot = state.engine.snapshot();
    let (address, playback) = match target {
        CueNavigationTarget::SelectedPlayback => {
            let selected = selected_playback(state, session)?;
            (PlaybackAddress::Pool(selected), selected)
        }
        CueNavigationTarget::Pool { playback_number } => {
            (PlaybackAddress::Pool(playback_number), playback_number)
        }
        CueNavigationTarget::ExplicitPage { page, slot } => (
            PlaybackAddress::ExplicitPage { page, slot },
            super::super::page_playback(&snapshot, page, slot)?,
        ),
    };
    if !snapshot
        .playbacks
        .iter()
        .any(|item| item.number == playback)
    {
        return Err(format!("playback {playback} does not exist"));
    }
    Ok(address)
}

/// Selection is desk-local, so another desk keeps its own selected Playback.
fn selected_playback(state: &AppState, session: &Session) -> Result<u16, String> {
    let show = state.active_show.read().clone().ok_or("no show is open")?;
    state
        .desk
        .lock()
        .selected_playback(session.desk.id, show.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| {
            "no playback is selected; select a playback or use CUE SET <playback> CUE <cue>"
                .to_owned()
        })
}

/// Temporary per-object v1 notification, isolated to the compatibility surfaces.
pub(super) fn emit_compatibility_change(
    state: &AppState,
    session: &Session,
    playback: u16,
    parsed: CueNavigationCommand,
) {
    super::super::emit(
        state,
        "playback_changed",
        serde_json::json!({
            "playback_number":playback,
            "action":if parsed.load { "load" } else { "go-to" },
            "cue_number":parsed.cue_number,
            "session_id":session.id,
        }),
    );
}

/// Entry point for the generic legacy executor's dispatch table.
///
/// The command line and history belong to the caller here, exactly as before: only callers that
/// enter through the Programming interaction boundary get the shared reset.
pub(crate) fn execute_compatibility(
    state: &AppState,
    session: &Session,
    context: &ActionContext,
    command: &str,
) -> Result<usize, String> {
    let parsed = super::cue_navigation_command::parse(command)?
        .ok_or("expected a CUE navigation command")?;
    let transition = execute(state, session, context, parsed)?;
    if transition.applied && !transition.replayed {
        emit_compatibility_change(state, session, transition.playback, parsed);
    }
    Ok(1)
}

fn action(parsed: CueNavigationCommand) -> PlaybackAction {
    let cue = CueNumber::new(parsed.cue_number);
    if parsed.load {
        PlaybackAction::Load(cue)
    } else {
        PlaybackAction::GoTo(cue)
    }
}

const fn surface(source: ActionSource) -> PlaybackSurface {
    match source {
        ActionSource::Osc => PlaybackSurface::Osc,
        ActionSource::Matter => PlaybackSurface::Matter,
        ActionSource::UserInterface | ActionSource::Http => PlaybackSurface::Virtual,
        _ => PlaybackSurface::Physical,
    }
}
