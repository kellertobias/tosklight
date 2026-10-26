//! Baked static anatomical detail, shared by native Ultra images and geometry snapshots.
//! Local +Z is the face/toe direction; authored crowd rotation remains authoritative.
use super::*;

pub(super) fn skin(mesh: &mut MeshData, seated: bool) {
    for side in [-1.0, 1.0] {
        ellipsoid(
            mesh,
            Vec3::new(side * 0.073, 0.907, 0.0),
            Vec3::new(0.025, 0.05, 0.035),
            Quat::IDENTITY,
        );
        // A thumb emerges from the side of each existing mitten, rather than a floating finger.
        ellipsoid(
            mesh,
            if seated {
                Vec3::new(side * 0.115, 0.48, 0.21)
            } else {
                Vec3::new(side * 0.165, 0.405, 0.047)
            },
            Vec3::new(0.025, 0.045, 0.025),
            Quat::IDENTITY,
        );
    }
    ellipsoid(
        mesh,
        Vec3::new(0.0, 0.902, 0.081),
        Vec3::new(0.023, 0.044, 0.038),
        Quat::IDENTITY,
    );
    ellipsoid(
        mesh,
        Vec3::new(0.0, 0.847, 0.018),
        Vec3::new(0.10, 0.06, 0.11),
        Quat::IDENTITY,
    );
}

pub(super) fn dark(mesh: &mut MeshData) {
    // Hair and features use the existing non-emissive dark material group; no new draw/material ABI.
    append(
        mesh,
        unit_sphere(8, 12),
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.15, 0.105, 0.15),
            Quat::IDENTITY,
            Vec3::new(0.0, 0.946, -0.012),
        ),
        true,
    );
    for side in [-1.0, 1.0] {
        ellipsoid(
            mesh,
            Vec3::new(side * 0.029, 0.918, 0.075),
            Vec3::new(0.015, 0.009, 0.015),
            Quat::IDENTITY,
        );
        ellipsoid(
            mesh,
            Vec3::new(side * 0.029, 0.933, 0.071),
            Vec3::new(0.043, 0.009, 0.012),
            Quat::IDENTITY,
        );
    }
    ellipsoid(
        mesh,
        Vec3::new(0.0, 0.866, 0.072),
        Vec3::new(0.035, 0.006, 0.009),
        Quat::IDENTITY,
    );
}

pub(super) fn shirt(mesh: &mut MeshData, hip: f32, shoulder: f32) {
    // Shoulder/chest/waist sections give the garment a human silhouette instead of a single oval.
    let rings = [
        (hip - 0.055, 0.095, 0.065),
        (hip + 0.03, 0.11, 0.075),
        (hip + 0.16, 0.125, 0.085),
        (shoulder - 0.04, 0.15, 0.08),
        (shoulder + 0.012, 0.12, 0.065),
        (shoulder + 0.035, 0.04, 0.045),
    ];
    let mut source = MeshData::default();
    for (y, rx, rz) in rings {
        for segment in 0..12 {
            let angle = segment as f32 * std::f32::consts::TAU / 12.0;
            source.vertices.push(Vertex {
                position: [angle.cos() * rx, y, angle.sin() * rz],
                normal: [0.0; 3],
                uv: [0.0; 2],
            });
        }
    }
    for ring in 0..5_u32 {
        for segment in 0..12_u32 {
            let a = ring * 12 + segment;
            let next = ring * 12 + (segment + 1) % 12;
            source
                .indices
                .extend([a, a + 12, next, next, a + 12, next + 12]);
        }
    }
    for (ring, reverse) in [(0_u32, true), (5, false)] {
        let centre = source.vertices.len() as u32;
        source.vertices.push(Vertex {
            position: [0.0, rings[ring as usize].0, 0.0],
            normal: [0.0; 3],
            uv: [0.0; 2],
        });
        for segment in 0..12 {
            let a = ring * 12 + segment;
            let b = ring * 12 + (segment + 1) % 12;
            source.indices.extend(if reverse {
                [centre, a, b]
            } else {
                [centre, b, a]
            });
        }
    }
    // Accumulate actual adjacent triangle normals, including sloped shoulders and tapered waist.
    for tri in source.indices.chunks_exact(3) {
        let positions = [tri[0], tri[1], tri[2]]
            .map(|i| Vec3::from_array(source.vertices[i as usize].position));
        let normal = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
        for i in tri {
            let v = &mut source.vertices[*i as usize];
            v.normal = (Vec3::from_array(v.normal) + normal).to_array();
        }
    }
    for vertex in &mut source.vertices {
        vertex.normal = Vec3::from_array(vertex.normal).normalize().to_array();
    }
    append(mesh, source, Mat4::IDENTITY, false);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detailed_shared_anatomy_has_a_small_fixed_triangle_budget() {
        for seated in [false, true] {
            let counts = [0, 1, 2].map(|part| super::super::build(part, seated).indices.len() / 3);
            assert_eq!(counts, [768, 656, 1152]);
            assert_eq!(counts.iter().sum::<usize>(), 2576);
        }
    }
    #[test]
    fn face_features_point_forward_and_hair_eyes_and_garment_are_actual_geometry() {
        for seated in [false, true] {
            let skin = super::super::build(0, seated);
            let dark = super::super::build(2, seated);
            let nose = skin
                .vertices
                .iter()
                .filter(|v| v.position[1] > 0.87 && v.position[1] < 0.93)
                .map(|v| v.position[2])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(nose > 0.095, "nose projects beyond front head surface+Z");
            for side in [-1.0, 1.0] {
                assert!(
                    dark.vertices.iter().any(|v| v.position[0] * side > 0.02
                        && v.position[1] > 0.913
                        && v.position[1] < 0.923
                        && v.position[2] > 0.078),
                    "both actual dark eye surfaces face+Z"
                );
            }
            assert!(
                dark.vertices.iter().any(|v| v.position[1] > 0.99),
                "real hair volume"
            );
            let mut shirt = MeshData::default();
            super::shirt(&mut shirt, 0.44, 0.77);
            assert!(
                shirt
                    .vertices
                    .iter()
                    .any(|v| (v.position[1] - 0.73).abs() < 1e-5 && v.position[0] > 0.149),
                "shoulder contour"
            );
        }
    }
}
