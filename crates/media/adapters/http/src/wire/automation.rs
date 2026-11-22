//! Playlist configuration intents and autonomous-playback projections.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntryView {
    pub id: String,
    pub asset_id: String,
    pub plays: u32,
    #[ts(type = "number | null")]
    pub hold_millis: Option<u64>,
    #[ts(type = "number")]
    pub fade_millis: u64,
    #[serde(default)]
    pub transform: PlaylistTransformView,
}

/// Step framing uses the same units and limits as ordinary layer controls.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaylistTransformView {
    pub scale_x: f32,
    pub scale_y: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
}

impl Default for PlaylistTransformView {
    fn default() -> Self {
        let transform = media_domain::playlist::PlaylistTransform::default();
        Self {
            scale_x: transform.scale_x,
            scale_y: transform.scale_y,
            position_x: transform.position_x,
            position_y: transform.position_y,
            rotation: transform.rotation,
        }
    }
}

/// The ordinary layer-control shape, without live source status or reset counters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct LayerPresetView {
    #[ts(
        type = "string | { once: { end_state: string } } | { 'reverse-once': { end_state: string } }"
    )]
    pub play_mode: media_domain::PlayMode,
    pub scale_x: f32,
    pub scale_y: f32,
    #[ts(type = "string")]
    pub scaling_mode: media_domain::ScalingMode,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub dimmer: f32,
    pub volume: f32,
    #[ts(type = "{red:number, green:number, blue:number}")]
    pub tint: media_domain::Tint,
    pub grayscale: f32,
    #[ts(
        type = "{address:{folder:number,file:number},scaleX:number,scaleY:number,positionX:number,positionY:number,invert:boolean,opacity:number,source:string}"
    )]
    pub mask: media_domain::layer::MaskState,
    #[ts(type = "Array<{select:number,strength:number,parameters:number[]}>")]
    pub effect_banks: [media_domain::EffectBankState; 2],
    #[ts(type = "unknown[]")]
    pub effects: [media_domain::EffectSlot; 4],
    #[ts(type = "{kind:'unity'} | {kind:'divide'|'multiply',factor:number}")]
    pub speed_multiplier: media_domain::SpeedMultiplier,
    pub playback_bpm: Option<u8>,
    pub blur: f32,
    #[ts(type = "string")]
    pub blend: media_domain::BlendMode,
    pub strobe_hz: Option<f32>,
    pub in_point: u16,
    pub out_point: u16,
    pub visualizer_controls: [u8; 4],
    #[ts(type = "unknown | null")]
    pub visualizer_tuning: Option<media_domain::visualizer_channels::VisualizerTuning>,
    #[ts(type = "{model:number,pan:number,tilt:number}")]
    pub model: media_domain::ModelMapping,
}

impl Default for LayerPresetView {
    fn default() -> Self {
        let p = media_domain::playlist::LayerPreset::default();
        Self {
            play_mode: p.play_mode,
            scale_x: p.scale_x,
            scale_y: p.scale_y,
            scaling_mode: p.scaling_mode,
            position_x: p.position_x,
            position_y: p.position_y,
            rotation: p.rotation,
            dimmer: p.dimmer,
            volume: p.volume,
            tint: p.tint,
            grayscale: p.grayscale,
            mask: p.mask,
            effect_banks: p.effect_banks,
            effects: p.effects,
            speed_multiplier: p.speed_multiplier,
            playback_bpm: p.playback_bpm,
            blur: p.blur,
            blend: p.blend,
            strobe_hz: p.strobe_hz,
            in_point: p.in_point,
            out_point: p.out_point,
            visualizer_controls: p.visualizer_controls,
            visualizer_tuning: p.visualizer_tuning,
            model: p.model,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AuxiliaryLayerView {
    pub layer: usize,
    pub mode: String,
    pub asset_id: Option<String>,
    #[serde(default)]
    pub controls: LayerPresetView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistView {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub layers: [usize; 2],
    pub master_dimmer: f32,
    pub master_volume: f32,
    #[ts(type = "number")]
    pub default_fade_millis: u64,
    pub entries: Vec<PlaylistEntryView>,
    pub auxiliary_layers: Vec<AuxiliaryLayerView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScheduleTriggerView {
    Fixed {
        start: String,
        end: Option<String>,
    },
    Weekly {
        weekdays: Vec<u8>,
        start_time: String,
        end_time: Option<String>,
        end_day_offset: u8,
        first_date: Option<String>,
        last_date: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleEventView {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub output_id: String,
    pub playlist_id: String,
    pub trigger: ScheduleTriggerView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AutomationOutputView {
    pub output_id: String,
    pub enabled: bool,
    pub fallback_playlist_id: Option<String>,
    #[ts(type = "number")]
    pub dmx_silence_seconds: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AutomationConfigurationView {
    pub playlists: Vec<PlaylistView>,
    pub events: Vec<ScheduleEventView>,
    pub outputs: Vec<AutomationOutputView>,
}

impl AutomationConfigurationView {
    pub fn of(configuration: &media_domain::playlist::AutomationConfiguration) -> Self {
        serde_json::from_value(
            serde_json::to_value(configuration).expect("configuration serializes"),
        )
        .expect("automation has a matching wire projection")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AutomationStatus {
    pub output_id: String,
    pub owner: String,
    pub playlist_id: Option<String>,
    pub event_id: Option<String>,
    pub playlist_name: Option<String>,
    pub entry_index: Option<usize>,
    pub play: u32,
    pub phase: String,
    pub error: Option<String>,
    pub last_occurrence_outcome: Option<String>,
    pub last_occurrence_name: Option<String>,
    #[ts(type = "number | null")]
    pub dmx_age_millis: Option<u64>,
    #[ts(type = "number | null")]
    pub resume_in_millis: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpcomingOccurrenceView {
    pub event_id: String,
    pub playlist_id: String,
    pub output_id: String,
    pub name: String,
    pub start: String,
    pub end: Option<String>,
    pub priority: i32,
    pub winning: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimeView {
    #[ts(type = "number")]
    pub automation_revision: u64,
    pub time_zone: String,
    pub status: Vec<AutomationStatus>,
    pub upcoming: Vec<UpcomingOccurrenceView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AutomationView {
    pub configuration: AutomationConfigurationView,
    #[serde(flatten)]
    pub runtime: AutomationRuntimeView,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEdit {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<[usize; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_dimmer: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_volume: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub default_fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<PlaylistEntryView>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auxiliary_layers: Option<Vec<AuxiliaryLayerView>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleEventEdit {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playlist_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<ScheduleTriggerView>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AutomationOutputEdit {
    pub request_id: String,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub fallback_playlist_id: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub dmx_silence_seconds: Option<u64>,
}

fn nullable<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAutomationObject {
    pub request_id: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum AutomationAction {
    #[serde(rename_all = "camelCase")]
    AutomationApplyNow {
        output_id: String,
    },
    #[serde(rename_all = "camelCase")]
    AutomationSetEnabled {
        output_id: String,
        enabled: bool,
    },
    AutomationPreviewStep {
        request: super::PreviewPlaylistStep,
    },
    AutomationStopPreview {
        request: super::StopPlaylistPreview,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct SetAutomationEnabled {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AutomationActionFrame {
    pub action_id: String,
    #[serde(flatten)]
    pub action: AutomationAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AutomationActionResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub preview: Option<super::PlaylistPreviewResult>,
    #[serde(rename = "type")]
    pub kind: String,
    pub action_id: String,
    pub ok: bool,
    pub message: Option<String>,
}
