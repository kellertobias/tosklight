//! Color Intent resolution: one device-independent target, driven as closely as each head's colour
//! engines allow.
//!
//! The target contract lives in [`light_core::color_intent`]. This module decides, per head, which
//! engine shows the colour and with which native values:
//!
//! 1. Continuous engines (additive emitters, CMY flags, hue/saturation) are solved first. Each
//!    reproduces the target's chromaticity at the brightest level it reaches; a colour outside its
//!    gamut is mapped to the nearest reproducible one by non-negative least squares.
//! 2. A fixed colour-wheel slot is used only when no continuous engine reaches the colour within
//!    [`APPROXIMATE_DELTA_UV`] and the slot is closer, or when the wheel is the head's only engine.
//!    Only steady slots take part; splits, scrolls, rotations, and effects never do.
//! 3. Every result carries a [`ColorResolutionQuality`] so an approximate, out-of-gamut,
//!    wheel-limited, uncalibrated, or unsupported colour is never presented as exact.
//!
//! A head without authored colour systems is resolved through systems inferred from its channel
//! attributes. Those carry no calibration, so their results always report
//! [`ColorResolutionQuality::Uncalibrated`].

use super::color_model::identifies_open_or_white;
use super::{
    CanonicalTransform, ChannelFunctionBehavior, ColorCalibrationStatus, ColorSystem,
    ColorSystemCalibration, ColorWheelSlot, EmitterBinding, FixtureChannel, FixtureMode,
    HeadColorSystem, SubtractiveCalibration,
};
use crate::srgb_to_xyz;
use light_core::color_intent::{
    APPROXIMATE_DELTA_UV, D65_WHITE, EXACT_DELTA_UV, delta_uv, normalized_chromaticity,
};
use light_core::{ColorResolutionQuality, Xyz};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use uuid::Uuid;

/// Which kind of engine produced a head's colour.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorIntentEngine {
    Additive,
    Subtractive,
    HueSaturation,
    Wheel,
}

/// The native values and the honesty report for one head's Color Intent.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorIntentResolution {
    /// Exact raw values per profile channel id.
    pub channels: HashMap<Uuid, u32>,
    pub quality: ColorResolutionQuality,
    pub engine: Option<ColorIntentEngine>,
    /// Chromaticity distance between the target and what the chosen engine is modelled to show.
    pub delta_uv: Option<f32>,
    /// The colour the chosen engine is modelled to show, at full intensity.
    pub achieved: Option<Xyz>,
    /// The chosen system's calibration revision; `None` for inferred systems.
    pub calibration_revision: Option<u32>,
}

impl ColorIntentResolution {
    fn unsupported() -> Self {
        Self {
            channels: HashMap::new(),
            quality: ColorResolutionQuality::Unsupported,
            engine: None,
            delta_uv: None,
            achieved: None,
            calibration_revision: None,
        }
    }
}

struct Candidate {
    channels: HashMap<Uuid, u32>,
    achieved: Xyz,
    delta: f32,
    engine: ColorIntentEngine,
    calibration: ColorSystemCalibration,
    authored: bool,
    /// Wheel only: the chosen slot's colour was measured rather than read from its name.
    measured_slot: bool,
}

impl FixtureMode {
    /// The colour systems Color Intent drives for one head: the authored ones, or systems inferred
    /// from the head's channel attributes when the profile authors none.
    pub fn intent_color_systems(&self, head_id: Uuid) -> Cow<'_, [HeadColorSystem]> {
        let authored = self
            .color_systems
            .iter()
            .filter(|system| system.head_id == head_id)
            .cloned()
            .collect::<Vec<_>>();
        if !authored.is_empty() {
            return Cow::Owned(authored);
        }
        Cow::Owned(self.inferred_color_systems(head_id))
    }

    /// Resolve a Color Intent target for one head. Deterministic: the same target and the same
    /// profile always produce the same values.
    pub fn resolve_intent(&self, head_id: Uuid, target: Xyz) -> ColorIntentResolution {
        let authored = self
            .color_systems
            .iter()
            .any(|system| system.head_id == head_id);
        let systems = self.intent_color_systems(head_id);
        if systems.is_empty() {
            return ColorIntentResolution::unsupported();
        }
        let target = normalized_chromaticity(target);
        let mut continuous: Option<Candidate> = None;
        let mut wheel: Option<Candidate> = None;
        for system in systems.iter() {
            let candidate = match &system.system {
                ColorSystem::DiscreteWheel { channel_id, slots } => {
                    self.wheel_candidate(*channel_id, slots, target)
                }
                _ => self.continuous_candidate(system, target),
            };
            let Some(mut candidate) = candidate else {
                continue;
            };
            candidate.calibration = system.calibration.clone();
            candidate.authored = authored;
            let slot = if candidate.engine == ColorIntentEngine::Wheel {
                &mut wheel
            } else {
                &mut continuous
            };
            if slot
                .as_ref()
                .is_none_or(|best| candidate.delta < best.delta - f32::EPSILON)
            {
                *slot = Some(candidate);
            }
        }
        let use_wheel = match (&continuous, &wheel) {
            (None, Some(_)) => true,
            (Some(continuous), Some(wheel)) => {
                continuous.delta > APPROXIMATE_DELTA_UV && wheel.delta < continuous.delta
            }
            _ => false,
        };
        let (chosen, other) = if use_wheel {
            (wheel, continuous.is_some())
        } else {
            (continuous, wheel.is_some())
        };
        let Some(chosen) = chosen else {
            return ColorIntentResolution::unsupported();
        };
        let mut channels = chosen.channels.clone();
        if other {
            // The engine not showing the colour must not tint it: park it open.
            for system in systems.iter() {
                let is_wheel = matches!(system.system, ColorSystem::DiscreteWheel { .. });
                if is_wheel == use_wheel {
                    continue;
                }
                for (channel, raw) in self.open_values(&system.system) {
                    channels.entry(channel).or_insert(raw);
                }
            }
        }
        ColorIntentResolution {
            channels,
            quality: classify(&chosen),
            engine: Some(chosen.engine),
            delta_uv: Some(chosen.delta),
            achieved: Some(chosen.achieved),
            calibration_revision: chosen.authored.then_some(chosen.calibration.revision),
        }
    }

    fn channel(&self, id: Uuid) -> Option<&FixtureChannel> {
        self.channels.iter().find(|channel| channel.id == id)
    }

    fn continuous_candidate(&self, system: &HeadColorSystem, target: Xyz) -> Option<Candidate> {
        let corrected = corrected(target, system.correction_matrix);
        match &system.system {
            ColorSystem::Additive { emitters } => {
                let visible = emitters
                    .iter()
                    .filter(|emitter| emitter.visible)
                    .collect::<Vec<_>>();
                if visible.is_empty() {
                    return None;
                }
                let basis = visible
                    .iter()
                    .map(|emitter| emitter.xyz)
                    .collect::<Vec<_>>();
                let limits = visible
                    .iter()
                    .map(|emitter| emitter.maximum_level.powf(emitter.response_curve))
                    .collect::<Vec<_>>();
                let levels = brightest_mix(&basis, &limits, corrected);
                let achieved = combine(&basis, &levels);
                let mut channels = HashMap::new();
                for (emitter, level) in visible.iter().zip(&levels) {
                    let channel = self.channel(emitter.channel_id)?;
                    let drive = level
                        .clamp(0.0, 1.0)
                        .powf(1.0 / emitter.response_curve)
                        .clamp(0.0, emitter.maximum_level);
                    channels.insert(channel.id, normalized_raw(channel, drive));
                }
                for emitter in emitters.iter().filter(|emitter| !emitter.visible) {
                    if let Some(channel) = self.channel(emitter.channel_id) {
                        channels.insert(channel.id, normalized_raw(channel, 0.0));
                    }
                }
                Some(Candidate {
                    channels,
                    delta: delta_uv(achieved, corrected),
                    achieved,
                    engine: ColorIntentEngine::Additive,
                    calibration: ColorSystemCalibration::default(),
                    authored: true,
                    measured_slot: false,
                })
            }
            ColorSystem::Subtractive {
                cyan_channel_id,
                magenta_channel_id,
                yellow_channel_id,
                filters,
            } => {
                let (flags, achieved) = solve_subtractive(filters.as_ref(), corrected);
                let mut channels = HashMap::new();
                for (id, level) in [
                    (*cyan_channel_id, flags[0]),
                    (*magenta_channel_id, flags[1]),
                    (*yellow_channel_id, flags[2]),
                ] {
                    let channel = self.channel(id)?;
                    channels.insert(id, normalized_raw(channel, level));
                }
                Some(Candidate {
                    channels,
                    delta: delta_uv(achieved, corrected),
                    achieved,
                    engine: ColorIntentEngine::Subtractive,
                    calibration: ColorSystemCalibration::default(),
                    authored: true,
                    measured_slot: false,
                })
            }
            ColorSystem::HueSaturation {
                hue_channel_id,
                saturation_channel_id,
                intensity_channel_id,
            } => {
                let linear = brightest_linear_srgb(corrected);
                let encoded = linear.map(encode_srgb);
                let (hue, saturation) = hue_saturation(encoded);
                let achieved = srgb_to_xyz(encoded[0], encoded[1], encoded[2]);
                let mut channels = HashMap::new();
                // Brightness is Intensity's: an HSI engine's own intensity stays full.
                for (id, level) in [(*hue_channel_id, hue), (*saturation_channel_id, saturation)]
                    .into_iter()
                    .chain(intensity_channel_id.map(|id| (id, 1.0)))
                {
                    let channel = self.channel(id)?;
                    channels.insert(id, normalized_raw(channel, level));
                }
                Some(Candidate {
                    channels,
                    delta: delta_uv(achieved, corrected),
                    achieved,
                    engine: ColorIntentEngine::HueSaturation,
                    calibration: ColorSystemCalibration::default(),
                    authored: true,
                    measured_slot: false,
                })
            }
            ColorSystem::DiscreteWheel { .. } => None,
        }
    }

    fn wheel_candidate(
        &self,
        channel_id: Uuid,
        slots: &[ColorWheelSlot],
        target: Xyz,
    ) -> Option<Candidate> {
        self.channel(channel_id)?;
        let (slot, color, delta) = slots
            .iter()
            .filter(|slot| slot.is_steady())
            .filter_map(|slot| {
                let color = slot.display_xyz()?;
                Some((slot, color, delta_uv(color, target)))
            })
            .fold(
                None::<(&ColorWheelSlot, Xyz, f32)>,
                |best, candidate| match best {
                    Some(best) if best.2 <= candidate.2 + f32::EPSILON => Some(best),
                    _ => Some(candidate),
                },
            )?;
        Some(Candidate {
            channels: HashMap::from([(channel_id, slot_center(slot))]),
            achieved: color,
            delta,
            engine: ColorIntentEngine::Wheel,
            calibration: ColorSystemCalibration::default(),
            authored: true,
            measured_slot: slot.measured_xyz.is_some(),
        })
    }

    /// Values that leave an engine transparent, used for the engine a hybrid head is not using.
    fn open_values(&self, system: &ColorSystem) -> Vec<(Uuid, u32)> {
        match system {
            ColorSystem::DiscreteWheel { channel_id, slots } => slots
                .iter()
                .find(|slot| {
                    identifies_open_or_white(&slot.label)
                        || identifies_open_or_white(&slot.semantic_id)
                })
                .map(|slot| vec![(*channel_id, slot_center(slot))])
                .unwrap_or_default(),
            ColorSystem::Subtractive {
                cyan_channel_id,
                magenta_channel_id,
                yellow_channel_id,
                ..
            } => [*cyan_channel_id, *magenta_channel_id, *yellow_channel_id]
                .into_iter()
                .filter_map(|id| Some((id, normalized_raw(self.channel(id)?, 0.0))))
                .collect(),
            ColorSystem::Additive { emitters } => {
                let basis = emitters
                    .iter()
                    .map(|emitter| emitter.xyz)
                    .collect::<Vec<_>>();
                let limits = emitters
                    .iter()
                    .map(|emitter| {
                        if emitter.visible {
                            emitter.maximum_level.powf(emitter.response_curve)
                        } else {
                            0.0
                        }
                    })
                    .collect::<Vec<_>>();
                let levels = brightest_mix(&basis, &limits, D65_WHITE);
                emitters
                    .iter()
                    .zip(levels)
                    .filter_map(|(emitter, level)| {
                        let channel = self.channel(emitter.channel_id)?;
                        let drive = level
                            .clamp(0.0, 1.0)
                            .powf(1.0 / emitter.response_curve)
                            .clamp(0.0, emitter.maximum_level);
                        Some((channel.id, normalized_raw(channel, drive)))
                    })
                    .collect()
            }
            ColorSystem::HueSaturation {
                saturation_channel_id,
                ..
            } => self
                .channel(*saturation_channel_id)
                .map(|channel| vec![(channel.id, normalized_raw(channel, 0.0))])
                .unwrap_or_default(),
        }
    }

    /// Systems read from channel attributes for a head the profile gives no colour system.
    fn inferred_color_systems(&self, head_id: Uuid) -> Vec<HeadColorSystem> {
        let channels = self
            .channels
            .iter()
            .filter(|channel| channel.head_id == head_id)
            .collect::<Vec<_>>();
        let uncalibrated = ColorSystemCalibration {
            status: ColorCalibrationStatus::Uncalibrated,
            revision: 0,
            source: None,
        };
        let identity = super::color_model::identity_color_correction();
        let mut systems = Vec::new();

        let emitters = channels
            .iter()
            .filter(|channel| channel.canonical_transform == CanonicalTransform::Identity)
            .filter_map(|channel| {
                let (name, xyz, visible) = nominal_emitter(&channel.attribute.0)?;
                Some(EmitterBinding {
                    channel_id: channel.id,
                    name: name.into(),
                    xyz,
                    maximum_level: 1.0,
                    response_curve: 1.0,
                    visible,
                })
            })
            .collect::<Vec<_>>();
        if emitters.iter().any(|emitter| emitter.visible) {
            systems.push(HeadColorSystem {
                head_id,
                correction_matrix: identity,
                system: ColorSystem::Additive { emitters },
                calibration: uncalibrated.clone(),
            });
        }

        let flag = |attribute: &str| {
            channels
                .iter()
                .find(|channel| {
                    channel.canonical_transform == CanonicalTransform::InvertNormalized
                        && &*channel.attribute.0 == attribute
                })
                .map(|channel| channel.id)
        };
        if let (Some(cyan), Some(magenta), Some(yellow)) =
            (flag("color.red"), flag("color.green"), flag("color.blue"))
        {
            systems.push(HeadColorSystem {
                head_id,
                correction_matrix: identity,
                system: ColorSystem::Subtractive {
                    cyan_channel_id: cyan,
                    magenta_channel_id: magenta,
                    yellow_channel_id: yellow,
                    filters: None,
                },
                calibration: uncalibrated.clone(),
            });
        }

        for channel in channels
            .iter()
            .filter(|channel| channel.attribute.0.starts_with("color.wheel"))
        {
            let slots = channel
                .functions
                .iter()
                .filter_map(|function| match &function.behavior {
                    ChannelFunctionBehavior::Fixed {
                        semantic_id, label, ..
                    }
                    | ChannelFunctionBehavior::Indexed {
                        semantic_id, label, ..
                    } => Some(ColorWheelSlot {
                        semantic_id: semantic_id.clone(),
                        label: if label.trim().is_empty() {
                            function.name.clone()
                        } else {
                            label.clone()
                        },
                        dmx_from: function.dmx_from,
                        dmx_to: function.dmx_to,
                        measured_xyz: None,
                        steady: None,
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if !slots.is_empty() {
                systems.push(HeadColorSystem {
                    head_id,
                    correction_matrix: identity,
                    system: ColorSystem::DiscreteWheel {
                        channel_id: channel.id,
                        slots,
                    },
                    calibration: uncalibrated.clone(),
                });
            }
        }
        systems
    }
}

impl ColorWheelSlot {
    /// Whether Color Intent may park the wheel in this slot to show a steady colour.
    pub fn is_steady(&self) -> bool {
        self.steady.unwrap_or_else(|| {
            let name = format!("{} {}", self.label, self.semantic_id).to_ascii_lowercase();
            !NON_STEADY_WORDS.iter().any(|word| name.contains(word))
        })
    }
}

/// Words that mark a wheel range as moving or mixed rather than one steady filter.
const NON_STEADY_WORDS: &[&str] = &[
    "split", "half", "scroll", "rotat", "spin", "rainbow", "effect", "random", "shake", "->", "→",
    "zone", "fade", "strobe", "flow", "cycle", "chase", "auto",
];

fn classify(candidate: &Candidate) -> ColorResolutionQuality {
    if candidate.calibration.status == ColorCalibrationStatus::Uncalibrated
        || (candidate.engine == ColorIntentEngine::Wheel && !candidate.measured_slot)
    {
        return ColorResolutionQuality::Uncalibrated;
    }
    let measured = candidate.calibration.status == ColorCalibrationStatus::Measured;
    if candidate.engine == ColorIntentEngine::Wheel {
        return if measured && candidate.delta <= EXACT_DELTA_UV {
            ColorResolutionQuality::Exact
        } else {
            ColorResolutionQuality::WheelLimited
        };
    }
    if candidate.delta > APPROXIMATE_DELTA_UV {
        ColorResolutionQuality::OutOfGamut
    } else if measured && candidate.delta <= EXACT_DELTA_UV {
        ColorResolutionQuality::Exact
    } else {
        ColorResolutionQuality::Approximate
    }
}

/// Typical emitter colour for an inferred additive channel, as (name, XYZ at full, visible).
/// Red, green, and blue are the sRGB primaries so an uncalibrated RGB fixture behaves like a
/// screen; the others are typical LED chromaticities.
fn nominal_emitter(attribute: &str) -> Option<(&'static str, Xyz, bool)> {
    let chromatic = |x: f32, y: f32, luminance: f32| Xyz {
        x: x / y * luminance,
        y: luminance,
        z: (1.0 - x - y) / y * luminance,
    };
    Some(match attribute {
        "color.red" => ("Red", srgb_to_xyz(1.0, 0.0, 0.0), true),
        "color.green" => ("Green", srgb_to_xyz(0.0, 1.0, 0.0), true),
        "color.blue" => ("Blue", srgb_to_xyz(0.0, 0.0, 1.0), true),
        "color.white" => ("White", D65_WHITE, true),
        "color.amber" => ("Amber", chromatic(0.575, 0.418, 0.6), true),
        "color.lime" => ("Lime", chromatic(0.405, 0.555, 0.8), true),
        "color.indigo" => ("Indigo", chromatic(0.165, 0.025, 0.03), true),
        "color.uv" => ("UV", chromatic(0.175, 0.005, 0.001), false),
        _ => return None,
    })
}

fn slot_center(slot: &ColorWheelSlot) -> u32 {
    slot.dmx_from + (slot.dmx_to - slot.dmx_from) / 2
}

fn normalized_raw(channel: &FixtureChannel, level: f32) -> u32 {
    let max = channel.resolution.max_raw();
    let raw = (f64::from(level.clamp(0.0, 1.0)) * f64::from(max)).round() as u32;
    if channel.invert {
        max.saturating_sub(raw)
    } else {
        raw
    }
}

fn corrected(value: Xyz, matrix: [[f32; 3]; 3]) -> Xyz {
    Xyz {
        x: matrix[0][0] * value.x + matrix[0][1] * value.y + matrix[0][2] * value.z,
        y: matrix[1][0] * value.x + matrix[1][1] * value.y + matrix[1][2] * value.z,
        z: matrix[2][0] * value.x + matrix[2][1] * value.y + matrix[2][2] * value.z,
    }
}

fn combine(basis: &[Xyz], levels: &[f32]) -> Xyz {
    basis.iter().zip(levels).fold(
        Xyz {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        |sum, (color, level)| Xyz {
            x: sum.x + color.x * level,
            y: sum.y + color.y * level,
            z: sum.z + color.z * level,
        },
    )
}

/// Emitter levels showing the target's chromaticity as brightly as the limits allow.
///
/// Non-negative least squares always has an optimum using at most three emitters, so every
/// subset of one to three emitters is solved exactly and the closest non-negative solution wins.
/// When several subsets reproduce the colour, their average is also exact and spreads the load,
/// so it is considered too. The winner is finally scaled until its first emitter reaches its limit.
fn brightest_mix(basis: &[Xyz], limits: &[f32], target: Xyz) -> Vec<f32> {
    let usable = (0..basis.len())
        .filter(|index| limits[*index] > 0.0)
        .collect::<Vec<_>>();
    let mut solutions: Vec<(Vec<f32>, f32)> = Vec::new();
    let mut consider = |subset: &[usize]| {
        let Some(coefficients) = least_squares(basis, subset, target) else {
            return;
        };
        if coefficients.iter().any(|value| *value < -1e-6) {
            return;
        }
        let mut levels = vec![0.0; basis.len()];
        for (index, value) in subset.iter().zip(coefficients) {
            levels[*index] = value.max(0.0);
        }
        let residual = residual(basis, &levels, target);
        solutions.push((levels, residual));
    };
    for (position, first) in usable.iter().enumerate() {
        consider(&[*first]);
        for (offset, second) in usable[position + 1..].iter().enumerate() {
            consider(&[*first, *second]);
            for third in &usable[position + offset + 2..] {
                consider(&[*first, *second, *third]);
            }
        }
    }
    let Some(best_residual) = solutions
        .iter()
        .map(|(_, residual)| *residual)
        .min_by(f32::total_cmp)
    else {
        return vec![0.0; basis.len()];
    };
    let tolerance = best_residual + 1e-5 * (1.0 + norm(target));
    let exact = solutions
        .iter()
        .filter(|(_, residual)| *residual <= tolerance)
        .map(|(levels, _)| levels.clone())
        .collect::<Vec<_>>();
    let mut candidates = exact.clone();
    if exact.len() > 1 {
        let mut average = vec![0.0; basis.len()];
        for levels in &exact {
            for (sum, level) in average.iter_mut().zip(levels) {
                *sum += level / exact.len() as f32;
            }
        }
        candidates.push(average);
    }
    candidates
        .into_iter()
        .map(|levels| scale_to_limits(levels, limits))
        .fold(None::<(Vec<f32>, f32)>, |best, levels| {
            let brightness = combine(basis, &levels).y;
            match best {
                Some(best) if best.1 >= brightness - 1e-6 => Some(best),
                _ => Some((levels, brightness)),
            }
        })
        .map(|(levels, _)| levels)
        .unwrap_or_else(|| vec![0.0; basis.len()])
}

fn scale_to_limits(levels: Vec<f32>, limits: &[f32]) -> Vec<f32> {
    let ratio = levels
        .iter()
        .zip(limits)
        .filter(|(_, limit)| **limit > 0.0)
        .map(|(level, limit)| level / limit)
        .fold(0.0_f32, f32::max);
    if ratio <= 1e-9 {
        return levels;
    }
    levels.into_iter().map(|level| level / ratio).collect()
}

fn residual(basis: &[Xyz], levels: &[f32], target: Xyz) -> f32 {
    let produced = combine(basis, levels);
    norm(Xyz {
        x: produced.x - target.x,
        y: produced.y - target.y,
        z: produced.z - target.z,
    })
}

fn norm(value: Xyz) -> f32 {
    (value.x.powi(2) + value.y.powi(2) + value.z.powi(2)).sqrt()
}

fn dot(left: Xyz, right: Xyz) -> f32 {
    left.x * right.x + left.y * right.y + left.z * right.z
}

/// Least-squares coefficients for the chosen emitters through their normal equations.
fn least_squares(basis: &[Xyz], subset: &[usize], target: Xyz) -> Option<Vec<f32>> {
    let size = subset.len();
    let mut matrix = vec![vec![0.0_f64; size + 1]; size];
    for (row, left) in subset.iter().enumerate() {
        for (column, right) in subset.iter().enumerate() {
            matrix[row][column] = f64::from(dot(basis[*left], basis[*right]));
        }
        matrix[row][size] = f64::from(dot(basis[*left], target));
    }
    // Gaussian elimination with partial pivoting on at most a 3×3 system.
    for pivot in 0..size {
        let best = (pivot..size)
            .max_by(|a, b| matrix[*a][pivot].abs().total_cmp(&matrix[*b][pivot].abs()))?;
        if matrix[best][pivot].abs() < 1e-10 {
            return None;
        }
        matrix.swap(pivot, best);
        for row in 0..size {
            if row == pivot {
                continue;
            }
            let factor = matrix[row][pivot] / matrix[pivot][pivot];
            for column in pivot..=size {
                matrix[row][column] -= factor * matrix[pivot][column];
            }
        }
    }
    Some(
        (0..size)
            .map(|row| (matrix[row][size] / matrix[row][row]) as f32)
            .collect(),
    )
}

const XYZ_TO_LINEAR_SRGB: [[f32; 3]; 3] = [
    [3.240_454_2, -1.537_138_5, -0.498_531_4],
    [-0.969_266, 1.876_010_8, 0.041_556],
    [0.055_643_4, -0.204_025_9, 1.057_225_2],
];

const LINEAR_SRGB_TO_XYZ: [[f32; 3]; 3] = [
    [0.412_456_4, 0.357_576_1, 0.180_437_5],
    [0.212_672_9, 0.715_152_2, 0.072_175],
    [0.019_333_9, 0.119_192, 0.950_304_1],
];

fn to_linear_srgb(value: Xyz) -> [f32; 3] {
    let v = corrected(value, XYZ_TO_LINEAR_SRGB);
    [v.x, v.y, v.z]
}

fn from_linear_srgb(value: [f32; 3]) -> Xyz {
    corrected(
        Xyz {
            x: value[0],
            y: value[1],
            z: value[2],
        },
        LINEAR_SRGB_TO_XYZ,
    )
}

/// The target's colour in linear sRGB, clipped into gamut and scaled so its largest primary is
/// full.
fn brightest_linear_srgb(target: Xyz) -> [f32; 3] {
    let linear = to_linear_srgb(target).map(|component| component.max(0.0));
    let maximum = linear.iter().copied().fold(0.0_f32, f32::max);
    if maximum <= 1e-9 {
        return [1.0, 1.0, 1.0];
    }
    linear.map(|component| component / maximum)
}

fn encode_srgb(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn hue_saturation([red, green, blue]: [f32; 3]) -> (f32, f32) {
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let delta = maximum - minimum;
    let saturation = if maximum <= 0.0 { 0.0 } else { delta / maximum };
    let hue = if delta == 0.0 {
        0.0
    } else if maximum == red {
        ((green - blue) / delta).rem_euclid(6.0) / 6.0
    } else if maximum == green {
        ((blue - red) / delta + 2.0) / 6.0
    } else {
        ((red - green) / delta + 4.0) / 6.0
    };
    (hue, saturation)
}

/// CMY flag levels for the target and the colour they are modelled to show.
///
/// The open beam and each flag's full-in output define a per-primary transmission in linear sRGB;
/// the flags multiply. The wanted colour is the target scaled as bright as the open beam allows,
/// and the flags are solved one at a time, each exactly, until they settle.
fn solve_subtractive(filters: Option<&SubtractiveCalibration>, target: Xyz) -> ([f32; 3], Xyz) {
    let (open, transmissions) = match filters {
        Some(filters) => {
            let open = to_linear_srgb(filters.open_xyz).map(|value| value.max(1e-6));
            let transmission = |full: Xyz| {
                let full = to_linear_srgb(full);
                [0, 1, 2].map(|index| (full[index] / open[index]).clamp(0.0, 1.0))
            };
            (
                open,
                [
                    transmission(filters.cyan_xyz),
                    transmission(filters.magenta_xyz),
                    transmission(filters.yellow_xyz),
                ],
            )
        }
        None => (
            [1.0, 1.0, 1.0],
            [[0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 0.0]],
        ),
    };
    let wanted_direction = to_linear_srgb(target).map(|value| value.max(0.0));
    let scale = (0..3)
        .filter(|index| wanted_direction[*index] > 1e-9)
        .map(|index| open[index] / wanted_direction[index])
        .fold(f32::INFINITY, f32::min);
    let wanted = if scale.is_finite() {
        wanted_direction.map(|value| value * scale)
    } else {
        open
    };
    let attenuation = transmissions.map(|transmission| transmission.map(|value| 1.0 - value));
    let output = |flags: &[f32; 3]| {
        [0, 1, 2].map(|primary| {
            open[primary]
                * (0..3)
                    .map(|flag| 1.0 - flags[flag] * attenuation[flag][primary])
                    .product::<f32>()
        })
    };
    // Start from the ideal inverse: each flag removes the primary it complements.
    let mut flags = [0, 1, 2].map(|index| (1.0 - wanted[index] / open[index]).clamp(0.0, 1.0));
    for _ in 0..32 {
        for flag in 0..3 {
            let others = [0, 1, 2].map(|primary| {
                open[primary]
                    * (0..3)
                        .filter(|other| *other != flag)
                        .map(|other| 1.0 - flags[other] * attenuation[other][primary])
                        .product::<f32>()
            });
            let (numerator, denominator) =
                (0..3).fold((0.0_f32, 0.0_f32), |(numerator, denominator), primary| {
                    let slope = others[primary] * attenuation[flag][primary];
                    (
                        numerator + slope * (others[primary] - wanted[primary]),
                        denominator + slope * slope,
                    )
                });
            if denominator > 1e-12 {
                flags[flag] = (numerator / denominator).clamp(0.0, 1.0);
            }
        }
    }
    let achieved = from_linear_srgb(output(&flags));
    (flags, achieved)
}
