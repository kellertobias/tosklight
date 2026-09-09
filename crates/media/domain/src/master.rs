//! Master state.
//!
//! The master applies final controls to the combined output, after every layer has composited.

use serde::{Deserialize, Serialize};

use crate::address::MediaAddress;
use crate::color::{FlipMirror, Tint};
use crate::layer::ScalingMode;

/// A fixed Layer Opacity Cycle rate relative to the detected beat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "factor")]
pub enum BeatRatio {
    #[default]
    Disabled,
    Divide(u8),
    Unity,
    Multiply(u8),
}

impl BeatRatio {
    /// Broad, jitter-resistant DMX bands. Zero alone is Off so a fresh patch is neutral.
    pub const fn from_dmx(value: u8) -> Self {
        match value {
            0 => Self::Disabled,
            1..=31 => Self::Divide(16),
            32..=63 => Self::Divide(8),
            64..=95 => Self::Divide(4),
            96..=127 => Self::Divide(2),
            128..=159 => Self::Unity,
            160..=191 => Self::Multiply(2),
            192..=223 => Self::Multiply(4),
            224..=239 => Self::Multiply(8),
            240..=255 => Self::Multiply(16),
        }
    }

    pub const fn dmx_range(self) -> (u8, u8) {
        match self {
            Self::Disabled => (0, 0),
            Self::Divide(16) => (1, 31),
            Self::Divide(8) => (32, 63),
            Self::Divide(4) => (64, 95),
            Self::Divide(2) => (96, 127),
            Self::Unity => (128, 159),
            Self::Multiply(2) => (160, 191),
            Self::Multiply(4) => (192, 223),
            Self::Multiply(8) => (224, 239),
            Self::Multiply(16) => (240, 255),
            // Construction outside the published set is normalized to Off.
            Self::Divide(_) | Self::Multiply(_) => (0, 0),
        }
    }

    pub fn factor(self) -> Option<f32> {
        match self {
            Self::Disabled => None,
            Self::Divide(divisor) if divisor > 0 => Some(1.0 / f32::from(divisor)),
            Self::Unity => Some(1.0),
            Self::Multiply(multiplier) => Some(f32::from(multiplier)),
            Self::Divide(_) => None,
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Disabled => "Off".to_owned(),
            Self::Divide(divisor) => format!("/{divisor}"),
            Self::Unity => "1x".to_owned(),
            Self::Multiply(multiplier) => format!("{multiplier}x"),
        }
    }
}

/// Output-edge shaping applied after the layers have been composited.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterShaper {
    /// Normalized inward movement of each edge (`0` is untouched, `1` reaches the opposite edge).
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
    /// Edge angles in degrees. Each edge rotates around the centre of its own side.
    pub left_rotation: f32,
    pub right_rotation: f32,
    pub top_rotation: f32,
    pub bottom_rotation: f32,
    /// Rotation of the complete four-edge shaper around the output centre.
    pub rotation: f32,
}

impl Default for MasterShaper {
    fn default() -> Self {
        Self {
            left: 0.0,
            right: 0.0,
            top: 0.0,
            bottom: 0.0,
            left_rotation: 0.0,
            right_rotation: 0.0,
            top_rotation: 0.0,
            bottom_rotation: 0.0,
            rotation: 0.0,
        }
    }
}

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
    /// Horizontal master-mask centre in half-output units.
    #[serde(default)]
    pub mask_position_x: f32,
    /// Vertical master-mask centre in half-output units.
    #[serde(default)]
    pub mask_position_y: f32,
    #[serde(default = "one")]
    pub scale_x: f32,
    #[serde(default = "one")]
    pub scale_y: f32,
    #[serde(default)]
    pub scaling_mode: ScalingMode,
    #[serde(default)]
    pub position_x: f32,
    #[serde(default)]
    pub position_y: f32,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default)]
    pub shaper: MasterShaper,
    /// The one fixed Master effect. Missing in old snapshots means Off.
    #[serde(default)]
    pub opacity_cycle: BeatRatio,
}

const fn one() -> f32 {
    1.0
}

impl Default for MasterState {
    fn default() -> Self {
        Self {
            dimmer: 1.0,
            volume: 1.0,
            tint: Tint::WHITE,
            flip_mirror: FlipMirror::default(),
            mask: MediaAddress::BLANK,
            mask_position_x: 0.0,
            mask_position_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            scaling_mode: ScalingMode::default(),
            position_x: 0.0,
            position_y: 0.0,
            rotation: 0.0,
            shaper: MasterShaper::default(),
            opacity_cycle: BeatRatio::Disabled,
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

    /// Resolves the operator's two scale values through the selected output scaling mode.
    pub fn effective_scale(&self) -> (f32, f32) {
        match self.scaling_mode {
            ScalingMode::Fit => {
                let scale = self.scale_x.min(self.scale_y);
                (scale, scale)
            }
            ScalingMode::Fill => {
                let scale = self.scale_x.max(self.scale_y);
                (scale, scale)
            }
            ScalingMode::Original => (1.0, 1.0),
            ScalingMode::Stretch => (self.scale_x, self.scale_y),
        }
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
        assert_eq!((master.mask_position_x, master.mask_position_y), (0.0, 0.0));
        assert_eq!((master.scale_x, master.scale_y), (1.0, 1.0));
        assert_eq!(master.shaper, MasterShaper::default());
        assert_eq!(master.opacity_cycle, BeatRatio::Disabled);
    }

    #[test]
    fn opacity_cycle_ratio_has_stable_dmx_bands_and_off_home() {
        let expected = [
            (0, BeatRatio::Disabled),
            (1, BeatRatio::Divide(16)),
            (32, BeatRatio::Divide(8)),
            (64, BeatRatio::Divide(4)),
            (96, BeatRatio::Divide(2)),
            (128, BeatRatio::Unity),
            (160, BeatRatio::Multiply(2)),
            (192, BeatRatio::Multiply(4)),
            (224, BeatRatio::Multiply(8)),
            (240, BeatRatio::Multiply(16)),
            (255, BeatRatio::Multiply(16)),
        ];
        for (raw, ratio) in expected {
            assert_eq!(BeatRatio::from_dmx(raw), ratio, "{raw}");
        }
        assert_eq!(BeatRatio::Divide(4).factor(), Some(0.25));
        assert_eq!(BeatRatio::Multiply(4).factor(), Some(4.0));
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

    #[test]
    fn legacy_master_json_defaults_new_geometry_and_shapers() {
        let master: MasterState = serde_json::from_str(
            r#"{"dimmer":1.0,"volume":1.0,"tint":{"red":1.0,"green":1.0,"blue":1.0},"flipMirror":"none","mask":{"folder":0,"file":0}}"#,
        )
        .unwrap();
        assert_eq!(master.scale_x, 1.0);
        assert_eq!(master.scale_y, 1.0);
        assert_eq!(master.shaper, MasterShaper::default());
    }

    #[test]
    fn master_scaling_modes_resolve_the_two_operator_axes() {
        let mut master = MasterState {
            scale_x: 0.5,
            scale_y: 1.5,
            ..Default::default()
        };
        assert_eq!(master.effective_scale(), (0.5, 0.5));
        master.scaling_mode = ScalingMode::Fill;
        assert_eq!(master.effective_scale(), (1.5, 1.5));
        master.scaling_mode = ScalingMode::Original;
        assert_eq!(master.effective_scale(), (1.0, 1.0));
        master.scaling_mode = ScalingMode::Stretch;
        assert_eq!(master.effective_scale(), (0.5, 1.5));
    }
}
