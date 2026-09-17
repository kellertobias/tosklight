//! What each native effect parameter accepts.
//!
//! A desk cannot guess these. Before this table existed, ToskLight Control derived a fader's
//! range from substrings of the parameter id, so it offered angles up to 360° that this server
//! rejects at 180°, and sent whole numbers as floating point into `u8` fields. Both looked to the
//! operator like a control that does nothing.
//!
//! The bounds here are the same ones `from_parameters` clamps to and the HTTP API validates
//! against, so anything that renders a control renders the range the server will actually accept.

/// What one parameter accepts: the closed range, and the smallest change worth sending.
///
/// A `step` of one or more means the parameter is a whole number — the value belongs in an
/// integer field, and a fraction is not a smaller change but an invalid one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectParameterBounds {
    pub minimum: f32,
    pub maximum: f32,
    pub step: f32,
}

impl EffectParameterBounds {
    const fn new(minimum: f32, maximum: f32, step: f32) -> Self {
        Self {
            minimum,
            maximum,
            step,
        }
    }

    /// A normalized 0–1 amount, the most common shape.
    const fn unit() -> Self {
        Self::new(0.0, 1.0, 0.01)
    }

    /// A whole-number parameter, which must reach its API field as an integer.
    pub const fn is_whole_number(self) -> bool {
        self.step >= 1.0
    }

    /// The nearest value this parameter can actually take.
    pub fn resolve(self, value: f32) -> f32 {
        let clamped = value.clamp(self.minimum, self.maximum);
        if self.is_whole_number() {
            clamped.round()
        } else {
            clamped
        }
    }

    /// What one effect-bank parameter byte asks for. Zero keeps the preset's own value, so a
    /// fresh patch plays every preset exactly as the library stores it; `1..=255` spans the
    /// closed range from its minimum to its maximum.
    pub fn from_dmx(self, raw: u8) -> Option<f32> {
        if raw == 0 {
            return None;
        }
        let fraction = f32::from(raw - 1) / 254.0;
        Some(self.resolve(self.minimum + fraction * (self.maximum - self.minimum)))
    }
}

/// The parameter ids an effect reports, in the order its effect-bank parameter channels follow.
pub fn effect_parameter_ids(effect_type: &str) -> &'static [&'static str] {
    use crate::layer::{
        ANALOG_TV_EFFECT, AnalogTvParameters, BEAT_FORM_FLASH_EFFECT, BEAT_GRID_WAVE_EFFECT,
        BEAT_MOVE_EFFECT, BEAT_SCALE_TURN_EFFECT, BEAT_SCAN_EFFECT, BLUR_EFFECT,
        BeatFormFlashParameters, BeatGridWaveParameters, BeatMoveParameters,
        BeatScaleTurnParameters, BeatScanParameters, BlurParameters, DIGITAL_TV_EFFECT,
        DRAWN_IMAGE_EFFECT, DigitalTvParameters, DrawnImageParameters, FEEDBACK_EFFECT,
        KALEIDOSCOPE_EFFECT, KaleidoscopeParameters, OPACITY_CYCLE_EFFECT, RASTERIZE_EFFECT,
        RasterizeParameters,
    };
    match effect_type {
        ANALOG_TV_EFFECT => &AnalogTvParameters::IDS,
        DIGITAL_TV_EFFECT => &DigitalTvParameters::IDS,
        BLUR_EFFECT => &BlurParameters::IDS,
        FEEDBACK_EFFECT => &FEEDBACK_PARAMETER_IDS,
        OPACITY_CYCLE_EFFECT => &["cycle-interval"],
        BEAT_MOVE_EFFECT => &BeatMoveParameters::IDS,
        KALEIDOSCOPE_EFFECT => &KaleidoscopeParameters::IDS,
        RASTERIZE_EFFECT => &RasterizeParameters::IDS,
        BEAT_SCAN_EFFECT => &BeatScanParameters::IDS,
        BEAT_SCALE_TURN_EFFECT => &BeatScaleTurnParameters::IDS,
        BEAT_GRID_WAVE_EFFECT => &BeatGridWaveParameters::IDS,
        BEAT_FORM_FLASH_EFFECT => &BeatFormFlashParameters::IDS,
        DRAWN_IMAGE_EFFECT => &DrawnImageParameters::IDS,
        crate::outline_effect::OUTLINE_EFFECT => &crate::outline_effect::OutlineParameters::IDS,
        _ => &[],
    }
}

/// Feedback's parameters, which have no typed struct constant of their own.
pub const FEEDBACK_PARAMETER_IDS: [&str; 3] =
    ["feedback-amount", "feedback-motion", "feedback-direction"];

/// One row per parameter id reported by the effects API, in no particular order.
const BOUNDS: &[(&str, EffectParameterBounds)] = &[
    ("tv-curvature", EffectParameterBounds::unit()),
    ("distortion", EffectParameterBounds::unit()),
    ("image-grain", EffectParameterBounds::unit()),
    ("glitching", EffectParameterBounds::unit()),
    ("compression-damage", EffectParameterBounds::unit()),
    ("block-size", EffectParameterBounds::unit()),
    ("tile-displacement", EffectParameterBounds::unit()),
    ("chroma-damage", EffectParameterBounds::unit()),
    ("blur-amount", EffectParameterBounds::unit()),
    ("blur-type", EffectParameterBounds::new(0.0, 4.0, 1.0)),
    ("feedback-amount", EffectParameterBounds::unit()),
    ("feedback-motion", EffectParameterBounds::unit()),
    // Choice parameters carry the index of the chosen option, so their range is the option list.
    (
        "feedback-direction",
        EffectParameterBounds::new(0.0, 7.0, 1.0),
    ),
    ("cycle-interval", EffectParameterBounds::new(0.0, 2.0, 1.0)),
    ("beat-move-amount", EffectParameterBounds::unit()),
    (
        "beat-move-direction",
        EffectParameterBounds::new(0.0, 3.0, 1.0),
    ),
    (
        "beat-move-decay",
        EffectParameterBounds::new(0.05, 5.0, 0.05),
    ),
    (
        "kaleidoscope-repetitions",
        EffectParameterBounds::new(0.0, 12.0, 1.0),
    ),
    (
        "kaleidoscope-angle",
        EffectParameterBounds::new(-180.0, 180.0, 1.0),
    ),
    ("rasterize-mode", EffectParameterBounds::new(0.0, 1.0, 1.0)),
    (
        "rasterize-dot-size",
        EffectParameterBounds::new(2.0, 32.0, 1.0),
    ),
    (
        "beat-scan-width",
        EffectParameterBounds::new(0.01, 0.25, 0.01),
    ),
    ("beat-scan-edge", EffectParameterBounds::new(0.0, 1.0, 1.0)),
    ("beat-scan-falloff", EffectParameterBounds::unit()),
    (
        "beat-scan-duration",
        EffectParameterBounds::new(0.2, 3.0, 0.05),
    ),
    ("beat-scale-amount", EffectParameterBounds::unit()),
    (
        "beat-turn-enabled",
        EffectParameterBounds::new(0.0, 1.0, 1.0),
    ),
    (
        "beat-turn-rotation",
        EffectParameterBounds::new(-30.0, 30.0, 1.0),
    ),
    (
        "beat-scale-decay",
        EffectParameterBounds::new(0.05, 5.0, 0.05),
    ),
    (
        "beat-grid-density",
        EffectParameterBounds::new(6.0, 64.0, 1.0),
    ),
    ("beat-grid-height", EffectParameterBounds::unit()),
    (
        "beat-grid-duration",
        EffectParameterBounds::new(0.2, 4.0, 0.05),
    ),
    (
        "beat-grid-origin",
        EffectParameterBounds::new(0.0, 4.0, 1.0),
    ),
    ("beat-grid-hue", EffectParameterBounds::new(0.0, 360.0, 1.0)),
    (
        "beat-grid-brightness",
        EffectParameterBounds::new(0.1, 2.0, 0.05),
    ),
    (
        "beat-form-enlargement",
        EffectParameterBounds::new(1.0, 4.0, 0.05),
    ),
    (
        "beat-form-lifetime",
        EffectParameterBounds::new(0.1, 5.0, 0.05),
    ),
    (
        "beat-form-density",
        EffectParameterBounds::new(1.0, 4.0, 1.0),
    ),
    ("beat-form-variation", EffectParameterBounds::unit()),
    ("drawn-strength", EffectParameterBounds::unit()),
    ("drawn-line-detail", EffectParameterBounds::unit()),
    ("outline-intensity", EffectParameterBounds::unit()),
    ("outline-beat-depth", EffectParameterBounds::unit()),
    (
        "outline-thickness",
        EffectParameterBounds::new(1.0, 8.0, 0.5),
    ),
    ("outline-hue", EffectParameterBounds::new(0.0, 360.0, 1.0)),
    ("outline-saturation", EffectParameterBounds::unit()),
    ("outline-sensitivity", EffectParameterBounds::unit()),
    (
        "outline-beat-decay",
        EffectParameterBounds::new(0.05, 5.0, 0.05),
    ),
];

/// What this parameter accepts. An unknown id falls back to a normalized amount, which is what
/// every parameter added without its own row has been so far.
pub fn effect_parameter_bounds(id: &str) -> EffectParameterBounds {
    BOUNDS
        .iter()
        .find(|(candidate, _)| *candidate == id)
        .map_or_else(EffectParameterBounds::unit, |(_, bounds)| *bounds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::{
        AnalogTvParameters, BeatFormFlashParameters, BeatGridWaveParameters, BeatMoveParameters,
        BeatScaleTurnParameters, BeatScanParameters, BlurParameters, DigitalTvParameters,
        DrawnImageParameters, KaleidoscopeParameters, RasterizeParameters,
    };

    /// Every id an effect reports has to have a row, or a desk falls back to 0–1 for a parameter
    /// that is nothing of the sort.
    #[test]
    fn every_reported_parameter_has_its_own_bounds() {
        let reported = [
            AnalogTvParameters::IDS.as_slice(),
            DigitalTvParameters::IDS.as_slice(),
            BeatMoveParameters::IDS.as_slice(),
            KaleidoscopeParameters::IDS.as_slice(),
            RasterizeParameters::IDS.as_slice(),
            BeatScanParameters::IDS.as_slice(),
            BeatScaleTurnParameters::IDS.as_slice(),
            BeatGridWaveParameters::IDS.as_slice(),
            BeatFormFlashParameters::IDS.as_slice(),
            DrawnImageParameters::IDS.as_slice(),
            crate::outline_effect::OutlineParameters::IDS.as_slice(),
            BlurParameters::IDS.as_slice(),
            ["feedback-amount", "feedback-motion"].as_slice(),
            ["feedback-direction", "cycle-interval"].as_slice(),
        ];
        for id in reported.into_iter().flatten() {
            assert!(
                BOUNDS.iter().any(|(candidate, _)| candidate == id),
                "{id} has no advertised bounds"
            );
        }
    }

    #[test]
    fn every_effect_reports_the_parameters_its_bank_channels_follow() {
        for effect_type in [
            crate::layer::ANALOG_TV_EFFECT,
            crate::layer::DIGITAL_TV_EFFECT,
            crate::layer::BLUR_EFFECT,
            crate::layer::FEEDBACK_EFFECT,
            crate::layer::OPACITY_CYCLE_EFFECT,
            crate::layer::BEAT_MOVE_EFFECT,
            crate::layer::KALEIDOSCOPE_EFFECT,
            crate::layer::RASTERIZE_EFFECT,
            crate::layer::BEAT_SCAN_EFFECT,
            crate::layer::BEAT_SCALE_TURN_EFFECT,
            crate::layer::BEAT_GRID_WAVE_EFFECT,
            crate::layer::BEAT_FORM_FLASH_EFFECT,
            crate::layer::DRAWN_IMAGE_EFFECT,
            crate::outline_effect::OUTLINE_EFFECT,
        ] {
            // A bank drives the first four; an effect with more keeps the rest at its preset.
            let ids = effect_parameter_ids(effect_type);
            assert!(!ids.is_empty(), "{effect_type} reports no parameters");
            for id in ids.iter().take(crate::personality::EFFECT_BANK_PARAMETERS) {
                assert!(
                    BOUNDS.iter().any(|(candidate, _)| candidate == id),
                    "{id} has no bounds to span"
                );
            }
        }
    }

    #[test]
    fn a_parameter_byte_keeps_the_preset_at_zero_and_spans_the_range_above() {
        let angle = effect_parameter_bounds("kaleidoscope-angle");
        assert_eq!(angle.from_dmx(0), None);
        assert_eq!(angle.from_dmx(1), Some(-180.0));
        assert_eq!(angle.from_dmx(255), Some(180.0));
        assert_eq!(angle.from_dmx(128), Some(0.0));

        let repetitions = effect_parameter_bounds("kaleidoscope-repetitions");
        assert_eq!(repetitions.from_dmx(1), Some(0.0), "the lowest byte is Off");
        assert_eq!(repetitions.from_dmx(255), Some(12.0));

        let amount = effect_parameter_bounds("blur-amount");
        assert_eq!(amount.from_dmx(255), Some(1.0));
    }

    /// The typed clamp and the advertised bounds are one contract.
    #[test]
    fn outline_bounds_match_the_ranges_its_parameters_clamp_to() {
        use crate::outline_effect::OutlineParameters;
        for (id, (low, high)) in OutlineParameters::IDS.iter().zip(OutlineParameters::RANGES) {
            let bounds = effect_parameter_bounds(id);
            assert_eq!((bounds.minimum, bounds.maximum), (low, high), "{id}");
        }
    }

    #[test]
    fn a_whole_number_parameter_resolves_to_a_whole_number() {
        let repetitions = effect_parameter_bounds("kaleidoscope-repetitions");
        assert!(repetitions.is_whole_number());
        assert_eq!(repetitions.resolve(7.4), 7.0);
        assert_eq!(repetitions.resolve(99.0), 12.0);
        assert_eq!(repetitions.resolve(0.0), 0.0);
    }

    #[test]
    fn a_continuous_parameter_keeps_its_fraction_inside_the_range() {
        let angle = effect_parameter_bounds("kaleidoscope-angle");
        assert_eq!(
            angle.resolve(300.0),
            180.0,
            "the desk offered 360 and 300 was refused"
        );
        let width = effect_parameter_bounds("beat-scan-width");
        assert!((width.resolve(0.12) - 0.12).abs() < f32::EPSILON);
        assert!((width.resolve(0.9) - 0.25).abs() < f32::EPSILON);
    }
}
