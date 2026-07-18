use super::{ColorSystem, FixtureMode, ProfileError};
use crate::{ColorCalibration, EmitterCalibration, mix_color};
use light_core::Xyz;
use std::collections::HashMap;
use uuid::Uuid;

impl FixtureMode {
    /// Resolve an abstract XYZ color through the configured head system. Additive calibration uses
    /// bounded non-negative optimization; missing calibration falls back deterministically to RGB
    /// or CMY. UV/non-visible emitters are excluded unless directly programmed.
    pub fn resolve_color(
        &self,
        head_id: Uuid,
        target: Xyz,
    ) -> Result<HashMap<Uuid, u32>, ProfileError> {
        let Some(system) = self
            .color_systems
            .iter()
            .find(|system| system.head_id == head_id)
        else {
            return Ok(HashMap::new());
        };
        let mut output = HashMap::new();
        match &system.system {
            ColorSystem::Additive { emitters } => {
                let visible = emitters
                    .iter()
                    .filter(|emitter| emitter.visible)
                    .collect::<Vec<_>>();
                let levels = if visible.len() >= 3 {
                    let calibration = ColorCalibration {
                        emitters: visible
                            .iter()
                            .map(|emitter| EmitterCalibration {
                                name: emitter.name.clone(),
                                xyz: emitter.xyz,
                                // Optimization happens in emitted-light space. The configured
                                // maximum is a drive limit, so convert it through the response
                                // curve before constraining the optical solution.
                                limit: emitter.maximum_level.powf(emitter.response_curve),
                            })
                            .collect(),
                        correction_matrix: system.correction_matrix,
                    };
                    mix_color(target, &calibration)
                        .map_err(|error| ProfileError::Invalid(error.to_string()))?
                } else {
                    let rgb = xyz_to_srgb(target);
                    visible
                        .iter()
                        .map(|emitter| {
                            let name = emitter.name.to_ascii_lowercase();
                            if name.contains("red") {
                                rgb.0
                            } else if name.contains("green") {
                                rgb.1
                            } else if name.contains("blue") {
                                rgb.2
                            } else if name.contains("white") {
                                rgb.0.min(rgb.1).min(rgb.2)
                            } else {
                                0.0
                            }
                        })
                        .collect()
                };
                for (emitter, level) in visible.into_iter().zip(levels) {
                    let channel = self
                        .channels
                        .iter()
                        .find(|channel| channel.id == emitter.channel_id)
                        .ok_or_else(|| {
                            ProfileError::Invalid("emitter references a missing channel".into())
                        })?;
                    // The optimizer/fallback yields an emitted-light level. Apply the inverse
                    // response curve to obtain the deterministic DMX drive value, retaining the
                    // configured maximum drive as the final bound.
                    let drive = level
                        .clamp(0.0, 1.0)
                        .powf(1.0 / emitter.response_curve)
                        .clamp(0.0, emitter.maximum_level);
                    let max = channel.resolution.max_raw();
                    let raw = (drive * max as f32).round() as u32;
                    output.insert(
                        channel.id,
                        if channel.invert {
                            max.saturating_sub(raw)
                        } else {
                            raw
                        },
                    );
                }
            }
            ColorSystem::Subtractive {
                cyan_channel_id,
                magenta_channel_id,
                yellow_channel_id,
            } => {
                let (red, green, blue) = xyz_to_srgb(target);
                for (id, level) in [
                    (*cyan_channel_id, 1.0 - red),
                    (*magenta_channel_id, 1.0 - green),
                    (*yellow_channel_id, 1.0 - blue),
                ] {
                    let channel = self
                        .channels
                        .iter()
                        .find(|channel| channel.id == id)
                        .ok_or_else(|| {
                            ProfileError::Invalid("CMY system references a missing channel".into())
                        })?;
                    let max = channel.resolution.max_raw();
                    let raw = (level.clamp(0.0, 1.0) * max as f32).round() as u32;
                    output.insert(
                        id,
                        if channel.invert {
                            max.saturating_sub(raw)
                        } else {
                            raw
                        },
                    );
                }
            }
            ColorSystem::DiscreteWheel { channel_id, slots } => {
                if let Some(slot) = slots
                    .iter()
                    .filter_map(|slot| {
                        slot.measured_xyz
                            .map(|xyz| (slot, color_distance(target, xyz)))
                    })
                    .min_by(|left, right| left.1.total_cmp(&right.1))
                    .map(|(slot, _)| slot)
                {
                    output.insert(
                        *channel_id,
                        slot.dmx_from + (slot.dmx_to - slot.dmx_from) / 2,
                    );
                }
            }
        }
        Ok(output)
    }
}

fn xyz_to_srgb(value: Xyz) -> (f32, f32, f32) {
    let linear = (
        3.240_454_2 * value.x - 1.537_138_5 * value.y - 0.498_531_4 * value.z,
        -0.969_266 * value.x + 1.876_010_8 * value.y + 0.041_556 * value.z,
        0.055_643_4 * value.x - 0.204_025_9 * value.y + 1.057_225_2 * value.z,
    );
    let encode = |value: f32| {
        let value = value.max(0.0);
        if value <= 0.003_130_8 {
            12.92 * value
        } else {
            1.055 * value.powf(1.0 / 2.4) - 0.055
        }
    };
    (
        encode(linear.0).clamp(0.0, 1.0),
        encode(linear.1).clamp(0.0, 1.0),
        encode(linear.2).clamp(0.0, 1.0),
    )
}

pub(super) fn color_distance(left: Xyz, right: Xyz) -> f32 {
    (left.x - right.x).powi(2) + (left.y - right.y).powi(2) + (left.z - right.z).powi(2)
}
