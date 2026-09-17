//! The canonical DMX personality footprint.
//!
//! This is domain data. Art-Net, sACN, the HTTP API, UI metadata, tests, and GDTF all derive
//! their channel layout from here rather than restating it, so the runtime and the exported
//! fixture can never disagree about what a slot means.
//!
//! This module owns the sizes and the one-universe constraint that configuration validates at
//! startup.

pub mod channels;
pub mod decode;

pub use channels::{ChannelSpec, LAYER_CHANNELS, MASTER_CHANNELS, Resolution};
pub use decode::{DecodedFrame, FrameError};

use serde::{Deserialize, Serialize};

/// Slots one layer occupies in the current mapping personality.
///
/// Eight of these layers and the master fill one universe exactly.
pub const LAYER_SLOTS: u16 = 59;

/// Parameter bytes each effect bank carries.
pub const EFFECT_BANK_PARAMETERS: usize = 4;

/// Visualizer parameter bytes each layer carries.
pub const VISUALIZER_PARAMETERS: usize = 4;

/// Slots the master section occupies. Mirroring is a negative scale, not a channel.
pub const MASTER_SLOTS: u16 = 40;

/// Slots in one DMX universe.
pub const UNIVERSE_SLOTS: u16 = 512;

/// How many layers a configured output exposes to the desk.
///
/// These are the only two personalities: two layers for a compact patch, eight for a full one.
/// Both use the 3D-object-mapping channel layout; the earlier layouts were retired before launch
/// and stored configurations are migrated onto this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayerPersonality {
    TwoLayers,
    #[default]
    EightLayers,
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
        SlotFootprint {
            layer_slots: self.layer_count() * LAYER_SLOTS,
            master_slots: MASTER_SLOTS,
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
            (118, 40, 158)
        );

        let eight = LayerPersonality::EightLayers.footprint();
        assert_eq!(
            (eight.layer_slots, eight.master_slots, eight.total()),
            (472, 40, 512)
        );

        assert_eq!(SlotFootprint::SINGLE_LAYER.total(), 59);
        assert_eq!(SlotFootprint::MASTER_ONLY.total(), 40);
    }

    #[test]
    fn the_master_begins_after_the_controlled_layers() {
        assert_eq!(LayerPersonality::TwoLayers.footprint().master_offset(), 118);
        assert_eq!(
            LayerPersonality::EightLayers.footprint().master_offset(),
            472
        );
    }

    #[test]
    fn an_eight_layer_output_must_fit_one_universe() {
        let eight = LayerPersonality::EightLayers.footprint();
        assert_eq!(eight.validate_start_address(1), Ok(()));
        assert_eq!(
            eight.validate_start_address(2),
            Err(StartAddressError::ExceedsUniverse {
                start_address: 2,
                required_slots: 512,
                highest_valid_start_address: 1,
            }),
            "eight mapping layers and the master fill the universe exactly"
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
    fn only_the_two_mapping_personalities_exist() {
        assert_eq!(LAYER_SLOTS, 59);
        assert_eq!(MASTER_SLOTS, 40);
        assert_eq!(
            serde_json::to_value(LayerPersonality::TwoLayers).unwrap(),
            "two-layers"
        );
        assert_eq!(
            serde_json::to_value(LayerPersonality::EightLayers).unwrap(),
            "eight-layers"
        );
        assert!(serde_json::from_value::<LayerPersonality>("mapping".into()).is_err());
    }
}
