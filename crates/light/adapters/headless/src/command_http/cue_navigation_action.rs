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
    ActionContext, PlaybackAction, PlaybackAddress, PlaybackCommand, PlaybackOutcome,
};

use super::super::{AppState, Session};

pub(super) struct CueNavigationTransition {
    pub playback: light_playback::PlaybackAddress,
    /// One real semantic transition; `false` for an exact no-change action.
    pub applied: bool,
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
            control_source: control_source(state, context)?,
        },
    )
    .map_err(|error| error.message)?;
    let playback = result
        .resolved
        .playback_address()
        .ok_or("Cue command resolved to a Cuelist without a playback")?;
    Ok(CueNavigationTransition {
        playback,
        applied: result.outcome == PlaybackOutcome::Applied,
    })
}

/// Resolves desk selection and page topology; the Cue itself is resolved by the Playback service.
pub(in crate::runtime) fn resolve_address(target: CommandPlaybackTarget) -> PlaybackAddress {
    match target {
        CommandPlaybackTarget::CurrentPage { position } => {
            PlaybackAddress::CurrentPage { position }
        }
        CommandPlaybackTarget::ExplicitPage { page, position } => PlaybackAddress::Playback(
            light_playback::PlaybackAddress::new(page, position)
                .expect("validated command address"),
        ),
    }
}
pub(super) fn control_source(
    state: &AppState,
    context: &ActionContext,
) -> Result<light_playback::PlaybackControlSource, String> {
    Ok(light_playback::PlaybackControlSource {
        surface_id: state
            .installation
            .selected_playback_surface(context.desk_id)
            .map_err(|error| error.to_string())?,
        control_id: 1,
    })
}

/// Temporary per-object facade notification, isolated to compatibility consumers.
pub(super) fn emit_compatibility_change(
    state: &AppState,
    session: &Session,
    playback: light_playback::PlaybackAddress,
    parsed: CueNavigationCommand,
) {
    super::super::emit(
        state,
        "playback_changed",
        serde_json::json!({
            "playback_address":playback,
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
