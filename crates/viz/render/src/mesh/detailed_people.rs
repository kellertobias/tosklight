//! Shared, modest closed volumes: real depth and smooth normals, no billboard or emissive ink.
use super::{MeshData, Vertex, unit_sphere};
use glam::{Mat3, Mat4, Quat, Vec3};
mod capsule;
mod features;

pub(super) fn build(part: u8, seated: bool) -> MeshData {
    let mut mesh = MeshData::default();
    let hip = if seated { 0.54 } else { 0.44 };
    let shoulder = if seated { 0.79 } else { 0.77 };
    match part {
        0 => {
            append(
                &mut mesh,
                unit_sphere(8, 12),
                Mat4::from_scale_rotation_translation(
                    Vec3::new(0.145, 0.20, 0.16),
                    Quat::IDENTITY,
                    Vec3::new(0.0, 0.90, 0.0),
                ),
                true,
            );
            features::skin(&mut mesh, seated);
            ellipsoid(
                &mut mesh,
                Vec3::new(0.0, 0.795, 0.0),
                Vec3::new(0.065, 0.07, 0.065),
                Quat::IDENTITY,
            );
            for side in [-1.0, 1.0] {
                let hand = if seated {
                    Vec3::new(side * 0.14, 0.48, 0.20)
                } else {
                    Vec3::new(side * 0.19, 0.40, 0.035)
                };
                ellipsoid(
                    &mut mesh,
                    hand,
                    Vec3::new(0.06, 0.085, 0.055),
                    Quat::IDENTITY,
                );
            }
        }
        1 => {
            // Substantial overlap at the waistband, rather than two nearly pointed lobes.
            ellipsoid(
                &mut mesh,
                Vec3::new(0.0, hip + 0.035, 0.0),
                Vec3::new(0.215, 0.18, 0.155),
                Quat::IDENTITY,
            );
            features::shirt(&mut mesh, hip, shoulder);
            for side in [-1.0, 1.0] {
                let upper = Vec3::new(side * 0.145, shoulder - 0.025, 0.0);
                let elbow = Vec3::new(
                    side * 0.20,
                    shoulder - 0.19,
                    if seated { 0.03 } else { 0.01 },
                );
                let wrist = if seated {
                    Vec3::new(side * 0.14, 0.50, 0.19)
                } else {
                    Vec3::new(side * 0.19, 0.445, 0.03)
                };
                limb(&mut mesh, upper, elbow, 0.085);
                limb(&mut mesh, elbow, wrist, 0.07);
            }
        }
        2 => {
            features::dark(&mut mesh);
            ellipsoid(
                &mut mesh,
                Vec3::new(0.0, hip, 0.0),
                Vec3::new(0.23, 0.16, 0.17),
                Quat::IDENTITY,
            );
            for side in [-1.0, 1.0] {
                let top = Vec3::new(side * 0.065, hip, 0.0);
                let knee = Vec3::new(
                    side * 0.075,
                    if seated { 0.38 } else { 0.25 },
                    if seated { 0.23 } else { 0.015 },
                );
                let ankle = Vec3::new(side * 0.075, 0.06, if seated { 0.26 } else { 0.015 });
                limb(&mut mesh, top, knee, 0.095);
                limb(&mut mesh, knee, ankle, 0.075);
                ellipsoid(
                    &mut mesh,
                    Vec3::new(ankle.x, 0.025, ankle.z + 0.035),
                    Vec3::new(0.09, 0.05, 0.16),
                    Quat::IDENTITY,
                );
            }
        }
        _ => unreachable!("only skin, top and bottom are audience material groups"),
    }
    mesh
}
fn limb(mesh: &mut MeshData, from: Vec3, to: Vec3, width: f32) {
    let delta = to - from;
    let model = Mat4::from_rotation_translation(
        Quat::from_rotation_arc(Vec3::Y, delta.normalize()),
        (from + to) * 0.5,
    );
    append(mesh, capsule::build(delta.length(), width), model, false);
}
fn ellipsoid(mesh: &mut MeshData, centre: Vec3, scale: Vec3, rotation: Quat) {
    append(
        mesh,
        unit_sphere(4, 8),
        Mat4::from_scale_rotation_translation(scale, rotation, centre),
        true,
    );
}
fn append(mesh: &mut MeshData, source: MeshData, model: Mat4, reverse: bool) {
    let normal = Mat3::from_mat4(model).inverse().transpose();
    let first = mesh.vertices.len() as u32;
    mesh.vertices
        .extend(source.vertices.into_iter().map(|vertex| {
            Vertex {
                position: model
                    .transform_point3(Vec3::from_array(vertex.position))
                    .to_array(),
                normal: (normal * Vec3::from_array(vertex.normal))
                    .normalize()
                    .to_array(),
                uv: vertex.uv,
            }
        }));
    for triangle in source.indices.chunks_exact(3) {
        let order = if reverse { [0, 2, 1] } else { [0, 1, 2] };
        mesh.indices
            .extend(order.map(|index| first + triangle[index]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detailed_anatomy_has_depth_finite_unit_normals_and_real_floor_height() {
        for seated in [false, true] {
            let mut low = f32::INFINITY;
            let mut high = f32::NEG_INFINITY;
            let mut front = f32::NEG_INFINITY;
            let mut back = f32::INFINITY;
            for part in 0..3 {
                let mesh = build(part, seated);
                assert!(!mesh.indices.is_empty());
                assert!(
                    mesh.indices.len() / 3 <= 1200,
                    "bounded shared per-material geometry"
                );
                for triangle in mesh.indices.chunks_exact(3) {
                    let a = mesh.vertices[triangle[0] as usize];
                    let b = mesh.vertices[triangle[1] as usize];
                    let c = mesh.vertices[triangle[2] as usize];
                    let face = (Vec3::from_array(b.position) - Vec3::from_array(a.position))
                        .cross(Vec3::from_array(c.position) - Vec3::from_array(a.position));
                    if face.length_squared() > 1e-12 {
                        let outward = Vec3::from_array(a.normal)
                            + Vec3::from_array(b.normal)
                            + Vec3::from_array(c.normal);
                        assert!(
                            face.dot(outward) > 0.0,
                            "closed anatomy has outward front faces"
                        );
                    }
                }
                for vertex in mesh.vertices {
                    let p = Vec3::from_array(vertex.position);
                    let n = Vec3::from_array(vertex.normal);
                    assert!(p.is_finite() && n.is_finite() && (n.length() - 1.0).abs() < 1e-5);
                    low = low.min(p.y);
                    high = high.max(p.y);
                    front = front.max(p.z);
                    back = back.min(p.z);
                }
            }
            assert!(low.abs() < 1e-5 && (high - 1.0).abs() < 1e-5);
            assert!(
                front - back > 0.15,
                "3D bodies remain visible from oblique viewpoints"
            );
        }
    }
}

#[cfg(test)]
mod connectivity_tests;
