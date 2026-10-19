//! A layer's ordered typed-effect slot, persisted controls and normalization.

use serde::{Deserialize, Serialize};

use super::{
    ANALOG_TV_EFFECT, AnalogTvParameters, BEAT_FORM_FLASH_EFFECT, BEAT_GRID_WAVE_EFFECT,
    BEAT_MOVE_EFFECT, BEAT_SCALE_TURN_EFFECT, BEAT_SCAN_EFFECT, BLUR_EFFECT,
    BeatFormFlashParameters, BeatGridWaveParameters, BeatMoveParameters, BeatScaleTurnParameters,
    BeatScanParameters, BlurParameters, DIGITAL_TV_EFFECT, DRAWN_IMAGE_EFFECT, DigitalTvParameters,
    DrawnImageParameters, FEEDBACK_EFFECT, FeedbackParameters, KALEIDOSCOPE_EFFECT,
    KaleidoscopeParameters, OPACITY_CYCLE_EFFECT, OpacityCycleInterval, RASTERIZE_EFFECT,
    RasterizeParameters,
};

/// One slot in a layer's ordered effect chain.
///
/// The four DMX bytes populate the primary amount of four configured slots. Effect identity and
/// any further parameters come from show configuration or a later expanded personality — the
/// bytes are not four hard-coded shader parameters.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectSlot {
    /// Legacy single text, used as the first entry when authored typography is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Runtime-only glyph mask; regenerated from authored text and never stored.
    #[serde(skip)]
    pub text_atlas: Option<std::sync::Arc<crate::slice_grid::SliceTextAtlas>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typography: Option<crate::slice_grid::SliceGridTypography>,
    /// A stable typed-effect identifier. Unknown future values remain preserved in stored shows
    /// and are reported as unsupported instead of being silently discarded.
    pub effect_type: Option<String>,
    pub enabled: bool,
    /// Stable layer/slot identity used to seed deterministic temporal effects.
    #[serde(default)]
    pub seed: u32,
    /// The normalized primary amount the DMX byte carries.
    pub mix: f32,
    #[serde(default)]
    pub beat_source: BeatSource,
    pub parameters: Vec<f32>,
}

/// The event stream that starts a beat-reactive effect.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BeatSource {
    LiveBeat,
    Kick,
    HiHat,
    Snare,
    #[default]
    DetectedBeat,
}

impl EffectSlot {
    /// An enabled slot of one typed effect at full mix.
    fn typed(effect_type: &str, parameters: Vec<f32>) -> Self {
        Self {
            text: None,
            text_atlas: None,
            typography: None,
            effect_type: Some(effect_type.to_owned()),
            enabled: true,
            seed: 0,
            beat_source: BeatSource::default(),
            mix: 1.0,
            parameters,
        }
    }

    pub fn analog_tv() -> Self {
        Self::typed(
            ANALOG_TV_EFFECT,
            AnalogTvParameters::default().as_array().to_vec(),
        )
    }

    pub fn analog_tv_parameters(&self) -> Option<AnalogTvParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(ANALOG_TV_EFFECT))
            .then(|| AnalogTvParameters::from_normalized(&self.parameters))
    }

    pub fn digital_tv() -> Self {
        Self::typed(
            DIGITAL_TV_EFFECT,
            DigitalTvParameters::default().as_array().to_vec(),
        )
    }

    pub fn digital_tv_parameters(&self) -> Option<DigitalTvParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(DIGITAL_TV_EFFECT))
            .then(|| DigitalTvParameters::from_normalized(&self.parameters))
    }

    pub fn opacity_cycle() -> Self {
        Self::typed(
            OPACITY_CYCLE_EFFECT,
            vec![OpacityCycleInterval::EveryBeat.parameter()],
        )
    }

    pub fn opacity_cycle_interval(&self) -> Option<OpacityCycleInterval> {
        (self.enabled
            && self.mix > 0.0
            && self.effect_type.as_deref() == Some(OPACITY_CYCLE_EFFECT))
        .then(|| OpacityCycleInterval::from_parameter(self.parameters.first().copied()))
    }

    pub fn blur() -> Self {
        Self::typed(BLUR_EFFECT, BlurParameters::default().as_array().to_vec())
    }

    pub fn blur_parameters(&self) -> Option<BlurParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(BLUR_EFFECT))
            .then(|| BlurParameters::from_normalized(&self.parameters))
    }

    pub fn feedback() -> Self {
        Self::typed(
            FEEDBACK_EFFECT,
            FeedbackParameters::default().as_array().to_vec(),
        )
    }

    pub fn feedback_parameters(&self) -> Option<FeedbackParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(FEEDBACK_EFFECT))
            .then(|| FeedbackParameters::from_normalized(&self.parameters))
    }

    pub fn beat_move() -> Self {
        Self::typed(
            BEAT_MOVE_EFFECT,
            BeatMoveParameters::default().as_array().to_vec(),
        )
    }

    pub fn beat_move_parameters(&self) -> Option<BeatMoveParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(BEAT_MOVE_EFFECT))
            .then(|| BeatMoveParameters::from_parameters(&self.parameters))
    }

    pub fn kaleidoscope() -> Self {
        Self::typed(
            KALEIDOSCOPE_EFFECT,
            KaleidoscopeParameters::default().as_array().to_vec(),
        )
    }

    pub fn kaleidoscope_parameters(&self) -> Option<KaleidoscopeParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(KALEIDOSCOPE_EFFECT))
            .then(|| KaleidoscopeParameters::from_parameters(&self.parameters))
    }

    pub fn rasterize() -> Self {
        Self::typed(
            RASTERIZE_EFFECT,
            RasterizeParameters::default().as_array().to_vec(),
        )
    }

    pub fn rasterize_parameters(&self) -> Option<RasterizeParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(RASTERIZE_EFFECT))
            .then(|| RasterizeParameters::from_parameters(&self.parameters))
    }

    pub fn beat_scan() -> Self {
        Self::typed(
            BEAT_SCAN_EFFECT,
            BeatScanParameters::default().as_array().to_vec(),
        )
    }

    pub fn beat_scan_parameters(&self) -> Option<BeatScanParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(BEAT_SCAN_EFFECT))
            .then(|| BeatScanParameters::from_parameters(&self.parameters))
    }

    pub fn beat_scale_turn() -> Self {
        Self::typed(
            BEAT_SCALE_TURN_EFFECT,
            BeatScaleTurnParameters::default().as_array().to_vec(),
        )
    }

    pub fn beat_scale_turn_parameters(&self) -> Option<BeatScaleTurnParameters> {
        (self.enabled
            && self.mix > 0.0
            && self.effect_type.as_deref() == Some(BEAT_SCALE_TURN_EFFECT))
        .then(|| BeatScaleTurnParameters::from_parameters(&self.parameters))
    }

    pub fn beat_grid_wave() -> Self {
        Self::typed(
            BEAT_GRID_WAVE_EFFECT,
            BeatGridWaveParameters::default().as_array().to_vec(),
        )
    }

    pub fn beat_grid_wave_parameters(&self) -> Option<BeatGridWaveParameters> {
        (self.enabled
            && self.mix > 0.0
            && self.effect_type.as_deref() == Some(BEAT_GRID_WAVE_EFFECT))
        .then(|| BeatGridWaveParameters::from_parameters(&self.parameters))
    }

    pub fn beat_form_flash() -> Self {
        Self::typed(
            BEAT_FORM_FLASH_EFFECT,
            BeatFormFlashParameters::default().as_array().to_vec(),
        )
    }

    pub fn beat_form_flash_parameters(&self) -> Option<BeatFormFlashParameters> {
        (self.enabled
            && self.mix > 0.0
            && self.effect_type.as_deref() == Some(BEAT_FORM_FLASH_EFFECT))
        .then(|| BeatFormFlashParameters::from_parameters(&self.parameters))
    }

    pub fn normalize(&mut self) {
        if let Some(text) = &mut self.text {
            *text = text
                .chars()
                .filter(|c| !c.is_control() || *c == '\n')
                .take(crate::slice_grid::MAX_SLICE_TEXT_CHARS)
                .collect();
        }
        self.mix = if self.mix.is_finite() {
            self.mix.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if self.effect_type.as_deref() == Some(crate::BEAT_SLICE_GRID_EFFECT) {
            self.parameters = crate::BeatSliceGridParameters::from_parameters(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(ANALOG_TV_EFFECT) {
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
        } else if self.effect_type.as_deref() == Some(BEAT_SCALE_TURN_EFFECT) {
            self.parameters = BeatScaleTurnParameters::from_parameters(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(BEAT_GRID_WAVE_EFFECT) {
            self.parameters = BeatGridWaveParameters::from_parameters(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(BEAT_FORM_FLASH_EFFECT) {
            self.parameters = BeatFormFlashParameters::from_parameters(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(DRAWN_IMAGE_EFFECT) {
            self.parameters = DrawnImageParameters::from_parameters(&self.parameters)
                .as_array()
                .to_vec();
        } else if self.effect_type.as_deref() == Some(crate::outline_effect::OUTLINE_EFFECT) {
            let outline =
                crate::outline_effect::OutlineParameters::from_parameters(&self.parameters);
            self.parameters = outline.as_array().to_vec();
        }
    }
}
