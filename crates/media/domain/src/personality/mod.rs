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

/// Slots one layer occupies in the v2 personality.
///
/// The legacy personality used 32. v2 spends two more so both mask axes can be 16-bit without
/// dropping an effect or playback control.
pub const LAYER_SLOTS: u16 = 34;

/// Slots the master section occupies: dimmer, volume, cyan/magenta/yellow, flip/mirror, and the
/// output-level master mask.
pub const MASTER_SLOTS: u16 = 7;

/// Slots in one DMX universe.
pub const UNIVERSE_SLOTS: u16 = 512;

/// Which personality version a show, an export, or a test is speaking.
///
/// v2 renumbers the Loop/Bounce/Once play-mode ranges and adds Reverse. That is a deliberate
/// versioned change, so every surface that reports a channel layout also reports which version
/// produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PersonalityVersion {
    /// The C++ application's 32-slot layer personality. Read for migration; not emitted.
    V1Legacy,
    /// The 34-slot layer personality this product ships.
    #[default]
    V2,
}

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
    /// A single-layer GDTF export: 34 slots, no master.
    pub const SINGLE_LAYER: Self = Self {
        layer_slots: LAYER_SLOTS,
        master_slots: 0,
    };

    /// A master-only GDTF export: 7 slots, no layers.
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
    /// eight-layer output needs a start address leaving 279 contiguous slots.
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
            (68, 7, 75)
        );

        let eight = LayerPersonality::EightLayers.footprint();
        assert_eq!(
            (eight.layer_slots, eight.master_slots, eight.total()),
            (272, 7, 279)
        );

        assert_eq!(SlotFootprint::SINGLE_LAYER.total(), 34);
        assert_eq!(SlotFootprint::MASTER_ONLY.total(), 7);
    }

    #[test]
    fn the_master_begins_after_the_controlled_layers() {
        assert_eq!(LayerPersonality::TwoLayers.footprint().master_offset(), 68);
        assert_eq!(
            LayerPersonality::EightLayers.footprint().master_offset(),
            272
        );
    }

    #[test]
    fn an_eight_layer_output_must_fit_one_universe() {
        let eight = LayerPersonality::EightLayers.footprint();
        assert_eq!(eight.validate_start_address(1), Ok(()));
        assert_eq!(eight.validate_start_address(234), Ok(()));
        assert_eq!(
            eight.validate_start_address(235),
            Err(StartAddressError::ExceedsUniverse {
                start_address: 235,
                required_slots: 279,
                highest_valid_start_address: 234,
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
    fn v2_is_the_emitted_personality_version() {
        assert_eq!(PersonalityVersion::default(), PersonalityVersion::V2);
    }
}
