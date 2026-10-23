//! Ephemeral live Timecode crossings feed the existing output-frame action/event claim flow.
use crate::{
    AutomaticPlaybackTransition, AutomaticPlaybackTransitionCause, CueListId, PlaybackCueReference,
    PlaybackEngine, PlaybackKey,
};

impl PlaybackEngine {
    pub(crate) fn timeline_cue_reference(&self, id: CueListId) -> Option<PlaybackCueReference> {
        let active = self.active.get(&PlaybackKey::CueList(id))?;
        let cue = self
            .cue_lists
            .get(&id)?
            .cues
            .iter()
            .find(|cue| Some(cue.id) == active.current_cue_id)?;
        Some(PlaybackCueReference {
            id: cue.id,
            number: cue.number.clone(),
        })
    }

    pub(crate) fn queue_timeline_transition(
        &mut self,
        id: CueListId,
        previous: Option<PlaybackCueReference>,
    ) {
        let Some(current) = self.timeline_cue_reference(id) else {
            return;
        };
        let Some(active) = self.active.get(&PlaybackKey::CueList(id)) else {
            return;
        };
        self.timeline_transitions.insert(
            id,
            AutomaticPlaybackTransition {
                playback_address: active.playback_address,
                cue_list_id: id,
                previous: previous.unwrap_or_else(|| current.clone()),
                current,
                cause: AutomaticPlaybackTransitionCause::Timecode,
                transition_ordinal: active.transition_ordinal,
                advanced_steps: 1,
            },
        );
    }

    pub(crate) fn take_timeline_transitions(&mut self) -> Vec<AutomaticPlaybackTransition> {
        let mut pending = std::mem::take(&mut self.timeline_transitions)
            .into_values()
            .filter(|transition| {
                self.timeline_controlled
                    .contains_key(&transition.cue_list_id)
                    && self
                        .active
                        .get(&PlaybackKey::CueList(transition.cue_list_id))
                        .is_some_and(|active| {
                            active.enabled
                                && !active.discrete_cue_actions_suppressed
                                && !active.transition_timing_bypassed
                                && active.current_cue_id == Some(transition.current.id)
                                && active.transition_ordinal == transition.transition_ordinal
                        })
            })
            .collect::<Vec<_>>();
        pending.sort_by_key(|transition| transition.cue_list_id.0);
        pending
    }
}
