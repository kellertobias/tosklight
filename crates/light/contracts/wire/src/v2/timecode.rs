//! Portable Timecode object and authoritative transport contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeAudioOutputDevices {
    pub devices: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeAudioImportResult {
    pub asset_id: Uuid,
    #[ts(type = "number")]
    pub asset_revision: u64,
    pub name: String,
    pub media_type: String,
    pub sample_rate: u32,
    pub channels: u16,
    #[ts(type = "number")]
    pub sample_frames: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeAudioWaveform {
    pub peaks: Vec<f32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeFrameRate {
    pub numerator: u32,
    pub denominator: u32,
    pub drop_frame: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeDefinition {
    pub id: Uuid,
    pub number: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub duration_frame: Option<u64>,
    #[ts(type = "number")]
    pub transport_offset_frame: u64,
    pub auto_start: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub audio: Option<TimecodeAudio>,
    #[serde(default)]
    pub markers: Vec<TimecodeMarker>,
    #[serde(default)]
    pub lanes: Vec<TimecodeLane>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeObjectRecord {
    #[ts(type = "number")]
    pub revision: u64,
    pub definition: TimecodeDefinition,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeCollectionSnapshot {
    #[ts(type = "number")]
    pub show_revision: u64,
    pub objects: Vec<TimecodeObjectRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeAudio {
    pub asset_id: Uuid,
    #[ts(type = "number")]
    pub asset_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub end_fade_frames: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeMarker {
    pub id: Uuid,
    #[ts(type = "number")]
    pub frame: u64,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeLane {
    pub id: Uuid,
    pub name: String,
    pub content: TimecodeLaneContent,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TimecodeLaneContent {
    CueList {
        cue_list_id: Uuid,
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
        fixture_id: Uuid,
        clips: Vec<TimecodeAudioPlayerClip>,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeAudioPlayerClip {
    pub id: Uuid,
    #[ts(type = "number")]
    pub start_frame: u64,
    #[ts(type = "number")]
    pub end_frame: u64,
    pub folder: u8,
    pub file: u8,
    #[serde(default)]
    pub repeat: bool,
    #[serde(default)]
    pub volume_keyframes: Vec<TimecodeVolumeKeyframe>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TimecodeClipStart {
    #[default]
    State,
    Cue,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TimecodeClipEnd {
    #[default]
    Release,
    Hold,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeCueListClip {
    pub id: Uuid,
    #[ts(type = "number")]
    pub start_frame: u64,
    #[ts(type = "number")]
    pub end_frame: u64,
    pub start_cue_id: Uuid,
    pub end_cue_id: Uuid,
    #[serde(default)]
    pub start_behavior: TimecodeClipStart,
    #[serde(default)]
    pub end_behavior: TimecodeClipEnd,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeSpeedKeyframe {
    pub id: Uuid,
    #[ts(type = "number")]
    pub frame: u64,
    pub bpm: f64,
    pub phase: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TimecodeCurve {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeVolumeKeyframe {
    pub id: Uuid,
    #[ts(type = "number")]
    pub frame: u64,
    pub value: f32,
    #[ts(type = "number")]
    pub fade_frames: u64,
    pub curve: TimecodeCurve,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeObjectActionRequest {
    pub request_id: String,
    pub action: TimecodeObjectAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimecodeObjectAction {
    Create {
        definition: TimecodeDefinition,
    },
    Update {
        timecode_id: Uuid,
        #[ts(type = "number")]
        expected_revision: u64,
        patch: TimecodePatch,
    },
    Delete {
        timecode_id: Uuid,
        #[ts(type = "number")]
        expected_revision: u64,
    },
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodePatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub duration_frame: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub transport_offset_frame: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub auto_start: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub audio: Option<TimecodeAudio>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub markers: Option<Vec<TimecodeMarker>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub lanes: Option<Vec<TimecodeLane>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimecodeTransportAction {
    Go,
    Pause,
    Stop,
    Rewind,
    Seek {
        #[ts(type = "number")]
        frame: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeTransportActionRequest {
    pub timecode_id: Uuid,
    pub action: TimecodeTransportAction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TimecodeTransportState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TimecodeCueListClipExecutionState {
    Armed,
    Active,
    Held,
    Released,
    Unable,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeCueListClipExecution {
    pub lane_id: Uuid,
    pub cue_list_id: Uuid,
    pub clip_id: Uuid,
    pub state: TimecodeCueListClipExecutionState,
    pub cue_id: Option<Uuid>,
    #[ts(type = "number | null")]
    pub cue_start_frame: Option<u64>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeTransportSnapshot {
    pub timecode_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    pub state: TimecodeTransportState,
    #[ts(type = "number")]
    pub frame: u64,
    #[ts(type = "number")]
    pub duration_frame: u64,
    pub audio_linked: bool,
    pub cue_list_clips: Vec<TimecodeCueListClipExecution>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_action_accepts_unknown_fields_at_each_typed_boundary() {
        assert!(
            serde_json::from_value::<TimecodeTransportActionRequest>(serde_json::json!({
                "timecode_id": Uuid::from_u128(1),
                "action": {"type": "seek", "frame": 44, "future": true},
                "future": true
            }))
            .is_ok()
        );
    }

    #[test]
    fn portable_definition_preserves_lane_and_keyframe_order() {
        let value = serde_json::json!({
            "id": Uuid::from_u128(1),
            "number": 1,
            "name": "Opener",
            "duration_frame": 880,
            "transport_offset_frame": 0,
            "auto_start": false,
            "markers": [],
            "lanes": [{
                "id": Uuid::from_u128(2),
                "name": "Speed A",
                "content": {
                    "kind": "speed_group",
                    "group": "A",
                    "keyframes": [
                        {"id": Uuid::from_u128(3), "frame": 44, "bpm": 120.0, "phase": 0.0},
                        {"id": Uuid::from_u128(4), "frame": 44, "bpm": 90.0, "phase": 0.25}
                    ]
                }
            }]
        });
        let decoded: TimecodeDefinition = serde_json::from_value(value).unwrap();
        let TimecodeLaneContent::SpeedGroup { keyframes, .. } = &decoded.lanes[0].content else {
            panic!("expected Speed Group lane");
        };
        assert_eq!(keyframes[0].bpm, 120.0);
        assert_eq!(keyframes[1].bpm, 90.0);
    }
}
