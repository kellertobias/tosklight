//! Color processing.
//!
//! DMX exposes tint subtractively — cyan, magenta, and yellow — while the domain and the renderer
//! work in linear RGB multipliers.
//!
//! # Transfer boundary
//!
//! Every Media source texture, the program target and the output surface carry sRGB-encoded
//! values: decoders, uploads and composition never convert them. The color stage is the one place
//! that needs linear light, so it decodes a sample exactly once, applies White Blend and tint
//! there, and encodes the result exactly once on the way back into the encoded pipeline.
//! [`MediaColor::apply_encoded`] is the CPU reference for that stage; the layer and master
//! shaders implement the same formula.

use serde::{Deserialize, Serialize};

use crate::dmx;
use crate::layer::LayerState;
use crate::master::MasterState;

/// Rec.709 / linear sRGB luminance coefficients. They apply to linear light only; applying them
/// to encoded values would desaturate in gamma space.
pub const REC709_LUMINANCE: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// The sRGB electro-optical transfer function (IEC 61966-2-1): an encoded `0..=1` value to linear
/// light. Negative input is treated as black.
pub fn srgb_to_linear(encoded: f32) -> f32 {
    let encoded = encoded.max(0.0);
    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

/// The inverse of [`srgb_to_linear`]: linear light to an encoded value.
pub fn linear_to_srgb(linear: f32) -> f32 {
    let linear = linear.max(0.0);
    if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

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

    /// A fully saturated, full-value colour at `degrees` around the hue wheel.
    pub fn from_hue(degrees: f32) -> Self {
        let sector = degrees.rem_euclid(360.0) / 60.0;
        let fraction = sector.fract();
        let (red, green, blue) = match sector as u32 {
            0 => (1.0, fraction, 0.0),
            1 => (1.0 - fraction, 1.0, 0.0),
            2 => (0.0, 1.0, fraction),
            3 => (0.0, 1.0 - fraction, 1.0),
            4 => (fraction, 0.0, 1.0),
            _ => (1.0, 0.0, 1.0 - fraction),
        };
        Self::new(red, green, blue)
    }

    /// This colour's hue in degrees, `0.0..360.0`. A grey reports zero.
    pub fn hue_degrees(self) -> f32 {
        let max = self.red.max(self.green).max(self.blue);
        let min = self.red.min(self.green).min(self.blue);
        let delta = max - min;
        if delta <= f32::EPSILON {
            return 0.0;
        }
        let sector = if max == self.red {
            ((self.green - self.blue) / delta).rem_euclid(6.0)
        } else if max == self.green {
            (self.blue - self.red) / delta + 2.0
        } else {
            (self.red - self.green) / delta + 4.0
        };
        (sector * 60.0).rem_euclid(360.0)
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

    /// The Rec.709 luminance of this color, read as linear light.
    pub fn luminance(self) -> f32 {
        let [red, green, blue] = REC709_LUMINANCE;
        self.red * red + self.green * green + self.blue * blue
    }

    /// Interpolates between this linear color and its luminance: White Blend on one linear value.
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

/// The Media adapter's reading of the shared Color intent.
///
/// The desk's Color intent reaches Media through the existing tint and grayscale controls: the
/// retained base color becomes the linear RGB [`Tint`], and White Blend drives the legacy
/// grayscale control as source desaturation. This is derived from those controls whenever it is
/// needed and is never stored, so it cannot become a second color authority.
///
/// Layer dimmer, alpha, masks and the master dimmer stay outside it: they remain independent
/// multipliers applied after the color stage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaColor {
    /// Linear RGB multiplier. [`Tint::WHITE`] is the neutral value, not an authored white.
    pub tint: Tint,
    /// Source desaturation in `0..=1`. `None` where the surface has no White Blend control, such
    /// as the master: that is unsupported rather than an authored 0%, and it renders unchanged.
    pub white_blend: Option<f32>,
}

impl MediaColor {
    /// The color stage of one layer: its tint and its grayscale control as White Blend.
    pub fn of_layer(layer: &LayerState) -> Self {
        Self {
            tint: layer.tint,
            white_blend: Some(layer.grayscale),
        }
    }

    /// The color stage of the master, which has a tint but no White Blend control.
    pub fn of_master(master: &MasterState) -> Self {
        Self {
            tint: master.tint,
            white_blend: None,
        }
    }

    /// The White Blend amount the renderer applies: clamped to `0..=1`, and zero when the control
    /// is absent or not a number.
    pub fn white_blend_amount(self) -> f32 {
        match self.white_blend {
            Some(amount) if amount.is_finite() => amount.clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// Applies White Blend and then the tint to one linear RGB value.
    pub fn apply_linear(self, linear: [f32; 3]) -> [f32; 3] {
        let blended = Tint::new(linear[0], linear[1], linear[2])
            .desaturate(self.white_blend_amount())
            .multiply(self.tint);
        [blended.red, blended.green, blended.blue]
    }

    /// The CPU reference for the render color stage: decode an sRGB-encoded value once, apply
    /// [`Self::apply_linear`], and encode the result once.
    pub fn apply_encoded(self, encoded: [f32; 3]) -> [f32; 3] {
        self.apply_linear(encoded.map(srgb_to_linear))
            .map(linear_to_srgb)
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
    fn luminance_uses_the_rec709_linear_weights() {
        assert!(close(Tint::new(1.0, 0.0, 0.0).luminance(), 0.2126));
        assert!(close(Tint::new(0.0, 1.0, 0.0).luminance(), 0.7152));
        assert!(close(Tint::new(0.0, 0.0, 1.0).luminance(), 0.0722));
        assert!(close(Tint::WHITE.luminance(), 1.0));
    }

    #[test]
    fn grayscale_interpolates_between_the_colour_and_its_luminance() {
        let red = Tint::new(1.0, 0.0, 0.0);
        assert_eq!(red.desaturate(0.0), red);

        let gray = red.desaturate(1.0);
        assert!(close(gray.red, 0.2126) && close(gray.green, 0.2126) && close(gray.blue, 0.2126));

        let half = red.desaturate(0.5);
        assert!(close(half.red, 0.6063));
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
