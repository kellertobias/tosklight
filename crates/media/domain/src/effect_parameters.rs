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
}

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
    ("feedback-amount", EffectParameterBounds::unit()),
    ("feedback-motion", EffectParameterBounds::unit()),
    // Choice parameters carry the index of the chosen option, so their range is the option list.
    (
        "feedback-direction",
        EffectParameterBounds::new(0.0, 5.0, 1.0),
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
        EffectParameterBounds::new(1.0, 16.0, 1.0),
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
        BeatScaleTurnParameters, BeatScanParameters, DigitalTvParameters, DrawnImageParameters,
        KaleidoscopeParameters, RasterizeParameters,
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
            ["blur-amount", "feedback-amount", "feedback-motion"].as_slice(),
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
    fn a_whole_number_parameter_resolves_to_a_whole_number() {
        let repetitions = effect_parameter_bounds("kaleidoscope-repetitions");
        assert!(repetitions.is_whole_number());
        assert_eq!(repetitions.resolve(7.4), 7.0);
        assert_eq!(repetitions.resolve(99.0), 16.0);
        assert_eq!(repetitions.resolve(0.0), 1.0);
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
