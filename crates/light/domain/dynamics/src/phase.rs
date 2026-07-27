use crate::{PhaseDistribution, PhaseOrdering};
use light_core::FixtureId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct SpatialPosition {
    pub x: f32,
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
            let radians = angle_degrees.to_radians();
            let direction = (radians.cos(), radians.sin());
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
                let left_distance = distance(positions.get(&left.1), center_x, center_z);
                let right_distance = distance(positions.get(&right.1), center_x, center_z);
                let ordering = left_distance.total_cmp(&right_distance);
                (if inward { ordering.reverse() } else { ordering })
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

fn spatial_ranks(
    ordering: &PhaseOrdering,
    ordered: &[(usize, FixtureId)],
    positions: &HashMap<FixtureId, SpatialPosition>,
) -> Vec<((usize, FixtureId), usize)> {
    let spatial = !matches!(
        ordering,
        PhaseOrdering::Selection | PhaseOrdering::RandomEachLoop { .. }
    );
    let mut rank = 0usize;
    let mut previous_key = None;
    ordered
        .iter()
        .copied()
        .enumerate()
        .map(|(ordered_index, item)| {
            if !spatial {
                return (item, ordered_index);
            }
            let key = spatial_key(ordering, positions.get(&item.1));
            if ordered_index > 0 && (key.is_none() || key != previous_key) {
                rank += 1;
            }
            previous_key = key;
            (item, rank)
        })
        .collect()
}

fn spatial_key(ordering: &PhaseOrdering, position: Option<&SpatialPosition>) -> Option<(u32, u32)> {
    let position = position?;
    let value = match *ordering {
        PhaseOrdering::GridLinear { angle_degrees } => {
            let radians = angle_degrees.to_radians();
            position.x * radians.cos() + position.z * radians.sin()
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
    Some((value.to_bits(), 0))
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
    let mut value = seed ^ loop_index.rotate_left(17);
    for byte in target.0.as_bytes() {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x100_0000_01b3);
    }
    value
}
