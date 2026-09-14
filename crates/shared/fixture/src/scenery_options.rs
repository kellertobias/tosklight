//! What an operator chooses for one placed Venue object beyond its size.
//!
//! A curtain is bought in a colour and a chain is rigged with what hangs at each end. Neither is a
//! property of the product the profile describes — the same curtain profile is black serge on one
//! stage and white muslin on the next — so both travel with the placement, beside its size.

use serde::{Deserialize, Serialize};

/// Per-placement choices for a generated Venue object. Every field is optional: absent reads as
/// what the object's kind is drawn with by default, which is what every object placed before these
/// choices existed reads as.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SceneryOptions {
    /// The object's colour as `#RRGGBB` in sRGB. Absent keeps its kind's own material — black
    /// serge for a curtain, raw aluminium for truss.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_srgb: Option<String>,
    /// What the top of a chain hangs from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_top: Option<ChainTopEnd>,
    /// What the bottom of a chain holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_bottom: Option<ChainBottomEnd>,
}

impl SceneryOptions {
    pub fn is_empty(&self) -> bool {
        self.colour_srgb.is_none() && self.chain_top.is_none() && self.chain_bottom.is_none()
    }

    /// Whether every choice is one the object can be drawn with.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(colour) = &self.colour_srgb
            && parse_srgb(colour).is_none()
        {
            return Err(format!(
                "scenery colour {colour:?} must be a #RRGGBB sRGB colour"
            ));
        }
        Ok(())
    }

    /// The chosen colour in linear light, for a renderer, when one is chosen.
    pub fn colour_linear(&self) -> Option<[f32; 3]> {
        let [red, green, blue] = parse_srgb(self.colour_srgb.as_deref()?)?;
        Some([
            srgb_to_linear(red),
            srgb_to_linear(green),
            srgb_to_linear(blue),
        ])
    }
}

/// The top of a chain: a hoist lifting it, or the chain made fast straight to the steel or truss.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainTopEnd {
    #[default]
    Motor,
    Direct,
}

/// The bottom of a chain: its hook shackled straight to the load, or a steelflex loop wrapped
/// around a beam or truss chord.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainBottomEnd {
    #[default]
    Direct,
    SteelflexLoop,
}

fn parse_srgb(value: &str) -> Option<[u8; 3]> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

fn srgb_to_linear(channel: u8) -> f32 {
    let value = f32::from(channel) / 255.0;
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_chosen_is_empty_and_serialises_to_nothing() {
        let options = SceneryOptions::default();
        assert!(options.is_empty());
        assert_eq!(
            serde_json::to_value(&options).unwrap(),
            serde_json::json!({})
        );
        let read: SceneryOptions = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(read, options);
    }

    #[test]
    fn a_colour_is_a_hex_srgb_triple() {
        let red = SceneryOptions {
            colour_srgb: Some("#FF0000".into()),
            ..SceneryOptions::default()
        };
        assert!(red.validate().is_ok());
        assert_eq!(red.colour_linear(), Some([1.0, 0.0, 0.0]));
        for invalid in ["red", "#F00", "#GG0000", "FF0000"] {
            let options = SceneryOptions {
                colour_srgb: Some(invalid.into()),
                ..SceneryOptions::default()
            };
            assert!(options.validate().is_err(), "{invalid} was accepted");
        }
    }

    #[test]
    fn chain_ends_use_their_rigging_names() {
        let options = SceneryOptions {
            chain_top: Some(ChainTopEnd::Direct),
            chain_bottom: Some(ChainBottomEnd::SteelflexLoop),
            ..SceneryOptions::default()
        };
        assert_eq!(
            serde_json::to_value(&options).unwrap(),
            serde_json::json!({ "chain_top": "direct", "chain_bottom": "steelflex_loop" })
        );
    }
}
