use std::collections::BTreeMap;

use super::{
    TimecodeAudioPlayerState, TimecodeClipEnd, TimecodeCueListState, TimecodeCueListStateKind,
    TimecodeCurve, TimecodeDefinition, TimecodeFrame, TimecodeLaneContent,
    TimecodeReconstructedState, TimecodeScheduledAction, TimecodeSpeedState,
    TimecodeVolumeKeyframe,
};

impl TimecodeDefinition {
    /// Pure reconstruction used by continuous playback, seek, and full-timeline loop.
    ///
    /// Lanes and their keyframes are visited in persisted order. A later same-frame value therefore
    /// deterministically replaces an earlier one. Each Cuelist lane reconstructs one authoritative
    /// clip state; a later clip supersedes an earlier held clip.
    pub fn state_at(&self, frame: TimecodeFrame) -> TimecodeReconstructedState {
        let frame = self
            .duration
            .map_or(frame, |duration| TimecodeFrame(frame.0.min(duration.0)));
        let mut cue_lists = Vec::new();
        let mut speed_groups = BTreeMap::new();
        let mut audio_volume = 1.0;
        let mut audio_players = Vec::new();

        for lane in &self.lanes {
            match &lane.content {
                TimecodeLaneContent::CueList { cue_list_id, clips } => {
                    let mut lane_state = None;
                    let mut ordered = clips.iter().collect::<Vec<_>>();
                    ordered.sort_by_key(|clip| (clip.start_frame, clip.end_frame, clip.id));
                    for clip in ordered {
                        if frame.0 < clip.start_frame.0 {
                            break;
                        }
                        let kind = if frame.0 < clip.end_frame.0 {
                            TimecodeCueListStateKind::Active
                        } else if clip.end_behavior == TimecodeClipEnd::Hold {
                            TimecodeCueListStateKind::Held
                        } else {
                            lane_state = None;
                            continue;
                        };
                        lane_state = Some(TimecodeCueListState {
                            lane_id: lane.id,
                            cue_list_id: *cue_list_id,
                            clip_id: clip.id,
                            start_cue_id: clip.start_cue_id,
                            end_cue_id: clip.end_cue_id,
                            start_behavior: clip.start_behavior,
                            kind,
                        });
                    }
                    if let Some(state) = lane_state {
                        cue_lists.push(state);
                    }
                }
                TimecodeLaneContent::SpeedGroup { group, keyframes } => {
                    for keyframe in keyframes.iter().filter(|item| item.frame.0 <= frame.0) {
                        speed_groups.insert(
                            group.clone(),
                            TimecodeSpeedState {
                                bpm: keyframe.bpm,
                                phase: keyframe.phase,
                            },
                        );
                    }
                }
                TimecodeLaneContent::AudioVolume { keyframes } => {
                    audio_volume = volume_at(keyframes, frame);
                }
                TimecodeLaneContent::AudioPlayer { fixture_id, clips } => {
                    if let Some(clip) = clips
                        .iter()
                        .find(|clip| clip.start_frame.0 <= frame.0 && frame.0 < clip.end_frame.0)
                    {
                        audio_players.push(TimecodeAudioPlayerState {
                            lane_id: lane.id,
                            fixture_id: *fixture_id,
                            clip_id: clip.id,
                            folder: clip.folder,
                            file: clip.file,
                            repeat: clip.repeat,
                            volume: volume_at(&clip.volume_keyframes, frame),
                            cursor_frame: TimecodeFrame(frame.0 - clip.start_frame.0),
                        });
                    }
                }
            }
        }

        TimecodeReconstructedState {
            frame,
            cue_lists,
            speed_groups,
            audio_volume,
            audio_players,
        }
    }

    /// Returns one ordered batch for a frame. The runtime must apply this vector atomically.
    pub fn actions_at(&self, frame: TimecodeFrame) -> Vec<TimecodeScheduledAction> {
        let mut actions = Vec::new();
        for lane in &self.lanes {
            match &lane.content {
                TimecodeLaneContent::CueList { cue_list_id, clips } => {
                    for clip in clips {
                        if clip.start_frame == frame {
                            actions.push(TimecodeScheduledAction::CueListStart {
                                lane_id: lane.id,
                                cue_list_id: *cue_list_id,
                                clip_id: clip.id,
                                start_cue_id: clip.start_cue_id,
                                end_cue_id: clip.end_cue_id,
                                start_behavior: clip.start_behavior,
                            });
                        }
                        if clip.end_frame == frame {
                            actions.push(TimecodeScheduledAction::CueListEnd {
                                lane_id: lane.id,
                                cue_list_id: *cue_list_id,
                                clip_id: clip.id,
                                end_behavior: clip.end_behavior,
                            });
                        }
                    }
                }
                TimecodeLaneContent::SpeedGroup { group, keyframes } => {
                    actions.extend(
                        keyframes
                            .iter()
                            .filter(|keyframe| keyframe.frame == frame)
                            .map(|keyframe| TimecodeScheduledAction::SpeedGroup {
                                lane_id: lane.id,
                                group: group.clone(),
                                keyframe_id: keyframe.id,
                                bpm: keyframe.bpm,
                                phase: keyframe.phase,
                            }),
                    );
                }
                TimecodeLaneContent::AudioVolume { keyframes } => {
                    actions.extend(
                        keyframes
                            .iter()
                            .filter(|keyframe| keyframe.frame == frame)
                            .map(|keyframe| TimecodeScheduledAction::AudioVolume {
                                lane_id: lane.id,
                                keyframe_id: keyframe.id,
                                value: keyframe.value,
                                fade_frames: keyframe.fade_frames,
                                curve: keyframe.curve,
                            }),
                    );
                }
                TimecodeLaneContent::AudioPlayer { .. } => {}
            }
        }
        actions
    }
}

fn volume_at(keyframes: &[TimecodeVolumeKeyframe], frame: TimecodeFrame) -> f32 {
    let mut segment = VolumeSegment::constant(1.0);
    for keyframe in keyframes {
        if frame.0 < keyframe.frame.0 {
            break;
        }
        let start = segment.value_at(keyframe.frame);
        segment = VolumeSegment {
            start,
            target: keyframe.value,
            frame: keyframe.frame,
            duration: keyframe.fade_frames,
            curve: keyframe.curve,
        };
    }
    segment.value_at(frame).clamp(0.0, 1.0)
}

#[derive(Clone, Copy)]
struct VolumeSegment {
    start: f32,
    target: f32,
    frame: TimecodeFrame,
    duration: u64,
    curve: TimecodeCurve,
}

impl VolumeSegment {
    const fn constant(value: f32) -> Self {
        Self {
            start: value,
            target: value,
            frame: TimecodeFrame::ZERO,
            duration: 0,
            curve: TimecodeCurve::Linear,
        }
    }

    fn value_at(self, frame: TimecodeFrame) -> f32 {
        if self.duration == 0 {
            return self.target;
        }
        let elapsed = frame.0.saturating_sub(self.frame.0);
        let progress = (elapsed as f32 / self.duration as f32).min(1.0);
        self.start + (self.target - self.start) * curve(progress, self.curve)
    }
}

fn curve(progress: f32, curve: TimecodeCurve) -> f32 {
    match curve {
        TimecodeCurve::Linear => progress,
        TimecodeCurve::EaseIn => progress * progress,
        TimecodeCurve::EaseOut => 1.0 - (1.0 - progress) * (1.0 - progress),
        TimecodeCurve::EaseInOut if progress < 0.5 => 2.0 * progress * progress,
        TimecodeCurve::EaseInOut => 1.0 - (-2.0 * progress + 2.0).powi(2) / 2.0,
    }
}
