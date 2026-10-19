//! Conservative per-light curtain candidates, packed for the fragment shader.
//!
//! The square frustum encloses the circular field cone. Rejecting a rectangle requires all
//! four corners to lie outside the same plane, so intersecting or enclosing cloth stays in.

use crate::instances::{GpuCurtainBlocker, GpuLight};
use bytemuck::Zeroable;
use glam::{Mat4, Vec3};

const PLANE_EPSILON: f32 = 0.001;

struct Frustum {
    apex: Vec3,
    planes: [(Vec3, f32); 6],
}

impl Frustum {
    fn for_light(light: &GpuLight) -> Option<Self> {
        let origin = Vec3::from_slice(&light.position_range[..3]);
        let direction = Vec3::from_slice(&light.direction_cos_outer[..3]);
        let cosine = light.direction_cos_outer[3];
        let offset = light.params[3];
        let range = light.position_range[3];
        // Near-hemispherical and invalid fields fall back to all curtains. In particular, never
        // clamp an obtuse cone into an acute frustum, which would incorrectly reject cloth.
        if !origin.is_finite()
            || !direction.is_finite()
            || direction.length_squared() < 1e-12
            || !cosine.is_finite()
            || !(0.001..=1.0).contains(&cosine)
            || !offset.is_finite()
            || !range.is_finite()
            || range < 0.0
        {
            return None;
        }
        let axis = direction.normalize();
        let offset = offset.max(0.0);
        let apex = origin - axis * offset;
        let reach = range + offset;
        let tangent = cosine.acos().tan();
        if !apex.is_finite() || !reach.is_finite() || !tangent.is_finite() {
            return None;
        }
        let reference = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let right = axis.cross(reference).normalize();
        let up = right.cross(axis).normalize();
        Some(Self {
            apex,
            planes: [
                ((right - axis * tangent).normalize(), 0.0),
                ((-right - axis * tangent).normalize(), 0.0),
                ((up - axis * tangent).normalize(), 0.0),
                ((-up - axis * tangent).normalize(), 0.0),
                (-axis, 0.0),
                (axis, reach),
            ],
        })
    }

    fn may_intersect(&self, corners: &[Vec3; 4]) -> bool {
        if corners.iter().any(|corner| !corner.is_finite()) {
            return true;
        }
        !self.planes.iter().any(|(normal, distance)| {
            corners
                .iter()
                .all(|corner| normal.dot(*corner - self.apex) - distance > PLANE_EPSILON)
        })
    }
}

fn world_corners(blocker: &GpuCurtainBlocker) -> [Vec3; 4] {
    let local_to_world = Mat4::from_cols_array_2d(&blocker.world_to_local).inverse();
    let [half_width, half_height, ..] = blocker.half_size;
    [
        Vec3::new(-half_width, -half_height, 0.0),
        Vec3::new(half_width, -half_height, 0.0),
        Vec3::new(half_width, half_height, 0.0),
        Vec3::new(-half_width, half_height, 0.0),
    ]
    .map(|corner| local_to_world.transform_point3(corner))
}

/// Each light's gate.w addresses one header record followed by its candidate rectangles.
/// The header has a zero matrix and half_size.z = candidate count. A gate of -1 means none.
/// Offsets count 80-byte records, matching the shader's storage array indexing.
pub(super) fn pack(
    lights: &mut [GpuLight],
    blockers: &[GpuCurtainBlocker],
) -> Vec<GpuCurtainBlocker> {
    let corners: Vec<_> = blockers.iter().map(world_corners).collect();
    let mut packed = Vec::new();
    for light in lights {
        light.gate[3] = -1.0;
        let frustum = Frustum::for_light(light);
        let header_index = packed.len();
        for (blocker, corners) in blockers.iter().zip(&corners) {
            if frustum
                .as_ref()
                .is_some_and(|frustum| !frustum.may_intersect(corners))
            {
                continue;
            }
            if packed.len() == header_index {
                packed.push(GpuCurtainBlocker::zeroed());
            }
            packed.push(*blocker);
        }
        if packed.len() > header_index {
            packed[header_index].half_size[2] = (packed.len() - header_index - 1) as f32;
            light.gate[3] = header_index as f32;
        }
    }
    packed
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    fn light() -> GpuLight {
        GpuLight {
            position_range: [0.0, 0.0, 0.0, 10.0],
            direction_cos_outer: [0.0, 0.0, 1.0, 0.95],
            ..GpuLight::zeroed()
        }
    }

    fn curtain(centre: Vec3, half_size: [f32; 2], rotation: Quat) -> GpuCurtainBlocker {
        GpuCurtainBlocker {
            world_to_local: Mat4::from_rotation_translation(rotation, centre)
                .inverse()
                .to_cols_array_2d(),
            half_size: [half_size[0], half_size[1], 0.0, 0.0],
        }
    }

    #[test]
    fn rejects_off_beam_behind_and_beyond_range_curtains() {
        let frustum = Frustum::for_light(&light()).unwrap();
        for centre in [
            Vec3::new(8.0, 0.0, 4.0),
            Vec3::new(0.0, 0.0, -2.0),
            Vec3::new(0.0, 0.0, 12.0),
        ] {
            assert!(!frustum.may_intersect(&world_corners(&curtain(
                centre,
                [0.5, 0.5],
                Quat::IDENTITY
            ))));
        }
    }

    #[test]
    fn keeps_intersecting_touching_and_rotated_curtains() {
        let frustum = Frustum::for_light(&light()).unwrap();
        let tangent = 0.95_f32.acos().tan();
        for blocker in [
            // All four corners lie outside the circular cone; the cloth still covers it.
            curtain(Vec3::new(0.0, 0.0, 4.0), [5.0, 5.0], Quat::IDENTITY),
            curtain(
                Vec3::new(4.0 * tangent + 0.5, 0.0, 4.0),
                [0.5, 0.5],
                Quat::IDENTITY,
            ),
            curtain(
                Vec3::new(0.0, 0.0, 4.0),
                [2.0, 2.0],
                Quat::from_rotation_y(1.2),
            ),
            curtain(Vec3::new(0.0, 0.0, 10.0), [1.0, 1.0], Quat::IDENTITY),
        ] {
            assert!(frustum.may_intersect(&world_corners(&blocker)));
        }
    }

    #[test]
    fn keeps_cloth_between_virtual_apex_and_lens_without_positive_near_clip() {
        let mut light = light();
        light.params[3] = 2.0;
        let frustum = Frustum::for_light(&light).unwrap();
        let blocker = curtain(Vec3::new(0.0, 0.0, -1.0), [0.2, 0.2], Quat::IDENTITY);
        assert!(frustum.may_intersect(&world_corners(&blocker)));
    }

    #[test]
    fn packs_headers_and_sets_empty_gate_to_minus_one() {
        let mut lights = [light(), light()];
        lights[1].direction_cos_outer[..3].copy_from_slice(&[0.0, 0.0, -1.0]);
        let blocker = curtain(Vec3::new(0.0, 0.0, 4.0), [0.5, 0.5], Quat::IDENTITY);
        let packed = pack(&mut lights, &[blocker]);
        assert_eq!(packed.len(), 2);
        assert_eq!(packed[0].half_size[2], 1.0);
        assert_eq!(packed[1].world_to_local, blocker.world_to_local);
        assert_eq!(lights[0].gate[3], 0.0);
        assert_eq!(lights[1].gate[3], -1.0);
        assert!(pack(&mut lights, &[]).is_empty());
        assert!(lights.iter().all(|light| light.gate[3] == -1.0));
    }

    #[test]
    fn wide_or_invalid_fields_keep_all_blockers() {
        let blocker = curtain(Vec3::new(100.0, 0.0, 4.0), [0.5, 0.5], Quat::IDENTITY);
        for cosine in [0.0, -0.5, f32::NAN] {
            let mut lights = [light()];
            lights[0].direction_cos_outer[3] = cosine;
            assert_eq!(pack(&mut lights, &[blocker]).len(), 2);
        }
    }
}
