use uuid::Uuid;

use super::{PlaybackAction, PlaybackCueReference, PlaybackRuntimeProjection};
use crate::{ActionContext, EventDraft, EventSource};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlaybackTransitionCause {
    Go,
    Back,
    Jump,
    Chaser,
    Follow,
    Wait,
    Timecode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackCueTransition {
    pub playback_number: Option<u16>,
    pub cue_list_id: Uuid,
    pub previous: Option<PlaybackCueReference>,
    pub current: Option<PlaybackCueReference>,
    pub cause: PlaybackTransitionCause,
    pub advanced_steps: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackRuntimeChange {
    pub projection: PlaybackRuntimeProjection,
    pub transition: Option<PlaybackCueTransition>,
}

/// Builds the one typed event for an adapter-owned atomic Playback mutation.
pub fn committed_playback_event(
    context: &ActionContext,
    action: PlaybackAction,
    configured_cause: Option<PlaybackTransitionCause>,
    before: PlaybackRuntimeProjection,
    projection: PlaybackRuntimeProjection,
) -> Option<EventDraft> {
    committed_playback_effect_event(context, action, configured_cause, before, projection, false)
}

/// Builds an event for an addressed runtime effect omitted from the compact public projection.
pub fn committed_playback_effect_event(
    context: &ActionContext,
    action: PlaybackAction,
    configured_cause: Option<PlaybackTransitionCause>,
    before: PlaybackRuntimeProjection,
    projection: PlaybackRuntimeProjection,
    addressed_effect_changed: bool,
) -> Option<EventDraft> {
    if !addressed_effect_changed && before == projection {
        return None;
    }
    let transition = manual_transition(action, configured_cause, &before, &projection);
    Some(EventDraft::playback_runtime_changed(
        None,
        PlaybackRuntimeChange {
            projection,
            transition,
        },
        EventSource::Action(context.source),
        Some(context.correlation_id),
    ))
}

fn manual_transition(
    action: PlaybackAction,
    configured_cause: Option<PlaybackTransitionCause>,
    before: &PlaybackRuntimeProjection,
    after: &PlaybackRuntimeProjection,
) -> Option<PlaybackCueTransition> {
    let cause = configured_cause.or_else(|| navigation_cause(action))?;
    let previous = before.current_cue().cloned();
    let current = after.current_cue().cloned();
    if previous == current {
        return None;
    }
    let cue_list_id = after.cue_list_id().or_else(|| before.cue_list_id())?;
    Some(PlaybackCueTransition {
        playback_number: after.playback_number.or(before.playback_number),
        cue_list_id: cue_list_id.0,
        previous,
        current,
        cause,
        advanced_steps: 1,
    })
}

const fn navigation_cause(action: PlaybackAction) -> Option<PlaybackTransitionCause> {
    match action {
        PlaybackAction::Go { pressed: true } | PlaybackAction::FastForward { pressed: true } => {
            Some(PlaybackTransitionCause::Go)
        }
        PlaybackAction::Back { pressed: true } | PlaybackAction::FastRewind { pressed: true } => {
            Some(PlaybackTransitionCause::Back)
        }
        PlaybackAction::GoTo(_) => Some(PlaybackTransitionCause::Jump),
        _ => None,
    }
}
