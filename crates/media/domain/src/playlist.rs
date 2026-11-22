//! Persisted playlist intent, independent of playback sessions and wall clocks.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::layer::MaskState;
use crate::{
    AssetId, BlendMode, EffectBankState, EffectSlot, LayerControls, LayerState, MediaAddress,
    ModelMapping, OutputId, PlayMode, ScalingMode, SpeedMultiplier, Tint,
};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutomationConfiguration {
    pub playlists: Vec<Playlist>,
    pub events: Vec<ScheduleEvent>,
    pub outputs: Vec<AutomationOutput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: Uuid,
    pub name: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
    /// Operator-facing layer numbers, starting at one.
    #[serde(default = "default_layers")]
    pub layers: [usize; 2],
    #[serde(default = "full_level")]
    pub master_dimmer: f32,
    #[serde(default = "full_level")]
    pub master_volume: f32,
    #[serde(default)]
    pub default_fade_millis: u64,
    #[serde(default)]
    pub entries: Vec<PlaylistEntry>,
    #[serde(default)]
    pub auxiliary_layers: Vec<AuxiliaryLayer>,
}

const fn default_layers() -> [usize; 2] {
    [2, 3]
}
const fn full_level() -> f32 {
    1.0
}

impl Default for Playlist {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Playlist".to_owned(),
            enabled: true,
            priority: 0,
            layers: [2, 3],
            master_dimmer: 1.0,
            master_volume: 1.0,
            default_fade_millis: 0,
            entries: Vec::new(),
            auxiliary_layers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    pub id: Uuid,
    pub asset_id: AssetId,
    #[serde(default = "one")]
    pub plays: u32,
    #[serde(default)]
    pub hold_millis: Option<u64>,
    #[serde(default)]
    pub fade_millis: u64,
    #[serde(default)]
    pub transform: PlaylistTransform,
}

/// Framing authored independently for one playlist step, in ordinary layer-control units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaylistTransform {
    pub scale_x: f32,
    pub scale_y: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
}

impl Default for PlaylistTransform {
    fn default() -> Self {
        Self {
            scale_x: 1.0,
            scale_y: 1.0,
            position_x: 0.0,
            position_y: 0.0,
            rotation: 0.0,
        }
    }
}

impl PlaylistTransform {
    pub fn validate(&self) -> Result<(), String> {
        for (name, value, minimum, maximum) in [
            ("scaleX", self.scale_x, 0.0, 10.0),
            ("scaleY", self.scale_y, 0.0, 10.0),
            ("positionX", self.position_x, -2.0, 2.0),
            ("positionY", self.position_y, -2.0, 2.0),
            ("rotation", self.rotation, -360.0, 360.0),
        ] {
            if !value.is_finite() || !(minimum..=maximum).contains(&value) {
                return Err(format!(
                    "{name} must be finite and between {minimum} and {maximum}"
                ));
            }
        }
        Ok(())
    }

    /// Shared by autonomous playback and step preview so their framing is identical.
    pub fn layer_preset(&self) -> LayerPreset {
        LayerPreset {
            scale_x: self.scale_x,
            scale_y: self.scale_y,
            position_x: self.position_x,
            position_y: self.position_y,
            rotation: self.rotation,
            ..LayerPreset::default()
        }
    }
}

const fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuxiliaryMode {
    #[default]
    Unchanged,
    Off,
    Fixed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuxiliaryLayer {
    pub layer: usize,
    #[serde(default)]
    pub mode: AuxiliaryMode,
    #[serde(default)]
    pub asset_id: Option<AssetId>,
    #[serde(default)]
    pub controls: LayerPreset,
}

/// Authorable layer controls. Runtime status and reset counters are never persisted here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LayerPreset {
    pub play_mode: PlayMode,
    pub scale_x: f32,
    pub scale_y: f32,
    pub scaling_mode: ScalingMode,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub dimmer: f32,
    pub volume: f32,
    pub tint: Tint,
    pub grayscale: f32,
    pub mask: MaskState,
    pub effect_banks: [EffectBankState; 2],
    pub effects: [EffectSlot; 4],
    pub speed_multiplier: SpeedMultiplier,
    pub playback_bpm: Option<u8>,
    pub blur: f32,
    pub blend: BlendMode,
    pub strobe_hz: Option<f32>,
    pub in_point: u16,
    pub out_point: u16,
    pub visualizer_controls: [u8; crate::personality::VISUALIZER_PARAMETERS],
    pub visualizer_tuning: Option<crate::visualizer_channels::VisualizerTuning>,
    pub model: ModelMapping,
}

impl Default for LayerPreset {
    fn default() -> Self {
        let layer = LayerState::default();
        Self::from(&layer)
    }
}

impl From<&LayerState> for LayerPreset {
    fn from(layer: &LayerState) -> Self {
        Self {
            play_mode: layer.play_mode,
            scale_x: layer.scale_x,
            scale_y: layer.scale_y,
            scaling_mode: layer.scaling_mode,
            position_x: layer.position_x,
            position_y: layer.position_y,
            rotation: layer.rotation,
            dimmer: layer.dimmer,
            volume: layer.volume,
            tint: layer.tint,
            grayscale: layer.grayscale,
            mask: layer.mask,
            effect_banks: layer.effect_banks,
            effects: layer.effects.clone(),
            speed_multiplier: layer.speed_multiplier,
            playback_bpm: layer.playback_bpm,
            blur: layer.blur,
            blend: layer.blend,
            strobe_hz: layer.strobe_hz,
            in_point: layer.in_point,
            out_point: layer.out_point,
            visualizer_controls: layer.visualizer_controls,
            visualizer_tuning: layer.visualizer_tuning.clone(),
            model: layer.model,
        }
    }
}

impl LayerPreset {
    pub fn controls(&self, address: MediaAddress) -> LayerControls {
        LayerControls {
            address: Some(address),
            play_mode: Some(self.play_mode),
            scale_x: Some(self.scale_x),
            scale_y: Some(self.scale_y),
            scaling_mode: Some(self.scaling_mode),
            position_x: Some(self.position_x),
            position_y: Some(self.position_y),
            rotation: Some(self.rotation),
            dimmer: Some(self.dimmer),
            volume: Some(self.volume),
            tint: Some(self.tint),
            grayscale: Some(self.grayscale),
            mask_address: Some(self.mask.address),
            mask_scale_x: Some(self.mask.scale_x),
            mask_scale_y: Some(self.mask.scale_y),
            mask_position_x: Some(self.mask.position_x),
            mask_position_y: Some(self.mask.position_y),
            mask_invert: Some(self.mask.invert),
            mask_opacity: Some(self.mask.opacity),
            speed_multiplier: Some(self.speed_multiplier),
            playback_bpm: Some(self.playback_bpm),
            blur: Some(self.blur),
            effects: Some(self.effects.clone()),
            effect_banks: Some(self.effect_banks),
            blend: Some(self.blend),
            strobe_hz: Some(self.strobe_hz),
            in_point: Some(self.in_point),
            out_point: Some(self.out_point),
            visualizer_controls: Some(self.visualizer_controls),
            visualizer_tuning: Some(self.visualizer_tuning.clone()),
            model: Some(self.model),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationOutput {
    pub output_id: OutputId,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub fallback_playlist_id: Option<Uuid>,
    #[serde(default = "default_dmx_silence_seconds")]
    pub dmx_silence_seconds: u64,
}

pub const fn default_dmx_silence_seconds() -> u64 {
    10
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleEvent {
    pub id: Uuid,
    pub name: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    pub output_id: OutputId,
    pub playlist_id: Uuid,
    pub trigger: ScheduleTrigger,
}

const fn enabled() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum ScheduleTrigger {
    Fixed {
        start: String,
        #[serde(default)]
        end: Option<String>,
    },
    Weekly {
        weekdays: Vec<u8>,
        start_time: String,
        #[serde(default)]
        end_time: Option<String>,
        #[serde(default)]
        end_day_offset: u8,
        #[serde(default)]
        first_date: Option<String>,
        #[serde(default)]
        last_date: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_a_playlist_never_invents_its_persisted_identity() {
        assert!(serde_json::from_str::<Playlist>(r#"{"name":"Draft","enabled":false}"#).is_err());
        let playlist = Playlist {
            enabled: false,
            ..Playlist::default()
        };
        let restored: Playlist =
            serde_json::from_str(&serde_json::to_string(&playlist).unwrap()).unwrap();
        assert_eq!(restored, playlist);
    }

    #[test]
    fn legacy_entries_and_partial_transform_values_keep_identity_homes() {
        let id = Uuid::new_v4();
        let asset = AssetId::new();
        let mut value = serde_json::json!({"id":id,"assetId":asset,"plays":2,"fadeMillis":500});
        let legacy: PlaylistEntry = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(legacy.transform, PlaylistTransform::default());
        value["transform"] = serde_json::json!({"rotation":90.0});
        let partial: PlaylistEntry = serde_json::from_value(value).unwrap();
        assert_eq!(partial.transform.scale_x, 1.0);
        assert_eq!(partial.transform.scale_y, 1.0);
        assert_eq!(partial.transform.rotation, 90.0);
        assert_eq!(partial.id, id);
        assert_eq!(partial.asset_id, asset);
    }

    #[test]
    fn step_framing_becomes_regular_layer_controls_and_rejects_invalid_values() {
        let transform = PlaylistTransform {
            scale_x: 0.4,
            scale_y: 0.7,
            position_x: -0.2,
            position_y: 0.3,
            rotation: 90.0,
        };
        transform.validate().unwrap();
        let controls = transform.layer_preset().controls(MediaAddress::new(1, 2));
        assert_eq!((controls.scale_x, controls.scale_y), (Some(0.4), Some(0.7)));
        assert_eq!(
            (controls.position_x, controls.position_y),
            (Some(-0.2), Some(0.3))
        );
        assert_eq!(controls.rotation, Some(90.0));
        PlaylistTransform {
            scale_x: 0.0,
            scale_y: 10.0,
            position_x: -2.0,
            position_y: 2.0,
            rotation: 360.0,
        }
        .validate()
        .unwrap();
        for bad in [
            PlaylistTransform {
                scale_x: -0.1,
                ..Default::default()
            },
            PlaylistTransform {
                scale_y: 10.1,
                ..Default::default()
            },
            PlaylistTransform {
                position_x: -2.1,
                ..Default::default()
            },
            PlaylistTransform {
                position_y: 2.1,
                ..Default::default()
            },
            PlaylistTransform {
                rotation: 361.0,
                ..Default::default()
            },
            PlaylistTransform {
                rotation: f32::NAN,
                ..Default::default()
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn presets_keep_authored_controls_without_source_health_or_reset_counters() {
        let layer = LayerState {
            dimmer: 0.4,
            position_x: 0.2,
            reset_trigger_id: 123,
            source_status: crate::SourceStatus::Loading,
            ..LayerState::default()
        };
        let preset = LayerPreset::from(&layer);
        let stored = serde_json::to_string(&preset).unwrap();
        assert!(!stored.contains("sourceStatus"));
        assert!(!stored.contains("resetTriggerId"));
        let restored: LayerPreset = serde_json::from_str(&stored).unwrap();
        let controls = restored.controls(MediaAddress::BLANK);
        assert_eq!(controls.dimmer, Some(0.4));
        assert_eq!(controls.position_x, Some(0.2));
        assert_eq!(controls.address, Some(MediaAddress::BLANK));
    }
}
