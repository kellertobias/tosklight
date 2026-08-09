//! The speed multiplier.
//!
//! DMX `127` is exactly `1×`. The channel is quantized into broad bands so ordinary DMX jitter
//! does not continually change playback speed — unlike the play-mode selector, an operator does
//! fade this fader, and a one-step wobble must not retime a video.

use serde::{Deserialize, Serialize};

/// The number of divisor bands below the deadband and multiplier bands above it.
const BANDS: u8 = 15;

/// The last divisor value. `0..=119` is fifteen exact 8-value bands.
const DIVISOR_TOP: u8 = 119;
/// The first multiplier value. `135..=255` is fifteen approximately 8-value bands.
const MULTIPLIER_BASE: u8 = 135;

/// A quantized division or multiplication of the effective playback rate, centered at `1×`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase", tag = "kind", content = "factor")]
pub enum SpeedMultiplier {
    /// `/2` through `/16`.
    Divide(u8),
    /// The deadband around DMX 127.
    #[default]
    Unity,
    /// `2×` through `16×`.
    Multiply(u8),
}

impl SpeedMultiplier {
    pub const fn from_dmx(value: u8) -> Self {
        if value <= DIVISOR_TOP {
            // Fifteen exact 8-value bands, widest division first.
            Self::Divide(16 - value / 8)
        } else if value < MULTIPLIER_BASE {
            Self::Unity
        } else {
            let offset = (value - MULTIPLIER_BASE) as u16;
            let span = (u8::MAX - MULTIPLIER_BASE) as u16 + 1;
            let band = (offset * BANDS as u16) / span;
            Self::Multiply(band as u8 + 2)
        }
    }

    /// The factor applied to the effective playback rate.
    pub fn factor(self) -> f32 {
        match self {
            Self::Divide(divisor) => 1.0 / f32::from(divisor),
            Self::Unity => 1.0,
            Self::Multiply(multiplier) => f32::from(multiplier),
        }
    }

    /// The name a desk shows.
    pub fn label(self) -> String {
        match self {
            Self::Divide(divisor) => format!("/{divisor}"),
            Self::Unity => "1×".to_owned(),
            Self::Multiply(multiplier) => format!("{multiplier}×"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dmx_127_is_exactly_one_times() {
        assert_eq!(SpeedMultiplier::from_dmx(127), SpeedMultiplier::Unity);
        assert_eq!(SpeedMultiplier::from_dmx(127).factor(), 1.0);
    }

    #[test]
    fn the_deadband_is_120_through_134() {
        for value in 120..=134u8 {
            assert_eq!(
                SpeedMultiplier::from_dmx(value),
                SpeedMultiplier::Unity,
                "{value}"
            );
        }
        assert_ne!(SpeedMultiplier::from_dmx(119), SpeedMultiplier::Unity);
        assert_ne!(SpeedMultiplier::from_dmx(135), SpeedMultiplier::Unity);
    }

    #[test]
    fn the_divisor_bands_are_fifteen_exact_eights() {
        assert_eq!(SpeedMultiplier::from_dmx(0), SpeedMultiplier::Divide(16));
        assert_eq!(SpeedMultiplier::from_dmx(7), SpeedMultiplier::Divide(16));
        assert_eq!(SpeedMultiplier::from_dmx(8), SpeedMultiplier::Divide(15));
        assert_eq!(SpeedMultiplier::from_dmx(112), SpeedMultiplier::Divide(2));
        assert_eq!(SpeedMultiplier::from_dmx(119), SpeedMultiplier::Divide(2));
    }

    #[test]
    fn the_multiplier_bands_span_two_to_sixteen() {
        assert_eq!(SpeedMultiplier::from_dmx(135), SpeedMultiplier::Multiply(2));
        assert_eq!(
            SpeedMultiplier::from_dmx(255),
            SpeedMultiplier::Multiply(16)
        );
    }

    #[test]
    fn every_integer_divisor_and_multiplier_is_reachable() {
        let mut reached = std::collections::BTreeSet::new();
        for value in 0..=255u8 {
            reached.insert(SpeedMultiplier::from_dmx(value));
        }
        for divisor in 2..=16u8 {
            assert!(
                reached.contains(&SpeedMultiplier::Divide(divisor)),
                "/{divisor}"
            );
        }
        for multiplier in 2..=16u8 {
            assert!(
                reached.contains(&SpeedMultiplier::Multiply(multiplier)),
                "{multiplier}×"
            );
        }
        assert!(reached.contains(&SpeedMultiplier::Unity));
        assert_eq!(
            reached.len(),
            31,
            "fifteen divisors, unity, fifteen multipliers"
        );
    }

    #[test]
    fn the_channel_is_monotonic() {
        let mut previous = SpeedMultiplier::from_dmx(0).factor();
        for value in 1..=255u8 {
            let factor = SpeedMultiplier::from_dmx(value).factor();
            assert!(
                factor >= previous,
                "speed went down between {} and {value}",
                value - 1
            );
            previous = factor;
        }
    }

    #[test]
    fn no_band_is_narrower_than_seven_values() {
        let mut width = 0;
        let mut previous = SpeedMultiplier::from_dmx(0);
        for value in 0..=255u8 {
            let current = SpeedMultiplier::from_dmx(value);
            if current == previous {
                width += 1;
                continue;
            }
            assert!(
                width >= 7,
                "{} is only {width} values wide",
                previous.label()
            );
            previous = current;
            width = 1;
        }
        assert!(
            width >= 7,
            "{} is only {width} values wide",
            previous.label()
        );
    }

    #[test]
    fn factors_and_labels_read_the_way_an_operator_expects() {
        assert_eq!(SpeedMultiplier::Divide(4).factor(), 0.25);
        assert_eq!(SpeedMultiplier::Multiply(4).factor(), 4.0);
        assert_eq!(SpeedMultiplier::Divide(16).label(), "/16");
        assert_eq!(SpeedMultiplier::Unity.label(), "1×");
        assert_eq!(SpeedMultiplier::Multiply(16).label(), "16×");
    }
}
