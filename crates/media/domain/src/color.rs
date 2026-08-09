//! Color processing.
//!
//! DMX exposes tint subtractively — cyan, magenta, and yellow — while the domain and the renderer
//! work in linear RGB multipliers.

use serde::{Deserialize, Serialize};

use crate::dmx;

/// The luminance weights the legacy renderer used, kept so a migrated show's grayscale looks the
/// same: `0.299 R + 0.587 G + 0.114 B`.
pub const LUMINANCE_WEIGHTS: [f32; 3] = [0.299, 0.587, 0.114];

/// A multiplicative RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tint {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

impl Tint {
    /// No tint.
    pub const WHITE: Self = Self {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
    };

    pub const fn new(red: f32, green: f32, blue: f32) -> Self {
        Self { red, green, blue }
    }

    /// Reads a subtractive cyan/magenta/yellow triple off the wire.
    pub fn from_subtractive(cyan: u8, magenta: u8, yellow: u8) -> Self {
        Self {
            red: dmx::subtractive(cyan),
            green: dmx::subtractive(magenta),
            blue: dmx::subtractive(yellow),
        }
    }

    /// Combines a layer tint with the master tint. Tints multiply; they never add.
    pub fn multiply(self, other: Self) -> Self {
        Self {
            red: self.red * other.red,
            green: self.green * other.green,
            blue: self.blue * other.blue,
        }
    }

    /// The luminance of this color under [`LUMINANCE_WEIGHTS`].
    pub fn luminance(self) -> f32 {
        let [red, green, blue] = LUMINANCE_WEIGHTS;
        self.red * red + self.green * green + self.blue * blue
    }

    /// Interpolates between the original color and its luminance.
    pub fn desaturate(self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let gray = self.luminance();
        Self {
            red: self.red + (gray - self.red) * amount,
            green: self.green + (gray - self.green) * amount,
            blue: self.blue + (gray - self.blue) * amount,
        }
    }
}

impl Default for Tint {
    fn default() -> Self {
        Self::WHITE
    }
}

/// How the completed composite is flipped before it reaches the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlipMirror {
    #[default]
    None,
    Horizontal,
    Vertical,
    Both,
}

impl FlipMirror {
    /// Reads the master flip channel. `0`–`3` map directly; a desk sending anything else is
    /// normalized into the four states rather than being ignored or treated as an error.
    pub const fn from_dmx(value: u8) -> Self {
        match value % 4 {
            0 => Self::None,
            1 => Self::Horizontal,
            2 => Self::Vertical,
            _ => Self::Both,
        }
    }

    pub const fn flips_horizontally(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }

    pub const fn flips_vertically(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f32, expected: f32) -> bool {
        (actual - expected).abs() < 1e-5
    }

    #[test]
    fn no_subtractive_tint_is_white() {
        assert_eq!(Tint::from_subtractive(0, 0, 0), Tint::WHITE);
    }

    #[test]
    fn full_cyan_removes_red() {
        let tint = Tint::from_subtractive(255, 0, 0);
        assert_eq!(tint, Tint::new(0.0, 1.0, 1.0));
    }

    #[test]
    fn tints_multiply_rather_than_add() {
        let half = Tint::new(0.5, 0.5, 0.5);
        assert_eq!(half.multiply(half), Tint::new(0.25, 0.25, 0.25));
        assert_eq!(half.multiply(Tint::WHITE), half);
    }

    #[test]
    fn luminance_uses_the_documented_weights() {
        assert!(close(Tint::new(1.0, 0.0, 0.0).luminance(), 0.299));
        assert!(close(Tint::new(0.0, 1.0, 0.0).luminance(), 0.587));
        assert!(close(Tint::new(0.0, 0.0, 1.0).luminance(), 0.114));
        assert!(close(Tint::WHITE.luminance(), 1.0));
    }

    #[test]
    fn grayscale_interpolates_between_the_colour_and_its_luminance() {
        let red = Tint::new(1.0, 0.0, 0.0);
        assert_eq!(red.desaturate(0.0), red);

        let gray = red.desaturate(1.0);
        assert!(close(gray.red, 0.299) && close(gray.green, 0.299) && close(gray.blue, 0.299));

        let half = red.desaturate(0.5);
        assert!(close(half.red, 0.6495));
    }

    #[test]
    fn grayscale_amounts_outside_the_range_are_clamped() {
        let red = Tint::new(1.0, 0.0, 0.0);
        assert_eq!(red.desaturate(-1.0), red);
        assert_eq!(red.desaturate(2.0), red.desaturate(1.0));
    }

    #[test]
    fn the_flip_channel_maps_the_four_documented_values() {
        assert_eq!(FlipMirror::from_dmx(0), FlipMirror::None);
        assert_eq!(FlipMirror::from_dmx(1), FlipMirror::Horizontal);
        assert_eq!(FlipMirror::from_dmx(2), FlipMirror::Vertical);
        assert_eq!(FlipMirror::from_dmx(3), FlipMirror::Both);
    }

    #[test]
    fn other_flip_bytes_are_normalized_rather_than_dropped() {
        assert_eq!(FlipMirror::from_dmx(4), FlipMirror::None);
        assert_eq!(FlipMirror::from_dmx(255), FlipMirror::Both);
        for value in 0..=255u8 {
            let flip = FlipMirror::from_dmx(value);
            assert_eq!(flip, FlipMirror::from_dmx(value % 4));
        }
    }

    #[test]
    fn both_flips_each_axis() {
        assert!(FlipMirror::Both.flips_horizontally() && FlipMirror::Both.flips_vertically());
        assert!(FlipMirror::Horizontal.flips_horizontally());
        assert!(!FlipMirror::Horizontal.flips_vertically());
        assert!(!FlipMirror::None.flips_horizontally());
    }
}
