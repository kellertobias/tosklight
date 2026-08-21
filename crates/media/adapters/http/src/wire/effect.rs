//! How an effect slot reports itself, including what each of its parameters accepts.
//!
//! The bounds travel with every parameter so a desk renders the range this server actually
//! validates instead of inferring one from the parameter's name.

use super::visualizer::VisualizerParametersView;
use media_domain::{
    ANALOG_TV_EFFECT, AnalogTvParameters, BEAT_FORM_FLASH_EFFECT, BEAT_GRID_WAVE_EFFECT,
    BEAT_MOVE_EFFECT, BEAT_SCALE_TURN_EFFECT, BEAT_SCAN_EFFECT, BLUR_EFFECT,
    BeatFormFlashParameters, BeatGridWaveParameters, BeatMoveParameters, BeatScaleTurnParameters,
    BeatScanParameters, BlurParameters, DIGITAL_TV_EFFECT, DRAWN_IMAGE_EFFECT, DigitalTvParameters,
    DrawnImageParameters, EffectSlot, FEEDBACK_EFFECT, FeedbackParameters, KALEIDOSCOPE_EFFECT,
    KaleidoscopeParameters, OPACITY_CYCLE_EFFECT, OpacityCycleInterval, RASTERIZE_EFFECT,
    RasterizeParameters,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct EffectParameterView {
    pub id: String,
    pub label: String,
    pub value: f32,
    pub default_value: f32,
    /// What this server accepts. A desk that renders a control renders this range, rather than
    /// guessing one and letting the operator drag into values that are always refused.
    pub minimum: f32,
    pub maximum: f32,
    /// A step of one or more marks a whole-number parameter.
    pub step: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct EffectSlotView {
    pub index: usize,
    pub effect_type: Option<String>,
    pub label: String,
    pub enabled: bool,
    pub mix: f32,
    pub supported: bool,
    pub capability_detail: Option<String>,
    pub parameters: Vec<EffectParameterView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visualizer_parameters: Option<VisualizerParametersView>,
}

/// Every effect this build renders, with the name an operator reads. Anything outside this list
/// is reported as unsupported rather than silently renamed or dropped.
const RENDERED_EFFECTS: [(&str, &str); 13] = [
    (ANALOG_TV_EFFECT, "Analog TV"),
    (DIGITAL_TV_EFFECT, "Digital TV"),
    (BLUR_EFFECT, "Blur"),
    (FEEDBACK_EFFECT, "Feedback"),
    (OPACITY_CYCLE_EFFECT, "Layer opacity cycle"),
    (BEAT_MOVE_EFFECT, "Beat Move"),
    (KALEIDOSCOPE_EFFECT, "Kaleidoscope"),
    (RASTERIZE_EFFECT, "Rasterized Print"),
    (BEAT_SCAN_EFFECT, "Beat Scan"),
    (BEAT_SCALE_TURN_EFFECT, "Beat Scale and Turn"),
    (BEAT_GRID_WAVE_EFFECT, "Beat Grid Wave"),
    (BEAT_FORM_FLASH_EFFECT, "Beat Form Flash"),
    (DRAWN_IMAGE_EFFECT, "Drawn Image"),
];

fn rendered_label(effect_type: Option<&str>) -> Option<&'static str> {
    let effect_type = effect_type?;
    RENDERED_EFFECTS
        .iter()
        .find(|(candidate, _)| *candidate == effect_type)
        .map(|(_, label)| *label)
}

impl EffectSlotView {
    pub(super) fn of(index: usize, effect: &EffectSlot) -> Self {
        let effect_type = effect.effect_type.as_deref();
        let rendered = rendered_label(effect_type);
        Self {
            index,
            effect_type: effect.effect_type.clone(),
            label: rendered
                .unwrap_or_else(|| effect_type.unwrap_or("None"))
                .to_owned(),
            enabled: effect.enabled,
            mix: effect.mix,
            supported: effect_type.is_none() || rendered.is_some(),
            capability_detail: (effect_type.is_some() && rendered.is_none())
                .then(|| "This Media Server build cannot render the selected effect.".to_owned()),
            parameters: effect_parameters(effect),
            visualizer_parameters: effect
                .visualizer_parameters
                .as_ref()
                .map(VisualizerParametersView::of),
        }
    }
}

/// What this effect exposes, in its own order, with the bounds each parameter accepts.
fn effect_parameters(effect: &EffectSlot) -> Vec<EffectParameterView> {
    let stored = effect.parameters.as_slice();
    match effect.effect_type.as_deref() {
        Some(ANALOG_TV_EFFECT) => parameter_views(
            &AnalogTvParameters::IDS,
            &AnalogTvParameters::LABELS,
            &AnalogTvParameters::from_normalized(stored).as_array(),
            &AnalogTvParameters::default().as_array(),
        ),
        Some(DIGITAL_TV_EFFECT) => parameter_views(
            &DigitalTvParameters::IDS,
            &DigitalTvParameters::LABELS,
            &DigitalTvParameters::from_normalized(stored).as_array(),
            &DigitalTvParameters::default().as_array(),
        ),
        Some(BLUR_EFFECT) => parameter_views(
            &["blur-amount"],
            &["Blur amount"],
            &BlurParameters::from_normalized(stored).as_array(),
            &BlurParameters::default().as_array(),
        ),
        Some(FEEDBACK_EFFECT) => parameter_views(
            &["feedback-amount", "feedback-motion", "feedback-direction"],
            &["Feedback amount", "Motion speed", "Motion direction"],
            &FeedbackParameters::from_normalized(stored).as_array(),
            &FeedbackParameters::default().as_array(),
        ),
        Some(OPACITY_CYCLE_EFFECT) => parameter_views(
            &["cycle-interval"],
            &["Interval"],
            &[effect
                .opacity_cycle_interval()
                .unwrap_or(OpacityCycleInterval::EveryBeat)
                .parameter()],
            &[OpacityCycleInterval::EveryBeat.parameter()],
        ),
        Some(BEAT_MOVE_EFFECT) => parameter_views(
            &BeatMoveParameters::IDS,
            &BeatMoveParameters::LABELS,
            &BeatMoveParameters::from_parameters(stored).as_array(),
            &BeatMoveParameters::default().as_array(),
        ),
        Some(KALEIDOSCOPE_EFFECT) => parameter_views(
            &KaleidoscopeParameters::IDS,
            &KaleidoscopeParameters::LABELS,
            &KaleidoscopeParameters::from_parameters(stored).as_array(),
            &KaleidoscopeParameters::default().as_array(),
        ),
        Some(RASTERIZE_EFFECT) => parameter_views(
            &RasterizeParameters::IDS,
            &RasterizeParameters::LABELS,
            &RasterizeParameters::from_parameters(stored).as_array(),
            &RasterizeParameters::default().as_array(),
        ),
        Some(BEAT_SCAN_EFFECT) => parameter_views(
            &BeatScanParameters::IDS,
            &BeatScanParameters::LABELS,
            &BeatScanParameters::from_parameters(stored).as_array(),
            &BeatScanParameters::default().as_array(),
        ),
        Some(BEAT_SCALE_TURN_EFFECT) => parameter_views(
            &BeatScaleTurnParameters::IDS,
            &BeatScaleTurnParameters::LABELS,
            &BeatScaleTurnParameters::from_parameters(stored).as_array(),
            &BeatScaleTurnParameters::default().as_array(),
        ),
        Some(BEAT_GRID_WAVE_EFFECT) => parameter_views(
            &BeatGridWaveParameters::IDS,
            &BeatGridWaveParameters::LABELS,
            &BeatGridWaveParameters::from_parameters(stored).as_array(),
            &BeatGridWaveParameters::default().as_array(),
        ),
        Some(BEAT_FORM_FLASH_EFFECT) => parameter_views(
            &BeatFormFlashParameters::IDS,
            &BeatFormFlashParameters::LABELS,
            &BeatFormFlashParameters::from_parameters(stored).as_array(),
            &BeatFormFlashParameters::default().as_array(),
        ),
        Some(DRAWN_IMAGE_EFFECT) => parameter_views(
            &DrawnImageParameters::IDS,
            &DrawnImageParameters::LABELS,
            &DrawnImageParameters::from_parameters(stored).as_array(),
            &DrawnImageParameters::default().as_array(),
        ),
        _ => Vec::new(),
    }
}

fn parameter_views(
    ids: &[&str],
    labels: &[&str],
    values: &[f32],
    defaults: &[f32],
) -> Vec<EffectParameterView> {
    ids.iter()
        .zip(labels)
        .zip(values.iter().zip(defaults))
        .map(|((id, label), (value, default_value))| {
            let bounds = media_domain::effect_parameter_bounds(id);
            EffectParameterView {
                id: (*id).to_owned(),
                label: (*label).to_owned(),
                value: *value,
                default_value: *default_value,
                minimum: bounds.minimum,
                maximum: bounds.maximum,
                step: bounds.step,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_persisted_effect_reports_an_actionable_capability_error() {
        let effect = EffectSlot {
            effect_type: Some("future-effect".to_owned()),
            enabled: true,
            mix: 1.0,
            parameters: vec![0.4],
            seed: 7,
            visualizer_parameters: None,
        };
        let view = EffectSlotView::of(2, &effect);

        assert_eq!(view.label, "future-effect");
        assert!(!view.supported);
        assert_eq!(
            view.capability_detail.as_deref(),
            Some("This Media Server build cannot render the selected effect.")
        );
    }

    /// A desk renders a control from what this server advertises, so every parameter carries the
    /// range and step the API validates against.
    #[test]
    fn every_parameter_reports_the_range_the_api_accepts() {
        let effect = EffectSlot::kaleidoscope();
        let view = EffectSlotView::of(0, &effect);

        let repetitions = view
            .parameters
            .iter()
            .find(|parameter| parameter.id == "kaleidoscope-repetitions")
            .expect("the kaleidoscope reports its repetitions");
        assert_eq!((repetitions.minimum, repetitions.maximum), (1.0, 16.0));
        assert_eq!(repetitions.step, 1.0, "a count moves in whole numbers");

        let angle = view
            .parameters
            .iter()
            .find(|parameter| parameter.id == "kaleidoscope-angle")
            .expect("the kaleidoscope reports its angle");
        assert_eq!((angle.minimum, angle.maximum), (-180.0, 180.0));
    }
}
