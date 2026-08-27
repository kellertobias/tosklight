//! Deterministic, budgeted depth-tested emissive particles.

use super::{FrameInstances, FrameStyle, MeshInstance, MeshKind};
use glam::{Mat4, Vec3};
use viz_scene::{ParticleFamily, ParticleTrigger, Scene, SceneValues};

#[cfg(test)]
pub const BUDGET_DRAFT: usize = 128;
#[cfg(test)]
pub const BUDGET_STANDARD: usize = 512;
#[cfg(test)]
pub const BUDGET_HIGH: usize = 2_048;
#[cfg(test)]
pub const BUDGET_ULTRA: usize = 8_192;

pub(super) fn push_effects(
    frame: &mut FrameInstances,
    scene: &Scene,
    values: &SceneValues,
    style: &FrameStyle,
) {
    let budget = style.effect_particle_budget;
    let mut sources = Vec::new();
    for (index, effect_frame) in values.effect_frames.iter().enumerate() {
        let Some(emitter_instance) = scene.emitters.get(index) else {
            continue;
        };
        let Some(fixture) = scene.fixtures.get(emitter_instance.fixture_index as usize) else {
            continue;
        };
        for (source_index, source) in effect_frame.emitters.iter().enumerate() {
            if source.trigger == ParticleTrigger::Off
                || source.intensity <= 0.0
                || source.density <= 0.0
            {
                continue;
            }
            let shape = match source.family {
                ParticleFamily::Flame => 96.0,
                ParticleFamily::Spark => 220.0,
                ParticleFamily::Debris => 0.0,
            };
            let requested = (shape * source.density * source.intensity).ceil() as usize;
            frame.particles_requested = frame.particles_requested.saturating_add(requested as u32);
            if requested > 0 {
                sources.push((index, source_index, fixture, source, requested));
            }
        }
    }
    if sources.is_empty() {
        return;
    }
    let allocations = allocate(
        &sources.iter().map(|source| source.4).collect::<Vec<_>>(),
        budget,
    );
    for ((emitter_index, source_index, fixture, source, _), drawn) in
        sources.into_iter().zip(allocations)
    {
        let (position, orientation) = fixture.placed_by(&values.position_points);
        let origin = position + orientation * Vec3::from(source.origin);
        let direction = (orientation * Vec3::from(source.direction)).normalize_or_zero();
        let base_seed = hash64(
            fixture.instance_id.as_bytes(),
            emitter_index as u64 ^ ((source_index as u64) << 32),
        );
        for particle in 0..drawn {
            let (phase, lateral) = sample(
                base_seed,
                particle,
                values.effect_frames[emitter_index].timeline_seconds,
                source.lifetime_seconds,
            );
            let spread = lateral * source.width_metres * (1.0 - phase * 0.65);
            let position =
                origin + direction * (source.reach_metres * phase) + orientation * spread;
            let size = match source.family {
                ParticleFamily::Flame => source.width_metres * (0.12 + 0.18 * (1.0 - phase)),
                ParticleFamily::Spark => 0.012 + source.width_metres * 0.025,
                ParticleFamily::Debris => continue,
            };
            let colour = Vec3::from(source.colour)
                * source.intensity
                * match source.family {
                    ParticleFamily::Flame => 3.4,
                    ParticleFamily::Spark => 5.0,
                    ParticleFamily::Debris => 0.0,
                }
                * (1.0 - phase * 0.65);
            frame.mesh(MeshKind::Sphere).push(MeshInstance::new(
                Mat4::from_scale_rotation_translation(
                    Vec3::splat(size),
                    glam::Quat::IDENTITY,
                    position,
                ),
                colour * 0.12,
                0.35,
                colour,
                0.0,
            ));
            frame.particles_drawn = frame.particles_drawn.saturating_add(1);
        }
    }
}

fn splitmix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E3779B97F4A7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
    value ^ (value >> 31)
}
fn hash64(bytes: &[u8], mut hash: u64) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001B3);
    }
    splitmix(hash)
}
fn unit(value: u64) -> f32 {
    (value as f64 / u64::MAX as f64) as f32 * 2.0 - 1.0
}
fn sample(seed: u64, particle: usize, timeline_seconds: f32, lifetime_seconds: f32) -> (f32, Vec3) {
    let random = splitmix(seed ^ particle as u64);
    let phase =
        ((random as f64 / u64::MAX as f64) as f32 + timeline_seconds / lifetime_seconds).fract();
    (
        phase,
        Vec3::new(
            unit(random.rotate_left(17)),
            0.0,
            unit(random.rotate_left(41)),
        ),
    )
}

fn allocate(requested: &[usize], budget: usize) -> Vec<usize> {
    let mut allocations = vec![0usize; requested.len()];
    for allocation in allocations.iter_mut().take(budget.min(requested.len())) {
        *allocation = 1;
    }
    let mut used: usize = allocations.iter().sum();
    while used < budget {
        let mut changed = false;
        for (slot, wanted) in requested.iter().enumerate() {
            if used >= budget {
                break;
            }
            if allocations[slot] < *wanted {
                allocations[slot] += 1;
                used += 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    allocations
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documented_quality_budgets_are_ordered_and_bounded() {
        assert!(BUDGET_DRAFT < BUDGET_STANDARD);
        assert!(BUDGET_STANDARD < BUDGET_HIGH);
        assert!(BUDGET_HIGH < BUDGET_ULTRA);
        assert_eq!(BUDGET_HIGH, 2_048);
        assert_eq!(BUDGET_ULTRA, 8_192);
    }
    #[test]
    fn overload_keeps_every_nozzle_before_filling_the_remaining_budget() {
        let requested = vec![220; 64];
        let high = allocate(&requested, BUDGET_HIGH);
        let ultra = allocate(&requested, BUDGET_ULTRA);
        assert_eq!(high.iter().sum::<usize>(), BUDGET_HIGH);
        assert_eq!(ultra.iter().sum::<usize>(), BUDGET_ULTRA);
        assert!(high.iter().all(|drawn| *drawn > 0));
        assert!(ultra.iter().all(|drawn| *drawn > 0));
        assert_eq!(
            high,
            allocate(&requested, BUDGET_HIGH),
            "subset selection is stable"
        );
    }
    #[test]
    fn fixed_identity_particle_and_timeline_produce_the_same_capture_geometry() {
        let seed = hash64(&[1, 7, 9, 33], 42);
        let first: Vec<_> = (0..256)
            .map(|particle| sample(seed, particle, 12.5, 2.25))
            .collect();
        let second: Vec<_> = (0..256)
            .map(|particle| sample(seed, particle, 12.5, 2.25))
            .collect();
        assert_eq!(first, second);
        assert_ne!(
            first,
            (0..256)
                .map(|particle| sample(seed ^ 1, particle, 12.5, 2.25))
                .collect::<Vec<_>>()
        );
    }
}
