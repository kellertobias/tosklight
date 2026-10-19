//! Layer state.
//!
//! The legacy `paused` and `black` fields are gone: pause is `PlayMode::Pause`, and there is no
//! independent blackout latch.

mod effect_slot;
pub use effect_slot::{BeatSource, EffectSlot};

use serde::{Deserialize, Serialize};

use crate::address::MediaAddress;
use crate::color::Tint;
pub use crate::layer_effects::{
    BeatFormFlashParameters, BeatGridWaveOrigin, BeatGridWaveParameters, DigitalTvParameters,
    DrawnImageParameters,
};
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
    /// Horizontal mask centre in half-layer units (`0` centred, `1` at the right edge).
    #[serde(default)]
    pub position_x: f32,
    /// Vertical mask centre in half-layer units (`0` centred, `1` at the bottom edge).
    #[serde(default)]
    pub position_y: f32,
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
            position_x: 0.0,
            position_y: 0.0,
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
pub const BEAT_SCALE_TURN_EFFECT: &str = "beat-scale-turn";
pub const BEAT_GRID_WAVE_EFFECT: &str = "beat-grid-wave";
pub const BEAT_FORM_FLASH_EFFECT: &str = "beat-form-flash";
pub const DRAWN_IMAGE_EFFECT: &str = "drawn-image";

/// One of the two DMX-controlled effect banks on a current layer.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectBankState {
    /// Zero is Off; `1..=255` addresses a persisted [`crate::EffectLibrary`] preset.
    pub select: u8,
    /// Normalized amount applied to the selected preset.
    pub strength: f32,
    /// Raw parameter bytes in the order the selected effect reports its parameters. Zero keeps
    /// the preset's stored value; `1..=255` spans that parameter's accepted range.
    #[serde(default)]
    pub parameters: [u8; crate::personality::EFFECT_BANK_PARAMETERS],
}

/// What a layer's 3D model channels select.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMapping {
    /// Zero draws the layer flat; `1..=255` maps it onto that numbered model.
    pub model: u8,
    /// Degrees around the vertical axis, applied first.
    pub pan: f32,
    /// Degrees around the horizontal axis, applied after pan. The layer's Rotation is the roll,
    /// applied last around the viewing axis.
    pub tilt: f32,
}

impl ModelMapping {
    pub const fn is_flat(self) -> bool {
        self.model == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatScaleTurnParameters {
    /// Additional uniform scale at the beat peak, where 0.2 means twenty percent larger.
    pub scale_amount: f32,
    pub turn_enabled: bool,
    pub rotation_degrees: f32,
    pub decay_seconds: f32,
}

impl BeatScaleTurnParameters {
    pub const IDS: [&'static str; 4] = [
        "beat-scale-amount",
        "beat-turn-enabled",
        "beat-turn-rotation",
        "beat-scale-decay",
    ];
    pub const LABELS: [&'static str; 4] =
        ["Scale amount", "Turn", "Rotation amount", "Return time"];

    pub fn from_parameters(values: &[f32]) -> Self {
        let defaults = Self::default();
        let bounded = |value: Option<f32>, fallback: f32, low: f32, high: f32| match value {
            Some(value) if value.is_finite() => value.clamp(low, high),
            _ => fallback,
        };
        Self {
            scale_amount: bounded(values.first().copied(), defaults.scale_amount, 0.0, 1.0),
            turn_enabled: values.get(1).copied().unwrap_or_default() >= 0.5,
            rotation_degrees: bounded(
                values.get(2).copied(),
                defaults.rotation_degrees,
                -30.0,
                30.0,
            ),
            decay_seconds: bounded(values.get(3).copied(), defaults.decay_seconds, 0.05, 5.0),
        }
    }

    pub const fn as_array(self) -> [f32; 4] {
        [
            self.scale_amount,
            if self.turn_enabled { 1.0 } else { 0.0 },
            self.rotation_degrees,
            self.decay_seconds,
        ]
    }
}

impl Default for BeatScaleTurnParameters {
    fn default() -> Self {
        Self {
            scale_amount: 0.15,
            turn_enabled: false,
            rotation_degrees: 5.0,
            decay_seconds: 0.35,
        }
    }
}

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
    /// Number of mirrored angular repetitions around the source centre. Zero is Off.
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
            .map(|value| value.round().clamp(0.0, 12.0) as u8)
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
    Shake,
    Tunnel,
}

impl FeedbackMotion {
    pub const ALL: [Self; 8] = [
        Self::Top,
        Self::Bottom,
        Self::Left,
        Self::Right,
        Self::RotateLeft,
        Self::RotateRight,
        Self::Shake,
        Self::Tunnel,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
            Self::RotateLeft => "rotate-left",
            Self::RotateRight => "rotate-right",
            Self::Shake => "shake",
            Self::Tunnel => "tunnel",
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
            Self::Shake => 6.0,
            Self::Tunnel => 7.0,
        }
    }

    pub fn from_parameter(value: f32) -> Self {
        Self::ALL
            .get(if value.is_finite() {
                value.round().clamp(0.0, 7.0) as usize
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
            amount: 0.94,
            motion: 0.12,
            direction: FeedbackMotion::Tunnel,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlurType {
    #[default]
    Gaussian,
    Shape,
    Radial,
    Linear,
    Axial,
}

impl BlurType {
    pub const ALL: [Self; 5] = [
        Self::Gaussian,
        Self::Shape,
        Self::Radial,
        Self::Linear,
        Self::Axial,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Gaussian => "gaussian",
            Self::Shape => "shape",
            Self::Radial => "radial",
            Self::Linear => "linear",
            Self::Axial => "axial",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|blur_type| blur_type.wire_name() == value)
    }

    pub const fn parameter(self) -> f32 {
        match self {
            Self::Gaussian => 0.0,
            Self::Shape => 1.0,
            Self::Radial => 2.0,
            Self::Linear => 3.0,
            Self::Axial => 4.0,
        }
    }

    pub fn from_parameter(value: f32) -> Self {
        Self::ALL
            .get(if value.is_finite() {
                value.round().clamp(0.0, 4.0) as usize
            } else {
                0
            })
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlurParameters {
    pub amount: f32,
    pub blur_type: BlurType,
}

impl Default for BlurParameters {
    fn default() -> Self {
        Self {
            amount: 0.35,
            blur_type: BlurType::Gaussian,
        }
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
            // Amount stays first so existing one-value presets keep their exact meaning.
            blur_type: BlurType::from_parameter(values.get(1).copied().unwrap_or_default()),
        }
    }

    pub const IDS: [&'static str; 2] = ["blur-amount", "blur-type"];
    pub const LABELS: [&'static str; 2] = ["Blur amount", "Blur type"];

    pub const fn as_array(self) -> [f32; 2] {
        [self.amount, self.blur_type.parameter()]
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
    /// White Blend: linear desaturation before the tint; see [`crate::color::MediaColor`].
    pub grayscale: f32,
    pub mask: MaskState,
    /// The two current effect selectors. Legacy personality layouts continue to use `effects`.
    #[serde(default)]
    pub effect_banks: [EffectBankState; 2],
    pub effects: [EffectSlot; 4],
    pub speed_multiplier: SpeedMultiplier,
    /// The per-layer DMX target tempo, used only when the output's tempo source is the channel.
    pub playback_bpm: Option<u8>,
    /// Blur amount from the dedicated playback fader and DMX channel.
    pub blur: f32,
    /// How this layer combines with the layers below it.
    #[serde(default)]
    pub blend: crate::blend::BlendMode,
    /// Flashes per second while the Blend mode / Strobe channel strobes the layer.
    #[serde(default)]
    pub strobe_hz: Option<f32>,
    /// Frames after the clip's start where the playback range begins.
    #[serde(default)]
    pub in_point: u16,
    /// Frames before the clip's end where the playback range stops, so zero is the last frame. A
    /// range whose Out point falls before its In point plays through to the clip's end.
    #[serde(default)]
    pub out_point: u16,
    /// Raw bytes of the layer's dedicated Visualizer Parameter channels, which the selected
    /// visualizer kind names and ranges. They never touch the effect banks.
    #[serde(default)]
    pub visualizer_controls: [u8; crate::personality::VISUALIZER_PARAMETERS],
    /// This layer's own tuning of a visualizer, replacing that visualizer's configured parameter
    /// block while the layer shows it. Ordinary media and any other address ignore it.
    #[serde(default)]
    pub visualizer_tuning: Option<crate::visualizer_channels::VisualizerTuning>,
    #[serde(default)]
    pub model: ModelMapping,
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
            effect_banks: Default::default(),
            effects: Default::default(),
            speed_multiplier: SpeedMultiplier::default(),
            playback_bpm: None,
            blur: 0.0,
            blend: crate::blend::BlendMode::Normal,
            strobe_hz: None,
            in_point: 0,
            out_point: 0,
            visualizer_controls: [0; crate::personality::VISUALIZER_PARAMETERS],
            visualizer_tuning: None,
            model: ModelMapping::default(),
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
mod tests;
