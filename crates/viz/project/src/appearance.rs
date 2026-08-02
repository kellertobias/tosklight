//! Deterministic installed-source and gel colour transforms for the visualizer.

use light_fixture::{FixtureProfile, GelAssignment, InstalledFixtureAppearance};

/// Lowest installed colour temperature accepted by the patch contract.
pub const MIN_COLOUR_TEMPERATURE_KELVIN: u32 = 1_000;
/// Highest installed colour temperature accepted by the patch contract.
pub const MAX_COLOUR_TEMPERATURE_KELVIN: u32 = 25_000;

const NEUTRAL: [f32; 3] = [1.0; 3];

/// Approximate a correlated colour temperature as linear RGB.
///
/// The input is clamped to the patch contract's 1000--25000 K range. The approximation is
/// evaluated in encoded sRGB and then decoded to linear RGB so it can be composed with live
/// renderer colour without mixing colour spaces.
pub fn colour_temperature_linear_rgb(kelvin: u32) -> [f32; 3] {
    let temperature =
        kelvin.clamp(MIN_COLOUR_TEMPERATURE_KELVIN, MAX_COLOUR_TEMPERATURE_KELVIN) as f32 / 100.0;

    let red = if temperature <= 66.0 {
        255.0
    } else {
        329.698_73 * (temperature - 60.0).powf(-0.133_204_76)
    };
    let green = if temperature <= 66.0 {
        99.470_8 * temperature.ln() - 161.119_57
    } else {
        288.122_16 * (temperature - 60.0).powf(-0.075_514_846)
    };
    let blue = if temperature >= 66.0 {
        255.0
    } else if temperature <= 19.0 {
        0.0
    } else {
        138.517_73 * (temperature - 10.0).ln() - 305.044_8
    };

    [red, green, blue].map(|channel| srgb_channel_to_linear(channel.clamp(0.0, 255.0) / 255.0))
}

/// Parse canonical CSS-style `#RRGGBB` encoded sRGB and decode it to linear RGB.
///
/// Only canonical uppercase hexadecimal digits are accepted. Shorthand, alpha, missing `#`, and
/// surrounding whitespace are rejected so callers cannot silently reinterpret persisted data.
pub fn parse_srgb_hex_linear(value: &str) -> Option<[f32; 3]> {
    let bytes = value.as_bytes();
    if bytes.len() != 7 || bytes[0] != b'#' {
        return None;
    }
    let channel = |offset| {
        let high = hex_digit(bytes[offset])?;
        let low = hex_digit(bytes[offset + 1])?;
        Some(srgb_channel_to_linear(f32::from(high * 16 + low) / 255.0))
    };
    Some([channel(1)?, channel(3)?, channel(5)?])
}

/// Apply installed-source temperature and gel transmission to a live linear renderer colour.
///
/// `None` represents a profile-default source or open-white gel. Every multiplication happens in
/// linear space and deliberately remains unnormalised: appearance filters may only preserve or
/// remove live light, never introduce it.
pub fn apply_installed_appearance(
    live_linear_rgb: [f32; 3],
    colour_temperature_kelvin: Option<u32>,
    gel_visualizer_linear_rgb: Option<[f32; 3]>,
) -> [f32; 3] {
    let temperature = colour_temperature_kelvin
        .map(colour_temperature_linear_rgb)
        .unwrap_or(NEUTRAL);
    let gel = gel_visualizer_linear_rgb.unwrap_or(NEUTRAL);
    std::array::from_fn(|index| live_linear_rgb[index] * temperature[index] * gel[index])
}

/// Resolve the portable installed appearance into the one linear multiplier every renderer uses.
///
/// An explicit installed temperature wins. Otherwise the selected immutable profile revision is
/// the fallback, including for an explicitly named source: the patch validator only requires a
/// separate CCT when neither place can supply one. Gel assignments use their embedded portable
/// visualizer colour, never a renderer-local catalog lookup.
pub fn installed_appearance_linear_rgb(
    profile: &FixtureProfile,
    appearance: &InstalledFixtureAppearance,
) -> [f32; 3] {
    let colour_temperature_kelvin = appearance.color_temperature_kelvin.or_else(|| {
        profile
            .physical
            .color_temperature_kelvin
            .filter(|kelvin| kelvin.is_finite())
            .map(|kelvin| kelvin.round())
            .filter(|kelvin| *kelvin >= 0.0 && *kelvin <= u32::MAX as f32)
            .map(|kelvin| kelvin as u32)
    });
    let gel_visualizer_linear_rgb = match &appearance.gel {
        GelAssignment::OpenWhite => None,
        GelAssignment::BuiltIn {
            embedded_fallback, ..
        } => parse_srgb_hex_linear(&embedded_fallback.visualizer_srgb),
        GelAssignment::Custom { color_srgb, .. } => parse_srgb_hex_linear(color_srgb),
    };
    apply_installed_appearance(
        NEUTRAL,
        colour_temperature_kelvin,
        gel_visualizer_linear_rgb,
    )
}

fn srgb_channel_to_linear(channel: f32) -> f32 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn assert_rgb(actual: [f32; 3], expected: [f32; 3]) {
        for index in 0..3 {
            assert!(
                (actual[index] - expected[index]).abs() <= EPSILON,
                "channel {index}: expected {}, got {}",
                expected[index],
                actual[index]
            );
        }
    }

    #[test]
    fn warm_and_cool_temperatures_have_stable_linear_vectors() {
        assert_rgb(
            colour_temperature_linear_rgb(3_200),
            [1.0, 0.477_115, 0.198_484],
        );
        assert_rgb(
            colour_temperature_linear_rgb(10_000),
            [0.588_681, 0.701_615, 1.0],
        );
    }

    #[test]
    fn temperature_is_clamped_to_the_supported_contract_range() {
        assert_eq!(
            colour_temperature_linear_rgb(0),
            colour_temperature_linear_rgb(MIN_COLOUR_TEMPERATURE_KELVIN)
        );
        assert_eq!(
            colour_temperature_linear_rgb(u32::MAX),
            colour_temperature_linear_rgb(MAX_COLOUR_TEMPERATURE_KELVIN)
        );
    }

    #[test]
    fn canonical_srgb_hex_decodes_to_linear_rgb() {
        assert_rgb(
            parse_srgb_hex_linear("#80A0FF").expect("canonical colour"),
            [0.215_861, 0.351_533, 1.0],
        );
        assert_eq!(parse_srgb_hex_linear("#FF0000"), Some([1.0, 0.0, 0.0]));
        assert_eq!(parse_srgb_hex_linear("#ff0000"), None);
        assert_eq!(parse_srgb_hex_linear("#FFF"), None);
        assert_eq!(parse_srgb_hex_linear(" #FFFFFF"), None);
        assert_eq!(parse_srgb_hex_linear("FFFFFF"), None);
        assert_eq!(parse_srgb_hex_linear("#GG0000"), None);
    }

    #[test]
    fn open_white_leaves_live_linear_colour_unchanged() {
        let live = [0.25, 0.5, 0.75];
        assert_eq!(apply_installed_appearance(live, None, None), live);
    }

    #[test]
    fn red_gel_and_warm_source_multiply_in_linear_space() {
        let live = [0.8, 0.6, 0.4];
        let red_gel = parse_srgb_hex_linear("#C01020").expect("red gel");
        assert_rgb(
            apply_installed_appearance(live, Some(3_200), Some(red_gel)),
            [0.421_692, 0.001_483, 0.001_147],
        );
    }

    #[test]
    fn installed_appearance_cannot_create_light_from_zero_live_colour() {
        let red_gel = parse_srgb_hex_linear("#FF0000").expect("red gel");
        assert_eq!(
            apply_installed_appearance([0.0; 3], Some(25_000), Some(red_gel)),
            [0.0; 3]
        );
    }

    #[test]
    fn portable_appearance_uses_profile_cct_and_embedded_gel_fallback() {
        let mut profile = FixtureProfile::blank();
        profile.physical.color_temperature_kelvin = Some(3_200.0);
        let appearance = InstalledFixtureAppearance {
            gel: GelAssignment::BuiltIn {
                catalog_id: "generic".into(),
                entry_id: "red".into(),
                embedded_fallback: light_fixture::GelDefinitionSnapshot {
                    number: "R".into(),
                    name: "Red".into(),
                    display_srgb: "#FF0000".into(),
                    visualizer_srgb: "#C01020".into(),
                },
            },
            ..InstalledFixtureAppearance::default()
        };
        assert_rgb(
            installed_appearance_linear_rgb(&profile, &appearance),
            apply_installed_appearance(NEUTRAL, Some(3_200), parse_srgb_hex_linear("#C01020")),
        );
    }

    #[test]
    fn explicit_temperature_overrides_the_profile_default() {
        let mut profile = FixtureProfile::blank();
        profile.physical.color_temperature_kelvin = Some(3_200.0);
        let appearance = InstalledFixtureAppearance {
            color_temperature_kelvin: Some(10_000),
            ..InstalledFixtureAppearance::default()
        };
        assert_rgb(
            installed_appearance_linear_rgb(&profile, &appearance),
            colour_temperature_linear_rgb(10_000),
        );
    }
}
