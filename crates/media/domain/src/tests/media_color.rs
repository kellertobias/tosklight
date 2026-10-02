//! Deterministic CPU reference cases for Media White Blend and tint (TL-569).
//!
//! The color stage decodes an sRGB-encoded source once, mixes it toward its Rec.709 luminance by
//! White Blend, multiplies by the linear tint, and encodes once. These cases pin that formula from
//! the DMX wire through [`MediaColor`], so the renderer's shader has a fixed reference to agree
//! with. Tolerances are stated per assertion; the transfer functions are exact to 1e-5.

use crate::color::{MediaColor, Tint, linear_to_srgb, srgb_to_linear};
use crate::layer::LayerState;
use crate::master::MasterState;
use crate::personality::channels::{layer, master};
use crate::personality::decode::{layer_state, master_state};
use crate::personality::{LAYER_SLOTS, MASTER_SLOTS};

const EPSILON: f32 = 1e-5;

fn close(actual: f32, expected: f32, tolerance: f32) -> bool {
    (actual - expected).abs() <= tolerance
}

fn assert_rgb(actual: [f32; 3], expected: [f32; 3], tolerance: f32, context: &str) {
    for (channel, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            close(*actual, expected, tolerance),
            "{context}: channel {channel} is {actual}, expected {expected}"
        );
    }
}

fn byte(value: u8) -> f32 {
    f32::from(value) / 255.0
}

fn layer_slots(cyan: u8, magenta: u8, yellow: u8, grayscale: u8) -> [u8; LAYER_SLOTS as usize] {
    let mut slots = [0u8; LAYER_SLOTS as usize];
    slots[layer::DIMMER] = 255;
    slots[layer::CYAN] = cyan;
    slots[layer::MAGENTA] = magenta;
    slots[layer::YELLOW] = yellow;
    slots[layer::GRAYSCALE] = grayscale;
    slots
}

#[test]
fn the_srgb_transfer_functions_are_exact_inverses_over_every_byte() {
    for value in 0..=255u8 {
        let encoded = byte(value);
        let round_trip = linear_to_srgb(srgb_to_linear(encoded));
        assert!(
            close(round_trip, encoded, EPSILON),
            "byte {value}: {round_trip} after a round trip"
        );
    }
    assert_eq!(srgb_to_linear(0.0), 0.0);
    assert!(close(srgb_to_linear(1.0), 1.0, EPSILON));
    assert!(close(srgb_to_linear(0.5), 0.214_041, EPSILON));
    assert!(close(linear_to_srgb(0.5), 0.735_357, EPSILON));
    assert_eq!(srgb_to_linear(-0.25), 0.0, "negative input is black");
}

#[test]
fn decoded_layer_controls_become_the_media_color_without_inventing_a_colour() {
    let decoded = layer_state(&layer_slots(0, 128, 255, 128));
    let color = MediaColor::of_layer(&decoded);
    assert_eq!(color.tint, Tint::new(1.0, 1.0 - byte(128), 0.0));
    assert_eq!(color.white_blend, Some(byte(128)));

    // A layer the desk has not touched is neutral: white tint, no White Blend.
    let untouched = MediaColor::of_layer(&layer_state(&layer_slots(0, 0, 0, 0)));
    assert_eq!(untouched.tint, Tint::WHITE);
    assert_eq!(untouched.white_blend, Some(0.0));
    assert_eq!(MediaColor::of_layer(&LayerState::default()), untouched);
}

#[test]
fn the_master_has_no_white_blend_control_rather_than_an_authored_zero() {
    let mut slots = [0u8; MASTER_SLOTS as usize];
    slots[master::DIMMER] = 255;
    slots[master::CYAN] = 255;
    let color = MediaColor::of_master(&master_state(&slots));
    assert_eq!(color.tint, Tint::new(0.0, 1.0, 1.0));
    assert_eq!(color.white_blend, None, "unsupported, not 0%");
    assert_eq!(color.white_blend_amount(), 0.0, "and it renders unchanged");
    assert_eq!(
        MediaColor::of_master(&MasterState::default()).tint,
        Tint::WHITE
    );
}

#[test]
fn zero_white_blend_with_a_white_tint_leaves_every_source_byte_unchanged() {
    let neutral = MediaColor::of_layer(&LayerState::default());
    for value in (0..=255u8).step_by(5) {
        let encoded = [byte(value), byte(255 - value), byte(value / 2)];
        assert_rgb(neutral.apply_encoded(encoded), encoded, EPSILON, "bypass");
    }
}

#[test]
fn full_white_blend_desaturates_in_linear_light_not_gamma_space() {
    let color = MediaColor {
        tint: Tint::WHITE,
        white_blend: Some(1.0),
    };
    // Encoded pure red: linear luminance 0.2126 encodes to 0.4984 (127/255). The legacy gamma
    // space formula produced 0.299 (76/255), visibly darker.
    let gray = color.apply_encoded([1.0, 0.0, 0.0]);
    assert_rgb(gray, [0.498_440; 3], EPSILON, "red at 100%");

    // A mid-grey stays exactly itself, and every result is neutral.
    let mid = color.apply_encoded([0.5, 0.5, 0.5]);
    assert_rgb(mid, [0.5; 3], EPSILON, "grey at 100%");
    let orange = color.apply_encoded([1.0, byte(128), 0.0]);
    assert!(close(orange[0], orange[1], EPSILON) && close(orange[1], orange[2], EPSILON));
}

#[test]
fn intermediate_white_blend_lies_between_the_source_and_its_grayscale() {
    let source = [1.0, 0.0, 0.0];
    let half = MediaColor {
        tint: Tint::WHITE,
        white_blend: Some(0.5),
    }
    .apply_encoded(source);
    // Linear red 1.0 → 0.6063, green/blue 0 → 0.1063, then encoded once.
    assert_rgb(
        half,
        [
            linear_to_srgb(0.6063),
            linear_to_srgb(0.1063),
            linear_to_srgb(0.1063),
        ],
        EPSILON,
        "red at 50%",
    );
    assert!(half[0] < 1.0 && half[0] > 0.498_440);
    assert!(half[1] > 0.0 && half[1] < 0.498_440);
}

#[test]
fn the_tint_stays_active_at_full_white_blend() {
    let red_tint = MediaColor {
        tint: Tint::new(1.0, 0.0, 0.0),
        white_blend: Some(1.0),
    };
    // A red tint on a fully desaturated image is a red image, not a white or grey one.
    let from_green = red_tint.apply_encoded([0.0, 1.0, 0.0]);
    assert_rgb(
        from_green,
        [linear_to_srgb(0.7152), 0.0, 0.0],
        EPSILON,
        "green source, red tint",
    );
    let from_white = red_tint.apply_encoded([1.0, 1.0, 1.0]);
    assert_rgb(
        from_white,
        [1.0, 0.0, 0.0],
        EPSILON,
        "white source, red tint",
    );

    // A fractional tint multiplies linear light: half the light, not half the code value.
    let half_tint = MediaColor {
        tint: Tint::new(0.5, 0.5, 0.5),
        white_blend: Some(1.0),
    };
    assert_rgb(
        half_tint.apply_encoded([1.0, 1.0, 1.0]),
        [linear_to_srgb(0.5); 3],
        EPSILON,
        "half tint",
    );
}

#[test]
fn black_stays_black_whatever_the_tint_and_white_blend() {
    for white_blend in [0.0, 0.5, 1.0] {
        for tint in [
            Tint::WHITE,
            Tint::new(1.0, 0.0, 0.0),
            Tint::new(0.2, 0.9, 0.4),
        ] {
            let color = MediaColor {
                tint,
                white_blend: Some(white_blend),
            };
            assert_eq!(color.apply_encoded([0.0; 3]), [0.0; 3]);
        }
    }
    // Full cyan, magenta and yellow on the wire is a black tint: every source goes black.
    let black_tint = MediaColor::of_layer(&layer_state(&layer_slots(255, 255, 255, 128)));
    assert_eq!(black_tint.apply_encoded([1.0, 0.5, 0.25]), [0.0; 3]);
}

#[test]
fn white_blend_leaves_dimmer_and_alpha_to_their_own_controls() {
    let decoded = LayerState {
        dimmer: 0.4,
        grayscale: 1.0,
        tint: Tint::new(0.0, 1.0, 0.0),
        ..LayerState::default()
    };
    let color = MediaColor::of_layer(&decoded);
    // The color stage returns only RGB; it has no input for dimmer or alpha and cannot scale them.
    let rgb = color.apply_encoded([1.0, 1.0, 1.0]);
    assert_rgb(
        rgb,
        [0.0, 1.0, 0.0],
        EPSILON,
        "dimmer does not darken the color stage",
    );
    assert_eq!(decoded.dimmer, 0.4);
}

#[test]
fn out_of_range_white_blend_is_clamped_and_not_a_number_is_ignored() {
    let over = MediaColor {
        tint: Tint::WHITE,
        white_blend: Some(3.0),
    };
    let full = MediaColor {
        white_blend: Some(1.0),
        ..over
    };
    assert_eq!(over.white_blend_amount(), 1.0);
    assert_eq!(
        over.apply_encoded([1.0, 0.0, 0.0]),
        full.apply_encoded([1.0, 0.0, 0.0])
    );
    let nan = MediaColor {
        white_blend: Some(f32::NAN),
        ..over
    };
    assert_eq!(nan.white_blend_amount(), 0.0);
    assert_eq!(
        MediaColor {
            white_blend: Some(-1.0),
            ..over
        }
        .white_blend_amount(),
        0.0
    );
}
