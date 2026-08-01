//! Resolve additive, subtractive, and fixed-wheel colour into linear RGB.

use crate::plan::ColourBinding;
use viz_dmx::DMX_SLOTS;

/// Emitter primaries in linear RGB, used to mix a fixture's own emitter set into a display colour.
const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const AMBER: [f32; 3] = [1.0, 0.62, 0.16];
const ULTRAVIOLET: [f32; 3] = [0.35, 0.1, 1.0];
const COLD_WHITE: [f32; 3] = [0.86, 0.93, 1.0];
const WARM_WHITE: [f32; 3] = [1.0, 0.83, 0.62];

/// Resolved colour plus the implied intensity for a fixture without a dimmer channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedColour {
    pub rgb: [f32; 3],
    /// Largest emitter contribution, used as the virtual dimmer when no intensity channel exists.
    pub level: f32,
    /// `false` when the binding carried no colour channels at all.
    pub explicit: bool,
}

impl Default for ResolvedColour {
    fn default() -> Self {
        Self {
            rgb: WHITE,
            level: 1.0,
            explicit: false,
        }
    }
}

pub fn resolve(binding: &ColourBinding, frame: &dyn Fn(u16) -> [u8; DMX_SLOTS]) -> ResolvedColour {
    if binding.is_empty() {
        return ResolvedColour::default();
    }
    let read = |channel: &Option<crate::binding::ChannelRef>| -> Option<f32> {
        let channel = channel.as_ref()?;
        Some(channel.normalised(&frame(channel.logical_universe)))
    };

    // Subtractive systems describe how much of each primary is removed.
    let cyan = read(&binding.cyan);
    let magenta = read(&binding.magenta);
    let yellow = read(&binding.yellow);
    if cyan.is_some() || magenta.is_some() || yellow.is_some() {
        let rgb = [
            1.0 - cyan.unwrap_or(0.0),
            1.0 - magenta.unwrap_or(0.0),
            1.0 - yellow.unwrap_or(0.0),
        ];
        return ResolvedColour {
            rgb,
            level: 1.0,
            explicit: true,
        };
    }

    let mut accumulated = [0.0_f32; 3];
    let mut level = 0.0_f32;
    let mut add = |amount: Option<f32>, primary: [f32; 3]| {
        let Some(amount) = amount else { return };
        level = level.max(amount);
        for index in 0..3 {
            accumulated[index] += primary[index] * amount;
        }
    };
    add(read(&binding.red), [1.0, 0.0, 0.0]);
    add(read(&binding.green), [0.0, 1.0, 0.0]);
    add(read(&binding.blue), [0.0, 0.0, 1.0]);
    add(read(&binding.white), WHITE);
    add(read(&binding.amber), AMBER);
    add(read(&binding.ultraviolet), ULTRAVIOLET);
    add(read(&binding.cold_white), COLD_WHITE);
    add(read(&binding.warm_white), WARM_WHITE);

    if level <= 0.0
        && let Some(wheel) = &binding.wheel
    {
        let slots = frame(wheel.logical_universe);
        let colour = wheel_colour(wheel, &slots);
        return ResolvedColour {
            rgb: colour,
            level: 1.0,
            explicit: true,
        };
    }
    if level <= 0.0 {
        return ResolvedColour {
            rgb: WHITE,
            level: 0.0,
            explicit: true,
        };
    }
    // Normalise so the emitter mix describes hue while `level` carries brightness.
    let peak = accumulated[0]
        .max(accumulated[1])
        .max(accumulated[2])
        .max(1e-5);
    ResolvedColour {
        rgb: [
            accumulated[0] / peak,
            accumulated[1] / peak,
            accumulated[2] / peak,
        ],
        level,
        explicit: true,
    }
}

/// Colour for a fixed wheel slot, taken from the matched function's label.
fn wheel_colour(wheel: &crate::binding::ChannelRef, frame: &[u8; DMX_SLOTS]) -> [f32; 3] {
    let Some(function) = wheel.function(frame) else {
        return WHITE;
    };
    named_colour(&function.name)
}

/// Map a gel or wheel slot name onto a representative linear RGB.
///
/// Names come from the fixture profile, so this is a shared vocabulary, not a product list.
pub fn named_colour(name: &str) -> [f32; 3] {
    let normalised = name.trim().to_ascii_lowercase();
    let contains = |needle: &str| normalised.contains(needle);
    if contains("open") || contains("white") || contains("clear") {
        return WHITE;
    }
    if contains("red") {
        return [1.0, 0.06, 0.05];
    }
    if contains("deep blue") || contains("congo") {
        return [0.06, 0.08, 1.0];
    }
    if contains("blue") {
        return [0.1, 0.3, 1.0];
    }
    if contains("green") {
        return [0.1, 1.0, 0.2];
    }
    if contains("amber") || contains("orange") {
        return AMBER;
    }
    if contains("yellow") {
        return [1.0, 0.95, 0.15];
    }
    if contains("magenta") || contains("pink") {
        return [1.0, 0.15, 0.7];
    }
    if contains("cyan") || contains("aqua") {
        return [0.1, 0.95, 1.0];
    }
    if contains("uv") || contains("purple") || contains("violet") {
        return ULTRAVIOLET;
    }
    if contains("lavender") {
        return [0.72, 0.6, 1.0];
    }
    WHITE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::ChannelRef;

    fn channel(slot: u16) -> ChannelRef {
        ChannelRef {
            logical_universe: 1,
            slots: vec![slot],
            max_raw: 255,
            invert: false,
            physical_min: 0.0,
            physical_max: 1.0,
            snap: false,
            default_raw: 0,
            functions: Vec::new(),
        }
    }

    fn frame_with(values: &[(usize, u8)]) -> impl Fn(u16) -> [u8; DMX_SLOTS] + use<> {
        let mut slots = [0_u8; DMX_SLOTS];
        for (index, value) in values {
            slots[*index] = *value;
        }
        move |_| slots
    }

    #[test]
    fn an_empty_binding_renders_white_at_full() {
        let resolved = resolve(&ColourBinding::default(), &frame_with(&[]));
        assert_eq!(resolved.rgb, WHITE);
        assert!(!resolved.explicit);
    }

    #[test]
    fn additive_emitters_mix_into_a_normalised_hue_and_level() {
        let binding = ColourBinding {
            red: Some(channel(1)),
            green: Some(channel(2)),
            blue: Some(channel(3)),
            ..ColourBinding::default()
        };
        let resolved = resolve(&binding, &frame_with(&[(0, 255), (1, 0), (2, 0)]));
        assert_eq!(resolved.rgb, [1.0, 0.0, 0.0]);
        assert_eq!(resolved.level, 1.0);
        let half = resolve(&binding, &frame_with(&[(0, 128), (1, 128), (2, 128)]));
        assert!((half.rgb[0] - 1.0).abs() < 1e-5);
        assert!(half.level > 0.4 && half.level < 0.6);
    }

    #[test]
    fn a_white_emitter_contributes_to_every_primary() {
        let binding = ColourBinding {
            white: Some(channel(1)),
            ..ColourBinding::default()
        };
        let resolved = resolve(&binding, &frame_with(&[(0, 255)]));
        assert_eq!(resolved.rgb, WHITE);
    }

    #[test]
    fn subtractive_colour_removes_primaries() {
        let binding = ColourBinding {
            cyan: Some(channel(1)),
            magenta: Some(channel(2)),
            yellow: Some(channel(3)),
            ..ColourBinding::default()
        };
        let resolved = resolve(&binding, &frame_with(&[(0, 255), (1, 0), (2, 0)]));
        assert_eq!(resolved.rgb, [0.0, 1.0, 1.0]);
    }

    #[test]
    fn wheel_slot_names_map_to_representative_colours() {
        assert_eq!(named_colour("Open"), WHITE);
        assert_eq!(named_colour("Congo Blue")[2], 1.0);
        assert!(named_colour("Deep Red")[0] > 0.9);
        assert_eq!(named_colour("something unlisted"), WHITE);
    }
}
