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
    pub timecode_source: Option<TimecodeSourceSelectionConfiguration>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub timecode_frame_rate: Option<Option<TimecodeFrameRateConfiguration>>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub timecode_external_loss_policy: Option<ExternalTimecodeLossPolicyConfiguration>,
    #[serde(default)]
    #[ts(optional = nullable)]
    #[ts(type = "number")]
    pub timecode_external_loss_timeout_millis: Option<u64>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub osc_timecode: Option<Option<OscTimecodeConfiguration>>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub timecode_audio_output_device: Option<Option<String>>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub timecode_audio_latency_trim_micros_by_output:
        Option<std::collections::BTreeMap<String, i64>>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub internal_audio_library_roots: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub internal_audio_output_devices: Option<std::collections::BTreeMap<String, String>>,
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
    pub cuelist_auto_off_at_zero_default: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub cuelist_auto_off_flash_release_default: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub start_after_first_recording: Option<bool>,
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
    pub highlight_look: Option<HighlightLookConfiguration>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub matter_enabled: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub pool_presentation: Option<PoolPresentationConfiguration>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub file_manager_system_picker_fallback: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub file_manager_roots: Option<Vec<FileManagerRoot>>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct HighlightLookConfiguration {
    pub intensity: f32,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub color: Option<HighlightLookColor>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub iris: Option<f32>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub zoom: Option<f32>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub focus: Option<f32>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub frost: Option<f32>,
    pub compatibility: HighlightLookCompatibility,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum HighlightLookCompatibility {
    Semantic,
    LegacyRaw,
    NeedsReview,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum HighlightLookColor {
    White,
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Amber,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PoolPresentationConfiguration {
    pub palette: PoolColorPalette,
    pub modes: std::collections::HashMap<String, PoolColorMode>,
    pub items: std::collections::HashMap<String, PoolItemPresentation>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PoolColorPalette {
    pub group: String,
    pub macro_color: String,
    pub dynamic: String,
    pub cuelist: String,
    pub sequence: String,
    pub preset: PresetPoolColorPalette,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PresetPoolColorPalette {
    pub mixed: String,
    pub intensity: String,
    pub color: String,
    pub position: String,
    pub beam: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PoolColorMode {
    Type,
    Individual,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PoolItemPresentation {
    #[serde(default)]
    #[ts(optional = nullable)]
    pub title: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub icon: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimecodeSourceSelectionConfiguration {
    Internal,
    External { source: String },
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TimecodeFrameRateConfiguration {
    pub numerator: u32,
    pub denominator: u32,
    pub drop_frame: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExternalTimecodeLossPolicyConfiguration {
    ContinueInternal,
    Pause,
    Stop,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_patch_accepts_named_semantic_highlight_look() {
        let request: ConfigurationUpdateRequest = serde_json::from_value(serde_json::json!({
            "request_id": "highlight-look",
            "patch": {
                "highlight_look": {
                    "intensity": 0.75,
                    "color": "amber",
                    "iris": null,
                    "zoom": 0.5,
                    "focus": null,
                    "frost": null,
                    "compatibility": "semantic"
                }
            }
        }))
        .unwrap();
        let look = request.patch.highlight_look.unwrap();
        assert_eq!(look.color, Some(HighlightLookColor::Amber));
        assert_eq!(look.compatibility, HighlightLookCompatibility::Semantic);
        assert_eq!(look.zoom, Some(0.5));
    }
}
