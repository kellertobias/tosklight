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
pub const BLUR_EFFECT: &str = "blur";
pub const FEEDBACK_EFFECT: &str = "feedback";
pub const BEAT_MOVE_EFFECT: &str = "beat-move";
pub const KALEIDOSCOPE_EFFECT: &str = "kaleidoscope";
pub const RASTERIZE_EFFECT: &str = "rasterize";
pub const BEAT_SCAN_EFFECT: &str = "beat-scan";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BeatScanEdge {
    #[default]
    Sharp,
    Soft,
}

impl BeatScanEdge {
    pub const ALL: [Self; 2] = [Self::Sharp, Self::Soft];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Sharp => "sharp",
            Self::Soft => "soft",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|edge| edge.wire_name() == value)
    }

    pub const fn parameter(self) -> f32 {
        match self {
            Self::Sharp => 0.0,
            Self::Soft => 1.0,
        }
    }

    pub fn from_parameter(value: f32) -> Self {
        if value.is_finite() && value >= 0.5 {
            Self::Soft
        } else {
            Self::Sharp
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatScanParameters {
    /// Line width as a fraction of the source height.
    pub width: f32,
    pub edge: BeatScanEdge,
    /// Soft-edge falloff relative to the configured line width.
    pub falloff: f32,
    /// Seconds from spawning until every line in the beat cluster has left the image.
    pub duration_seconds: f32,
}

impl BeatScanParameters {
    pub const IDS: [&'static str; 4] = [
        "beat-scan-width",
        "beat-scan-edge",
        "beat-scan-falloff",
        "beat-scan-duration",
    ];
    pub const LABELS: [&'static str; 4] = ["Scan width", "Edge", "Edge falloff", "Travel time"];

    pub fn from_parameters(values: &[f32]) -> Self {
        let defaults = Self::default();
        let bounded = |value: Option<f32>, fallback: f32, low: f32, high: f32| match value {
            Some(value) if value.is_finite() => value.clamp(low, high),
            _ => fallback,
        };
        Self {
            width: bounded(values.first().copied(), defaults.width, 0.01, 0.25),
            edge: BeatScanEdge::from_parameter(values.get(1).copied().unwrap_or_default()),
            falloff: bounded(values.get(2).copied(), defaults.falloff, 0.0, 1.0),
            duration_seconds: bounded(values.get(3).copied(), defaults.duration_seconds, 0.2, 3.0),
        }
    }

    pub const fn as_array(self) -> [f32; 4] {
        [
            self.width,
            self.edge.parameter(),
            self.falloff,
            self.duration_seconds,
        ]
    }
}

impl Default for BeatScanParameters {
    fn default() -> Self {
        Self {
            width: 0.06,
            edge: BeatScanEdge::Sharp,
            falloff: 0.45,
            duration_seconds: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RasterizeMode {
    #[default]
    BlackAndWhite,
    Cmyk,
}

impl RasterizeMode {
    pub const ALL: [Self; 2] = [Self::BlackAndWhite, Self::Cmyk];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::BlackAndWhite => "black-and-white",
            Self::Cmyk => "cmyk",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.wire_name() == value)
    }

    pub const fn parameter(self) -> f32 {
        match self {
            Self::BlackAndWhite => 0.0,
            Self::Cmyk => 1.0,
        }
    }

    pub fn from_parameter(value: f32) -> Self {
        if value.is_finite() && value >= 0.5 {
            Self::Cmyk
        } else {
            Self::BlackAndWhite
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterizeParameters {
    pub mode: RasterizeMode,
    /// Apparent print-cell size in source pixels.
    pub dot_size: f32,
}

impl RasterizeParameters {
    pub const IDS: [&'static str; 2] = ["rasterize-mode", "rasterize-dot-size"];
    pub const LABELS: [&'static str; 2] = ["Print mode", "Dot size"];

    pub fn from_parameters(values: &[f32]) -> Self {
        let defaults = Self::default();
        Self {
            mode: RasterizeMode::from_parameter(values.first().copied().unwrap_or_default()),
            dot_size: values
                .get(1)
                .copied()
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(2.0, 32.0))
                .unwrap_or(defaults.dot_size),
        }
    }

    pub const fn as_array(self) -> [f32; 2] {
        [self.mode.parameter(), self.dot_size]
    }
}

impl Default for RasterizeParameters {
    fn default() -> Self {
        Self {
            mode: RasterizeMode::BlackAndWhite,
            dot_size: 8.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KaleidoscopeParameters {
    /// Number of mirrored angular repetitions around the source centre.
    pub repetitions: u8,
    /// Rotation of the mirror axis in degrees.
    pub angle_degrees: f32,
}

impl KaleidoscopeParameters {
    pub const IDS: [&'static str; 2] = ["kaleidoscope-repetitions", "kaleidoscope-angle"];
    pub const LABELS: [&'static str; 2] = ["Mirror repetitions", "Angle"];

    pub fn from_parameters(values: &[f32]) -> Self {
        let defaults = Self::default();
        let repetitions = values
            .first()
            .copied()
            .filter(|value| value.is_finite())
            .map(|value| value.round().clamp(1.0, 16.0) as u8)
            .unwrap_or(defaults.repetitions);
        let angle_degrees = values
            .get(1)
            .copied()
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(-180.0, 180.0))
            .unwrap_or(defaults.angle_degrees);
        Self {
            repetitions,
            angle_degrees,
        }
    }

    pub const fn as_array(self) -> [f32; 2] {
        [self.repetitions as f32, self.angle_degrees]
    }
}

impl Default for KaleidoscopeParameters {
    fn default() -> Self {
        Self {
            repetitions: 6,
            angle_degrees: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BeatMoveDirection {
    #[default]
    Up,
    Down,
    Left,
    Right,
}

impl BeatMoveDirection {
    pub const ALL: [Self; 4] = [Self::Up, Self::Down, Self::Left, Self::Right];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|direction| direction.wire_name() == value)
    }

    pub const fn parameter(self) -> f32 {
        match self {
            Self::Up => 0.0,
            Self::Down => 1.0,
            Self::Left => 2.0,
            Self::Right => 3.0,
        }
    }

    pub fn from_parameter(value: f32) -> Self {
        Self::ALL
            .get(if value.is_finite() {
                value.round().clamp(0.0, 3.0) as usize
            } else {
                0
            })
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatMoveParameters {
    /// Offset in output half-widths at the instant a beat lands.
    pub amount: f32,
    pub direction: BeatMoveDirection,
    /// Seconds until the temporary offset returns fully to rest.
    pub decay_seconds: f32,
}

impl BeatMoveParameters {
    pub const IDS: [&'static str; 3] =
        ["beat-move-amount", "beat-move-direction", "beat-move-decay"];
    pub const LABELS: [&'static str; 3] = ["Movement amount", "Direction", "Return time"];

    pub fn from_parameters(values: &[f32]) -> Self {
        let defaults = Self::default();
        let finite = |value: Option<f32>, fallback: f32, low: f32, high: f32| match value {
            Some(value) if value.is_finite() => value.clamp(low, high),
            _ => fallback,
        };
        Self {
            amount: finite(values.first().copied(), defaults.amount, 0.0, 1.0),
            direction: BeatMoveDirection::from_parameter(
                values.get(1).copied().unwrap_or_default(),
            ),
            decay_seconds: finite(values.get(2).copied(), defaults.decay_seconds, 0.05, 5.0),
        }
    }

    pub const fn as_array(self) -> [f32; 3] {
        [self.amount, self.direction.parameter(), self.decay_seconds]
    }
}

impl Default for BeatMoveParameters {
    fn default() -> Self {
        Self {
            amount: 0.15,
            direction: BeatMoveDirection::Up,
            decay_seconds: 0.35,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FeedbackMotion {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
    RotateLeft,
    RotateRight,
}

impl FeedbackMotion {
    pub const ALL: [Self; 6] = [
        Self::Top,
        Self::Bottom,
        Self::Left,
        Self::Right,
        Self::RotateLeft,
        Self::RotateRight,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
            Self::RotateLeft => "rotate-left",
            Self::RotateRight => "rotate-right",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|motion| motion.wire_name() == value)
    }

    pub const fn parameter(self) -> f32 {
        match self {
            Self::Top => 0.0,
            Self::Bottom => 1.0,
            Self::Left => 2.0,
            Self::Right => 3.0,
            Self::RotateLeft => 4.0,
            Self::RotateRight => 5.0,
        }
    }

    pub fn from_parameter(value: f32) -> Self {
        Self::ALL
            .get(if value.is_finite() {
                value.round().clamp(0.0, 5.0) as usize
            } else {
                0
            })
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedbackParameters {
    /// Weight of the retained frame. High values create longer trails.
    pub amount: f32,
    /// Per-second translation or rotation speed, normalized for the operator control.
    pub motion: f32,
    pub direction: FeedbackMotion,
}

impl Default for FeedbackParameters {
    fn default() -> Self {
        Self {
            amount: 0.82,
            motion: 0.25,
            direction: FeedbackMotion::Top,
        }
    }
}

impl FeedbackParameters {
    pub fn from_normalized(values: &[f32]) -> Self {
        let defaults = Self::default();
        let bounded = |value: Option<f32>, fallback: f32| match value {
            Some(value) if value.is_finite() => value.clamp(0.0, 1.0),
            _ => fallback,
        };
        Self {
            amount: bounded(values.first().copied(), defaults.amount),
            motion: bounded(values.get(1).copied(), defaults.motion),
            direction: FeedbackMotion::from_parameter(
                values
                    .get(2)
                    .copied()
                    .unwrap_or(defaults.direction.parameter()),
            ),
        }
    }

    pub const fn as_array(self) -> [f32; 3] {
        [self.amount, self.motion, self.direction.parameter()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlurParameters {
    pub amount: f32,
}

impl Default for BlurParameters {
    fn default() -> Self {
        Self { amount: 0.35 }
    }
}

impl BlurParameters {
    pub fn from_normalized(values: &[f32]) -> Self {
        let amount = values.first().copied().unwrap_or(Self::default().amount);
        Self {
            amount: if amount.is_finite() {
                amount.clamp(0.0, 1.0)
            } else {
                Self::default().amount
            },
        }
    }

    pub const fn as_array(self) -> [f32; 1] {
        [self.amount]
    }
}

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

    pub fn blur() -> Self {
        Self {
            effect_type: Some(BLUR_EFFECT.to_owned()),
            enabled: true,
            seed: 0,
            mix: 1.0,
            parameters: BlurParameters::default().as_array().to_vec(),
            visualizer_parameters: None,
        }
    }

    pub fn blur_parameters(&self) -> Option<BlurParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(BLUR_EFFECT))
            .then(|| BlurParameters::from_normalized(&self.parameters))
    }

    pub fn feedback() -> Self {
        Self {
            effect_type: Some(FEEDBACK_EFFECT.to_owned()),
            enabled: true,
            seed: 0,
            mix: 1.0,
            parameters: FeedbackParameters::default().as_array().to_vec(),
            visualizer_parameters: None,
        }
    }

    pub fn feedback_parameters(&self) -> Option<FeedbackParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(FEEDBACK_EFFECT))
            .then(|| FeedbackParameters::from_normalized(&self.parameters))
    }

    pub fn beat_move() -> Self {
        Self {
            effect_type: Some(BEAT_MOVE_EFFECT.to_owned()),
            enabled: true,
            seed: 0,
            mix: 1.0,
            parameters: BeatMoveParameters::default().as_array().to_vec(),
            visualizer_parameters: None,
        }
    }

    pub fn beat_move_parameters(&self) -> Option<BeatMoveParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(BEAT_MOVE_EFFECT))
            .then(|| BeatMoveParameters::from_parameters(&self.parameters))
    }

    pub fn kaleidoscope() -> Self {
        Self {
            effect_type: Some(KALEIDOSCOPE_EFFECT.to_owned()),
            enabled: true,
            seed: 0,
            mix: 1.0,
            parameters: KaleidoscopeParameters::default().as_array().to_vec(),
            visualizer_parameters: None,
        }
    }

    pub fn kaleidoscope_parameters(&self) -> Option<KaleidoscopeParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(KALEIDOSCOPE_EFFECT))
            .then(|| KaleidoscopeParameters::from_parameters(&self.parameters))
    }

    pub fn rasterize() -> Self {
        Self {
            effect_type: Some(RASTERIZE_EFFECT.to_owned()),
            enabled: true,
            seed: 0,
            mix: 1.0,
            parameters: RasterizeParameters::default().as_array().to_vec(),
            visualizer_parameters: None,
        }
    }

    pub fn rasterize_parameters(&self) -> Option<RasterizeParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(RASTERIZE_EFFECT))
            .then(|| RasterizeParameters::from_parameters(&self.parameters))
    }

    pub fn beat_scan() -> Self {
        Self {
            effect_type: Some(BEAT_SCAN_EFFECT.to_owned()),
            enabled: true,
            seed: 0,
            mix: 1.0,
            parameters: BeatScanParameters::default().as_array().to_vec(),
            visualizer_parameters: None,
        }
    }

    pub fn beat_scan_parameters(&self) -> Option<BeatScanParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(BEAT_SCAN_EFFECT))
            .then(|| BeatScanParameters::from_parameters(&self.parameters))
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
        } else if self.effect_type.as_deref() == Some(BLUR_EFFECT) {
            self.parameters = BlurParameters::from_normalized(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(FEEDBACK_EFFECT) {
            self.parameters = FeedbackParameters::from_normalized(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(BEAT_MOVE_EFFECT) {
            self.parameters = BeatMoveParameters::from_parameters(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(KALEIDOSCOPE_EFFECT) {
            self.parameters = KaleidoscopeParameters::from_parameters(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(RASTERIZE_EFFECT) {
            self.parameters = RasterizeParameters::from_parameters(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(BEAT_SCAN_EFFECT) {
            self.parameters = BeatScanParameters::from_parameters(&self.parameters)
                .as_array()
                .to_vec();
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

    #[test]
    fn feedback_has_stable_defaults_directions_and_safe_normalization() {
        let mut effect = EffectSlot::feedback();
        assert_eq!(effect.effect_type.as_deref(), Some(FEEDBACK_EFFECT));
        assert_eq!(
            effect.feedback_parameters(),
            Some(FeedbackParameters::default())
        );
        assert_eq!(
            FeedbackMotion::parse("rotate-right"),
            Some(FeedbackMotion::RotateRight)
        );
        assert_eq!(FeedbackMotion::parse("diagonal"), None);

        effect.parameters = vec![2.0, -1.0, 5.0];
        effect.normalize();
        assert_eq!(
            effect.feedback_parameters(),
            Some(FeedbackParameters {
                amount: 1.0,
                motion: 0.0,
                direction: FeedbackMotion::RotateRight,
            })
        );
        effect.enabled = false;
        assert_eq!(
            effect.feedback_parameters(),
            None,
            "disabled clears the temporal path"
        );
    }

    #[test]
    fn beat_move_has_bounded_persisted_controls_and_safe_bypass() {
        let mut effect = EffectSlot::beat_move();
        let defaults = effect.beat_move_parameters().expect("enabled");
        assert_eq!(defaults, BeatMoveParameters::default());
        for direction in BeatMoveDirection::ALL {
            assert_eq!(
                BeatMoveDirection::parse(direction.wire_name()),
                Some(direction)
            );
            assert_eq!(
                BeatMoveDirection::from_parameter(direction.parameter()),
                direction
            );
        }
        effect.parameters = vec![f32::INFINITY, 99.0, -2.0];
        effect.normalize();
        assert_eq!(
            effect.parameters,
            vec![0.15, BeatMoveDirection::Right.parameter(), 0.05]
        );
        effect.enabled = false;
        assert_eq!(effect.beat_move_parameters(), None);
    }

    #[test]
    fn kaleidoscope_has_integer_repetitions_bounded_angle_and_safe_bypass() {
        let mut effect = EffectSlot::kaleidoscope();
        assert_eq!(
            effect.kaleidoscope_parameters(),
            Some(KaleidoscopeParameters::default())
        );

        effect.parameters = vec![1.6, 250.0];
        effect.normalize();
        assert_eq!(effect.parameters, vec![2.0, 180.0]);

        effect.parameters = vec![-4.0, f32::NAN];
        effect.normalize();
        assert_eq!(effect.parameters, vec![1.0, 0.0]);

        effect.enabled = false;
        assert_eq!(effect.kaleidoscope_parameters(), None);
    }

    #[test]
    fn rasterize_has_stable_modes_bounded_dot_size_and_safe_bypass() {
        let mut effect = EffectSlot::rasterize();
        assert_eq!(
            effect.rasterize_parameters(),
            Some(RasterizeParameters::default())
        );
        assert_eq!(
            RasterizeMode::parse("black-and-white"),
            Some(RasterizeMode::BlackAndWhite)
        );
        assert_eq!(RasterizeMode::parse("cmyk"), Some(RasterizeMode::Cmyk));
        assert_eq!(RasterizeMode::parse("rgb"), None);

        effect.parameters = vec![1.0, 100.0];
        effect.normalize();
        assert_eq!(effect.parameters, vec![1.0, 32.0]);
        effect.parameters = vec![0.0, f32::NAN];
        effect.normalize();
        assert_eq!(effect.parameters, vec![0.0, 8.0]);

        effect.enabled = false;
        assert_eq!(effect.rasterize_parameters(), None);
    }

    #[test]
    fn beat_scan_has_typed_edges_bounded_timing_and_no_spawn_count() {
        let mut effect = EffectSlot::beat_scan();
        assert_eq!(
            effect.beat_scan_parameters(),
            Some(BeatScanParameters::default())
        );
        assert_eq!(BeatScanEdge::parse("sharp"), Some(BeatScanEdge::Sharp));
        assert_eq!(BeatScanEdge::parse("soft"), Some(BeatScanEdge::Soft));
        assert_eq!(BeatScanEdge::parse("blurred"), None);
        assert!(
            BeatScanParameters::IDS
                .iter()
                .all(|id| !id.contains("count"))
        );

        effect.parameters = vec![0.5, 1.0, 2.0, 9.0];
        effect.normalize();
        assert_eq!(effect.parameters, vec![0.25, 1.0, 1.0, 3.0]);
        effect.enabled = false;
        assert_eq!(effect.beat_scan_parameters(), None);
    }
}
