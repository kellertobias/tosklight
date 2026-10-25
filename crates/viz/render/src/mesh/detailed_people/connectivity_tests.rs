//! Analytic triangle intersections check the actual emitted surfaces, not only nominal endpoints.
use super::*;
fn inside(mesh: &MeshData, point: Vec3) -> bool {
    let ray = Vec3::new(0.2671, 0.5343, 0.8017).normalize();
    let mut hits = Vec::new();
    for tri in mesh.indices.chunks_exact(3) {
        let a = Vec3::from_array(mesh.vertices[tri[0] as usize].position);
        let edge1 = Vec3::from_array(mesh.vertices[tri[1] as usize].position) - a;
        let edge2 = Vec3::from_array(mesh.vertices[tri[2] as usize].position) - a;
        let p = ray.cross(edge2);
        let determinant = edge1.dot(p);
        if determinant.abs() < 1e-9 {
            continue;
        }
        let offset = point - a;
        let u = offset.dot(p) / determinant;
        let q = offset.cross(edge1);
        let v = ray.dot(q) / determinant;
        let distance = edge2.dot(q) / determinant;
        if u >= 0.0 && v >= 0.0 && u + v <= 1.0 && distance > 1e-6 {
            hits.push((distance, if determinant < 0.0 { 1 } else { -1 }));
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    hits.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-5 && a.1 == b.1);
    // Signed winding measures the union of overlapping outward solids. Even/odd parity
    // would incorrectly call a point inside BOTH the waistband and torso empty space.
    hits.iter().map(|(_, sign)| sign).sum::<i32>() > 0
}
#[test]
fn actual_capsules_overlap_with_a_finite_joint_volume_at_bent_elbows_and_knees() {
    for incoming in [Vec3::new(0.01, 0.3, -0.015), Vec3::new(-0.01, 0.16, -0.23)] {
        let mut upper = MeshData::default();
        let mut lower = MeshData::default();
        limb(&mut upper, incoming, Vec3::ZERO, 0.095);
        limb(&mut lower, Vec3::ZERO, Vec3::new(0.0, -0.32, 0.03), 0.075);
        // A shared volume with 65% of the narrower joint radius must lie INSIDE both meshes.
        // Merely touching pointed ellipsoid endpoints fails these off-axis triangle ray tests.
        let radius = 0.075 * 0.5 * 0.65;
        for direction in [
            Vec3::X,
            -Vec3::X,
            Vec3::Y,
            -Vec3::Y,
            Vec3::Z,
            -Vec3::Z,
            Vec3::new(1.0, 1.0, 1.0).normalize(),
            Vec3::new(-1.0, 1.0, -1.0).normalize(),
        ] {
            let point = direction * radius;
            assert!(
                inside(&upper, point),
                "upper segment lost shared joint volume: {point:?}"
            );
            assert!(
                inside(&lower, point),
                "lower segment lost shared joint volume: {point:?}"
            );
        }
    }
}
#[test]
fn waistband_has_overlapping_clothed_volume_and_head_uses_smoother_curvature() {
    for seated in [false, true] {
        let hip = if seated { 0.54 } else { 0.44 };
        let top = build(1, seated);
        let bottom = build(2, seated);
        for x in [-0.05, 0.0, 0.05] {
            for z in [-0.03, 0.0, 0.03] {
                let point = Vec3::new(x, hip + 0.025, z);
                assert!(
                    inside(&top, point) && inside(&bottom, point),
                    "waistband separated at {point:?}"
                );
            }
        }
        let skin = build(0, seated);
        let head_vertices = (8 + 1) * (12 + 1);
        assert!(skin.vertices.len() >= head_vertices);
        let highest_ring = &skin.vertices[13..26];
        let radius = highest_ring
            .iter()
            .map(|v| Vec3::from_array(v.position).x.abs())
            .fold(0.0_f32, f32::max);
        assert!(
            radius < 0.03,
            "smaller first polar ring avoids the old four-ring pointed head"
        );
    }
}
