//! Immutable Playback projections shared by outcomes, events, and narrow repair snapshots.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaybackRuntimeIdentity {
    Playback { playback_number: u16 },
    CueList { cue_list_id: Uuid },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackRuntimeProjection {
    pub scope: PlaybackShowScope,
    pub requested: PlaybackRuntimeIdentity,
    pub playback_number: Option<u16>,
    #[serde(flatten)]
    pub target: PlaybackTargetProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum PlaybackTargetProjection {
    Missing,
    CueList {
        cue_list_id: Uuid,
        runtime: Option<Box<CueListRuntimeProjection>>,
    },
    Group {
        group_id: String,
        master: f32,
        flash_level: f32,
    },
    SpeedGroup {
        group: String,
        runtime: Box<SpeedGroupRuntimeProjection>,
    },
    GrandMaster {
        runtime: GrandMasterRuntimeProjection,
    },
    ProgrammerFade {
        #[ts(type = "number")]
        millis: u64,
    },
    CueFade {
        #[ts(type = "number")]
        millis: u64,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackCueReference {
    pub id: Uuid,
    pub number: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct CueListRuntimeProjection {
    pub cue_index: usize,
    pub previous_index: Option<usize>,
    pub current: Option<PlaybackCueReference>,
    pub loaded: Option<PlaybackCueReference>,
    pub normal_next: Option<PlaybackCueReference>,
    pub effective_next: Option<PlaybackCueReference>,
    pub effective_next_is_loaded: bool,
    pub paused: bool,
    pub activated_at: String,
    pub master: f32,
    pub fader_position: f32,
    pub fader_pickup_required: bool,
    pub flash: bool,
    pub temporary: bool,
    pub temporary_active: bool,
    pub temporary_master: f32,
    pub swap_active: bool,
    pub enabled: bool,
    pub transition_timing_bypassed: bool,
    pub manual_xfade_position: f32,
    pub manual_xfade_direction: ManualXFadeDirection,
    pub manual_xfade_progress: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ManualXFadeDirection {
    TowardsHigh,
    TowardsLow,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SpeedGroupRuntimeProjection {
    pub manual_bpm: f64,
    pub sound_bpm: Option<f64>,
    pub effective_bpm: f64,
    pub source: SpeedSource,
    pub sound_status: SoundStatus,
    pub paused: bool,
    pub phase_advancing: bool,
    pub speed_master_scale: f64,
    pub sound_multiplier: f64,
    pub source_available: bool,
    pub usable_signal: bool,
    pub input_level: f32,
    pub selected_band_level: f32,
    pub synchronized_with: Option<u8>,
    #[ts(type = "number")]
    pub phase_origin_millis: u64,
    pub beat_phase: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SpeedSource {
    Manual,
    Sound,
    HeldSound,
    ManualFallback,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SoundStatus {
    Disabled,
    Active {
        detected_bpm: f64,
        confidence: f32,
    },
    Holding {
        reason: SoundLossReason,
        #[ts(type = "number")]
        remaining_millis: u64,
    },
    ManualFallback {
        reason: SoundLossReason,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SoundLossReason {
    SourceUnavailable,
    NoUsableSignal,
    LowConfidence,
    TempoOutsideRange,
    WaitingForAnalysis,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GrandMasterRuntimeProjection {
    pub level: f32,
    pub effective_level: f32,
    pub blackout: bool,
    pub flash_active: bool,
    pub dynamics_paused: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackDeskProjection {
    pub scope: PlaybackShowScope,
    pub desk_id: Uuid,
    pub active_page: u8,
    pub selected_playback: Option<u16>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackShowScope {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackRuntimeChange {
    pub projection: PlaybackRuntimeProjection,
    pub transition: Option<PlaybackCueTransition>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackCueTransition {
    pub playback_number: Option<u16>,
    pub cue_list_id: Uuid,
    pub previous: Option<PlaybackCueReference>,
    pub current: Option<PlaybackCueReference>,
    pub cause: PlaybackTransitionCause,
    #[ts(type = "number")]
    pub advanced_steps: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackTransitionCause {
    Go,
    Back,
    Jump,
    Chaser,
    Follow,
    Wait,
    Timecode,
}
