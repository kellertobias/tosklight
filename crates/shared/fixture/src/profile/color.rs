use super::{ColorSystem, FixtureMode, ProfileError};
use crate::{ColorCalibration, EmitterCalibration, HighlightColor, mix_color};
use light_core::Xyz;
use light_core::{rgb_to_hsv, xyz_to_srgb};
use std::collections::HashMap;
use uuid::Uuid;

impl FixtureMode {
    /// Resolve one named operator Highlight color. An authored semantic wheel slot wins before
    /// calibrated mixing or the ordinary closest-measured-slot fallback.
    pub fn resolve_highlight_color(
        &self,
        head_id: Uuid,
        color: HighlightColor,
    ) -> Result<HashMap<Uuid, u32>, ProfileError> {
        if let Some(ColorSystem::DiscreteWheel { channel_id, slots }) = self
            .color_systems
            .iter()
            .find(|system| system.head_id == head_id)
            .map(|system| &system.system)
            && let Some(slot) = slots
                .iter()
                .find(|slot| semantic_slot_matches(&slot.semantic_id, color))
        {
            return Ok(HashMap::from([(
                *channel_id,
                slot.dmx_from + (slot.dmx_to - slot.dmx_from) / 2,
            )]));
        }
        self.resolve_color(head_id, color.to_xyz())
    }

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
                    output.insert(channel.id, normalized_raw(channel, drive));
                }
            }
            ColorSystem::Subtractive {
                cyan_channel_id,
                magenta_channel_id,
                yellow_channel_id,
                ..
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
                    output.insert(id, normalized_raw(channel, level));
                }
            }
            ColorSystem::HueSaturation {
                hue_channel_id,
                saturation_channel_id,
                intensity_channel_id,
            } => {
                let target = corrected_xyz(target, system.correction_matrix);
                let (red, green, blue) = xyz_to_srgb(target);
                let (hue, saturation, intensity) = rgb_to_hsv(red, green, blue);
                for (id, level) in [(*hue_channel_id, hue), (*saturation_channel_id, saturation)]
                    .into_iter()
                    .chain(intensity_channel_id.map(|id| (id, intensity)))
                {
                    let channel = self
                        .channels
                        .iter()
                        .find(|channel| channel.id == id)
                        .ok_or_else(|| {
                            ProfileError::Invalid(
                                "hue/saturation system references a missing channel".into(),
                            )
                        })?;
                    output.insert(id, normalized_raw(channel, level));
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

fn normalized_raw(channel: &super::FixtureChannel, level: f32) -> u32 {
    let max = channel.resolution.max_raw();
    let raw = (level.clamp(0.0, 1.0) * max as f32).round() as u32;
    if channel.invert {
        max.saturating_sub(raw)
    } else {
        raw
    }
}

fn corrected_xyz(value: Xyz, matrix: [[f32; 3]; 3]) -> Xyz {
    Xyz {
        x: matrix[0][0] * value.x + matrix[0][1] * value.y + matrix[0][2] * value.z,
        y: matrix[1][0] * value.x + matrix[1][1] * value.y + matrix[1][2] * value.z,
        z: matrix[2][0] * value.x + matrix[2][1] * value.y + matrix[2][2] * value.z,
    }
}

fn semantic_slot_matches(semantic_id: &str, color: HighlightColor) -> bool {
    let normalized = semantic_id
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_");
    match color {
        HighlightColor::White => matches!(
            normalized.as_str(),
            "white" | "open" | "clear" | "no_color" | "nocolor"
        ),
        HighlightColor::Red => normalized == "red",
        HighlightColor::Green => normalized == "green",
        HighlightColor::Blue => normalized == "blue",
        HighlightColor::Cyan => normalized == "cyan",
        HighlightColor::Magenta => normalized == "magenta",
        HighlightColor::Amber => normalized == "amber",
    }
}

pub(super) fn color_distance(left: Xyz, right: Xyz) -> f32 {
    (left.x - right.x).powi(2) + (left.y - right.y).powi(2) + (left.z - right.z).powi(2)
}
