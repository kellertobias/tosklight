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

pub(super) fn may_activate_playback(action: PlaybackAction) -> bool {
    match action {
        PlaybackAction::Go { pressed }
        | PlaybackAction::Back { pressed }
        | PlaybackAction::On { pressed }
        | PlaybackAction::Toggle { pressed }
        | PlaybackAction::FastForward { pressed }
        | PlaybackAction::FastRewind { pressed }
        | PlaybackAction::Temp { pressed } => pressed,
        // Flash/Swap release can promote temporary state into an enabled Playback, and a
        // configured button can resolve to either action. Capturing candidates on both phases is
        // intentionally conservative; the application boundary publishes only actual deltas.
        PlaybackAction::Flash { .. }
        | PlaybackAction::Swap { .. }
        | PlaybackAction::ConfiguredButton { .. }
        // A zero fader value still activates an inactive manual-XFade Playback.
        | PlaybackAction::Master(_) => true,
        PlaybackAction::GoTo(_) => true,
        PlaybackAction::Crossfade { enabled } | PlaybackAction::Temporary { enabled, .. } => {
            enabled
        }
        _ => false,
    }
}

pub(super) fn may_trigger_auto_off(
    action: PlaybackAction,
    definition: &light_playback::PlaybackDefinition,
) -> bool {
    match action {
        PlaybackAction::Go { pressed }
        | PlaybackAction::Pause { pressed }
        | PlaybackAction::On { pressed }
        | PlaybackAction::Toggle { pressed }
        | PlaybackAction::FastForward { pressed } => pressed,
        PlaybackAction::ConfiguredButton { number, pressed } => {
            pressed && configured_button_triggers_auto_off(definition, number)
        }
        PlaybackAction::Master(_) => definition.fader == light_playback::PlaybackFaderMode::Master,
        PlaybackAction::GoTo(_) => true,
        _ => false,
    }
}

fn configured_button_triggers_auto_off(
    definition: &light_playback::PlaybackDefinition,
    number: u8,
) -> bool {
    use light_playback::PlaybackButtonAction as Button;
    let Some(index) = number.checked_sub(1) else {
        return false;
    };
    if number > definition.button_count {
        return false;
    }
    definition
        .buttons
        .get(usize::from(index))
        .is_some_and(|action| {
            matches!(
                action,
                Button::On | Button::Toggle | Button::Go | Button::Pause | Button::FastForward
            )
        })
}
