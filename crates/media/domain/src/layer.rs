//! Layer state.
//!
//! The legacy `paused` and `black` fields are gone: pause is `PlayMode::Pause`, and there is no
//! independent blackout latch.

use serde::{Deserialize, Serialize};

use crate::address::MediaAddress;
use crate::color::Tint;
use crate::playback::PlayMode;
use crate::speed::SpeedMultiplier;

/// How a source is fitted to the output before the operator's own scale is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScalingMode {
    /// Uniformly scale until the complete source fits inside the output.
    #[default]
    Fit,
    /// Uniformly scale until the source covers the output, cropping the overflow.
    Fill,
    /// One source pixel maps to one output pixel.
    Original,
    /// Independently scale width and height to the output.
    Stretch,
}

impl ScalingMode {
    /// Four 64-value ranges.
    pub const fn from_dmx(value: u8) -> Self {
        match value {
            0..=63 => Self::Fit,
            64..=127 => Self::Fill,
            128..=191 => Self::Original,
            192..=255 => Self::Stretch,
        }
    }

    pub const fn dmx_range(self) -> (u8, u8) {
        match self {
            Self::Fit => (0, 63),
            Self::Fill => (64, 127),
            Self::Original => (128, 191),
            Self::Stretch => (192, 255),
        }
    }

    pub const ALL: [Self; 4] = [Self::Fit, Self::Fill, Self::Original, Self::Stretch];
}

/// Whether a mask reads its strength from the mask texture's alpha or its luminance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaskSource {
    #[default]
    Luminance,
    Alpha,
}

/// A layer's mask. The mask has its own transform around the layer center rather than inheriting
/// the source layer's, so an operator can hold a mask still while the media moves behind it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaskState {
    pub address: MediaAddress,
    pub scale_x: f32,
    pub scale_y: f32,
    pub invert: bool,
    /// Blends between the unmasked layer and the masked result.
    pub opacity: f32,
    pub source: MaskSource,
}

impl Default for MaskState {
    fn default() -> Self {
        Self {
            address: MediaAddress::BLANK,
            scale_x: 1.0,
            scale_y: 1.0,
            invert: false,
            opacity: 0.0,
            source: MaskSource::default(),
        }
    }
}

impl MaskState {
    /// A missing mask means no mask, never a black layer.
    pub fn is_active(&self) -> bool {
        !self.address.is_blank() && self.opacity > 0.0
    }
}

/// The first typed effect shipped by the Media Server.
pub const ANALOG_TV_EFFECT: &str = "analog-tv";
pub const DIGITAL_TV_EFFECT: &str = "digital-tv";
pub const OPACITY_CYCLE_EFFECT: &str = "opacity-cycle";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpacityCycleInterval {
    EveryBeat,
    EveryHalfBeat,
    EverySecond,
}

impl OpacityCycleInterval {
    pub fn from_parameter(value: Option<f32>) -> Self {
        match value
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
            .round() as i32
        {
            1 => Self::EveryHalfBeat,
            2 => Self::EverySecond,
            _ => Self::EveryBeat,
        }
    }

    pub const fn parameter(self) -> f32 {
        match self {
            Self::EveryBeat => 0.0,
            Self::EveryHalfBeat => 1.0,
            Self::EverySecond => 2.0,
        }
    }
}

/// Typed Analog TV parameters, normalized to `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalogTvParameters {
    pub curvature: f32,
    pub distortion: f32,
    pub image_grain: f32,
    pub glitching: f32,
}

impl Default for AnalogTvParameters {
    fn default() -> Self {
        Self {
            curvature: 0.30,
            distortion: 0.18,
            image_grain: 0.20,
            glitching: 0.08,
        }
    }
}

impl AnalogTvParameters {
    pub const IDS: [&'static str; 4] = ["tv-curvature", "distortion", "image-grain", "glitching"];
    pub const LABELS: [&'static str; 4] =
        ["TV curvature", "Distortion", "Image grain", "Glitching"];

    pub fn from_normalized(values: &[f32]) -> Self {
        let defaults = Self::default().as_array();
        let mut resolved = defaults;
        for (index, value) in values.iter().copied().take(4).enumerate() {
            resolved[index] = if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                defaults[index]
            };
        }
        Self {
            curvature: resolved[0],
            distortion: resolved[1],
            image_grain: resolved[2],
            glitching: resolved[3],
        }
    }

    pub const fn as_array(self) -> [f32; 4] {
        [
            self.curvature,
            self.distortion,
            self.image_grain,
            self.glitching,
        ]
    }
}

/// Typed Digital TV/DVB-T damage parameters, normalized to `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DigitalTvParameters {
    pub compression_damage: f32,
    pub block_size: f32,
    pub tile_displacement: f32,
    pub chroma_damage: f32,
    pub glitching: f32,
}

impl Default for DigitalTvParameters {
    fn default() -> Self {
        Self {
            compression_damage: 0.35,
            block_size: 0.35,
            tile_displacement: 0.25,
            chroma_damage: 0.20,
            glitching: 0.15,
        }
    }
}

impl DigitalTvParameters {
    pub const IDS: [&'static str; 5] = [
        "compression-damage",
        "block-size",
        "tile-displacement",
        "chroma-damage",
        "glitching",
    ];
    pub const LABELS: [&'static str; 5] = [
        "Compression damage",
        "Block size",
        "Tile displacement",
        "Chroma damage",
        "Glitching",
    ];

    pub fn from_normalized(values: &[f32]) -> Self {
        let defaults = Self::default().as_array();
        let mut resolved = defaults;
        for (index, value) in values.iter().copied().take(5).enumerate() {
            resolved[index] = if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                defaults[index]
            };
        }
        Self {
            compression_damage: resolved[0],
            block_size: resolved[1],
            tile_displacement: resolved[2],
            chroma_damage: resolved[3],
            glitching: resolved[4],
        }
    }

    pub const fn as_array(self) -> [f32; 5] {
        [
            self.compression_damage,
            self.block_size,
            self.tile_displacement,
            self.chroma_damage,
            self.glitching,
        ]
    }
}

/// One slot in a layer's ordered effect chain.
///
/// The four DMX bytes populate the primary amount of four configured slots. Effect identity and
/// any further parameters come from show configuration or a later expanded personality — the
/// bytes are not four hard-coded shader parameters.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectSlot {
    /// A stable typed-effect identifier. Unknown future values remain preserved in stored shows
    /// and are reported as unsupported instead of being silently discarded.
    pub effect_type: Option<String>,
    pub enabled: bool,
    /// Stable layer/slot identity used to seed deterministic temporal effects.
    #[serde(default)]
    pub seed: u32,
    /// The normalized primary amount the DMX byte carries.
    pub mix: f32,
    pub parameters: Vec<f32>,
    /// Per-layer overrides used when slot one controls a generated visualizer source.
    #[serde(default)]
    pub visualizer_parameters: Option<crate::visualizer::VisualizerParameters>,
}

impl EffectSlot {
    pub fn analog_tv() -> Self {
        Self {
            effect_type: Some(ANALOG_TV_EFFECT.to_owned()),
            enabled: true,
            seed: 0,
            mix: 1.0,
            parameters: AnalogTvParameters::default().as_array().to_vec(),
            visualizer_parameters: None,
        }
    }

    pub fn analog_tv_parameters(&self) -> Option<AnalogTvParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(ANALOG_TV_EFFECT))
            .then(|| AnalogTvParameters::from_normalized(&self.parameters))
    }

    pub fn digital_tv() -> Self {
        Self {
            effect_type: Some(DIGITAL_TV_EFFECT.to_owned()),
            enabled: true,
            seed: 0,
            mix: 1.0,
            parameters: DigitalTvParameters::default().as_array().to_vec(),
            visualizer_parameters: None,
        }
    }

    pub fn digital_tv_parameters(&self) -> Option<DigitalTvParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(DIGITAL_TV_EFFECT))
            .then(|| DigitalTvParameters::from_normalized(&self.parameters))
    }

    pub fn opacity_cycle() -> Self {
        Self {
            effect_type: Some(OPACITY_CYCLE_EFFECT.to_owned()),
            enabled: true,
            seed: 0,
            mix: 1.0,
            parameters: vec![OpacityCycleInterval::EveryBeat.parameter()],
            visualizer_parameters: None,
        }
    }

    pub fn opacity_cycle_interval(&self) -> Option<OpacityCycleInterval> {
        (self.enabled
            && self.mix > 0.0
            && self.effect_type.as_deref() == Some(OPACITY_CYCLE_EFFECT))
        .then(|| OpacityCycleInterval::from_parameter(self.parameters.first().copied()))
    }

    pub fn normalize(&mut self) {
        self.mix = if self.mix.is_finite() {
            self.mix.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if self.effect_type.as_deref() == Some(ANALOG_TV_EFFECT) {
            self.parameters = AnalogTvParameters::from_normalized(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(DIGITAL_TV_EFFECT) {
            self.parameters = DigitalTvParameters::from_normalized(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(OPACITY_CYCLE_EFFECT) {
            self.parameters = vec![
                OpacityCycleInterval::from_parameter(self.parameters.first().copied()).parameter(),
            ];
        }
        self.visualizer_parameters = self
            .visualizer_parameters
            .map(crate::visualizer::VisualizerParameters::clamped);
    }
}

/// Why a layer's selected source is not drawing.
///
/// Categories only. Absolute paths, decoder internals, and arbitrary exception text never reach
/// this type, because it is published to CITP, the API, and the UI alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceFailure {
    MissingFile,
    UnsupportedCodec,
    DecodeFailed,
    GpuUploadFailed,
}

impl SourceFailure {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingFile => "MissingFile",
            Self::UnsupportedCodec => "UnsupportedCodec",
            Self::DecodeFailed => "DecodeFailed",
            Self::GpuUploadFailed => "GpuUploadFailed",
        }
    }

    /// Whether asking again could succeed. A missing file can appear; an unsupported codec
    /// cannot become supported without an import.
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::MissingFile | Self::DecodeFailed | Self::GpuUploadFailed
        )
    }
}

/// The runtime health of a layer's selected source.
///
/// Source selection and source health are separate. A failure never clears the selected address
/// or makes the layer look unselected: the renderer draws the layer transparent while the API,
/// the React UI, the logs, and CITP all report the same projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum SourceStatus {
    /// The address is blank.
    #[default]
    Unselected,
    Loading,
    Ready,
    Failed {
        failure: SourceFailure,
    },
    /// A single Once pass has finished. Not a failure and not a pause; a real terminal state.
    Completed,
}

impl SourceStatus {
    pub const fn is_failed(self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}

/// Everything one layer holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerState {
    pub address: MediaAddress,
    pub play_mode: PlayMode,
    pub scale_x: f32,
    pub scale_y: f32,
    pub scaling_mode: ScalingMode,
    /// Layer center position relative to the output, in half-widths.
    pub position_x: f32,
    pub position_y: f32,
    /// Degrees around the layer center.
    pub rotation: f32,
    pub dimmer: f32,
    pub volume: f32,
    pub tint: Tint,
    /// Blends from source color to luminance.
    pub grayscale: f32,
    pub mask: MaskState,
    pub effects: [EffectSlot; 4],
    pub speed_multiplier: SpeedMultiplier,
    /// The per-layer DMX target tempo, used only when the output's tempo source is the channel.
    pub playback_bpm: Option<u8>,
    pub source_status: SourceStatus,
    /// Incremented to restart media without changing the selected address.
    pub reset_trigger_id: u32,
}

impl Default for LayerState {
    fn default() -> Self {
        Self {
            address: MediaAddress::BLANK,
            play_mode: PlayMode::default(),
            scale_x: 1.0,
            scale_y: 1.0,
            scaling_mode: ScalingMode::default(),
            position_x: 0.0,
            position_y: 0.0,
            rotation: 0.0,
            dimmer: 1.0,
            volume: 1.0,
            tint: Tint::WHITE,
            grayscale: 0.0,
            mask: MaskState::default(),
            effects: Default::default(),
            speed_multiplier: SpeedMultiplier::default(),
            playback_bpm: None,
            source_status: SourceStatus::default(),
            reset_trigger_id: 0,
        }
    }
}

impl LayerState {
    /// Whether the layer contributes pixels at all.
    ///
    /// A layer does not draw when its dimmer is at zero, when no source resolves for its address,
    /// or when the selected source cannot load.
    pub fn draws(&self) -> bool {
        self.dimmer > 0.0
            && !self.address.is_blank()
            && !self.source_status.is_failed()
            && self.source_status != SourceStatus::Unselected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaling_modes_tile_the_channel_in_four_sixty_four_value_ranges() {
        let mut next = 0u16;
        for mode in ScalingMode::ALL {
            let (low, high) = mode.dmx_range();
            assert_eq!(u16::from(low), next);
            assert_eq!(high - low + 1, 64);
            next = u16::from(high) + 1;
        }
        assert_eq!(next, 256);
        for value in 0..=255u8 {
            let mode = ScalingMode::from_dmx(value);
            let (low, high) = mode.dmx_range();
            assert!((low..=high).contains(&value), "{value}");
        }
    }

    #[test]
    fn a_fresh_layer_is_neutral_and_selects_nothing() {
        let layer = LayerState::default();
        assert_eq!(layer.address, MediaAddress::BLANK);
        assert_eq!(layer.scale_x, 1.0);
        assert_eq!(layer.dimmer, 1.0);
        assert_eq!(layer.tint, Tint::WHITE);
        assert_eq!(layer.source_status, SourceStatus::Unselected);
        assert_eq!(layer.playback_bpm, None);
        assert!(!layer.draws());
    }

    #[test]
    fn a_layer_stops_drawing_for_each_documented_reason() {
        let ready = LayerState {
            address: MediaAddress::new(1, 1),
            source_status: SourceStatus::Ready,
            ..Default::default()
        };
        assert!(ready.draws());

        assert!(
            !LayerState {
                dimmer: 0.0,
                ..ready.clone()
            }
            .draws(),
            "dimmer at zero"
        );
        assert!(
            !LayerState {
                address: MediaAddress::BLANK,
                ..ready.clone()
            }
            .draws(),
            "no source selected"
        );
        assert!(
            !LayerState {
                source_status: SourceStatus::Failed {
                    failure: SourceFailure::MissingFile
                },
                ..ready.clone()
            }
            .draws(),
            "the selected source cannot load"
        );
    }

    #[test]
    fn a_failure_keeps_the_selected_address() {
        let layer = LayerState {
            address: MediaAddress::new(3, 7),
            source_status: SourceStatus::Failed {
                failure: SourceFailure::DecodeFailed,
            },
            ..Default::default()
        };
        assert_eq!(
            layer.address,
            MediaAddress::new(3, 7),
            "the desk's selection must survive"
        );
        assert!(layer.source_status.is_failed());
    }

    #[test]
    fn failure_codes_are_categories_and_say_whether_a_retry_can_help() {
        assert_eq!(SourceFailure::MissingFile.code(), "MissingFile");
        assert!(SourceFailure::MissingFile.is_retryable());
        assert!(!SourceFailure::UnsupportedCodec.is_retryable());
    }

    #[test]
    fn a_missing_mask_is_no_mask() {
        let mut mask = MaskState {
            opacity: 1.0,
            ..Default::default()
        };
        assert!(
            !mask.is_active(),
            "a blank mask address must not black the layer out"
        );
        mask.address = MediaAddress::new(2, 5);
        assert!(mask.is_active());
        mask.opacity = 0.0;
        assert!(!mask.is_active());
    }

    #[test]
    fn effect_slots_start_empty() {
        let layer = LayerState::default();
        assert_eq!(layer.effects.len(), 4);
        for slot in &layer.effects {
            assert_eq!(slot.effect_type, None);
            assert!(!slot.enabled);
            assert_eq!(slot.mix, 0.0);
        }
    }

    #[test]
    fn analog_tv_has_typed_restrained_defaults_and_normalized_endpoints() {
        let mut effect = EffectSlot::analog_tv();
        assert_eq!(effect.effect_type.as_deref(), Some(ANALOG_TV_EFFECT));
        assert!(effect.enabled);
        assert_eq!(
            effect.analog_tv_parameters(),
            Some(AnalogTvParameters::default())
        );
        assert_eq!(AnalogTvParameters::IDS.len(), effect.parameters.len());

        effect.mix = 2.0;
        effect.parameters = vec![-1.0, 0.4, 1.5, f32::NAN];
        effect.normalize();
        assert_eq!(effect.mix, 1.0);
        assert_eq!(effect.parameters, vec![0.0, 0.4, 1.0, 0.08]);

        effect.mix = 0.0;
        assert_eq!(effect.analog_tv_parameters(), None, "zero mix bypasses");
        effect.mix = 1.0;
        effect.enabled = false;
        assert_eq!(effect.analog_tv_parameters(), None, "disabled bypasses");
    }

    #[test]
    fn digital_tv_has_five_typed_restrained_defaults_and_normalized_endpoints() {
        let mut effect = EffectSlot::digital_tv();
        assert_eq!(effect.effect_type.as_deref(), Some(DIGITAL_TV_EFFECT));
        assert_eq!(
            effect.digital_tv_parameters(),
            Some(DigitalTvParameters::default())
        );
        assert_eq!(DigitalTvParameters::IDS.len(), effect.parameters.len());

        effect.parameters = vec![-1.0, 0.25, 0.5, 1.5, f32::NAN];
        effect.normalize();
        assert_eq!(effect.parameters, vec![0.0, 0.25, 0.5, 1.0, 0.15]);
    }
}
