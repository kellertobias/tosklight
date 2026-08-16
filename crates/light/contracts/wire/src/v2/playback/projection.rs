//! Immutable Playback projections shared by outcomes, events, and narrow repair snapshots.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlaybackRuntimeIdentity {
    Playback {
        playback_number: u16,
    },
    Virtual {
        page: u8,
        playback_number: u16,
    },
    CueList {
        cue_list_id: Uuid,
    },
    Group {
        #[schemars(length(min = 1, max = 256))]
        group_id: String,
    },
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
    Dynamic {
        dynamic_id: Option<Uuid>,
        last_known_pool_number: u16,
        embedded: bool,
        runtime: Option<Box<DynamicPlaybackRuntimeProjection>>,
    },
    Macro {
        macro_id: Uuid,
    },
    Timecode {
        timecode_id: Uuid,
    },
    Group {
        group_id: String,
        master: f32,
        flash_level: f32,
        fader_position: f32,
        fader_pickup_required: bool,
        fader_pickup_target: Option<f32>,
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
pub struct DynamicPlaybackRuntimeProjection {
    pub playback_number: u16,
    pub enabled: bool,
    pub paused: bool,
    pub flash: bool,
    pub activated_at: String,
    pub fader_value: f32,
    pub fader_pickup_required: bool,
    pub fader_pickup_target: Option<f32>,
    pub size: f32,
    pub master: f32,
    pub local_speed_numerator: u32,
    pub local_speed_denominator: u32,
    #[ts(type = "number | null")]
    pub learned_duration_millis: Option<u64>,
    pub state: DynamicPlaybackRuntimeState,
    pub instance_id: Option<Uuid>,
    pub controller_id: Uuid,
    pub winning_controller_id: Option<Uuid>,
    pub controller_status: DynamicPlaybackControllerStatus,
    pub target_count: usize,
    pub compatible_target_count: usize,
    pub missing_target_count: usize,
    pub unpatched_target_count: usize,
    pub lane_count: usize,
    pub supported_address_count: usize,
    pub skipped_address_count: usize,
    pub speed_source: DynamicPlaybackSpeedSource,
    pub effective_speed_multiplier: f32,
    #[ts(type = "number | null")]
    pub effective_duration_millis: Option<u64>,
    pub warning: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicPlaybackRuntimeState {
    Off,
    Zero,
    Pending,
    Active,
    Paused,
    Hidden,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicPlaybackControllerStatus {
    Winning,
    Losing,
    Missing,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicPlaybackSpeedSource {
    Fixed,
    SpeedGroup,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackCueReference {
    pub id: Uuid,
    pub number: String,
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
    pub deleted_cue_hold: Option<DeletedCueHoldProjection>,
    pub paused: bool,
    pub activated_at: String,
    pub paused_at: Option<String>,
    pub cue_timing: Option<CueTimingRuntimeProjection>,
    #[ts(type = "number")]
    pub transition_ordinal: u64,
    pub master: f32,
    pub fader_position: f32,
    pub fader_pickup_required: bool,
    pub fader_pickup_target: Option<f32>,
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

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DeletedCueHoldProjection {
    pub deleted_number: String,
    pub previous_number: Option<String>,
    pub next_number: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct CueTimingRuntimeProjection {
    pub cue_id: Uuid,
    #[ts(type = "number")]
    pub in_delay_millis: u64,
    #[ts(type = "number")]
    pub in_fade_millis: u64,
    #[ts(type = "number")]
    pub out_delay_millis: u64,
    #[ts(type = "number")]
    pub out_fade_millis: u64,
    #[ts(type = "number")]
    pub completion_millis: u64,
    pub active_trigger: Option<CueTriggerTimingProjection>,
    pub completed_trigger_cue_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct CueTriggerTimingProjection {
    pub cue: PlaybackCueReference,
    pub kind: CueTriggerTimingKind,
    pub started_at: String,
    #[ts(type = "number")]
    pub duration_millis: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueTriggerTimingKind {
    Follow,
    Wait,
    Link,
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

/// One sampled volatile runtime row for a numbered Playback. Static topology stays on the
/// snapshot + revisioned event path; a telemetry tick carries only playbacks whose sampled
/// values changed since the previous tick.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackTelemetrySample {
    pub playback_number: u16,
    pub enabled: bool,
    pub master: f32,
    pub current_cue: Option<PlaybackCueReference>,
    /// 0..=1 progress into the current Cue transition, or null while no Cuelist is active.
    pub fade_progress: Option<f32>,
    pub flash: bool,
    pub temporary_active: bool,
    pub swap_active: bool,
}

/// A sampled telemetry tick taken on a render-frame divider nearest ~10 Hz.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackTelemetryTick {
    pub scope: PlaybackShowScope,
    /// Completed render frame this tick was sampled on.
    #[ts(type = "number")]
    pub frame: u64,
    /// The telemetry sample rate implied by the configured output rate and its divider.
    pub sample_rate_hz: f32,
    /// Only the playbacks whose sampled values changed since the previous tick.
    pub samples: Vec<PlaybackTelemetrySample>,
    /// Playback numbers that stopped reporting since the previous tick (released/offline).
    pub released: Vec<u16>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackCueTransition {
    pub playback_number: Option<u16>,
    pub cue_list_id: Uuid,
    pub previous: Option<PlaybackCueReference>,
    pub current: Option<PlaybackCueReference>,
    pub cause: PlaybackTransitionCause,
    #[ts(type = "number")]
    pub transition_ordinal: u64,
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
    Link,
}
