//! How a layer combines with the layers beneath it, and the strobe that shares its channel.
//!
//! One byte carries both: `0..=127` selects a blend mode in bands of sixteen, `128..=249` strobes
//! the layer from slow to fast while blending normally, and `250..=255` is Normal again with no
//! strobe, so a fader pushed to full never leaves a layer flashing.

use serde::{Deserialize, Serialize};

/// The compositing operation a layer uses against everything already drawn below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlendMode {
    #[default]
    Normal,
    Add,
    Screen,
    Multiply,
    Overlay,
    Difference,
    Lighten,
    Darken,
}

impl BlendMode {
    pub const ALL: [Self; 8] = [
        Self::Normal,
        Self::Add,
        Self::Screen,
        Self::Multiply,
        Self::Overlay,
        Self::Difference,
        Self::Lighten,
        Self::Darken,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Add => "Add",
            Self::Screen => "Screen",
            Self::Multiply => "Multiply",
            Self::Overlay => "Overlay",
            Self::Difference => "Difference",
            Self::Lighten => "Lighten",
            Self::Darken => "Darken",
        }
    }

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Add => "add",
            Self::Screen => "screen",
            Self::Multiply => "multiply",
            Self::Overlay => "overlay",
            Self::Difference => "difference",
            Self::Lighten => "lighten",
            Self::Darken => "darken",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.wire_name() == value)
    }

    /// The stable index the renderer receives.
    pub const fn index(self) -> u32 {
        self as u32
    }

    /// The inclusive DMX band that selects this mode without strobe.
    pub const fn dmx_range(self) -> (u8, u8) {
        let from = (self as u8) * 16;
        (from, from + 15)
    }
}

/// The lowest and highest byte that strobe the layer.
pub const STROBE_DMX: (u8, u8) = (128, 249);
/// The strobe rate at [`STROBE_DMX`]'s lower and upper byte.
pub const STROBE_HZ: (f32, f32) = (1.0, 25.0);

/// What one Blend mode / Strobe byte asks for.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayerBlend {
    pub mode: BlendMode,
    /// Flashes per second, or `None` for a steady layer.
    pub strobe_hz: Option<f32>,
}

impl LayerBlend {
    pub fn from_dmx(value: u8) -> Self {
        match value {
            0..=127 => Self {
                mode: BlendMode::ALL[usize::from(value / 16)],
                strobe_hz: None,
            },
            128..=249 => {
                let fraction =
                    f32::from(value - STROBE_DMX.0) / f32::from(STROBE_DMX.1 - STROBE_DMX.0);
                Self {
                    mode: BlendMode::Normal,
                    strobe_hz: Some(STROBE_HZ.0 + fraction * (STROBE_HZ.1 - STROBE_HZ.0)),
                }
            }
            250..=255 => Self::default(),
        }
    }
}

/// Whether a strobing layer is lit at this instant. Each flash is lit for half of its period.
pub fn strobe_lit(strobe_hz: Option<f32>, seconds: f64) -> bool {
    match strobe_hz {
        Some(hz) if hz.is_finite() && hz > 0.0 => (seconds * f64::from(hz)).fract() < 0.5,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_low_half_selects_eight_modes_in_bands_of_sixteen() {
        for (index, mode) in BlendMode::ALL.into_iter().enumerate() {
            let (from, to) = mode.dmx_range();
            assert_eq!(usize::from(from), index * 16);
            for value in from..=to {
                assert_eq!(
                    LayerBlend::from_dmx(value),
                    LayerBlend {
                        mode,
                        strobe_hz: None
                    }
                );
            }
        }
        assert_eq!(BlendMode::Darken.dmx_range(), (112, 127));
    }

    #[test]
    fn the_strobe_range_runs_slow_to_fast_with_normal_blending() {
        let slow = LayerBlend::from_dmx(128);
        let fast = LayerBlend::from_dmx(249);
        assert_eq!(slow.mode, BlendMode::Normal);
        assert_eq!(slow.strobe_hz, Some(1.0));
        assert_eq!(fast.strobe_hz, Some(25.0));
        assert!(LayerBlend::from_dmx(200).strobe_hz.unwrap() > 1.0);
    }

    #[test]
    fn the_top_of_the_channel_is_normal_without_strobe() {
        for value in 250..=255 {
            assert_eq!(LayerBlend::from_dmx(value), LayerBlend::default());
        }
    }

    #[test]
    fn a_strobe_is_lit_for_half_of_each_period() {
        assert!(strobe_lit(None, 12.34));
        assert!(strobe_lit(Some(2.0), 0.1));
        assert!(!strobe_lit(Some(2.0), 0.3));
        assert!(strobe_lit(Some(2.0), 0.5));
    }
}
