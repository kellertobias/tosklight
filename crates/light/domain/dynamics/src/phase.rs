use crate::{PhaseDistribution, PhaseOrdering, RankedSelection};
use light_core::FixtureId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct SpatialPosition {
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhasePosition {
    pub target: FixtureId,
    pub degrees: f32,
}

pub fn project_phase(
    distribution: &PhaseDistribution,
    targets: &[FixtureId],
    positions: &HashMap<FixtureId, SpatialPosition>,
    loop_index: u64,
) -> Vec<PhasePosition> {
    let mut ordered = targets.iter().copied().enumerate().collect::<Vec<_>>();
    match distribution.ordering {
        PhaseOrdering::Selection => {}
        PhaseOrdering::GridLinear { angle_degrees } => {
            let direction = direction_cosines(angle_degrees);
            ordered.sort_by(|left, right| {
                projected(positions.get(&left.1), direction)
                    .total_cmp(&projected(positions.get(&right.1), direction))
                    .then_with(|| left.0.cmp(&right.0))
            });
        }
        PhaseOrdering::RadialOut { center_x, center_z }
        | PhaseOrdering::RadialIn { center_x, center_z } => {
            let inward = matches!(distribution.ordering, PhaseOrdering::RadialIn { .. });
            ordered.sort_by(|left, right| {
                let left_position = positions.get(&left.1);
                let right_position = positions.get(&right.1);
                let positioned_first = right_position.is_some().cmp(&left_position.is_some());
                let ordering = distance(left_position, center_x, center_z).total_cmp(&distance(
                    right_position,
                    center_x,
                    center_z,
                ));
                positioned_first
                    .then(if inward { ordering.reverse() } else { ordering })
                    .then_with(|| left.0.cmp(&right.0))
            });
        }
        PhaseOrdering::Axial { center_x, center_z } => {
            ordered.sort_by(|left, right| {
                angle(positions.get(&left.1), center_x, center_z)
                    .total_cmp(&angle(positions.get(&right.1), center_x, center_z))
                    .then_with(|| left.0.cmp(&right.0))
            });
        }
        PhaseOrdering::RandomEachLoop { seed } => {
            ordered.sort_by_key(|(_, target)| deterministic_key(seed, loop_index, *target));
        }
    }

    let ranked = spatial_ranks(&distribution.ordering, &ordered, positions);
    let block = usize::from(distribution.block_size.max(1));
    let rank_count = ranked
        .last()
        .map_or(0, |(_, rank)| rank.saturating_add(1))
        .div_ceil(block);
    let repeats = usize::from(distribution.repeats.max(1)).min(rank_count.max(1));
    ranked
        .into_iter()
        .map(|((_, target), spatial_rank)| {
            let rank = spatial_rank / block;
            let (_, local, length) = balanced_repeat(rank, rank_count, repeats);
            let local = if distribution.wings {
                local.min(length.saturating_sub(1).saturating_sub(local))
            } else {
                local
            };
            let effective_length = if distribution.wings {
                length.div_ceil(2)
            } else {
                length
            };
            let distributed = if distribution.anchors_degrees.len() >= 2 {
                anchor_phase(&distribution.anchors_degrees, local, effective_length)
            } else if effective_length <= 1 {
                0.0
            } else {
                local as f32 / effective_length as f32 * distribution.span_degrees
            };
            PhasePosition {
                target,
                degrees: distribution.offset_degrees + distributed,
            }
        })
        .collect()
}

/// Projects an authoritative spatial ranking into phase degrees.
///
/// Equal spatial ranks receive identical phase. Blocks, repeats, wings, and anchors operate on
/// `ranked.rank_count`, not on the number of fixtures in the selection.
pub fn project_ranked_phase(
    distribution: &PhaseDistribution,
    ranked: &RankedSelection,
) -> Vec<PhasePosition> {
    let block = usize::from(distribution.block_size.max(1));
    let rank_count = ranked.rank_count.div_ceil(block);
    let repeats = usize::from(distribution.repeats.max(1)).min(rank_count.max(1));
    ranked
        .ordered_fixture_ids
        .iter()
        .filter_map(|target| {
            let spatial_rank = ranked.rank_by_fixture.get(target).copied()?;
            let rank = spatial_rank / block;
            let (_, local, length) = balanced_repeat(rank, rank_count, repeats);
            let local = if distribution.wings {
                local.min(length.saturating_sub(1).saturating_sub(local))
            } else {
                local
            };
            let effective_length = if distribution.wings {
                length.div_ceil(2)
            } else {
                length
            };
            let distributed = if distribution.anchors_degrees.len() >= 2 {
                anchor_phase(&distribution.anchors_degrees, local, effective_length)
            } else if effective_length <= 1 {
                0.0
            } else {
                local as f32 / effective_length as f32 * distribution.span_degrees
            };
            Some(PhasePosition {
                target: *target,
                degrees: distribution.offset_degrees + distributed,
            })
        })
        .collect()
}

fn spatial_ranks(
    ordering: &PhaseOrdering,
    ordered: &[(usize, FixtureId)],
    positions: &HashMap<FixtureId, SpatialPosition>,
) -> Vec<((usize, FixtureId), usize)> {
    let spatial = !matches!(
        ordering,
        PhaseOrdering::Selection | PhaseOrdering::RandomEachLoop { .. }
    );
    if !spatial {
        return ordered
            .iter()
            .copied()
            .enumerate()
            .map(|(ordered_index, item)| (item, ordered_index))
            .collect();
    }
    let keys = ordered
        .iter()
        .map(|item| spatial_key(ordering, positions.get(&item.1)))
        .collect::<Vec<_>>();
    // One grid line, one rank. Compared with a tolerance rather than for exact equality: the keys
    // come from floating-point positions, so two fixtures on the same line agree to within
    // rounding rather than bit for bit.
    let tolerance = key_tolerance(keys.iter().copied().flatten());
    let mut rank = 0usize;
    let mut previous_key: Option<f32> = None;
    ordered
        .iter()
        .copied()
        .zip(keys)
        .enumerate()
        .map(|(ordered_index, (item, key))| {
            let apart = match (key, previous_key) {
                (Some(key), Some(previous)) => (key - previous).abs() > tolerance,
                // A fixture with no position stands on no line, and neither does the next one.
                _ => true,
            };
            if ordered_index > 0 && apart {
                rank += 1;
            }
            previous_key = key;
            (item, rank)
        })
        .collect()
}

/// How far apart two keys must be to count as different grid lines; see `spatial::key_tolerance`.
fn key_tolerance(keys: impl Iterator<Item = f32>) -> f32 {
    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    for key in keys {
        if key < low {
            low = key;
        }
        if key > high {
            high = key;
        }
    }
    let spread = if low.is_finite() && high.is_finite() {
        high - low
    } else {
        0.0
    };
    (spread * 1e-4).max(1e-6)
}

fn spatial_key(ordering: &PhaseOrdering, position: Option<&SpatialPosition>) -> Option<f32> {
    let position = position?;
    let value = match *ordering {
        PhaseOrdering::GridLinear { angle_degrees } => {
            let (cosine, sine) = direction_cosines(angle_degrees);
            position.x * cosine + position.z * sine
        }
        PhaseOrdering::RadialOut { center_x, center_z }
        | PhaseOrdering::RadialIn { center_x, center_z } => {
            (position.x - center_x).hypot(position.z - center_z)
        }
        PhaseOrdering::Axial { center_x, center_z } => {
            (position.z - center_z).atan2(position.x - center_x)
        }
        PhaseOrdering::Selection | PhaseOrdering::RandomEachLoop { .. } => return None,
    };
    Some(value)
}

/// The cosine and sine of an angle in degrees, exact on the quarter turns.
///
/// `90f32.to_radians().cos()` is -4.4e-8, not zero, which is enough to split a row that should
/// share one phase. See `spatial::direction_cosines`.
fn direction_cosines(angle_degrees: f32) -> (f32, f32) {
    match angle_degrees.rem_euclid(360.0) {
        a if a == 0.0 => (1.0, 0.0),
        a if a == 90.0 => (0.0, 1.0),
        a if a == 180.0 => (-1.0, 0.0),
        a if a == 270.0 => (0.0, -1.0),
        a => {
            let radians = a.to_radians();
            (radians.cos(), radians.sin())
        }
    }
}

fn projected(position: Option<&SpatialPosition>, direction: (f32, f32)) -> f32 {
    position
        .map(|position| position.x * direction.0 + position.z * direction.1)
        .unwrap_or(f32::INFINITY)
}

fn distance(position: Option<&SpatialPosition>, x: f32, z: f32) -> f32 {
    position
        .map(|position| (position.x - x).hypot(position.z - z))
        .unwrap_or(f32::INFINITY)
}

fn angle(position: Option<&SpatialPosition>, x: f32, z: f32) -> f32 {
    position
        .map(|position| (position.z - z).atan2(position.x - x))
        .unwrap_or(f32::INFINITY)
}

fn balanced_repeat(rank: usize, count: usize, repeats: usize) -> (usize, usize, usize) {
    let base = count / repeats;
    let extras = count % repeats;
    let mut start = 0;
    for repeat in 0..repeats {
        let length = base + usize::from(repeat < extras);
        if rank < start + length {
            return (repeat, rank - start, length);
        }
        start += length;
    }
    (0, 0, count.max(1))
}

fn anchor_phase(anchors: &[f32], index: usize, count: usize) -> f32 {
    if count <= 1 {
        return anchors[0];
    }
    let segment_count = anchors.len() - 1;
    let base = count / segment_count;
    let extras = count % segment_count;
    let mut start = 0;
    for segment in 0..segment_count {
        let length = base + usize::from(segment < extras);
        if index < start + length {
            let local = index - start;
            let mix = if segment + 1 == segment_count {
                (local + 1) as f32 / length.max(1) as f32
            } else {
                local as f32 / length.max(1) as f32
            };
            return anchors[segment] + (anchors[segment + 1] - anchors[segment]) * mix;
        }
        start += length;
    }
    *anchors.last().unwrap_or(&anchors[0])
}

fn deterministic_key(seed: u64, loop_index: u64, target: FixtureId) -> u64 {
    let target = target.0.as_u128();
    let folded_target = target as u64 ^ (target >> 64) as u64;
    let mut value = seed ^ folded_target ^ loop_index.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
