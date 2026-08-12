//! Which text entry answers at which address, and how it looks.
//!
//! The same separation the generated visualizers use: an entry's *content* is
//! [`crate::text::TextEntry`], which knows nothing about fonts, and its appearance is a
//! [`TextStyle`] beside it. A desk selects an address; the address names both.

use serde::{Deserialize, Serialize};

use crate::address::{AddressClass, MediaAddress};
use crate::color::Tint;
use crate::text::TextEntry;

/// The bank text entries are assigned into by default.
///
/// The whole `200..=249` range is text; shipping one populated bank is a starting point.
pub const DEFAULT_BANK: u8 = 200;

/// How a text entry is drawn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextStyle {
    /// A family name the machine is asked for. An absent family falls back rather than failing:
    /// an operator who typed a font this machine does not have should still see their words.
    pub family: String,
    /// Height as a fraction of the output's height, so a look survives a change of resolution.
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub alignment: Alignment,
    pub colour: Tint,
}

/// Where a line sits horizontally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Alignment {
    Left,
    #[default]
    Center,
    Right,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: "sans-serif".to_owned(),
            size: 0.2,
            bold: false,
            italic: false,
            alignment: Alignment::default(),
            colour: Tint::WHITE,
        }
    }
}

impl TextStyle {
    /// Brings an edited style back into a range that can actually be drawn.
    pub fn clamped(&self) -> Self {
        Self {
            size: if self.size.is_finite() {
                self.size.clamp(0.01, 2.0)
            } else {
                0.2
            },
            ..self.clone()
        }
    }
}

/// One address a text entry answers at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSlot {
    pub address: MediaAddress,
    /// What an operator calls this slot in a list.
    pub name: String,
    pub entry: TextEntry,
    pub style: TextStyle,
}

/// Why a text-catalog edit was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TextCatalogError {
    #[error("that address is not in the text range")]
    NotTextSpace,
    #[error("another text entry already answers at that address")]
    AddressTaken,
}

/// The text a desk can address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextCatalog {
    pub version: u32,
    pub slots: Vec<TextSlot>,
}

pub const CATALOG_VERSION: u32 = 1;

impl Default for TextCatalog {
    /// One shipped example per kind, so a first run has something to select and an operator can
    /// see what each kind does before writing their own.
    fn default() -> Self {
        Self {
            version: CATALOG_VERSION,
            slots: vec![
                TextSlot {
                    // File one: the first usable slot, because file zero is blank in every bank.
                    address: MediaAddress::new(DEFAULT_BANK, 1),
                    name: "Clock".to_owned(),
                    entry: TextEntry::new(crate::text::TextKind::Clock),
                    style: TextStyle::default(),
                },
                TextSlot {
                    address: MediaAddress::new(DEFAULT_BANK, 2),
                    name: "Ten minutes".to_owned(),
                    entry: TextEntry::new(crate::text::TextKind::CountdownFromDuration {
                        duration: std::time::Duration::from_secs(600),
                    }),
                    style: TextStyle::default(),
                },
            ],
        }
    }
}

impl TextCatalog {
    pub fn resolve(&self, address: MediaAddress) -> Option<&TextSlot> {
        self.slots.iter().find(|slot| slot.address == address)
    }

    pub fn assign(&mut self, slot: TextSlot) -> Result<(), TextCatalogError> {
        if slot.address.classify() != AddressClass::TextBank {
            return Err(TextCatalogError::NotTextSpace);
        }
        if self.resolve(slot.address).is_some() {
            return Err(TextCatalogError::AddressTaken);
        }
        self.slots.push(slot);
        self.slots
            .sort_by_key(|slot| (slot.address.folder, slot.address.file));
        Ok(())
    }

    pub fn remove(&mut self, address: MediaAddress) -> bool {
        let before = self.slots.len();
        self.slots.retain(|slot| slot.address != address);
        self.slots.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextKind;

    #[test]
    fn every_shipped_slot_is_addressable_and_none_is_blank() {
        let catalog = TextCatalog::default();
        assert!(!catalog.slots.is_empty());
        for slot in &catalog.slots {
            assert_eq!(
                slot.address.classify(),
                AddressClass::TextBank,
                "{}",
                slot.name
            );
            assert!(!slot.address.is_blank(), "{} is unreachable", slot.name);
        }
    }

    #[test]
    fn text_may_only_be_assigned_inside_the_text_range() {
        let mut catalog = TextCatalog::default();
        let slot = |folder, file| TextSlot {
            address: MediaAddress::new(folder, file),
            name: "Cue".to_owned(),
            entry: TextEntry::new(TextKind::Static {
                text: "Stand by".to_owned(),
            }),
            style: TextStyle::default(),
        };

        assert_eq!(
            catalog.assign(slot(1, 5)),
            Err(TextCatalogError::NotTextSpace),
            "a library address is not text space"
        );
        assert_eq!(
            catalog.assign(slot(250, 5)),
            Err(TextCatalogError::NotTextSpace),
            "the generated range is not text space"
        );
        assert_eq!(
            catalog.assign(slot(DEFAULT_BANK, 1)),
            Err(TextCatalogError::AddressTaken)
        );

        assert_eq!(catalog.assign(slot(201, 9)), Ok(()));
        assert!(catalog.resolve(MediaAddress::new(201, 9)).is_some());
    }

    #[test]
    fn an_edited_style_cannot_produce_something_undrawable() {
        let absurd = TextStyle {
            size: -4.0,
            ..Default::default()
        }
        .clamped();
        assert!(absurd.size > 0.0);

        let broken = TextStyle {
            size: f32::NAN,
            ..Default::default()
        }
        .clamped();
        assert!(broken.size.is_finite());
    }

    #[test]
    fn removing_a_slot_makes_the_address_answer_nothing() {
        let mut catalog = TextCatalog::default();
        let address = MediaAddress::new(DEFAULT_BANK, 1);
        assert!(catalog.remove(address));
        assert_eq!(catalog.resolve(address), None);
        assert!(!catalog.remove(address), "removing twice is not a change");
    }
}
