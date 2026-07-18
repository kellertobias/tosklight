use light_playback::{
    AutomaticPlaybackTransition, AutomaticPlaybackTransitionCause, PlaybackCueReference,
};

use crate::{
    CueReference, EventDraft, EventSource, PlaybackCueTransition, PlaybackTransitionCause,
};

/// Maps scheduler-owned domain transitions to installation-global application events.
///
/// Callers collect the domain transitions while advancing playback, release their domain lock,
/// and only then publish these drafts through the event bus.
pub fn automatic_playback_events(
    transitions: impl IntoIterator<Item = AutomaticPlaybackTransition>,
) -> Vec<EventDraft> {
    transitions
        .into_iter()
        .map(automatic_playback_event)
        .collect()
}

fn automatic_playback_event(transition: AutomaticPlaybackTransition) -> EventDraft {
    EventDraft::playback_transition(
        None,
        PlaybackCueTransition {
            playback_number: transition.playback_number,
            cue_list_id: transition.cue_list_id.0,
            previous: Some(cue_reference(transition.previous)),
            current: Some(cue_reference(transition.current)),
            cause: transition_cause(transition.cause),
            advanced_steps: transition.advanced_steps,
        },
        EventSource::Runtime,
        None,
    )
}

fn cue_reference(reference: PlaybackCueReference) -> CueReference {
    CueReference {
        id: reference.id,
        number: reference.number,
    }
}

fn transition_cause(cause: AutomaticPlaybackTransitionCause) -> PlaybackTransitionCause {
    match cause {
        AutomaticPlaybackTransitionCause::Chaser => PlaybackTransitionCause::Chaser,
        AutomaticPlaybackTransitionCause::Follow => PlaybackTransitionCause::Follow,
        AutomaticPlaybackTransitionCause::Wait => PlaybackTransitionCause::Wait,
        AutomaticPlaybackTransitionCause::Timecode => PlaybackTransitionCause::Timecode,
    }
}

#[cfg(test)]
mod tests {
    use light_playback::{
        AutomaticPlaybackTransition, AutomaticPlaybackTransitionCause, PlaybackCueReference,
    };
    use uuid::Uuid;

    use super::*;
    use crate::{ApplicationEvent, PlaybackEvent};

    #[test]
    fn automatic_transitions_become_installation_global_runtime_events() {
        let cue_list_id = Default::default();
        let drafts = automatic_playback_events([AutomaticPlaybackTransition {
            playback_number: Some(12),
            cue_list_id,
            previous: domain_cue(2, 1.0),
            current: domain_cue(3, 4.0),
            cause: AutomaticPlaybackTransitionCause::Chaser,
            advanced_steps: 7,
        }]);

        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].desk_id, None);
        assert_eq!(drafts[0].source, EventSource::Runtime);
        let ApplicationEvent::Playback(PlaybackEvent::CueTransition(event)) = &drafts[0].payload;
        assert_eq!(event.cue_list_id, cue_list_id.0);
        assert_eq!(event.cause, PlaybackTransitionCause::Chaser);
        assert_eq!(event.advanced_steps, 7);
        assert_eq!(event.previous.as_ref().map(|cue| cue.number), Some(1.0));
        assert_eq!(event.current.as_ref().map(|cue| cue.number), Some(4.0));
    }

    fn domain_cue(value: u128, number: f64) -> PlaybackCueReference {
        PlaybackCueReference {
            id: Uuid::from_u128(value),
            number,
        }
    }
}
