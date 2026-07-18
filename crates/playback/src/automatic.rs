use chrono::{DateTime, Duration as ChronoDuration, Utc};
use light_core::CueListId;
use uuid::Uuid;

use super::{
    ActivePlayback, Cue, CueList, CueListMode, CueTrigger, PlaybackEngine,
    PlaybackMasterTransition, WrapMode, advance_chaser_steps, cue_completion_millis,
    effective_chaser_step_millis,
};

/// The scheduler-owned reason that advanced a playback without an operator action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutomaticPlaybackTransitionCause {
    Chaser,
    Follow,
    Wait,
    Timecode,
}

/// Stable Cue identity captured at an automatic transition boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackCueReference {
    pub id: Uuid,
    pub number: f64,
}

/// One final playback state produced by a scheduler tick.
#[derive(Clone, Debug, PartialEq)]
pub struct AutomaticPlaybackTransition {
    pub playback_number: Option<u16>,
    pub cue_list_id: CueListId,
    pub previous: PlaybackCueReference,
    pub current: PlaybackCueReference,
    pub cause: AutomaticPlaybackTransitionCause,
    /// Number of semantic Cue steps consumed by this one final-state transition.
    pub advanced_steps: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaybackTickResult {
    pub transitions: Vec<AutomaticPlaybackTransition>,
}

impl PlaybackEngine {
    pub fn tick(&mut self, now: DateTime<Utc>, timecode_frame: Option<u64>) -> PlaybackTickResult {
        if self.dynamics_paused_at.is_some() {
            return PlaybackTickResult::default();
        }
        self.advance_master_transitions(now);
        PlaybackTickResult {
            transitions: self.advance_automatic_cues(now, timecode_frame),
        }
    }

    fn advance_master_transitions(&mut self, now: DateTime<Utc>) {
        let releases = self
            .active
            .iter_mut()
            .filter_map(|(key, playback)| update_master_transition(playback, now).then_some(*key))
            .collect::<Vec<_>>();
        for key in releases {
            if let Some(playback) = self.active.get_mut(&key) {
                playback.enabled = false;
            }
        }
        for playback in self.temporary.values_mut() {
            update_master_transition(playback, now);
        }
    }

    fn advance_automatic_cues(
        &mut self,
        now: DateTime<Utc>,
        timecode_frame: Option<u64>,
    ) -> Vec<AutomaticPlaybackTransition> {
        let keys = self.active.keys().copied().collect::<Vec<_>>();
        let mut transitions = Vec::with_capacity(keys.len());
        for key in keys {
            let Some(playback) = self.active.get_mut(&key) else {
                continue;
            };
            let Some(cue_list) = self.cue_lists.get(&playback.cue_list_id) else {
                continue;
            };
            let transition = advance_automatic_playback(
                playback,
                cue_list,
                AutomaticTiming {
                    now,
                    timecode_frame,
                    speed_groups_bpm: self.speed_groups_bpm,
                    speed_groups_paused: self.speed_groups_paused,
                    sequence_master_fade_millis: self.sequence_master_fade_millis,
                },
            );
            transitions.extend(transition);
        }
        transitions
    }
}

#[derive(Clone, Copy)]
struct AutomaticTiming {
    now: DateTime<Utc>,
    timecode_frame: Option<u64>,
    speed_groups_bpm: [f64; 5],
    speed_groups_paused: [bool; 5],
    sequence_master_fade_millis: u64,
}

fn update_master_transition(playback: &mut ActivePlayback, now: DateTime<Utc>) -> bool {
    let Some(transition) = playback.master_transition.clone() else {
        return false;
    };
    let progress = transition_progress(&transition, now);
    playback.master = transition.from + (transition.to - transition.from) * progress;
    if progress < 1.0 {
        return false;
    }
    playback.master_transition = None;
    transition.release_after
}

fn transition_progress(transition: &PlaybackMasterTransition, now: DateTime<Utc>) -> f32 {
    if transition.duration_millis == 0 {
        return 1.0;
    }
    ((now - transition.started_at).num_milliseconds().max(0) as f32
        / transition.duration_millis as f32)
        .clamp(0.0, 1.0)
}

fn advance_automatic_playback(
    playback: &mut ActivePlayback,
    cue_list: &CueList,
    timing: AutomaticTiming,
) -> Option<AutomaticPlaybackTransition> {
    if !playback.enabled || playback.paused || chaser_is_paused(cue_list, &timing) {
        return None;
    }
    if let Some(transition) = advance_timecode(playback, cue_list, &timing) {
        return Some(transition);
    }
    if cue_list.mode == CueListMode::Chaser {
        return advance_chaser(playback, cue_list, &timing);
    }
    advance_follow_or_wait(playback, cue_list, &timing)
}

fn chaser_is_paused(cue_list: &CueList, timing: &AutomaticTiming) -> bool {
    cue_list.mode == CueListMode::Chaser
        && cue_list
            .speed_group
            .as_ref()
            .is_some_and(|group| timing.speed_groups_paused[speed_group_index(group)])
}

fn speed_group_index(group: &str) -> usize {
    group
        .as_bytes()
        .first()
        .copied()
        .unwrap_or(b'A')
        .saturating_sub(b'A')
        .min(4) as usize
}

fn advance_timecode(
    playback: &mut ActivePlayback,
    cue_list: &CueList,
    timing: &AutomaticTiming,
) -> Option<AutomaticPlaybackTransition> {
    let index = timecode_index(cue_list, timing.timecode_frame?)?;
    if index == playback.cue_index {
        return None;
    }
    let previous_index = playback.cue_index;
    playback.previous_index = Some(previous_index);
    set_current_cue(playback, cue_list, index);
    playback.deleted_cue_hold = None;
    playback.activated_at = timing.now;
    Some(cue_transition(
        playback,
        cue_list,
        previous_index,
        AutomaticPlaybackTransitionCause::Timecode,
        previous_index.abs_diff(index) as u64,
    ))
}

fn timecode_index(cue_list: &CueList, frame: u64) -> Option<usize> {
    cue_list
        .cues
        .iter()
        .enumerate()
        .filter_map(|(index, cue)| match cue.trigger {
            CueTrigger::Timecode { frame: cue_frame } if cue_frame <= frame => Some(index),
            _ => None,
        })
        .next_back()
}

fn advance_chaser(
    playback: &mut ActivePlayback,
    cue_list: &CueList,
    timing: &AutomaticTiming,
) -> Option<AutomaticPlaybackTransition> {
    let elapsed = elapsed_since_activation(playback, timing.now);
    let step_millis = effective_chaser_step_millis(cue_list, &timing.speed_groups_bpm);
    let requested_steps = elapsed / step_millis;
    if requested_steps == 0 {
        return None;
    }
    let previous_index = playback.cue_index;
    let advanced_steps = advance_chaser_steps(playback, cue_list, requested_steps);
    advance_chaser_clock(playback, step_millis, requested_steps);
    (advanced_steps > 0).then(|| {
        cue_transition(
            playback,
            cue_list,
            previous_index,
            AutomaticPlaybackTransitionCause::Chaser,
            advanced_steps,
        )
    })
}

fn advance_chaser_clock(playback: &mut ActivePlayback, step_millis: u64, steps: u64) {
    let elapsed = step_millis.saturating_mul(steps);
    playback.activated_at +=
        ChronoDuration::milliseconds(i64::try_from(elapsed).unwrap_or(i64::MAX));
}

fn advance_follow_or_wait(
    playback: &mut ActivePlayback,
    cue_list: &CueList,
    timing: &AutomaticTiming,
) -> Option<AutomaticPlaybackTransition> {
    let next_index = next_cue_index(playback.cue_index, cue_list)?;
    let (cause, trigger_delay) = automatic_trigger(&cue_list.cues[next_index])?;
    let current = &cue_list.cues[playback.cue_index];
    let completion = cue_completion_millis(cue_list, current, timing.sequence_master_fade_millis);
    let trigger_delay = if cue_list.disable_cue_timing {
        0
    } else {
        trigger_delay
    };
    if elapsed_since_activation(playback, timing.now) < completion.saturating_add(trigger_delay) {
        return None;
    }
    let previous_index = playback.cue_index;
    playback.deleted_cue_transition_source = None;
    playback.previous_index = Some(previous_index);
    set_current_cue(playback, cue_list, next_index);
    if next_index == 0 {
        playback.tracking_wrap = cue_list.effective_wrap_mode() == WrapMode::Tracking;
    }
    playback.activated_at = timing.now;
    Some(cue_transition(playback, cue_list, previous_index, cause, 1))
}

fn next_cue_index(current: usize, cue_list: &CueList) -> Option<usize> {
    if current + 1 < cue_list.cues.len() {
        Some(current + 1)
    } else {
        (cue_list.effective_wrap_mode() != WrapMode::Off).then_some(0)
    }
}

fn automatic_trigger(cue: &Cue) -> Option<(AutomaticPlaybackTransitionCause, u64)> {
    match cue.trigger {
        CueTrigger::Follow { delay_millis } => {
            Some((AutomaticPlaybackTransitionCause::Follow, delay_millis))
        }
        CueTrigger::Wait { delay_millis } => {
            Some((AutomaticPlaybackTransitionCause::Wait, delay_millis))
        }
        _ => None,
    }
}

fn elapsed_since_activation(playback: &ActivePlayback, now: DateTime<Utc>) -> u64 {
    (now - playback.activated_at).num_milliseconds().max(0) as u64
}

fn set_current_cue(playback: &mut ActivePlayback, cue_list: &CueList, index: usize) {
    playback.cue_index = index;
    playback.current_cue_id = Some(cue_list.cues[index].id);
    playback.current_cue_number = Some(cue_list.cues[index].number);
}

fn cue_transition(
    playback: &ActivePlayback,
    cue_list: &CueList,
    previous_index: usize,
    cause: AutomaticPlaybackTransitionCause,
    advanced_steps: u64,
) -> AutomaticPlaybackTransition {
    AutomaticPlaybackTransition {
        playback_number: playback.playback_number,
        cue_list_id: playback.cue_list_id,
        previous: cue_reference(&cue_list.cues[previous_index]),
        current: cue_reference(&cue_list.cues[playback.cue_index]),
        cause,
        advanced_steps,
    }
}

fn cue_reference(cue: &Cue) -> PlaybackCueReference {
    PlaybackCueReference {
        id: cue.id,
        number: cue.number,
    }
}
