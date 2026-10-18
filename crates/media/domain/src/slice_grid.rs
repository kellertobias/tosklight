//! Beat-shaped typography and image slices. Text is optional for old effect slots.
use crate::EffectSlot;

pub const BEAT_SLICE_GRID_EFFECT: &str = "beat-slice-grid";
pub const DEFAULT_SLICE_TEXT: &str = "PIXEL";
pub const MAX_SLICE_TEXT_CHARS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatSliceGridParameters {
    pub columns: f32,
    pub rows: f32,
    pub movement: f32,
    pub text_amount: f32,
}
impl BeatSliceGridParameters {
    pub const IDS: [&'static str; 4] = [
        "slice-columns",
        "slice-rows",
        "slice-movement",
        "slice-text-amount",
    ];
    pub const LABELS: [&'static str; 4] = ["Columns", "Rows", "Beat movement", "Text cells"];
    pub fn from_parameters(values: &[f32]) -> Self {
        let defaults = Self::default().as_array();
        let mut result = defaults;
        for (index, id) in Self::IDS.iter().enumerate() {
            if let Some(value) = values.get(index).filter(|v| v.is_finite()) {
                result[index] = crate::effect_parameter_bounds(id).resolve(*value);
            }
        }
        Self {
            columns: result[0],
            rows: result[1],
            movement: result[2],
            text_amount: result[3],
        }
    }
    pub const fn as_array(self) -> [f32; 4] {
        [self.columns, self.rows, self.movement, self.text_amount]
    }
}
impl Default for BeatSliceGridParameters {
    fn default() -> Self {
        Self {
            columns: 4.0,
            rows: 2.0,
            movement: 0.8,
            text_amount: 0.5,
        }
    }
}
impl EffectSlot {
    pub fn beat_slice_grid() -> Self {
        Self {
            effect_type: Some(BEAT_SLICE_GRID_EFFECT.to_owned()),
            enabled: true,
            mix: 1.0,
            parameters: BeatSliceGridParameters::default().as_array().to_vec(),
            text: Some(DEFAULT_SLICE_TEXT.to_owned()),
            ..Self::default()
        }
    }
    pub fn beat_slice_grid_parameters(&self) -> Option<BeatSliceGridParameters> {
        (self.enabled
            && self.mix > 0.0
            && self.effect_type.as_deref() == Some(BEAT_SLICE_GRID_EFFECT))
        .then(|| BeatSliceGridParameters::from_parameters(&self.parameters))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_slots_load_and_new_text_roundtrips() {
        let legacy = r#"{"effectType":"blur","enabled":true,"mix":1,"parameters":[0.4]}"#;
        let old: EffectSlot = serde_json::from_str(legacy).unwrap();
        assert_eq!(old.text, None);
        let mut effect = EffectSlot::beat_slice_grid();
        effect.text = Some("Live Show Ä!".to_owned());
        let stored = serde_json::to_string(&effect).unwrap();
        assert_eq!(serde_json::from_str::<EffectSlot>(&stored).unwrap(), effect);
    }
    #[test]
    fn controls_are_bounded_and_transient_beat_state_is_not_saved() {
        let mut effect = EffectSlot::beat_slice_grid();
        effect.parameters = vec![100.0, -2.0, f32::NAN, 0.7, 42.0];
        effect.normalize();
        assert_eq!(effect.parameters, vec![8.0, 1.0, 0.8, 0.7]);
    }
}
