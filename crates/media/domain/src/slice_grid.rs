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

pub const MAX_SLICE_TEXTS: usize = 8;
pub const MAX_SLICE_ENTRY_CHARS: usize = 512;
pub const SLICE_TEXT_WIDTH: u32 = 512;
pub const SLICE_TEXT_HEIGHT: u32 = 256;

/// Authored typography; absent metadata reads the old single `text` field.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SliceGridTypography {
    pub texts: Vec<String>,
    pub font: String,
    pub text_color: [u8; 3],
    pub background_color: [u8; 3],
}
impl Default for SliceGridTypography {
    fn default() -> Self {
        Self {
            texts: vec![DEFAULT_SLICE_TEXT.to_owned()],
            font: "Arial Black".to_owned(),
            text_color: [242, 4, 6],
            background_color: [250, 250, 250],
        }
    }
}
impl SliceGridTypography {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.texts.is_empty() || self.texts.len() > MAX_SLICE_TEXTS {
            return Err("typography.texts must contain 1 to 8 text entries");
        }
        if self.texts.iter().any(|text| {
            text.chars().count() > MAX_SLICE_ENTRY_CHARS || text.split('\n').count() > 16
        }) {
            return Err("each text entry accepts up to 512 characters and 16 lines");
        }
        if self.font.trim().is_empty() || self.font.chars().count() > 128 {
            return Err("typography.font must name a font family using 1 to 128 characters");
        }
        Ok(())
    }
}
/// Runtime-only raster data; shared across frame clones and uploaded only when its identity changes.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceTextAtlas {
    pub pixels: Vec<u8>,
    pub aspects: Vec<f32>,
}
impl EffectSlot {
    pub fn slice_typography(&self) -> SliceGridTypography {
        self.typography
            .clone()
            .unwrap_or_else(|| SliceGridTypography {
                texts: vec![
                    self.text
                        .clone()
                        .unwrap_or_else(|| DEFAULT_SLICE_TEXT.to_owned()),
                ],
                ..SliceGridTypography::default()
            })
    }
}

#[cfg(test)]
mod typography_tests {
    use super::*;
    #[test]
    fn old_single_text_maps_to_first_entry_and_new_multiline_metadata_roundtrips() {
        let legacy = r#"{"effectType":"beat-slice-grid","enabled":true,"mix":1,"parameters":[4,2,0.8,0.5],"text":"OLD\nPOSTER"}"#;
        let old: EffectSlot = serde_json::from_str(legacy).unwrap();
        assert_eq!(old.slice_typography().texts, vec!["OLD\nPOSTER"]);
        let mut effect = EffectSlot::beat_slice_grid();
        effect.typography = Some(SliceGridTypography {
            texts: vec!["HELLO\nWORLD".into(), "SECOND\nENTRY".into()],
            font: "serif".into(),
            text_color: [3, 44, 255],
            background_color: [40, 10, 7],
        });
        effect.normalize();
        let stored = serde_json::to_string(&effect).unwrap();
        assert_eq!(serde_json::from_str::<EffectSlot>(&stored).unwrap(), effect);
        assert_eq!(effect.slice_typography().texts[0], "HELLO\nWORLD");
        assert!(!stored.contains("textAtlas"));
    }
    #[test]
    fn typography_limits_are_explicit() {
        let mut typography = SliceGridTypography::default();
        typography.texts = vec!["".into(); 8];
        assert!(typography.validate().is_ok());
        typography.texts.push("x".into());
        assert!(typography.validate().is_err());
        typography.texts = vec!["x".repeat(513)];
        assert!(typography.validate().is_err());
        typography.texts = vec!["x\n".repeat(16)];
        assert!(typography.validate().is_err());
    }
}
