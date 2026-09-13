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

pub use channels::{
    ChannelSpec, EFFECT_BANK_LAYER_CHANNELS, EFFECT_BANK_MASTER_CHANNELS, LAYER_CHANNELS,
    MASTER_CHANNELS, Resolution, layer_channels, master_channels,
};
pub use decode::{DecodedFrame, FrameError};

use serde::{Deserialize, Serialize};

/// Slots one layer occupies in the current mapping personality.
///
/// Eight of these layers and the master fill one universe exactly.
pub const LAYER_SLOTS: u16 = 59;

/// The effect-bank layer before blend, playback range, parameters, and 3D mapping were added.
pub const EFFECT_BANK_LAYER_SLOTS: u16 = 39;

/// Parameter bytes each effect bank carries in the current personality.
pub const EFFECT_BANK_PARAMETERS: usize = 4;

/// Visualizer parameter bytes each layer carries in the current personality.
pub const VISUALIZER_PARAMETERS: usize = 4;

/// Slots the current master section occupies. Mirroring is a negative scale, not a channel.
pub const MASTER_SLOTS: u16 = 40;

/// The effect-bank master, which still carries Flip/mirror.
pub const EFFECT_BANK_MASTER_SLOTS: u16 = 41;

/// The original published block before Blur and mask-position controls were appended.
pub const LEGACY_LAYER_SLOTS: u16 = 34;
pub const LEGACY_MASTER_SLOTS: u16 = 7;
/// The v2 block added master-mask positioning but predates master geometry and shapers.
pub const CURRENT_MASTER_SLOTS: u16 = 11;
/// The v3 expanded master before the fixed Layer Opacity Cycle channel was appended.
pub const EXTENDED_MASTER_SLOTS: u16 = 40;

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
    Current,
    Extended,
    EffectBanks,
    /// Blend, playback range, effect and visualizer parameters, and 3D model mapping.
    #[default]
    Mapping,
}

impl PersonalityLayout {
    pub const fn layer_slots(self) -> u16 {
        match self {
            Self::Legacy => LEGACY_LAYER_SLOTS,
            Self::Current | Self::Extended | Self::EffectBanks => EFFECT_BANK_LAYER_SLOTS,
            Self::Mapping => LAYER_SLOTS,
        }
    }

    pub const fn master_slots(self) -> u16 {
        match self {
            Self::Legacy => LEGACY_MASTER_SLOTS,
            Self::Current => CURRENT_MASTER_SLOTS,
            Self::Extended => EXTENDED_MASTER_SLOTS,
            Self::EffectBanks => EFFECT_BANK_MASTER_SLOTS,
            Self::Mapping => MASTER_SLOTS,
        }
    }

    /// Whether the wire carries the two Effect Select/Strength banks.
    pub const fn carries_effect_banks(self) -> bool {
        matches!(self, Self::EffectBanks | Self::Mapping)
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
        self.footprint_for(PersonalityLayout::Mapping)
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
            (118, 40, 158)
        );

        let eight = LayerPersonality::EightLayers.footprint();
        assert_eq!(
            (eight.layer_slots, eight.master_slots, eight.total()),
            (472, 40, 512)
        );

        assert_eq!(SlotFootprint::SINGLE_LAYER.total(), 59);
        assert_eq!(SlotFootprint::MASTER_ONLY.total(), 40);

        assert_eq!(
            LayerPersonality::TwoLayers
                .footprint_for(PersonalityLayout::Current)
                .total(),
            89
        );
        assert_eq!(
            LayerPersonality::EightLayers
                .footprint_for(PersonalityLayout::EffectBanks)
                .total(),
            353,
            "the effect-bank layout keeps its published footprint"
        );
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
    fn the_extended_blocks_include_complete_master_control() {
        assert_eq!(LAYER_SLOTS, 59);
        assert_eq!(EFFECT_BANK_LAYER_SLOTS, 39);
        assert_eq!(CURRENT_MASTER_SLOTS, 11);
        assert_eq!(MASTER_SLOTS, 40);
        assert_eq!(EFFECT_BANK_MASTER_SLOTS, 41);
        assert_eq!(EXTENDED_MASTER_SLOTS, 40);
    }
}
