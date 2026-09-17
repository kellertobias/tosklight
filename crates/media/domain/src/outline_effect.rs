//! Outline: detected edges drawn over, or instead of, the layer's picture (TL-426).
//!
//! One Intensity control carries the whole look. Zero is the untouched source; the outlines fade
//! in up to one half, where they sit over the fully visible picture; above one half the picture
//! darkens until, at one, only the outlines remain on black. A beat can push Intensity toward one
//! and let it fall back over Beat decay; Beat depth zero, the default, leaves it static.

use crate::layer::EffectSlot;

pub const OUTLINE_EFFECT: &str = "outline";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutlineParameters {
    /// `0` original, `0.5` outlines over the picture, `1` outlines only on black.
    pub intensity: f32,
    /// How far a landed beat pushes Intensity toward one. Zero switches beat modulation off.
    pub beat_depth: f32,
    /// Line width in source pixels.
    pub thickness: f32,
    pub hue_degrees: f32,
    /// Zero draws white lines; one draws the fully saturated hue.
    pub saturation: f32,
    /// Higher values also outline softer contrast.
    pub sensitivity: f32,
    /// Seconds a beat pulse takes to fall back to the resting Intensity.
    pub beat_decay_seconds: f32,
}

impl OutlineParameters {
    /// Bank parameter channels follow this order, so the first four are the live controls.
    pub const IDS: [&'static str; 7] = [
        "outline-intensity",
        "outline-beat-depth",
        "outline-thickness",
        "outline-hue",
        "outline-saturation",
        "outline-sensitivity",
        "outline-beat-decay",
    ];
    pub const LABELS: [&'static str; 7] = [
        "Intensity",
        "Beat depth",
        "Line thickness",
        "Line hue",
        "Line saturation",
        "Edge sensitivity",
        "Beat decay",
    ];
    /// Closed ranges in [`Self::IDS`] order; `effect_parameters` advertises the same ones.
    pub const RANGES: [(f32, f32); 7] = [
        (0.0, 1.0),
        (0.0, 1.0),
        (1.0, 8.0),
        (0.0, 360.0),
        (0.0, 1.0),
        (0.0, 1.0),
        (0.05, 5.0),
    ];

    /// Missing or non-finite values take their default, so a preset stored with fewer
    /// parameters, or none, loads as a complete Outline.
    pub fn from_parameters(values: &[f32]) -> Self {
        let defaults = Self::default().as_array();
        let resolved: [f32; 7] = std::array::from_fn(|index| {
            let (low, high) = Self::RANGES[index];
            match values.get(index) {
                Some(value) if value.is_finite() => value.clamp(low, high),
                _ => defaults[index],
            }
        });
        Self {
            intensity: resolved[0],
            beat_depth: resolved[1],
            thickness: resolved[2],
            hue_degrees: resolved[3],
            saturation: resolved[4],
            sensitivity: resolved[5],
            beat_decay_seconds: resolved[6],
        }
    }

    pub const fn as_array(self) -> [f32; 7] {
        [
            self.intensity,
            self.beat_depth,
            self.thickness,
            self.hue_degrees,
            self.saturation,
            self.sensitivity,
            self.beat_decay_seconds,
        ]
    }

    /// The line colour as linear RGB in `0..=1`.
    pub fn colour(self) -> [f32; 3] {
        let hue = self.hue_degrees.rem_euclid(360.0) / 60.0;
        let channel = |offset: f32| {
            let k = (offset + hue).rem_euclid(6.0);
            1.0 - self.saturation * k.min(4.0 - k).clamp(0.0, 1.0)
        };
        [channel(5.0), channel(3.0), channel(1.0)]
    }
}

impl Default for OutlineParameters {
    fn default() -> Self {
        Self {
            intensity: 0.5,
            beat_depth: 0.0,
            thickness: 2.0,
            hue_degrees: 0.0,
            saturation: 0.0,
            sensitivity: 0.5,
            beat_decay_seconds: 0.4,
        }
    }
}

impl EffectSlot {
    pub fn outline() -> Self {
        Self {
            effect_type: Some(OUTLINE_EFFECT.to_owned()),
            enabled: true,
            seed: 0,
            mix: 1.0,
            parameters: OutlineParameters::default().as_array().to_vec(),
            visualizer_parameters: None,
        }
    }

    pub fn outline_parameters(&self) -> Option<OutlineParameters> {
        (self.enabled && self.mix > 0.0 && self.effect_type.as_deref() == Some(OUTLINE_EFFECT))
            .then(|| OutlineParameters::from_parameters(&self.parameters))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preset_stored_without_parameters_loads_every_default() {
        let stored = r#"{"effectType":"outline","enabled":true,"mix":1.0,"parameters":[]}"#;
        let mut effect: EffectSlot = serde_json::from_str(stored).expect("an old preset loads");
        effect.normalize();
        assert_eq!(
            effect.parameters,
            OutlineParameters::default().as_array().to_vec()
        );
        assert_eq!(
            effect.outline_parameters(),
            Some(OutlineParameters::default())
        );
    }

    #[test]
    fn a_short_or_invalid_preset_keeps_what_it_has_and_clamps_the_rest() {
        let mut effect = EffectSlot::outline();
        effect.parameters = vec![0.9, f32::NAN, 40.0];
        effect.normalize();
        let parameters = OutlineParameters::from_parameters(&effect.parameters);
        assert_eq!(parameters.intensity, 0.9);
        assert_eq!(parameters.beat_depth, 0.0, "beat modulation stays off");
        assert_eq!(parameters.thickness, 8.0);
        assert_eq!(parameters.beat_decay_seconds, 0.4);
        assert_eq!(effect.parameters.len(), OutlineParameters::IDS.len());
    }

    #[test]
    fn bypass_and_other_types_report_no_outline() {
        let mut effect = EffectSlot::outline();
        effect.mix = 0.0;
        assert_eq!(effect.outline_parameters(), None);
        assert_eq!(EffectSlot::blur().outline_parameters(), None);
    }

    #[test]
    fn line_colour_is_white_until_saturated() {
        let white = OutlineParameters::default();
        assert_eq!(white.colour(), [1.0, 1.0, 1.0]);
        let red = OutlineParameters {
            saturation: 1.0,
            ..white
        };
        assert_eq!(red.colour(), [1.0, 0.0, 0.0]);
        let cyan = OutlineParameters {
            hue_degrees: 180.0,
            ..red
        };
        assert_eq!(cyan.colour(), [0.0, 1.0, 1.0]);
    }
}
