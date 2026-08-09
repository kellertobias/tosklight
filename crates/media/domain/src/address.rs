//! Media addressing.
//!
//! A `(folder, file)` pair on the wire names a media item, a text entry, or a generated
//! visualizer. Selecting an address is separate from whether that address resolves: a layer keeps
//! the address the desk chose even while its source is loading or has failed.

use serde::{Deserialize, Serialize};

/// The wire address of a layer's source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAddress {
    pub folder: u8,
    pub file: u8,
}

impl MediaAddress {
    pub const BLANK: Self = Self { folder: 0, file: 0 };

    pub const fn new(folder: u8, file: u8) -> Self {
        Self { folder, file }
    }

    /// What kind of source this address names, and whether it names one at all.
    pub const fn classify(self) -> AddressClass {
        // File 0 and file 255 are blank sentinels in every bank, so a desk can clear a layer
        // from either end of the fader without hunting for an empty slot.
        if self.file == 0 || self.file == 255 {
            return AddressClass::Blank;
        }
        match self.folder {
            // Folder 0 carries no media on the wire. The folder itself remains valid on disk;
            // it is only DMX that reads zero as "nothing selected".
            0 => AddressClass::Blank,
            1..=199 => AddressClass::Library,
            200..=219 => AddressClass::TextBank,
            220..=255 => AddressClass::GeneratedVisualizer,
        }
    }

    /// Whether this address selects nothing. A blank layer draws transparent and reports no
    /// source rather than reporting a failure.
    pub const fn is_blank(self) -> bool {
        matches!(self.classify(), AddressClass::Blank)
    }
}

impl std::fmt::Display for MediaAddress {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:03}/{:03}", self.folder, self.file)
    }
}

/// The address space an address falls in.
///
/// No part of `220..=255` is reserved media space: the generated-source catalog may initially
/// populate only some of it, but the range belongs to generated sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AddressClass {
    /// Nothing selected.
    Blank,
    /// An image or video from the filesystem library, folders `001–199`.
    Library,
    /// One of twenty text-source banks, folders `200–219`.
    TextBank,
    /// A generated-visualizer bank, folders `220–255`.
    GeneratedVisualizer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_zero_selects_nothing_on_the_wire() {
        assert_eq!(MediaAddress::new(0, 1).classify(), AddressClass::Blank);
        assert_eq!(MediaAddress::new(0, 254).classify(), AddressClass::Blank);
        assert!(MediaAddress::BLANK.is_blank());
    }

    #[test]
    fn file_zero_and_file_255_are_blank_in_every_bank() {
        for folder in [1, 199, 200, 219, 220, 255] {
            assert_eq!(
                MediaAddress::new(folder, 0).classify(),
                AddressClass::Blank,
                "{folder}"
            );
            assert_eq!(
                MediaAddress::new(folder, 255).classify(),
                AddressClass::Blank,
                "{folder}"
            );
        }
    }

    #[test]
    fn the_library_range_is_one_to_one_hundred_ninety_nine() {
        assert_eq!(MediaAddress::new(1, 1).classify(), AddressClass::Library);
        assert_eq!(
            MediaAddress::new(199, 254).classify(),
            AddressClass::Library
        );
        assert_ne!(MediaAddress::new(200, 1).classify(), AddressClass::Library);
    }

    #[test]
    fn twenty_text_banks_sit_between_the_library_and_the_visualizers() {
        assert_eq!(MediaAddress::new(200, 1).classify(), AddressClass::TextBank);
        assert_eq!(
            MediaAddress::new(219, 254).classify(),
            AddressClass::TextBank
        );
        assert_eq!(
            MediaAddress::new(220, 1).classify(),
            AddressClass::GeneratedVisualizer
        );
    }

    #[test]
    fn the_whole_generated_range_belongs_to_generated_sources() {
        for folder in 220..=255u8 {
            assert_eq!(
                MediaAddress::new(folder, 1).classify(),
                AddressClass::GeneratedVisualizer,
                "folder {folder} must not be reserved media space"
            );
        }
    }

    #[test]
    fn every_address_classifies() {
        for folder in 0..=255u8 {
            for file in 0..=255u8 {
                let _ = MediaAddress::new(folder, file).classify();
            }
        }
    }

    #[test]
    fn addresses_display_as_three_digit_pairs() {
        assert_eq!(MediaAddress::new(7, 12).to_string(), "007/012");
    }
}

/// Stable identity of one library asset.
///
/// Distinct from a [`MediaAddress`]: the address is where a desk points, and reindexing moves it.
/// The identity follows the asset through renames, moves, and reindexing, which is what caches,
/// playback sessions, and the catalog key on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetId(uuid::Uuid);

impl AssetId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub const fn from_uuid(value: uuid::Uuid) -> Self {
        Self(value)
    }

    pub const fn as_uuid(&self) -> uuid::Uuid {
        self.0
    }
}

impl Default for AssetId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AssetId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod asset_identity_tests {
    use super::*;

    #[test]
    fn an_asset_identity_is_independent_of_where_a_desk_points_at_it() {
        let asset = AssetId::new();
        assert_ne!(asset, AssetId::new());
        assert_eq!(AssetId::from_uuid(asset.as_uuid()), asset);
    }
}
