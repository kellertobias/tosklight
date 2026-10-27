//! Independent f32 comparisons with the previous length/division implementation.
use glam::Vec3;

const NODES: [f32; 4] = [-0.8611363116, -0.3399810436, 0.3399810436, 0.8611363116];
const WEIGHTS: [f32; 4] = [0.3478548451, 0.6521451549, 0.6521451549, 0.3478548451];

fn envelope(distance: f32, reach: f32) -> f32 {
    let t = ((distance - reach * 0.8) / (reach - reach * 0.8)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}
fn optimized_envelope(squared: f32, reach: f32) -> f32 {
    let flat = reach * 0.8;
    if squared <= flat * flat {
        return 1.0;
    }
    if squared >= reach * reach {
        return 0.0;
    }
    envelope(squared.sqrt(), reach)
}
fn close(actual: f32, expected: f32) {
    assert!(actual.is_finite() && expected.is_finite());
    assert!(
        (actual - expected).abs() <= expected.abs() * 0.0002 + 0.000002,
        "optimized {actual} differs from old {expected}"
    );
}
// Old shared radial expression, deliberately retaining generic pow and both separate clamps.
fn radial(to_light: Vec3, half_angle: f32, feather: f32, uniformity: f32) -> f32 {
    let cosine = -to_light.dot(Vec3::NEG_Y);
    let outer = half_angle.cos();
    if cosine <= outer {
        return 0.0;
    }
    let inner = (half_angle * (1.0 - feather).clamp(0.05, 1.0))
        .cos()
        .max(outer + 0.0005);
    let t = ((cosine - outer) / (inner - outer)).clamp(0.0, 1.0);
    let edge = t * t * (3.0 - 2.0 * t);
    let radius = ((1.0 - cosine) / (1.0 - outer).max(1e-5)).clamp(0.0, 1.0);
    let hot = 0.35 + 2.4 * (1.0 - radius).powf(2.5);
    edge * (hot * (1.0 - uniformity) + uniformity)
}

#[test]
fn squared_range_fast_paths_preserve_flat_taper_and_boundary_values() {
    for reach in [0.1001_f32, 0.5, 4.0, 120.0, 10000.0] {
        for fraction in [
            0.0, 0.0001, 0.7999999, 0.8, 0.8000001, 0.85, 0.99, 0.9999999, 1.0, 1.0000001, 2.0,
        ] {
            // Non-axis-aligned distances exercise dot/sqrt rounding at both range boundaries.
            let offset = Vec3::new(0.36, -0.48, 0.8) * (reach * fraction);
            close(
                optimized_envelope(offset.length_squared(), reach),
                envelope(offset.length(), reach),
            );
        }
    }
}

#[test]
fn squared_energy_and_angular_normalization_preserve_near_apex_and_soft_profiles() {
    for offset in [
        Vec3::ZERO,
        Vec3::splat(1e-9),
        Vec3::new(0.0, 0.00009999, 0.0),
        Vec3::new(0.0, 0.00010001, 0.0),
        Vec3::new(0.01, 0.049, 0.0),
        Vec3::new(0.0, 0.05, 0.0),
        Vec3::new(0.7, 4.0, 0.3),
        Vec3::new(20.0, 120.0, 0.0),
    ] {
        let distance = offset.length();
        let squared = offset.length_squared();
        close(
            36.0 / squared.max(0.0025),
            36.0 / (distance * distance).max(0.0025),
        );
        let old_direction = offset / distance.max(1e-4);
        let new_direction = offset * squared.max(1e-8).sqrt().recip();
        for (degrees, feather, uniformity) in [
            (24.0_f32, 0.3, 0.55),
            (13.0, 0.72, 0.3),
            (4.5, 0.08, 0.3),
            (4.5, 0.60, 0.3),
            (60.0, 0.8, 0.55),
        ] {
            close(
                radial(new_direction, degrees.to_radians(), feather, uniformity),
                radial(old_direction, degrees.to_radians(), feather, uniformity),
            );
        }
    }
}

fn sampled(
    a: f32,
    b: f32,
    origin: Vec3,
    direction: Vec3,
    apex: Vec3,
    lens: Vec3,
    reach: f32,
    profile: (f32, f32, f32),
    optimized: bool,
) -> (f32, f32) {
    let mut raw = 0.0;
    let mut shaped = 0.0;
    for (node, weight) in NODES.into_iter().zip(WEIGHTS) {
        let point = origin + direction * ((a + b) * 0.5 + (b - a) * 0.5 * node);
        let offset = apex - point;
        let lens_offset = point - lens;
        let (energy, to_light, range) = if optimized {
            let squared = offset.length_squared();
            (
                36.0 / squared.max(0.0025),
                offset * squared.max(1e-8).sqrt().recip(),
                optimized_envelope(lens_offset.length_squared(), reach),
            )
        } else {
            let distance = offset.length();
            (
                36.0 / (distance * distance).max(0.0025),
                offset / distance.max(1e-4),
                envelope(lens_offset.length(), reach),
            )
        };
        let measured = weight * energy * range;
        raw += measured;
        shaped += measured * radial(to_light, profile.0.to_radians(), profile.1, profile.2);
    }
    (
        raw * (b - a) * 0.5,
        if raw > 0.0 { shaped / raw } else { 0.0 },
    )
}

#[test]
fn unchanged_four_nodes_preserve_taper_energy_and_profile_ratio_across_split_intervals() {
    for (degrees, feather, uniformity) in [
        (24.0_f32, 0.3, 0.55),
        (13.0, 0.72, 0.3),
        (4.5, 0.08, 0.3),
        (4.5, 0.60, 0.3),
        (60.0, 0.8, 0.55),
    ] {
        for (origin, direction, apex, lens, reach) in [
            (
                Vec3::new(-2.0, 0.0, 0.0),
                Vec3::X,
                Vec3::new(0.0, 4.0, 0.0),
                Vec3::new(0.0, 3.8, 0.0),
                5.0,
            ),
            (
                Vec3::new(0.0, 0.00001, 0.0),
                Vec3::X,
                Vec3::ZERO,
                Vec3::new(0.0, -0.02, 0.0),
                0.1001,
            ),
            (
                Vec3::ZERO,
                Vec3::new(1.0, -0.1, 0.2).normalize(),
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::new(0.0, 1.9, 0.0),
                4.0,
            ),
        ] {
            // Whole interval and disjoint visible pieces around curtains, including lens-near spans.
            for (a, b) in [
                (0.0, 0.0001),
                (0.0, 0.05),
                (0.0, 4.0),
                (0.1, 0.3),
                (1.1, 1.8),
                (3.2, 4.0),
            ] {
                let old = sampled(
                    a,
                    b,
                    origin,
                    direction,
                    apex,
                    lens,
                    reach,
                    (degrees, feather, uniformity),
                    false,
                );
                let new = sampled(
                    a,
                    b,
                    origin,
                    direction,
                    apex,
                    lens,
                    reach,
                    (degrees, feather, uniformity),
                    true,
                );
                close(new.0, old.0);
                close(new.1, old.1);
            }
        }
    }
}

#[test]
fn squared_normalization_preserves_fresnel_par_acl_field_edge_samples() {
    for (degrees, feather, uniformity) in [
        (24.0_f32, 0.3, 0.55),
        (13.0, 0.72, 0.3),
        (4.5, 0.08, 0.3),
        (4.5, 0.60, 0.3),
        (60.0, 0.8, 0.55),
    ] {
        let half_angle = degrees.to_radians();
        for fraction in [0.0, 0.25, 0.8, 0.92, 0.99, 0.99999, 1.0, 1.00001, 1.1] {
            let angle = half_angle * fraction;
            for distance in [0.000099_f32, 0.000101, 0.05, 4.0, 120.0] {
                let offset = Vec3::new(angle.sin(), angle.cos(), 0.0) * distance;
                let old = offset / offset.length().max(1e-4);
                let new = offset * offset.length_squared().max(1e-8).sqrt().recip();
                let previous = radial(old, half_angle, feather, uniformity);
                let optimized = radial(new, half_angle, feather, uniformity);
                // Normalization differs by float rounding. Near a narrow cone's zero-energy
                // rim, relative-to-that-pixel error is unbounded; bound against source peak.
                let peak = 2.75 * (1.0 - uniformity) + uniformity;
                assert!(
                    (optimized - previous).abs() <= peak * 0.0002 + 0.000002,
                    "{degrees} degrees, feather {feather}, fraction {fraction}, distance {distance}: {optimized} vs {previous}"
                );
            }
        }
    }
}
