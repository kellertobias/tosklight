//! Persisted, addressed effect presets.
//!
//! A layer selects these presets with a DMX byte. Slot zero is deliberately absent and always
//! means Off; usable presets occupy `1..=255` and retain their address across restarts.

use serde::{Deserialize, Serialize};

use crate::{EffectSlot, RasterizeMode, RasterizeParameters};

/// Where the shipped Outline preset lives, for new libraries and for the migration that adds it.
pub const OUTLINE_PRESET_SLOT: u8 = 13;
pub const OUTLINE_PRESET_NAME: &str = "Outline";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectPreset {
    pub slot: u8,
    pub name: String,
    pub effect: EffectSlot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectLibrary {
    pub entries: Vec<EffectPreset>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EffectLibraryError {
    #[error("effect slot 0 is Off and cannot hold a preset")]
    OffSlot,
    #[error("effect slot {slot} is assigned more than once")]
    DuplicateSlot { slot: u8 },
    #[error("effect slot {slot} needs a name")]
    EmptyName { slot: u8 },
    #[error("effect slot {slot} needs a supported effect type")]
    EmptyEffect { slot: u8 },
}

impl EffectLibrary {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn resolve(&self, slot: u8) -> Option<&EffectPreset> {
        (slot != 0)
            .then(|| self.entries.iter().find(|entry| entry.slot == slot))
            .flatten()
    }

    pub fn assign(
        &mut self,
        slot: u8,
        name: impl Into<String>,
        mut effect: EffectSlot,
    ) -> Result<(), EffectLibraryError> {
        let name = name.into();
        validate_entry(slot, &name, &effect)?;
        effect.normalize();
        effect.enabled = true;
        effect.mix = 1.0;
        let preset = EffectPreset { slot, name, effect };
        if let Some(existing) = self.entries.iter_mut().find(|entry| entry.slot == slot) {
            *existing = preset;
        } else {
            self.entries.push(preset);
            self.entries.sort_by_key(|entry| entry.slot);
        }
        Ok(())
    }

    /// Adds the shipped Outline preset to a library stored before it existed, unless its slot is
    /// taken or the operator already keeps an Outline elsewhere. Nothing stored is replaced.
    pub fn add_outline_if_free(&mut self) {
        let has_outline = self.entries.iter().any(|entry| {
            entry.effect.effect_type.as_deref() == Some(crate::outline_effect::OUTLINE_EFFECT)
        });
        if !has_outline && self.resolve(OUTLINE_PRESET_SLOT).is_none() {
            self.entries.push(preset(
                OUTLINE_PRESET_SLOT,
                OUTLINE_PRESET_NAME,
                EffectSlot::outline(),
            ));
            self.entries.sort_by_key(|entry| entry.slot);
        }
    }

    pub fn remove(&mut self, slot: u8) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.slot != slot);
        self.entries.len() != before
    }

    pub fn validate(&self) -> Result<(), EffectLibraryError> {
        let mut occupied = [false; 256];
        for entry in &self.entries {
            validate_entry(entry.slot, &entry.name, &entry.effect)?;
            if occupied[usize::from(entry.slot)] {
                return Err(EffectLibraryError::DuplicateSlot { slot: entry.slot });
            }
            occupied[usize::from(entry.slot)] = true;
        }
        Ok(())
    }
}

fn validate_entry(slot: u8, name: &str, effect: &EffectSlot) -> Result<(), EffectLibraryError> {
    if slot == 0 {
        return Err(EffectLibraryError::OffSlot);
    }
    if name.trim().is_empty() {
        return Err(EffectLibraryError::EmptyName { slot });
    }
    if effect.effect_type.is_none() {
        return Err(EffectLibraryError::EmptyEffect { slot });
    }
    Ok(())
}

impl Default for EffectLibrary {
    fn default() -> Self {
        let mut entries = vec![
            preset(1, "TV/CRT/VHS Simulation", EffectSlot::analog_tv()),
            preset(
                2,
                "Digital Video/ Glitch Simulation",
                EffectSlot::digital_tv(),
            ),
            preset(3, "Blur", EffectSlot::blur()),
            preset(4, "Feedback", EffectSlot::feedback()),
            preset(5, "Beat Move", EffectSlot::beat_move()),
            preset(6, "Beat Scan", EffectSlot::beat_scan()),
            preset(7, "Beat Scale & Turn", EffectSlot::beat_scale_turn()),
            preset(8, "Beat form Flash", EffectSlot::beat_form_flash()),
            preset(9, "Kaleidoscope", EffectSlot::kaleidoscope()),
            preset(10, "B/W Rasterize", EffectSlot::rasterize()),
            preset(12, "Drawn Image Style", EffectSlot::drawn_image()),
            preset(
                OUTLINE_PRESET_SLOT,
                OUTLINE_PRESET_NAME,
                EffectSlot::outline(),
            ),
        ];
        let mut cmyk = EffectSlot::rasterize();
        cmyk.parameters = RasterizeParameters {
            mode: RasterizeMode::Cmyk,
            ..Default::default()
        }
        .as_array()
        .to_vec();
        entries.push(preset(11, "CMYK Rasterize", cmyk));
        entries.sort_by_key(|entry| entry.slot);
        Self { entries }
    }
}

fn preset(slot: u8, name: &str, mut effect: EffectSlot) -> EffectPreset {
    effect.enabled = true;
    effect.mix = 1.0;
    EffectPreset {
        slot,
        name: name.to_owned(),
        effect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_always_off_and_defaults_have_stable_addresses() {
        let library = EffectLibrary::default();
        assert!(library.resolve(0).is_none());
        assert_eq!(library.resolve(1).unwrap().name, "TV/CRT/VHS Simulation");
        assert_eq!(library.resolve(11).unwrap().name, "CMYK Rasterize");
        assert_eq!(library.resolve(13).unwrap().name, "Outline");
        assert!(library.validate().is_ok());
    }

    #[test]
    fn assignment_is_sorted_replaceable_and_validated() {
        let mut library = EffectLibrary::empty();
        library.assign(9, "Blur", EffectSlot::blur()).unwrap();
        library
            .assign(2, "Feedback", EffectSlot::feedback())
            .unwrap();
        assert_eq!(
            library
                .entries
                .iter()
                .map(|entry| entry.slot)
                .collect::<Vec<_>>(),
            [2, 9]
        );
        library.assign(2, "TV", EffectSlot::analog_tv()).unwrap();
        assert_eq!(library.resolve(2).unwrap().name, "TV");
        assert_eq!(
            library.assign(0, "Off", EffectSlot::blur()),
            Err(EffectLibraryError::OffSlot)
        );
    }

    #[test]
    fn outline_is_added_to_an_older_library_only_where_nothing_is_stored() {
        let mut older = EffectLibrary::default();
        older.remove(OUTLINE_PRESET_SLOT);
        older.add_outline_if_free();
        assert_eq!(older, EffectLibrary::default());

        let mut occupied = EffectLibrary::empty();
        occupied
            .assign(13, "House blur", EffectSlot::blur())
            .unwrap();
        occupied.add_outline_if_free();
        assert_eq!(occupied.entries.len(), 1);
        assert_eq!(occupied.resolve(13).unwrap().name, "House blur");

        let mut own = EffectLibrary::empty();
        own.assign(40, "My lines", EffectSlot::outline()).unwrap();
        own.add_outline_if_free();
        assert_eq!(own.entries.len(), 1, "an operator's own Outline is enough");
    }

    #[test]
    fn duplicate_stored_slots_are_rejected() {
        let preset = preset(7, "One", EffectSlot::blur());
        let library = EffectLibrary {
            entries: vec![preset.clone(), preset],
        };
        assert_eq!(
            library.validate(),
            Err(EffectLibraryError::DuplicateSlot { slot: 7 })
        );
    }
}
