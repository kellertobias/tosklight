use crate::{ByteOrder, ColorCalibration, DmxCurve, FixtureError, Parameter};
use light_core::{DmxAddress, Xyz};

pub fn encode_parameter(
    frame: &mut [u8; 512],
    base: DmxAddress,
    parameter: &Parameter,
    value: f32,
) -> Result<(), FixtureError> {
    if base == 0 || base > 512 {
        return Err(FixtureError::Invalid(
            "DMX addresses are 1-based and must be within 1-512".into(),
        ));
    }
    let mut value = value.clamp(0.0, 1.0);
    if parameter.metadata.invert {
        value = 1.0 - value;
    }
    value = match parameter.metadata.curve {
        DmxCurve::Linear => value,
        DmxCurve::Square => value * value,
        DmxCurve::SquareRoot => value.sqrt(),
        DmxCurve::SmoothStep => value * value * (3.0 - 2.0 * value),
    };
    let bytes = parameter.components.len();
    if !(1..=4).contains(&bytes) {
        return Err(FixtureError::Invalid(
            "parameters require 1-4 channel components".into(),
        ));
    }
    let max = (1_u64 << (bytes * 8)) - 1;
    let encoded = (value * max as f32).round() as u64;
    for (index, component) in parameter.components.iter().enumerate() {
        let shift = match component.byte_order {
            ByteOrder::MsbFirst => 8 * (bytes - index - 1),
            ByteOrder::LsbFirst => 8 * index,
        };
        let slot = usize::from(base - 1) + usize::from(component.offset);
        if slot >= 512 {
            return Err(FixtureError::Invalid(
                "encoded parameter exceeds universe".into(),
            ));
        }
        frame[slot] = ((encoded >> shift) & 0xff) as u8;
    }
    Ok(())
}

pub fn apply_virtual_dimmer(channels: &mut [f32], emitter_indices: &[usize], intensity: f32) {
    let intensity = intensity.clamp(0.0, 1.0);
    for index in emitter_indices {
        if let Some(channel) = channels.get_mut(*index) {
            *channel = (*channel * intensity).clamp(0.0, 1.0);
        }
    }
}

pub fn srgb_to_xyz(red: f32, green: f32, blue: f32) -> Xyz {
    let linear = |value: f32| {
        let value = value.clamp(0.0, 1.0);
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let r = linear(red);
    let g = linear(green);
    let b = linear(blue);
    Xyz {
        x: 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b,
        y: 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175 * b,
        z: 0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b,
    }
}

/// Finds bounded emitter levels using projected gradient descent. This supports arbitrary RGBW/A/UV
/// emitter sets without assuming that extra emitters are merely white-channel extraction.
pub fn mix_color(target: Xyz, calibration: &ColorCalibration) -> Result<Vec<f32>, FixtureError> {
    if calibration.emitters.is_empty() {
        return Err(FixtureError::Invalid(
            "color calibration has no emitters".into(),
        ));
    }
    let target = multiply_matrix(calibration.correction_matrix, target);
    let mut levels = vec![0.0_f32; calibration.emitters.len()];
    let norm = calibration
        .emitters
        .iter()
        .map(|emitter| emitter.xyz.x.powi(2) + emitter.xyz.y.powi(2) + emitter.xyz.z.powi(2))
        .sum::<f32>()
        .max(0.001);
    let rate = 0.8 / norm;
    for _ in 0..256 {
        let produced = calibration.emitters.iter().zip(&levels).fold(
            Xyz {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            |sum, (emitter, level)| Xyz {
                x: sum.x + emitter.xyz.x * level,
                y: sum.y + emitter.xyz.y * level,
                z: sum.z + emitter.xyz.z * level,
            },
        );
        let error = Xyz {
            x: produced.x - target.x,
            y: produced.y - target.y,
            z: produced.z - target.z,
        };
        for (level, emitter) in levels.iter_mut().zip(&calibration.emitters) {
            let gradient =
                2.0 * (error.x * emitter.xyz.x + error.y * emitter.xyz.y + error.z * emitter.xyz.z);
            *level = (*level - rate * gradient).clamp(0.0, emitter.limit);
        }
    }
    Ok(levels)
}

fn multiply_matrix(matrix: [[f32; 3]; 3], value: Xyz) -> Xyz {
    Xyz {
        x: matrix[0][0] * value.x + matrix[0][1] * value.y + matrix[0][2] * value.z,
        y: matrix[1][0] * value.x + matrix[1][1] * value.y + matrix[1][2] * value.z,
        z: matrix[2][0] * value.x + matrix[2][1] * value.y + matrix[2][2] * value.z,
    }
}
