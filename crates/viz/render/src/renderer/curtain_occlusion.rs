//! Conservative per-light curtain candidates, packed for the fragment shader.
//!
//! The square frustum encloses the circular field cone. Rejecting a rectangle requires all
//! four corners to lie outside the same plane, so intersecting or enclosing cloth stays in.

use crate::instances::{GpuCurtainBlocker, GpuCurtainShadow, GpuLight};
use bytemuck::Zeroable;
use glam::{Mat4, Vec3, Vec4};

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
    if blocker.half_size[2] > 1.5 {
        return blocker.world_to_local.map(|p| Vec3::from_slice(&p[..3]));
    }
    let local_to_world = Mat4::from_cols_array_2d(&blocker.world_to_local).inverse();
    if blocker.half_size[2] > 0.5 {
        return [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::ZERO]
            .map(|p| local_to_world.transform_point3(p));
    }
    let [half_width, half_height, ..] = blocker.half_size;
    [
        Vec3::new(-half_width, -half_height, 0.0),
        Vec3::new(half_width, -half_height, 0.0),
        Vec3::new(half_width, half_height, 0.0),
        Vec3::new(-half_width, half_height, 0.0),
    ]
    .map(|corner| local_to_world.transform_point3(corner))
}

/// Project a finite rectangle from this lamp into its exact convex shadow volume.
/// Constructing the planes in cloth-local space avoids subtracting large world positions when
/// the lamp sits close to an edge. A lamp exactly on the cloth plane cannot cross that plane.
fn shadow_volume(light: &GpuLight, blocker: &GpuCurtainBlocker) -> Option<GpuCurtainShadow> {
    if blocker.half_size[2] > 0.5 {
        return triangle_shadow_volume(light, blocker);
    }
    let transform = Mat4::from_cols_array_2d(&blocker.world_to_local);
    let origin = transform.transform_point3(Vec3::from_slice(&light.position_range[..3]));
    let [width, height, ..] = blocker.half_size;
    if !origin.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || width < 0.0
        || height < 0.0
        || origin.z == 0.0
    {
        return None;
    }
    let side = origin.z.signum();
    let distance = origin.z.abs();
    let local = [
        Vec4::new(-origin.z, 0.0, origin.x - width * side, width * distance),
        Vec4::new(origin.z, 0.0, -origin.x - width * side, width * distance),
        Vec4::new(0.0, -origin.z, origin.y - height * side, height * distance),
        Vec4::new(0.0, origin.z, -origin.y - height * side, height * distance),
        Vec4::new(0.0, 0.0, -side, 0.0),
    ];
    let to_world_plane = transform.transpose();
    let planes = local.map(|plane| {
        let world = to_world_plane * plane;
        // A zero side plane is harmless and conservatively admits the boundary. In ordinary
        // geometry all five normals are nonzero, including arbitrarily near-coplanar lamps.
        let length = world.truncate().length();
        if length > 0.0 {
            (world / length).to_array()
        } else {
            [0.0; 4]
        }
    });
    Some(GpuCurtainShadow { planes })
}

/// Triangle shadows reuse the convex five-plane storage without approximating an aperture.
fn triangle_shadow_volume(
    light: &GpuLight,
    blocker: &GpuCurtainBlocker,
) -> Option<GpuCurtainShadow> {
    let corners = world_corners(blocker);
    let [a, b, c, d] = corners;
    let origin = Vec3::from_slice(&light.position_range[..3]);
    let first = (b - a).cross(c - a);
    let second = if blocker.half_size[2] > 1.5 {
        (c - a).cross(d - a)
    } else {
        Vec3::ZERO
    };
    let normal = if first.length_squared() > second.length_squared() {
        first
    } else {
        second
    }
    .normalize_or_zero();
    let side = normal.dot(origin - a);
    if side.abs() < 1e-7 {
        return None;
    }
    let centre = if blocker.half_size[2] > 1.5 {
        (a + b + c + d) / 4.0
    } else {
        (a + b + c) / 3.0
    };
    // An interior shadow point follows the lamp-to-surface ray. Moving only along the
    // surface normal can leave the shadow of a small off-axis opening reveal.
    let behind = centre + (centre - origin);
    let mut planes = [[0.0; 4]; 5];
    let edges = if blocker.half_size[2] > 1.5 {
        vec![(a, b), (b, c), (c, d), (d, a)]
    } else {
        vec![(a, b), (b, c), (c, a)]
    };
    for (index, (u, v)) in edges.into_iter().enumerate() {
        let mut n = (u - origin).cross(v - origin).normalize_or_zero();
        if n.dot(behind - origin) < 0.0 {
            n = -n;
        }
        planes[index] = n.extend(-n.dot(origin)).to_array();
    }
    let n = -normal * side.signum();
    planes[4] = n.extend(-n.dot(a)).to_array();
    Some(GpuCurtainShadow { planes })
}

/// Each light's gate.w addresses one header followed by its projected shadow volumes.
/// The header has planes[4][2] = candidate count. A gate of -1 means none.
/// Offsets count 80-byte records, matching the shader's storage array indexing.
pub(crate) fn pack(
    lights: &mut [GpuLight],
    blockers: &[GpuCurtainBlocker],
) -> Vec<GpuCurtainShadow> {
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
            let Some(shadow) = shadow_volume(light, blocker) else {
                continue;
            };
            if packed.len() == header_index {
                packed.push(GpuCurtainShadow::zeroed());
            }
            packed.push(shadow);
        }
        if packed.len() > header_index {
            packed[header_index].planes[4][2] = (packed.len() - header_index - 1) as f32;
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
        assert_eq!(packed[0].planes[4][2], 1.0);
        assert_eq!(
            packed[1].planes,
            shadow_volume(&lights[0], &blocker).unwrap().planes
        );
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

    fn projected_blocks(shadow: &GpuCurtainShadow, destination: Vec3) -> bool {
        let point = destination.extend(1.0);
        Vec4::from_array(shadow.planes[4]).dot(point) > 0.0
            && shadow.planes[..4]
                .iter()
                .all(|plane| Vec4::from_array(*plane).dot(point) >= 0.0)
    }

    #[test]
    fn triangle_blockers_keep_exact_apertures_and_work_from_both_sides() {
        let transform = Mat4::from_cols(
            Vec3::X.extend(0.0),
            Vec3::Y.extend(0.0),
            Vec3::Z.extend(0.0),
            Vec3::new(0.0, 0.0, 4.0).extend(1.0),
        );
        let blocker = GpuCurtainBlocker {
            world_to_local: transform.inverse().to_cols_array_2d(),
            half_size: [1.0, 1.0, 1.0, 0.0],
        };
        for (lamp_z, destination_z) in [(0.0, 8.0), (8.0, 0.0)] {
            let mut source = light();
            source.position_range = [0.0, 0.0, lamp_z, 20.0];
            let shadow = shadow_volume(&source, &blocker).unwrap();
            assert!(projected_blocks(
                &shadow,
                Vec3::new(0.4, 0.4, destination_z)
            ));
            assert!(!projected_blocks(
                &shadow,
                Vec3::new(1.8, 1.8, destination_z)
            ));
            assert!(!projected_blocks(
                &shadow,
                Vec3::new(-0.1, 0.4, destination_z)
            ));
        }
    }

    fn reference_blocks(light: &GpuLight, blocker: &GpuCurtainBlocker, destination: Vec3) -> bool {
        let transform = Mat4::from_cols_array_2d(&blocker.world_to_local);
        let start = transform.transform_point3(Vec3::from_slice(&light.position_range[..3]));
        let end = transform.transform_point3(destination);
        if start.z * end.z >= 0.0 {
            return false;
        }
        let fraction = start.z / (start.z - end.z);
        let crossing = start.lerp(end, fraction);
        crossing.x.abs() <= blocker.half_size[0] && crossing.y.abs() <= blocker.half_size[1]
    }

    #[test]
    fn projected_planes_match_finite_segment_intersections_for_posed_cloth() {
        // Deterministic non-grid samples exercise both cloth sides, lamps beyond the cloth
        // edges, and destinations inside the sheet, outside its opening and behind the lamp.
        let mut seed = 0xa91e_683du32;
        let mut random = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            ((seed >> 8) as f32 / 16777216.0) * 20.0 - 10.0
        };
        for rotation in [
            Quat::IDENTITY,
            Quat::from_euler(glam::EulerRot::XYZ, 0.6, 1.2, -0.4),
        ] {
            let blocker = curtain(Vec3::new(2.0, -1.0, 4.0), [1.7, 2.3], rotation);
            for _ in 0..24 {
                let mut light = light();
                light.position_range[..3].copy_from_slice(&[random(), random(), random()]);
                let shadow = shadow_volume(&light, &blocker).unwrap();
                for _ in 0..512 {
                    let destination = Vec3::new(random(), random(), random());
                    assert_eq!(
                        projected_blocks(&shadow, destination),
                        reference_blocks(&light, &blocker, destination)
                    );
                }
            }
        }
    }

    #[test]
    fn handles_coplanar_near_coplanar_and_long_throw_lamps() {
        let blocker = curtain(Vec3::ZERO, [1.0, 1.0], Quat::IDENTITY);
        let mut light = light();
        assert!(shadow_volume(&light, &blocker).is_none());
        for z in [-1e-8, 1e-8, -1e5, 1e5] {
            light.position_range[2] = z;
            let shadow = shadow_volume(&light, &blocker).unwrap();
            for point in [
                Vec3::new(0.0, 0.0, -z),
                Vec3::new(10.0, 0.0, -z),
                Vec3::new(0.0, 0.0, z),
            ] {
                assert_eq!(
                    projected_blocks(&shadow, point),
                    reference_blocks(&light, &blocker, point)
                );
            }
        }
        assert_eq!(std::mem::size_of::<GpuCurtainShadow>(), 80);
    }
}
