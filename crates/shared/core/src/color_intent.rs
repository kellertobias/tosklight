//! The device-independent colour a show programs in Color Intent mode, and how close a fixture
//! came to it.
//!
//! # The target
//!
//! A Color Intent target is a CIE 1931 2° XYZ tristimulus value relative to the D65 reference white
//! [`D65_WHITE`], serialised as the ordinary [`Xyz`] object `{ "x", "y", "z" }` stored under the
//! `color` attribute. Components are finite and non-negative; `y` is the relative luminance with
//! the reference white at `1.0`, so every target lies in `0.0..=`[`MAX_TARGET_COMPONENT`].
//!
//! Only the target's *chromaticity* is the colour. Brightness belongs to the Intensity feature:
//! in Intent mode a fixture reproduces the target's chromaticity at the brightest level its
//! colour engine reaches, and Intensity dims it from there. A target of zero luminance has no
//! chromaticity and resolves to the engine's own white, so a colour value can never become a
//! second dimmer.
//!
//! # Comparing colours
//!
//! Two colours are compared by their distance in the CIE 1976 u′v′ chromaticity diagram,
//! [`delta_uv`]. A result within [`EXACT_DELTA_UV`] is visually indistinguishable on stage; one
//! within [`APPROXIMATE_DELTA_UV`] is recognisably the same colour; anything further is reported as
//! out of gamut.

use crate::Xyz;
use serde::{Deserialize, Serialize};

/// CIE D65 reference white, normalised to a relative luminance of one.
pub const D65_WHITE: Xyz = Xyz {
    x: 0.950_47,
    y: 1.0,
    z: 1.088_83,
};

/// The largest component a stored target may carry. D65 white's `z` is the largest component any
/// in-gamut reference colour reaches; the margin leaves room for rounding in authored values.
pub const MAX_TARGET_COMPONENT: f32 = 1.2;

/// Chromaticity distance under which a reproduction counts as exact.
pub const EXACT_DELTA_UV: f32 = 0.004;

/// Chromaticity distance under which a reproduction still counts as the requested colour.
pub const APPROXIMATE_DELTA_UV: f32 = 0.02;

/// How a show programs colour. Absent from shows written before Color Intent existed, which
/// therefore keep [`ColorProgrammingModel::Direct`].
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorProgrammingModel {
    /// Fixture-native colour channels are programmed directly; a whole-colour target is resolved
    /// exactly as authored, luminance included.
    #[default]
    Direct,
    /// One device-independent target per fixture; native channels are resolver output only.
    Intent,
}

impl ColorProgrammingModel {
    pub const fn is_direct(&self) -> bool {
        matches!(self, Self::Direct)
    }
}

/// How faithfully one fixture head reproduces its Color Intent target, most trustworthy first.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorResolutionQuality {
    /// Measured calibration and a chromaticity within [`EXACT_DELTA_UV`].
    Exact,
    /// Within [`APPROXIMATE_DELTA_UV`], or exact against nominal rather than measured data.
    Approximate,
    /// The closest colour a continuous engine reaches is further than [`APPROXIMATE_DELTA_UV`].
    OutOfGamut,
    /// A fixed colour-wheel slot provides the colour and does not match it exactly.
    WheelLimited,
    /// The profile declares no calibration, so the output is a best effort from channel names.
    Uncalibrated,
    /// The head has no colour engine the resolver can drive; its colour channels stay untouched.
    Unsupported,
}

impl ColorResolutionQuality {
    /// A short operator-facing label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Exact => "Exact",
            Self::Approximate => "Approximate",
            Self::OutOfGamut => "Out of gamut",
            Self::WheelLimited => "Wheel-limited",
            Self::Uncalibrated => "Uncalibrated",
            Self::Unsupported => "Unsupported",
        }
    }
}

/// Whether a stored target satisfies the documented numeric range.
pub fn valid_target(target: Xyz) -> bool {
    [target.x, target.y, target.z]
        .into_iter()
        .all(|component| component.is_finite() && (0.0..=MAX_TARGET_COMPONENT).contains(&component))
}

/// CIE 1976 u′v′ chromaticity, or `None` for black, which has none.
pub fn chromaticity_uv(value: Xyz) -> Option<(f32, f32)> {
    let denominator = value.x + 15.0 * value.y + 3.0 * value.z;
    (denominator.is_finite() && denominator > 1e-6)
        .then(|| (4.0 * value.x / denominator, 9.0 * value.y / denominator))
}

/// Chromaticity distance between two colours. Black is treated as D65 white, the colour a
/// zero-luminance target resolves to.
pub fn delta_uv(left: Xyz, right: Xyz) -> f32 {
    let white = chromaticity_uv(D65_WHITE).unwrap_or((0.1978, 0.4683));
    let left = chromaticity_uv(left).unwrap_or(white);
    let right = chromaticity_uv(right).unwrap_or(white);
    ((left.0 - right.0).powi(2) + (left.1 - right.1).powi(2)).sqrt()
}

/// The target with its luminance set to one, keeping its chromaticity. Black becomes D65 white.
pub fn normalized_chromaticity(target: Xyz) -> Xyz {
    if !(target.y.is_finite() && target.y > 1e-6) || chromaticity_uv(target).is_none() {
        return D65_WHITE;
    }
    Xyz {
        x: target.x / target.y,
        y: 1.0,
        z: target.z / target.y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_has_the_d65_chromaticity_and_is_its_own_match() {
        let (u, v) = chromaticity_uv(D65_WHITE).unwrap();
        assert!((u - 0.1978).abs() < 1e-3 && (v - 0.4683).abs() < 1e-3);
        assert_eq!(delta_uv(D65_WHITE, D65_WHITE), 0.0);
    }

    #[test]
    fn luminance_does_not_change_the_colour() {
        let red = Xyz {
            x: 0.4124,
            y: 0.2126,
            z: 0.0193,
        };
        let dim_red = Xyz {
            x: red.x * 0.1,
            y: red.y * 0.1,
            z: red.z * 0.1,
        };
        assert!(delta_uv(red, dim_red) < 1e-5);
        let normalized = normalized_chromaticity(dim_red);
        assert!((normalized.y - 1.0).abs() < 1e-6);
        assert!(delta_uv(normalized, red) < 1e-5);
    }

    #[test]
    fn black_resolves_as_white_rather_than_as_a_dimmer() {
        let black = Xyz {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        assert_eq!(normalized_chromaticity(black), D65_WHITE);
        assert!(delta_uv(black, D65_WHITE) < 1e-6);
    }

    #[test]
    fn range_rejects_negative_non_finite_and_oversized_targets() {
        assert!(valid_target(D65_WHITE));
        for component in [-0.1, f32::NAN, f32::INFINITY, 1.5] {
            assert!(!valid_target(Xyz {
                x: component,
                y: 0.5,
                z: 0.5
            }));
        }
    }

    #[test]
    fn quality_orders_from_most_to_least_trustworthy() {
        assert!(ColorResolutionQuality::Exact < ColorResolutionQuality::Approximate);
        assert!(ColorResolutionQuality::WheelLimited < ColorResolutionQuality::Unsupported);
    }
}
