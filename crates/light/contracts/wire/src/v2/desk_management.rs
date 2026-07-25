//! Typed requests for installation settings and desk-owned operator state.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ConfigurationUpdateRequest {
    pub request_id: String,
    pub patch: ConfigurationPatch,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ConfigurationPatch {
    #[serde(default)]
    #[ts(optional = nullable)]
    pub frame_rate_hz: Option<u16>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub output_bind_ip: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub osc_bind: Option<Option<String>>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub art_timecode_bind: Option<Option<String>>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub midi_inputs: Option<Vec<String>>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub rtp_midi_bind: Option<Option<String>>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub timecode_sources: Option<Vec<TimecodeSourceConfiguration>>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub osc_timecode: Option<Option<OscTimecodeConfiguration>>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub backup_retention: Option<usize>,
    #[serde(default)]
    #[ts(optional = nullable)]
    #[ts(type = "number")]
    pub autosave_interval_seconds: Option<u64>,
    #[serde(default)]
    #[ts(optional = nullable)]
    #[ts(type = "number")]
    pub programmer_fade_millis: Option<u64>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub command_line_at_uses_programmer_fade: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    #[ts(type = "number")]
    pub sequence_master_fade_millis: Option<u64>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub preload_programmer_changes: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub preload_physical_playback_actions: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub preload_virtual_playback_actions: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub patch_preview_highlight_dmx: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub matter_enabled: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub file_manager_system_picker_fallback: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub file_manager_roots: Option<Vec<FileManagerRoot>>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeSourceConfiguration {
    pub source_prefix: String,
    pub priority: i16,
    pub fallback: bool,
    #[ts(type = "number")]
    pub loss_timeout_millis: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OscTimecodeConfiguration {
    pub address: String,
    pub rate: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FileManagerRoot {
    pub id: String,
    pub label: String,
    pub path: String,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SpeedGroupSettingsUpdateRequest {
    pub request_id: String,
    pub source: SpeedGroupSource,
    pub configuration: SoundToLightConfiguration,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpeedGroupSource {
    Manual,
    SpeedGroup {
        group: super::speed_group::SpeedGroupId,
    },
    SoundToLight,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SoundToLightConfiguration {
    pub analysis_mode: SoundAnalysisMode,
    pub frequency: FrequencySelection,
    pub input_gain_db: f32,
    pub confidence_threshold: f32,
    pub smoothing: f32,
    pub minimum_bpm: f64,
    pub maximum_bpm: f64,
    #[ts(type = "number")]
    pub signal_hold_millis: u64,
    pub multiplier: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SoundAnalysisMode {
    TempoBpm,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FrequencySelection {
    Preset { preset: FrequencyPreset },
    Custom { low_hz: u16, high_hz: u16 },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FrequencyPreset {
    Sub,
    Low,
    Mid,
    High,
    FullRange,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SpeedGroupLiveActionRequest {
    pub action: SpeedGroupLiveAction,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub bpm: Option<f64>,
    #[serde(default)]
    #[ts(optional = nullable)]
    #[ts(type = "number")]
    pub captured_at_millis: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputMasterActionRequest {
    #[serde(default)]
    #[ts(optional = nullable)]
    pub grand_master: Option<f32>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub blackout: Option<bool>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SpeedGroupLiveAction {
    SetBpm,
    Learn,
    Double,
    Half,
    Pause,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SoundObservation {
    #[ts(type = "number")]
    pub captured_at_millis: u64,
    pub source_available: bool,
    pub usable_signal: bool,
    pub level: f32,
    pub selected_band_level: f32,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub detected_bpm: Option<f64>,
    pub confidence: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DeskLockConfigurationUpdateRequest {
    pub request_id: String,
    pub message: String,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub wallpaper: Option<String>,
    pub unlock_mode: DeskUnlockMode,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub pin: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeskUnlockMode {
    Button,
    Pin,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DeskUnlockRequest {
    #[serde(default)]
    #[ts(optional = nullable)]
    pub pin: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct UserCreateRequest {
    pub request_id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}
