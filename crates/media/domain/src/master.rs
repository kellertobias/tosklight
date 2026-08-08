//! Master state.
//!
//! The master applies final controls to the combined output, after every layer has composited.

use serde::{Deserialize, Serialize};

use crate::address::MediaAddress;
use crate::color::{FlipMirror, Tint};

/// The section that begins immediately after the configured number of controlled layers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterState {
    /// Final output intensity, applied as a black overlay over the finished composite.
    pub dimmer: f32,
    /// Multiplier for every layer's audio.
    pub volume: f32,
    pub tint: Tint,
    pub flip_mirror: FlipMirror,
    /// An output-level library mask applied to the completed composite.
    pub mask: MediaAddress,
}

impl Default for MasterState {
    fn default() -> Self {
        Self {
            dimmer: 1.0,
            volume: 1.0,
            tint: Tint::WHITE,
            flip_mirror: FlipMirror::default(),
            mask: MediaAddress::BLANK,
        }
    }
}

impl MasterState {
    /// The effective audio level of a layer: `clamp(layer × master, 0, 1)`.
    pub fn effective_volume(&self, layer_volume: f32) -> f32 {
        (layer_volume * self.volume).clamp(0.0, 1.0)
    }

    pub fn has_mask(&self) -> bool {
        !self.mask.is_blank()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_master_is_neutral() {
        let master = MasterState::default();
        assert_eq!(master.dimmer, 1.0);
        assert_eq!(master.volume, 1.0);
        assert_eq!(master.tint, Tint::WHITE);
        assert_eq!(master.flip_mirror, FlipMirror::None);
        assert!(!master.has_mask());
    }

    #[test]
    fn effective_volume_multiplies_and_clamps() {
        let master = MasterState {
            volume: 0.5,
            ..Default::default()
        };
        assert_eq!(master.effective_volume(1.0), 0.5);
        assert_eq!(master.effective_volume(0.5), 0.25);
        assert_eq!(master.effective_volume(0.0), 0.0);

        let loud = MasterState {
            volume: 4.0,
            ..Default::default()
        };
        assert_eq!(
            loud.effective_volume(1.0),
            1.0,
            "the product never exceeds unity"
        );
        assert_eq!(loud.effective_volume(-1.0), 0.0);
    }

    #[test]
    fn a_blank_master_mask_address_is_no_mask() {
        let mut master = MasterState::default();
        assert!(!master.has_mask());
        master.mask = MediaAddress::new(4, 9);
        assert!(master.has_mask());
        master.mask = MediaAddress::new(4, 255);
        assert!(!master.has_mask(), "file 255 is a blank sentinel here too");
    }
}
