//! Configuration-dependent semantic metadata for Playback runtime events.

use light_application::{
    ActionContext, ActionError, ActionErrorKind, PlaybackAction, PlaybackTransitionCause,
    ResolvedPlaybackAddress,
};

use super::ServerPlaybackPorts;

pub(super) fn transition_cause(
    ports: &ServerPlaybackPorts<'_>,
    _context: &ActionContext,
    address: ResolvedPlaybackAddress,
    action: PlaybackAction,
) -> Result<Option<PlaybackTransitionCause>, ActionError> {
    let PlaybackAction::ConfiguredButton {
        number: button,
        pressed: true,
    } = action
    else {
        return Ok(None);
    };
    let ResolvedPlaybackAddress::Pool { number, .. } = address else {
        return Ok(None);
    };
    let definition = ports
        .state
        .engine
        .snapshot()
        .playbacks
        .iter()
        .find(|definition| definition.number == number)
        .cloned()
        .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "playback"))?;
    let configured = definition
        .buttons
        .get(usize::from(button.saturating_sub(1)))
        .filter(|_| button > 0 && button <= definition.button_count);
    Ok(configured.and_then(|action| match action {
        light_playback::PlaybackButtonAction::Go
        | light_playback::PlaybackButtonAction::FastForward => Some(PlaybackTransitionCause::Go),
        light_playback::PlaybackButtonAction::GoMinus
        | light_playback::PlaybackButtonAction::FastRewind => Some(PlaybackTransitionCause::Back),
        _ => None,
    }))
}
