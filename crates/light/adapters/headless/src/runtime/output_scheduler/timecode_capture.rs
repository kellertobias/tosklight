//! Same-tick source refresh after accepted automatic Cue transport actions.
//! This component does not enter a Playback operation; its caller owns ordering and events.
use light_engine::{Engine, EngineError, PreparedOutputFrame};
use light_playback::CueAction;

pub(in crate::runtime) struct SettledTimecodeCapture {
    pub frame: Result<PreparedOutputFrame, EngineError>,
    /// Earlier accepted transitions retain their separate event and change-lead claims.
    pub accepted: Option<PreparedOutputFrame>,
}

pub(in crate::runtime) fn settle(
    engine: &Engine,
    frame: PreparedOutputFrame,
    mut apply: impl FnMut(&CueAction) -> Result<Option<u64>, String>,
    reconcile: impl FnOnce() -> Result<(), String>,
) -> SettledTimecodeCapture {
    let batches = claim_automatic_cue_action_batches(&frame);
    let changed = batches.iter().any(|(_, actions)| {
        actions
            .iter()
            .any(|action| !matches!(action, CueAction::Jump { .. }))
    });
    if !changed {
        return SettledTimecodeCapture {
            frame: Ok(frame),
            accepted: None,
        };
    }
    let mut failure = None;
    for (id, actions) in batches {
        let mut completion = 0;
        for action in actions {
            match apply(&action) {
                Ok(value) => completion = completion.max(value.unwrap_or(0)),
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        engine.set_cue_external_completion_millis(id, completion);
    }
    // Even a partial transport failure must reconcile the changes actually accepted.
    if let Err(error) = reconcile() {
        failure.get_or_insert(error);
    }
    let result = match failure {
        Some(error) => Err(EngineError::Invalid(error)),
        None => engine
            .try_refresh_output_frame(&frame)
            .ok_or(EngineError::StalePreparedFrame),
    };
    SettledTimecodeCapture {
        frame: result,
        accepted: Some(frame),
    }
}

/// Resolves only final-state transitions captured by this authoritative tick. The token is
/// independent of event publication, so a failed DMX frame still gets one action dispatch.
pub(in crate::runtime) fn claim_automatic_cue_action_batches(
    frame: &light_engine::PreparedOutputFrame,
) -> Vec<(light_core::CueListId, Vec<light_playback::CueAction>)> {
    let snapshot = frame.snapshot();
    let mut batches = Vec::new();
    for transition in frame.claim_automatic_cue_action_transitions() {
        let Some(playback) = frame.captured_active_playbacks().iter().find(|playback| {
            playback.cue_list_id == transition.cue_list_id
                && playback.playback_address == transition.playback_address
                && playback.current_cue_id == Some(transition.current.id)
                && playback.transition_ordinal == transition.transition_ordinal
        }) else {
            continue;
        };
        if playback.transition_timing_bypassed || playback.discrete_cue_actions_suppressed {
            continue;
        }
        let cue_list_id = transition.cue_list_id;
        let Some(cue) = snapshot
            .cue_lists
            .iter()
            .find(|cue_list| cue_list.id == cue_list_id)
            .and_then(|cue_list| {
                cue_list
                    .cues
                    .iter()
                    .find(|cue| cue.id == transition.current.id)
            })
        else {
            continue;
        };
        batches.push((cue_list_id, cue.actions.clone()));
    }
    batches
}
