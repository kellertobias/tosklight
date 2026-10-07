//! Pure reference-space conversions shared by virtual authoring and fixture adapters.
use crate::Xyz;

pub fn srgb_to_xyz(red: f32, green: f32, blue: f32) -> Xyz {
    let linear = |value: f32| {
        let value = value.clamp(0.0, 1.0);
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (linear(red), linear(green), linear(blue));
    Xyz {
        x: 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b,
        y: 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175 * b,
        z: 0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b,
    }
}

/// Linear sRGB/Rec.709 primaries of an XYZ value, unbounded: out-of-gamut components are
/// negative or above one. No transfer function is applied; the inverse of [`srgb_to_xyz`]'s
/// matrix stage.
pub fn xyz_to_linear_srgb(value: Xyz) -> [f64; 3] {
    let (x, y, z) = (f64::from(value.x), f64::from(value.y), f64::from(value.z));
    [
        3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z,
        -0.969_266 * x + 1.876_010_8 * y + 0.041_556 * z,
        0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z,
    ]
}

/// A bounded display approximation. The caller retains exact out-of-gamut XYZ separately.
pub fn xyz_to_srgb(value: Xyz) -> (f32, f32, f32) {
    let encode = |linear: f64| {
        let linear = linear.clamp(0.0, 1.0);
        if linear <= 0.003_130_8 {
            (12.92 * linear) as f32
        } else {
            (1.055 * linear.powf(1.0 / 2.4) - 0.055) as f32
        }
    };
    let [red, green, blue] = xyz_to_linear_srgb(value);
    (encode(red), encode(green), encode(blue))
}

/// Hue is in revolutions, saturation/value in 0..1, matching the existing picker conversion.
pub fn rgb_to_hsv(red: f32, green: f32, blue: f32) -> (f32, f32, f32) {
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let delta = maximum - minimum;
    let saturation = if maximum == 0.0 { 0.0 } else { delta / maximum };
    let hue = if delta == 0.0 {
        0.0
    } else if maximum == red {
        ((green - blue) / delta).rem_euclid(6.0) / 6.0
    } else if maximum == green {
        ((blue - red) / delta + 2.0) / 6.0
    } else {
        ((red - green) / delta + 4.0) / 6.0
    };
    (hue, saturation, maximum)
}
