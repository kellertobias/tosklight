//! CPU reference checks for the Draft integral and independently parsed shader interfaces.
use glam::DVec3;

fn raw(a: f64, b: f64, along: f64, height: f64) -> f64 {
    if b <= a {
        return 0.0;
    }
    let (x, y) = (a + along, b + along);
    if height == 0.0 {
        return 36.0 * (b - a) / (x * y);
    }
    36.0 * (height * (b - a)).atan2(height * height + x * y) / height
}
fn clamped(a: f64, b: f64, along: f64, height: f64) -> f64 {
    if height >= 0.05 {
        return raw(a, b, along, height);
    }
    let half_width = (0.0025 - height * height).sqrt();
    let left = a.max(-along - half_width);
    let right = b.min(-along + half_width);
    if right <= left {
        return raw(a, b, along, height);
    }
    raw(a, left, along, height) + (right - left) * (36.0 / 0.0025) + raw(right, b, along, height)
}
fn reference(a: f64, b: f64, function: impl Fn(f64) -> f64) -> f64 {
    let count = 200_000;
    let step = (b - a) / f64::from(count);
    (0..count)
        .map(|i| function(a + (f64::from(i) + 0.5) * step))
        .sum::<f64>()
        * step
}
#[test]
fn stable_inverse_square_matches_independent_dense_reference_and_is_additive() {
    for (a, b, along, height) in [
        (0.0, 4.0, -2.0, 0.0),
        (0.0, 4.0, -2.0, 1e-9),
        (0.0, 4.0, -2.0, 0.049999),
        (0.0, 4.0, -2.0, 0.05),
        (0.0, 4.0, -2.0, 0.4),
        (0.0, 0.001, 300.0, 0.01),
        (0.0, 5.0, 1.0, 0.0),
    ] {
        let actual = clamped(a, b, along, height);
        let expected = reference(a, b, |t| {
            36.0 / ((t + along).powi(2) + height * height).max(0.0025)
        });
        assert!(actual.is_finite() && actual >= 0.0);
        assert!(
            (actual - expected).abs() <= expected.abs() * 2e-6 + 1e-9,
            "{actual} != {expected}"
        );
        let middle = (a + b) * 0.5;
        assert!(
            (actual - clamped(a, middle, along, height) - clamped(middle, b, along, height)).abs()
                < actual * 1e-10 + 1e-9
        );
    }
}
#[test]
fn range_taper_quadrature_matches_lens_centred_reference_with_virtual_apex_offset() {
    let nodes = [-0.8611363116, -0.3399810436, 0.3399810436, 0.8611363116];
    let weights = [0.3478548451, 0.6521451549, 0.6521451549, 0.3478548451];
    for apex_offset in [0.0, 0.1, 2.0, 20.0] {
        let origin = DVec3::new(0.3, 0.0, 0.0);
        let direction = DVec3::Z;
        let apex = DVec3::new(0.0, 0.0, -apex_offset);
        let (a, b) = (
            (8.0_f64.powi(2) - 0.09).sqrt(),
            (10.0_f64.powi(2) - 0.09).sqrt(),
        );
        let value = |t: f64| {
            let point = origin + direction * t;
            let u = ((point.length() - 8.0) / 2.0).clamp(0.0, 1.0);
            let envelope = 1.0 - u * u * (3.0 - 2.0 * u);
            36.0 / (point - apex).length_squared().max(0.0025) * envelope
        };
        let approximation: f64 = nodes
            .iter()
            .zip(weights)
            .map(|(node, weight)| weight * value((a + b) * 0.5 + (b - a) * 0.5 * node))
            .sum::<f64>()
            * (b - a)
            * 0.5;
        let expected = reference(a, b, value);
        assert!(
            (approximation - expected).abs() < expected * 1e-4,
            "{approximation} != {expected}"
        );
    }
}
#[test]
fn dedicated_draft_shaders_parse_and_validate_without_gpu() {
    let common = include_str!("../../shaders/common.wgsl");
    for (name, source) in [
        (
            "surface",
            format!(
                "{common}\n{}",
                include_str!("../../shaders/draft_surface.wgsl")
            ),
        ),
        (
            "beam",
            format!(
                "{common}\n{}\n{}",
                include_str!("../../shaders/draft_integral.wgsl"),
                include_str!("../../shaders/draft_beam.wgsl")
            ),
        ),
        (
            "upsample",
            include_str!("../../shaders/draft_upsample.wgsl").to_owned(),
        ),
    ] {
        let module = naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|error| panic!("{name}: {}", error.emit_to_string(&source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|error| panic!("{name}: {error:?}"));
    }
}
