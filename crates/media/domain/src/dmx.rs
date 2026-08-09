//! DMX value mappings.
//!
//! Every conversion between wire bytes and domain values lives here, so Art-Net, sACN, the HTTP
//! API, the React UI, the tests, and the GDTF export all agree by construction rather than by
//! three implementations happening to match.

/// Combines a big-endian coarse/fine pair into one 16-bit value.
pub const fn sixteen_bit(coarse: u8, fine: u8) -> u16 {
    u16::from_be_bytes([coarse, fine])
}

/// The 16-bit value a domain fraction of full scale corresponds to. The inverse of
/// [`sixteen_bit`] for round-tripping the UI and GDTF metadata.
pub const fn split(value: u16) -> (u8, u8) {
    let [coarse, fine] = value.to_be_bytes();
    (coarse, fine)
}

/// A byte as a `0.0..=1.0` fraction. Dimmer, volume, grayscale, mask opacity, and the effect
/// amounts all read this way.
pub fn unit(value: u8) -> f32 {
    f32::from(value) / 255.0
}

/// The exact 16-bit midpoint. Scale, position, rotation, and mask scale all put their neutral
/// value here, so a desk fader parked at half is neutral on every one of them.
pub const MIDPOINT: u16 = 32_768;

/// Maps a 16-bit value onto a piecewise-linear range with [`MIDPOINT`] pinned to `middle`.
///
/// Each half is linear on its own, which is what makes `32768` exactly neutral instead of
/// approximately neutral. A single linear map across the whole range cannot do that: it would
/// leave the neutral value between two adjacent DMX steps.
pub fn piecewise(value: u16, low: f32, middle: f32, high: f32) -> f32 {
    if value <= MIDPOINT {
        let fraction = f32::from(value) / f32::from(MIDPOINT);
        low + (middle - low) * fraction
    } else {
        let span = f32::from(u16::MAX - MIDPOINT);
        let fraction = f32::from(value - MIDPOINT) / span;
        middle + (high - middle) * fraction
    }
}

/// Layer scale: `0 → 0×`, `32768 → 1×`, `65535 → 10×`.
pub fn layer_scale(value: u16) -> f32 {
    piecewise(value, 0.0, 1.0, 10.0)
}

/// Mask scale: `0 → 0×`, `32768 → 1×`, `65535 → 2×`.
pub fn mask_scale(value: u16) -> f32 {
    piecewise(value, 0.0, 1.0, 2.0)
}

/// Layer position on one axis, in output half-widths: `-2.0` to `+2.0`, centered at the midpoint.
///
/// `0.0` is centered, `±1.0` puts the layer's center on an edge, and `±2.0` moves it a further
/// half-screen outside.
pub fn position(value: u16) -> f32 {
    piecewise(value, -2.0, 0.0, 2.0)
}

/// Layer rotation in degrees: `-360°` to `+360°`, with `0°` at the midpoint.
pub fn rotation(value: u16) -> f32 {
    piecewise(value, -360.0, 0.0, 360.0)
}

/// One subtractive tint component. DMX sends cyan, magenta, and yellow; the domain holds red,
/// green, and blue.
pub fn subtractive(value: u8) -> f32 {
    1.0 - unit(value)
}

/// The per-layer target tempo. Zero means off; every other value is that many beats per minute.
pub const fn playback_bpm(value: u8) -> Option<u8> {
    if value == 0 { None } else { Some(value) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f32, expected: f32) -> bool {
        (actual - expected).abs() < 1e-4
    }

    #[test]
    fn coarse_and_fine_bytes_are_big_endian() {
        assert_eq!(sixteen_bit(0x00, 0x00), 0);
        assert_eq!(sixteen_bit(0x80, 0x00), 32_768);
        assert_eq!(sixteen_bit(0xFF, 0xFF), 65_535);
        assert_eq!(sixteen_bit(0x12, 0x34), 0x1234);
        assert_eq!(split(0x1234), (0x12, 0x34));
    }

    #[test]
    fn a_byte_spans_zero_to_one_inclusive() {
        assert_eq!(unit(0), 0.0);
        assert_eq!(unit(255), 1.0);
        assert!(close(unit(128), 0.501_960_8));
    }

    #[test]
    fn layer_scale_pins_one_times_to_the_midpoint() {
        assert_eq!(layer_scale(0), 0.0);
        assert_eq!(layer_scale(MIDPOINT), 1.0);
        assert_eq!(layer_scale(u16::MAX), 10.0);
        assert!(close(layer_scale(MIDPOINT / 2), 0.5));
    }

    #[test]
    fn mask_scale_tops_out_at_two_times() {
        assert_eq!(mask_scale(0), 0.0);
        assert_eq!(mask_scale(MIDPOINT), 1.0);
        assert_eq!(mask_scale(u16::MAX), 2.0);
    }

    #[test]
    fn position_is_centered_at_the_midpoint() {
        assert_eq!(position(0), -2.0);
        assert_eq!(position(MIDPOINT), 0.0);
        assert_eq!(position(u16::MAX), 2.0);
        assert!(
            close(position(MIDPOINT / 2), -1.0),
            "quarter travel is one half-width out"
        );
    }

    #[test]
    fn rotation_spans_a_full_turn_each_way() {
        assert_eq!(rotation(0), -360.0);
        assert_eq!(rotation(MIDPOINT), 0.0);
        assert_eq!(rotation(u16::MAX), 360.0);
        assert!(close(rotation(MIDPOINT / 2), -180.0));
    }

    #[test]
    fn tint_arrives_subtractively() {
        assert_eq!(subtractive(0), 1.0, "no cyan is full red");
        assert_eq!(subtractive(255), 0.0, "full cyan removes all red");
    }

    #[test]
    fn a_zero_playback_bpm_byte_means_off() {
        assert_eq!(playback_bpm(0), None);
        assert_eq!(playback_bpm(1), Some(1));
        assert_eq!(playback_bpm(128), Some(128));
        assert_eq!(playback_bpm(255), Some(255));
    }
}
