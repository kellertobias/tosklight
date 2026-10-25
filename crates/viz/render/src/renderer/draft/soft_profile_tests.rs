//! Independent dense references for Draft's energy-normalised four-point angular estimate.
use super::{clamped, reference};

const NODES: [f64; 4] = [-0.8611363116, -0.3399810436, 0.3399810436, 0.8611363116];
const WEIGHTS: [f64; 4] = [0.3478548451, 0.6521451549, 0.6521451549, 0.3478548451];

fn field(x: f64, distance: f64, half_angle: f64, feather: f64, uniformity: f64) -> f64 {
    let cosine = distance / x.hypot(distance);
    let outer = half_angle.cos();
    if cosine <= outer {
        return 0.0;
    }
    let inner = (half_angle * (1.0 - feather).clamp(0.05, 1.0))
        .cos()
        .max(outer + 0.0005);
    let rim = ((cosine - outer) / (inner - outer)).clamp(0.0, 1.0);
    let edge = rim * rim * (3.0 - 2.0 * rim);
    let radial = ((1.0 - cosine) / (1.0 - outer)).clamp(0.0, 1.0);
    let hot = 0.35 + 2.4 * (1.0 - radial).powf(2.5);
    edge * (hot * (1.0 - uniformity) + uniformity)
}
fn weighted(a: f64, b: f64, along: f64, height: f64, profile: impl Fn(f64) -> f64) -> f64 {
    let mut raw = 0.0;
    let mut shaped = 0.0;
    for (node, weight) in NODES.into_iter().zip(WEIGHTS) {
        let t = (a + b) * 0.5 + (b - a) * 0.5 * node;
        let measure = weight * 36.0 / ((t + along).powi(2) + height * height).max(0.0025);
        raw += measure;
        shaped += measure * profile(t);
    }
    clamped(a, b, along, height) * shaped / raw
}

#[test]
fn fresnel_par_and_very_wide_soft_pools_match_independent_dense_rays() {
    for (degrees, feather, uniformity) in
        [(24.0_f64, 0.58, 0.55), (13.0, 0.72, 0.3), (50.0, 0.8, 0.55)]
    {
        let angle = degrees.to_radians();
        for distance in [0.1, 3.0, 12.0] {
            let radius = distance * angle.tan();
            let profile = |x| field(x, distance, angle, feather, uniformity);
            let expected = reference(-radius, radius, |x| {
                36.0 / (x * x + distance * distance).max(0.0025) * profile(x)
            });
            let actual = weighted(-radius, radius, 0.0, distance, profile);
            assert!(actual.is_finite() && actual >= 0.0);
            // Draft estimates angular variation only; the undegraded High/Ultra formula stays exact.
            assert!(
                (actual - expected).abs() < expected * 0.04,
                "{degrees}°, {distance}m: {actual} != {expected}"
            );
        }
    }
}

#[test]
fn camera_inside_and_near_lens_constant_field_preserves_clamped_analytic_energy() {
    for (a, b, along, height) in [
        (0.0, 4.0, -2.0, 0.0),
        (0.0, 4.0, -2.0, 1e-9),
        (0.0, 0.001, 300.0, 0.01),
    ] {
        let expected = reference(a, b, |t| {
            1.4 * 36.0 / ((t + along).powi(2) + height * height).max(0.0025)
        });
        let actual = weighted(a, b, along, height, |_| 1.4);
        assert!((actual - expected).abs() < expected * 2e-6 + 1e-9);
    }
}

#[test]
fn soft_field_has_a_central_hotspot_a_continuous_fade_and_no_outside_field_spill() {
    let angle = 40.0_f64.to_radians();
    let distance = 4.0;
    let radius = distance * angle.tan();
    let profile = |fraction| field(radius * fraction, distance, angle, 0.72, 0.3);
    assert!(profile(0.0) > profile(0.4));
    assert!(profile(0.4) > profile(0.8));
    assert!(profile(0.8) > profile(0.99));
    assert!(profile(0.99) < profile(0.0) * 0.01);
    assert_eq!(profile(1.001), 0.0);
    assert_eq!(profile(1.5), 0.0);
}
