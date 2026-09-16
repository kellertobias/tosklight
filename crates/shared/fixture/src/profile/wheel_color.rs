//! The colour a discrete colour-wheel slot shows when nobody has measured it.
//!
//! A slot's `measured_xyz` is authoritative. Most manufacturer charts only name their filters, so a
//! slot without a measurement is shown by the colour its name describes: "Open" is white,
//! "Deep Red" a darker red, "CTO 3200K" a warm tint. A name nothing here recognises shows no colour
//! of its own, which leaves the simulator's fallback in charge exactly as before.

use super::ColorWheelSlot;
use crate::srgb_to_xyz;
use light_core::Xyz;

/// Named filter colours as sRGB, matched word by word in the order a name spells them.
const NAMED_COLORS: &[(&str, [f32; 3])] = &[
    ("open", [1.0, 1.0, 1.0]),
    ("white", [1.0, 1.0, 1.0]),
    ("clear", [1.0, 1.0, 1.0]),
    ("cto", [1.0, 0.78, 0.55]),
    ("ctb", [0.78, 0.87, 1.0]),
    ("ctc", [1.0, 0.9, 0.8]),
    ("congo", [0.25, 0.0, 0.6]),
    ("uv", [0.35, 0.0, 0.8]),
    ("red", [1.0, 0.0, 0.0]),
    ("fire", [1.0, 0.3, 0.0]),
    ("orange", [1.0, 0.5, 0.0]),
    ("amber", [1.0, 0.72, 0.0]),
    ("gold", [1.0, 0.82, 0.3]),
    ("straw", [1.0, 0.9, 0.6]),
    ("yellow", [1.0, 1.0, 0.0]),
    ("lime", [0.6, 1.0, 0.0]),
    ("green", [0.0, 1.0, 0.0]),
    ("aquamarine", [0.4, 1.0, 0.8]),
    ("teal", [0.0, 0.6, 0.6]),
    ("cyan", [0.0, 1.0, 1.0]),
    ("blue", [0.0, 0.2, 1.0]),
    ("lavender", [0.7, 0.55, 1.0]),
    ("lilac", [0.8, 0.6, 1.0]),
    ("mauve", [0.8, 0.5, 0.8]),
    ("purple", [0.5, 0.0, 1.0]),
    ("violet", [0.55, 0.0, 1.0]),
    ("magenta", [1.0, 0.0, 1.0]),
    ("pink", [1.0, 0.45, 0.7]),
    ("rose", [1.0, 0.5, 0.6]),
];

/// The nominal sRGB colour a slot name describes, if any word of it names one.
pub fn nominal_wheel_srgb(name: &str) -> Option<[f32; 3]> {
    let lowered = name.to_ascii_lowercase();
    let words = lowered
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let color = words.iter().find_map(|word| {
        NAMED_COLORS
            .iter()
            .find(|(candidate, _)| candidate == word)
            .map(|(_, rgb)| *rgb)
    })?;
    let white = words
        .iter()
        .any(|word| matches!(*word, "pale" | "light" | "tint"));
    let dark = words.iter().any(|word| matches!(*word, "deep" | "dark"));
    Some(color.map(|channel| {
        if white {
            channel + (1.0 - channel) * 0.5
        } else if dark {
            channel * 0.7
        } else {
            channel
        }
    }))
}

impl ColorWheelSlot {
    /// The colour this slot puts on stage: its measurement, else the colour its name describes.
    pub fn display_xyz(&self) -> Option<Xyz> {
        self.measured_xyz.or_else(|| {
            nominal_wheel_srgb(&self.label)
                .or_else(|| nominal_wheel_srgb(&self.semantic_id))
                .map(|[red, green, blue]| srgb_to_xyz(red, green, blue))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_names_describe_their_colour() {
        assert_eq!(nominal_wheel_srgb("Open"), Some([1.0, 1.0, 1.0]));
        assert_eq!(nominal_wheel_srgb("deep_red"), Some([0.7, 0.0, 0.0]));
        assert_eq!(nominal_wheel_srgb("Congo Blue"), Some([0.25, 0.0, 0.6]));
        assert_eq!(nominal_wheel_srgb("CTO 3200K"), Some([1.0, 0.78, 0.55]));
        assert_eq!(
            nominal_wheel_srgb("Pale Lavender"),
            Some([0.85, 0.775, 1.0])
        );
        assert_eq!(nominal_wheel_srgb("Rotation stop"), None);
    }

    #[test]
    fn a_measurement_wins_over_the_name() {
        let measured = Xyz {
            x: 0.1,
            y: 0.2,
            z: 0.3,
        };
        let mut slot = ColorWheelSlot {
            semantic_id: "red".into(),
            label: "Red".into(),
            dmx_from: 0,
            dmx_to: 9,
            measured_xyz: Some(measured),
        };
        assert_eq!(slot.display_xyz().map(|xyz| xyz.x), Some(0.1));
        slot.measured_xyz = None;
        assert_eq!(slot.display_xyz(), Some(srgb_to_xyz(1.0, 0.0, 0.0)));
    }
}
