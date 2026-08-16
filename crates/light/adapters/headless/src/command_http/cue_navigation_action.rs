//! Shared CUE navigation execution over the typed Playback application service.
//!
//! Every surface — v2 command-line HTTP, the compatibility WebSocket, and OSC keys — resolves the
//! same parsed grammar into the same `PlaybackAction::GoTo`/`PlaybackAction::Load` and executes it
//! through the same application service. Only the temporary v1 `playback_changed` notification is
//! surface-specific, and it stays out of the v2 path.

use super::{
    cue_navigation_command::CueNavigationCommand, playback_address_command::CommandPlaybackTarget,
};
use light_application::{
    ActionContext, ActionSource, PlaybackAction, PlaybackAddress, PlaybackCommand, PlaybackOutcome,
    PlaybackSurface,
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
    let address = resolve_address(parsed.target);
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
fn resolve_address(target: CommandPlaybackTarget) -> PlaybackAddress {
    match target {
        CommandPlaybackTarget::CurrentPage { slot } => PlaybackAddress::CurrentPage { slot },
        CommandPlaybackTarget::ExplicitPage { page, slot } => {
            PlaybackAddress::ExplicitPage { page, slot }
        }
        CommandPlaybackTarget::Virtual(address) => PlaybackAddress::Virtual(address),
    }
}

/// Temporary per-object facade notification, isolated to compatibility consumers.
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

fn action(parsed: CueNavigationCommand) -> PlaybackAction {
    if parsed.load {
        PlaybackAction::Load(parsed.cue_number)
    } else {
        PlaybackAction::GoTo(parsed.cue_number)
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
