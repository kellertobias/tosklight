//! The canonical DMX personality footprint.
//!
//! This is domain data. Art-Net, sACN, the HTTP API, UI metadata, tests, and GDTF all derive
//! their channel layout from here rather than restating it, so the runtime and the exported
//! fixture can never disagree about what a slot means.
//!
//! The full per-channel table lands with the domain slice; this module owns the sizes and the
//! one-universe constraint that configuration validates at startup.

pub mod channels;
pub mod decode;

pub use channels::{ChannelSpec, LAYER_CHANNELS, MASTER_CHANNELS, Resolution};
pub use decode::{DecodedFrame, FrameError};

use serde::{Deserialize, Serialize};

/// Slots one layer occupies in the current personality.
///
/// Mask position uses the same 16-bit centred representation as layer position, and Blur has a
/// dedicated playback channel rather than consuming an effect slot.
pub const LAYER_SLOTS: u16 = 39;

/// Slots the master section occupies, including 16-bit X/Y position for its output-level mask.
pub const MASTER_SLOTS: u16 = 11;

/// The original published block before Blur and mask-position controls were appended.
pub const LEGACY_LAYER_SLOTS: u16 = 34;
pub const LEGACY_MASTER_SLOTS: u16 = 7;

/// Slots in one DMX universe.
pub const UNIVERSE_SLOTS: u16 = 512;

/// How many layers a configured output exposes to the desk.
///
/// Both personalities are supported products, not a migration step: two layers for a compact
/// patch, eight for a full one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayerPersonality {
    TwoLayers,
    #[default]
    EightLayers,
}

/// Which immutable channel layout a configured output decodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PersonalityLayout {
    Legacy,
    #[default]
    Current,
}

impl PersonalityLayout {
    pub const fn layer_slots(self) -> u16 {
        match self {
            Self::Legacy => LEGACY_LAYER_SLOTS,
            Self::Current => LAYER_SLOTS,
        }
    }

    pub const fn master_slots(self) -> u16 {
        match self {
            Self::Legacy => LEGACY_MASTER_SLOTS,
            Self::Current => MASTER_SLOTS,
        }
    }
}

impl LayerPersonality {
    /// The number of layers the desk controls.
    ///
    /// The legacy renderer always held eight layers while a `fullMode` flag quietly changed how
    /// many of them DMX updated. Layer count is explicit here so rendering, API state, the UI,
    /// CITP, and GDTF agree.
    pub const fn layer_count(self) -> u16 {
        match self {
            Self::TwoLayers => 2,
            Self::EightLayers => 8,
        }
    }

    /// The contiguous slot footprint this personality needs, layers followed by the master.
    pub const fn footprint(self) -> SlotFootprint {
        self.footprint_for(PersonalityLayout::Current)
    }

    pub const fn footprint_for(self, layout: PersonalityLayout) -> SlotFootprint {
        SlotFootprint {
            layer_slots: self.layer_count() * layout.layer_slots(),
            master_slots: layout.master_slots(),
        }
    }
}

/// A contiguous block of DMX slots, split into the part the layers own and the part the master
/// owns. The master always begins immediately after the configured number of controlled layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotFootprint {
    pub layer_slots: u16,
    pub master_slots: u16,
}

impl SlotFootprint {
    /// A single-layer GDTF export: one complete current layer, no master.
    pub const SINGLE_LAYER: Self = Self {
        layer_slots: LAYER_SLOTS,
        master_slots: 0,
    };

    /// A master-only GDTF export: one complete current master, no layers.
    pub const MASTER_ONLY: Self = Self {
        layer_slots: 0,
        master_slots: MASTER_SLOTS,
    };

    pub const fn total(self) -> u16 {
        self.layer_slots + self.master_slots
    }

    /// The zero-based payload offset of the master section within the footprint.
    pub const fn master_offset(self) -> u16 {
        self.layer_slots
    }

    /// Validates a one-based DMX start address as the operator and the configuration file state
    /// it. The footprint must fit one universe: configuration never spans universes, so an
    /// eight-layer output needs a start address leaving 287 contiguous slots.
    pub const fn validate_start_address(self, start_address: u16) -> Result<(), StartAddressError> {
        if start_address == 0 || start_address > UNIVERSE_SLOTS {
            return Err(StartAddressError::OutOfRange { start_address });
        }
        let last = start_address + self.total() - 1;
        if last > UNIVERSE_SLOTS {
            return Err(StartAddressError::ExceedsUniverse {
                start_address,
                required_slots: self.total(),
                highest_valid_start_address: UNIVERSE_SLOTS - self.total() + 1,
            });
        }
        Ok(())
    }

    /// The zero-based payload offset of a validated one-based start address.
    pub const fn payload_offset(start_address: u16) -> u16 {
        start_address - 1
    }
}

/// Why a configured start address cannot carry a personality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StartAddressError {
    #[error("DMX start address {start_address} is outside 1..=512")]
    OutOfRange { start_address: u16 },
    #[error(
        "DMX start address {start_address} leaves too little room for {required_slots} slots; \
         the highest valid start address is {highest_valid_start_address}"
    )]
    ExceedsUniverse {
        start_address: u16,
        required_slots: u16,
        highest_valid_start_address: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_footprints_hold() {
        let two = LayerPersonality::TwoLayers.footprint();
        assert_eq!(
            (two.layer_slots, two.master_slots, two.total()),
            (78, 11, 89)
        );

        let eight = LayerPersonality::EightLayers.footprint();
        assert_eq!(
            (eight.layer_slots, eight.master_slots, eight.total()),
            (312, 11, 323)
        );

        assert_eq!(SlotFootprint::SINGLE_LAYER.total(), 39);
        assert_eq!(SlotFootprint::MASTER_ONLY.total(), 11);
    }

    #[test]
    fn the_master_begins_after_the_controlled_layers() {
        assert_eq!(LayerPersonality::TwoLayers.footprint().master_offset(), 78);
        assert_eq!(
            LayerPersonality::EightLayers.footprint().master_offset(),
            312
        );
    }

    #[test]
    fn an_eight_layer_output_must_fit_one_universe() {
        let eight = LayerPersonality::EightLayers.footprint();
        assert_eq!(eight.validate_start_address(1), Ok(()));
        assert_eq!(eight.validate_start_address(190), Ok(()));
        assert_eq!(
            eight.validate_start_address(191),
            Err(StartAddressError::ExceedsUniverse {
                start_address: 191,
                required_slots: 323,
                highest_valid_start_address: 190,
            })
        );
    }

    #[test]
    fn start_addresses_are_one_based() {
        let two = LayerPersonality::TwoLayers.footprint();
        assert_eq!(
            two.validate_start_address(0),
            Err(StartAddressError::OutOfRange { start_address: 0 })
        );
        assert_eq!(
            two.validate_start_address(513),
            Err(StartAddressError::OutOfRange { start_address: 513 })
        );
        assert_eq!(SlotFootprint::payload_offset(1), 0);
        assert_eq!(SlotFootprint::payload_offset(234), 233);
    }

    #[test]
    fn the_current_blocks_include_mask_position() {
        assert_eq!(LAYER_SLOTS, 39);
        assert_eq!(MASTER_SLOTS, 11);
    }
}
