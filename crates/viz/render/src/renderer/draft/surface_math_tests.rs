//! Independent CPU f32 reference for exact Draft-only algebra, including physical range edges.
use glam::Vec3;

fn contribution(offset: Vec3, normal: Vec3, reach: f32, cheap: bool) -> f32 {
    let squared = offset.length_squared();
    if cheap && (squared > reach * reach || normal.dot(offset) <= 0.0) {
        return 0.0;
    }
    let distance = squared.sqrt();
    if distance > reach {
        return 0.0;
    }
    let diffuse = normal.dot(offset / distance.max(1e-4)).max(0.0);
    let taper = ((distance - reach * 0.8) / (reach * 0.2)).clamp(0.0, 1.0);
    diffuse * (1.0 - taper * taper * (3.0 - 2.0 * taper))
}

#[test]
fn draft_cheap_rejects_preserve_diffuse_and_physical_range_taper() {
    for reach in [0.0001_f32, 0.1, 1.0, 20.0, 1000.0, 1e10] {
        for scale in [
            0.0, 1e-20, 1e-5, 0.1, 0.799999, 0.8, 0.9999999, 1.0, 1.0000001, 2.0,
        ] {
            for direction in [
                Vec3::X,
                Vec3::Y,
                Vec3::Z,
                Vec3::new(1.0, -2.0, 3.0).normalize(),
            ] {
                for normal in [
                    direction,
                    -direction,
                    Vec3::new(0.3, 0.8, -0.5).normalize(),
                    direction.any_orthonormal_vector(),
                ] {
                    let offset = direction * (reach * scale);
                    let old = contribution(offset, normal, reach, false);
                    let new = contribution(offset, normal, reach, true);
                    assert!(old.is_finite() && new.is_finite());
                    assert!((old - new).abs() <= 3e-7, "old{old} new{new} at{offset:?}");
                }
            }
        }
    }
}

#[test]
fn draft_hotspot_square_root_matches_pow_across_the_complete_radial_domain() {
    for index in 0..=10000 {
        let centre = index as f32 / 10000.0;
        let old = 0.35 + 2.4 * centre.powf(2.5);
        let new = 0.35 + 2.4 * centre * centre * centre.sqrt();
        assert!(
            (old - new).abs() <= 5e-7,
            "old{old} new{new} centre{centre}"
        );
    }
    for centre in [
        f32::from_bits(1),
        f32::MIN_POSITIVE,
        1e-20,
        1e-10,
        1.0 - f32::EPSILON,
    ] {
        let old = 0.35 + 2.4 * centre.powf(2.5);
        let new = 0.35 + 2.4 * centre * centre * centre.sqrt();
        assert!(new.is_finite() && (old - new).abs() <= 5e-7);
    }
}

#[test]
fn actual_draft_surface_shader_validates_with_shared_optics() {
    let source = format!(
        "{}\n{}",
        include_str!("../../shaders/common.wgsl"),
        include_str!("../../shaders/draft_surface.wgsl")
    );
    let module = naga::front::wgsl::parse_str(&source).expect("actual Draft WGSL parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("actual Draft shared optics shader validates");
}
