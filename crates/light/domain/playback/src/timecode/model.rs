use std::collections::BTreeMap;

use light_core::{CueListId, FixtureId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimecodeId(pub Uuid);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimecodeLaneId(pub Uuid);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimecodeClipId(pub Uuid);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimecodeKeyframeId(pub Uuid);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimecodeMarkerId(pub Uuid);

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct TimecodeFrame(pub u64);

impl TimecodeFrame {
    pub const ZERO: Self = Self(0);
}

/// A rational frame rate. `30_000 / 1_001` represents 29.97 fps without float drift.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TimecodeFrameRate {
    numerator: u32,
    denominator: u32,
    #[serde(default)]
    drop_frame: bool,
}

impl TimecodeFrameRate {
    pub fn new(numerator: u32, denominator: u32, drop_frame: bool) -> Result<Self, String> {
        if numerator == 0 || denominator == 0 {
            return Err("Timecode frame rate terms must be non-zero".into());
        }
        if drop_frame && (numerator != 30_000 || denominator != 1_001) {
            return Err("drop-frame numbering requires 30000/1001 fps".into());
        }
        Ok(Self {
            numerator,
            denominator,
            drop_frame,
        })
    }

    pub fn whole_frames(frames_per_second: u32) -> Result<Self, String> {
        Self::new(frames_per_second, 1, false)
    }

    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    pub const fn denominator(self) -> u32 {
        self.denominator
    }

    pub const fn drop_frame(self) -> bool {
        self.drop_frame
    }

    /// Resolves elapsed microseconds to the last frame whose boundary has been reached.
    pub fn frame_at_micros(self, elapsed_micros: u64) -> TimecodeFrame {
        let scaled = u128::from(elapsed_micros) * u128::from(self.numerator);
        let divisor = 1_000_000_u128 * u128::from(self.denominator);
        TimecodeFrame(u64::try_from(scaled / divisor).unwrap_or(u64::MAX))
    }

    /// Converts a frame boundary between known rates without passing through floating point.
    pub fn convert(self, frame: TimecodeFrame, target: Self) -> TimecodeFrame {
        let scaled =
            u128::from(frame.0) * u128::from(self.denominator) * u128::from(target.numerator);
        let divisor = u128::from(self.numerator) * u128::from(target.denominator);
        TimecodeFrame(u64::try_from(scaled / divisor).unwrap_or(u64::MAX))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimecodeDefinition {
    pub id: TimecodeId,
    pub number: u32,
    pub name: String,
    /// Configured end. `None` lets an attached audio asset supply the resolved runtime duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<TimecodeFrame>,
    #[serde(default)]
    pub transport_offset: TimecodeFrame,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<TimecodeAudio>,
    #[serde(default)]
    pub markers: Vec<TimecodeMarker>,
    #[serde(default)]
    pub lanes: Vec<TimecodeLane>,
}

impl TimecodeDefinition {
    /// Maps an external transport frame onto the editor's zero-based local timeline.
    pub fn local_frame_at_transport(&self, transport: TimecodeFrame) -> Option<TimecodeFrame> {
        transport
            .0
            .checked_sub(self.transport_offset.0)
            .map(TimecodeFrame)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TimecodeAudio {
    pub asset_id: Uuid,
    pub asset_revision: u64,
    /// The name of the file this audio was imported from.
    ///
    /// The asset id identifies the managed copy; it says nothing an operator recognises. This is
    /// what they chose, kept so the lane and the settings can name it. Absent in every show
    /// imported before it was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_fade_frames: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TimecodeMarker {
    pub id: TimecodeMarkerId,
    pub frame: TimecodeFrame,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimecodeLane {
    pub id: TimecodeLaneId,
    pub name: String,
    #[serde(flatten)]
    pub content: TimecodeLaneContent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TimecodeLaneContent {
    CueList {
        cue_list_id: CueListId,
        clips: Vec<TimecodeCueListClip>,
    },
    SpeedGroup {
        group: String,
        keyframes: Vec<TimecodeSpeedKeyframe>,
    },
    AudioVolume {
        keyframes: Vec<TimecodeVolumeKeyframe>,
    },
    AudioPlayer {
        fixture_id: FixtureId,
        clips: Vec<TimecodeAudioPlayerClip>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimecodeAudioPlayerClip {
    pub id: TimecodeClipId,
    pub start_frame: TimecodeFrame,
    pub end_frame: TimecodeFrame,
    pub folder: u8,
    pub file: u8,
    #[serde(default)]
    pub repeat: bool,
    #[serde(default)]
    pub volume_keyframes: Vec<TimecodeVolumeKeyframe>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimecodeClipStart {
    #[default]
    State,
    Cue,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimecodeClipEnd {
    #[default]
    Release,
    Hold,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TimecodeCueListClip {
    pub id: TimecodeClipId,
    pub start_frame: TimecodeFrame,
    pub end_frame: TimecodeFrame,
    pub start_cue_id: Uuid,
    pub end_cue_id: Uuid,
    #[serde(default)]
    pub start_behavior: TimecodeClipStart,
    #[serde(default)]
    pub end_behavior: TimecodeClipEnd,
    /// Transition points for Cues that would otherwise wait for a manual GO,
    /// offset from the clip start so the clip stays movable.
    #[serde(default)]
    pub cue_starts: Vec<TimecodeCueStart>,
    /// Frames over which the clip's own level rises from nothing at the clip start.
    ///
    /// This is the clip's contribution, not any Cue's timing: it scales whatever the driven
    /// Cuelist is putting out for as long as this clip owns it. Zero is a hard start, which is
    /// what every show recorded before clip fades existed did.
    #[serde(default)]
    pub in_fade_frames: u64,
    /// Frames over which the clip's own level falls to nothing, measured back from the clip end.
    #[serde(default)]
    pub out_fade_frames: u64,
}

impl TimecodeCueListClip {
    /// The clip's own level at one frame, from its in fade and out fade.
    ///
    /// Outside the clip the level is nothing; a clip too short to hold both fades gets the lower
    /// of the two rather than a discontinuity where they cross.
    pub fn level_at(&self, frame: TimecodeFrame) -> f32 {
        if frame < self.start_frame || frame > self.end_frame {
            return 0.0;
        }
        let rising = ramp(
            frame.0.saturating_sub(self.start_frame.0),
            self.in_fade_frames,
        );
        let falling = ramp(
            self.end_frame.0.saturating_sub(frame.0),
            self.out_fade_frames,
        );
        rising.min(falling)
    }
}

/// A linear ramp: full level once `elapsed` has covered `over`, and full immediately when the
/// fade is zero frames long.
fn ramp(elapsed: u64, over: u64) -> f32 {
    if over == 0 || elapsed >= over {
        return 1.0;
    }
    elapsed as f32 / over as f32
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TimecodeCueStart {
    pub cue_id: Uuid,
    pub offset_frame: TimecodeFrame,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimecodeSpeedKeyframe {
    pub id: TimecodeKeyframeId,
    pub frame: TimecodeFrame,
    pub bpm: f64,
    /// Explicit normalized phase. A value of zero restarts on the keyframe boundary.
    pub phase: f64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimecodeCurve {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimecodeVolumeKeyframe {
    pub id: TimecodeKeyframeId,
    pub frame: TimecodeFrame,
    pub value: f32,
    #[serde(default)]
    pub fade_frames: u64,
    #[serde(default)]
    pub curve: TimecodeCurve,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimecodeReconstructedState {
    pub frame: TimecodeFrame,
    pub cue_lists: Vec<TimecodeCueListState>,
    pub speed_groups: BTreeMap<String, TimecodeSpeedState>,
    pub audio_volume: f32,
    pub audio_players: Vec<TimecodeAudioPlayerState>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimecodeAudioPlayerState {
    pub lane_id: TimecodeLaneId,
    pub fixture_id: FixtureId,
    pub clip_id: TimecodeClipId,
    pub folder: u8,
    pub file: u8,
    pub repeat: bool,
    pub volume: f32,
    pub cursor_frame: TimecodeFrame,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimecodeCueListStateKind {
    Active,
    Held,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimecodeCueListState {
    pub lane_id: TimecodeLaneId,
    pub cue_list_id: CueListId,
    pub clip_id: TimecodeClipId,
    pub start_cue_id: Uuid,
    pub end_cue_id: Uuid,
    pub start_behavior: TimecodeClipStart,
    pub kind: TimecodeCueListStateKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimecodeSpeedState {
    pub bpm: f64,
    pub phase: f64,
}

/// One member of the atomic batch scheduled on a frame, in persisted lane/keyframe order.
#[derive(Clone, Debug, PartialEq)]
pub enum TimecodeScheduledAction {
    CueListStart {
        lane_id: TimecodeLaneId,
        cue_list_id: CueListId,
        clip_id: TimecodeClipId,
        start_cue_id: Uuid,
        end_cue_id: Uuid,
        start_behavior: TimecodeClipStart,
    },
    CueListEnd {
        lane_id: TimecodeLaneId,
        cue_list_id: CueListId,
        clip_id: TimecodeClipId,
        end_behavior: TimecodeClipEnd,
    },
    SpeedGroup {
        lane_id: TimecodeLaneId,
        group: String,
        keyframe_id: TimecodeKeyframeId,
        bpm: f64,
        phase: f64,
    },
    AudioVolume {
        lane_id: TimecodeLaneId,
        keyframe_id: TimecodeKeyframeId,
        value: f32,
        fade_frames: u64,
        curve: TimecodeCurve,
    },
}
