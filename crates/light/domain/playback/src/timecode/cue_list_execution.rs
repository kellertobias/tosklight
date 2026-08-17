use crate::{CueList, CueTrigger};

use super::{
    TimecodeClipEnd, TimecodeClipId, TimecodeClipStart, TimecodeDefinition, TimecodeFrame,
    TimecodeFrameRate, TimecodeId, TimecodeLaneContent, TimecodeLaneId, TimecodeTransportState,
};
use light_core::CueListId;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimecodeCueListClipExecutionKind {
    Armed,
    Active,
    Held,
    Released,
    Unable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimecodeCueListClipExecution {
    pub timecode_id: TimecodeId,
    pub lane_id: TimecodeLaneId,
    pub cue_list_id: CueListId,
    pub clip_id: TimecodeClipId,
    pub kind: TimecodeCueListClipExecutionKind,
    pub cue_id: Option<Uuid>,
    pub cue_start_frame: Option<TimecodeFrame>,
    pub start_behavior: TimecodeClipStart,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimecodeCueTimingDefaults {
    pub frame_rate: TimecodeFrameRate,
    pub sequence_fade_millis: u64,
    pub release_fade_millis: u64,
}

impl TimecodeDefinition {
    /// Resolves every Cuelist clip against stable Cue identities at one authoritative frame.
    /// Timecode-trigger frames inside a clip are relative to the clip start.
    pub fn cue_list_clip_execution(
        &self,
        cue_lists: &[CueList],
        frame: TimecodeFrame,
        transport: TimecodeTransportState,
        timing: TimecodeCueTimingDefaults,
    ) -> Vec<TimecodeCueListClipExecution> {
        let mut result = Vec::new();
        for lane in &self.lanes {
            let TimecodeLaneContent::CueList { cue_list_id, clips } = &lane.content else {
                continue;
            };
            let cue_list = cue_lists
                .iter()
                .find(|candidate| candidate.id == *cue_list_id);
            let mut ordered = clips.iter().collect::<Vec<_>>();
            ordered.sort_by_key(|clip| (clip.start_frame, clip.end_frame, clip.id));
            for clip in ordered {
                let base = TimecodeCueListClipExecution {
                    timecode_id: self.id,
                    lane_id: lane.id,
                    cue_list_id: *cue_list_id,
                    clip_id: clip.id,
                    kind: TimecodeCueListClipExecutionKind::Armed,
                    cue_id: None,
                    cue_start_frame: None,
                    start_behavior: clip.start_behavior,
                    message: None,
                };
                let Some(cue_list) = cue_list else {
                    result.push(unable(base, "assigned Cuelist does not exist"));
                    continue;
                };
                let Some(start_index) = cue_list
                    .cues
                    .iter()
                    .position(|cue| cue.id == clip.start_cue_id)
                else {
                    result.push(unable(base, "start Cue does not exist"));
                    continue;
                };
                let Some(end_index) = cue_list
                    .cues
                    .iter()
                    .position(|cue| cue.id == clip.end_cue_id)
                else {
                    result.push(unable(base, "end Cue does not exist"));
                    continue;
                };
                if start_index > end_index {
                    result.push(unable(base, "start Cue follows end Cue in the Cuelist"));
                    continue;
                }
                if clip.end_frame.0 <= clip.start_frame.0 {
                    result.push(unable(base, "clip end must follow its start"));
                    continue;
                }
                let schedule = match cue_schedule(
                    cue_list,
                    start_index,
                    end_index,
                    clip.start_frame,
                    clip.end_frame,
                    timing,
                ) {
                    Ok(schedule) => schedule,
                    Err(message) => {
                        result.push(unable(base, &message));
                        continue;
                    }
                };
                if transport == TimecodeTransportState::Stopped || frame.0 < clip.start_frame.0 {
                    result.push(base);
                    continue;
                }
                if frame.0 >= clip.end_frame.0 {
                    result.push(match clip.end_behavior {
                        TimecodeClipEnd::Release => TimecodeCueListClipExecution {
                            kind: TimecodeCueListClipExecutionKind::Released,
                            ..base
                        },
                        TimecodeClipEnd::Hold => TimecodeCueListClipExecution {
                            kind: TimecodeCueListClipExecutionKind::Held,
                            cue_id: schedule.last().map(|(_, cue_id)| *cue_id),
                            cue_start_frame: schedule.last().map(|(start, _)| *start),
                            ..base
                        },
                    });
                    continue;
                }
                let (cue_start_frame, cue_id) = schedule
                    .iter()
                    .filter(|(start, _)| start.0 <= frame.0)
                    .copied()
                    .next_back()
                    .unwrap_or((clip.start_frame, clip.start_cue_id));
                result.push(TimecodeCueListClipExecution {
                    kind: TimecodeCueListClipExecutionKind::Active,
                    cue_id: Some(cue_id),
                    cue_start_frame: Some(cue_start_frame),
                    ..base
                });
            }
        }
        result
    }
}

fn cue_schedule(
    cue_list: &CueList,
    start_index: usize,
    end_index: usize,
    clip_start: TimecodeFrame,
    clip_end: TimecodeFrame,
    timing: TimecodeCueTimingDefaults,
) -> Result<Vec<(TimecodeFrame, Uuid)>, String> {
    let mut schedule: Vec<(TimecodeFrame, Uuid)> = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut index = start_index;
    let mut scheduled_start = clip_start;
    loop {
        if !visited.insert(index) {
            return Err("Cue Link forms a cycle inside the clip".into());
        }
        let cue = &cue_list.cues[index];
        let start = scheduled_start;
        if schedule
            .last()
            .is_some_and(|(previous, _)| start.0 < previous.0)
        {
            return Err(format!(
                "Cue {} starts before the preceding Cue",
                cue.number
            ));
        }
        if start.0 >= clip_end.0 {
            return Err(format!("Cue {} starts outside the clip", cue.number));
        }
        let completion = cue_completion_frames(cue_list, cue, timing);
        let completed_at = TimecodeFrame(start.0.saturating_add(completion));
        schedule.push((start, cue.id));
        if index == end_index {
            break;
        }
        let next_index = match cue.trigger {
            CueTrigger::Link {
                cue_id,
                delay_millis,
            } => {
                scheduled_start = TimecodeFrame(
                    completed_at
                        .0
                        .saturating_add(millis_to_frames(delay_millis, timing.frame_rate)),
                );
                cue_list
                    .cues
                    .iter()
                    .position(|candidate| candidate.id == cue_id)
                    .filter(|target| (start_index..=end_index).contains(target))
                    .ok_or_else(|| format!("Cue {} links outside the clip range", cue.number))?
            }
            _ => {
                let next_index = index + 1;
                let next = &cue_list.cues[next_index];
                scheduled_start = match next.trigger {
                    CueTrigger::Manual | CueTrigger::Link { .. } => {
                        return Err(format!(
                            "Cue {} requires manual GO and is not automatically scheduled",
                            next.number
                        ));
                    }
                    CueTrigger::Wait { delay_millis } => TimecodeFrame(
                        start
                            .0
                            .saturating_add(millis_to_frames(delay_millis, timing.frame_rate)),
                    ),
                    CueTrigger::Follow { delay_millis } => TimecodeFrame(
                        completed_at
                            .0
                            .saturating_add(millis_to_frames(delay_millis, timing.frame_rate)),
                    ),
                    CueTrigger::Timecode { frame } => {
                        TimecodeFrame(clip_start.0.saturating_add(frame))
                    }
                };
                next_index
            }
        };
        index = next_index;
    }
    Ok(schedule)
}

fn cue_completion_frames(
    cue_list: &CueList,
    cue: &crate::Cue,
    timing: TimecodeCueTimingDefaults,
) -> u64 {
    if cue_list.disable_cue_timing {
        return 0;
    }
    let in_fade = if cue.fade_millis == 0 {
        timing.sequence_fade_millis
    } else {
        cue.fade_millis
    };
    let out_delay = match cue.out_delay_link {
        Some(crate::CueOutDelayLink::InFade) => in_fade,
        None => cue.out_delay_millis.unwrap_or(cue.delay_millis),
    };
    let out_fade = match cue.out_fade_link {
        Some(crate::CueOutFadeLink::Release) => timing.release_fade_millis,
        None => cue.out_fade_millis.unwrap_or(in_fade),
    };
    millis_to_frames(
        cue.delay_millis
            .saturating_add(in_fade)
            .max(out_delay.saturating_add(out_fade)),
        timing.frame_rate,
    )
}

fn millis_to_frames(millis: u64, rate: TimecodeFrameRate) -> u64 {
    let numerator = u128::from(millis).saturating_mul(u128::from(rate.numerator()));
    let denominator = u128::from(rate.denominator()).saturating_mul(1_000);
    u64::try_from((numerator.saturating_add(denominator / 2)) / denominator).unwrap_or(u64::MAX)
}

fn unable(base: TimecodeCueListClipExecution, message: &str) -> TimecodeCueListClipExecution {
    TimecodeCueListClipExecution {
        kind: TimecodeCueListClipExecutionKind::Unable,
        message: Some(message.into()),
        ..base
    }
}
