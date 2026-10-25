//! Optical invariants independent of raster resolution, including f32 near-axis limits.
fn profile(cosine: f32, outer: f32, authored_inner: f32, uniformity: f32, legacy: bool) -> f32 {
    let cosine = cosine.min(1.0);
    if cosine <= outer {
        return 0.0;
    }
    let span = 1.0 - outer;
    let width = if legacy {
        authored_inner.max(outer + 0.0005) - outer
    } else {
        (authored_inner - outer)
            .max(0.0005_f32.min(span * 0.01))
            .min(span)
    };
    let x = ((cosine - outer) / width).clamp(0.0, 1.0);
    let edge = x * x * (3.0 - 2.0 * x);
    let radial_span = if legacy { span.max(1e-5) } else { span };
    let radial = ((1.0 - cosine) / radial_span).clamp(0.0, 1.0);
    let hotspot = 0.35 + 2.4 * (1.0 - radial).powf(2.5);
    edge * (hotspot * (1.0 - uniformity) + uniformity)
}
fn cone(full_degrees: f32, feather: f32) -> (f32, f32) {
    let half = (full_degrees * 0.5).to_radians();
    (half.cos(), (half * (1.0 - feather)).cos())
}
#[test]
fn narrow_transition_reaches_its_authored_axial_core_without_fixed_epsilon_loss() {
    for angle in [
        0.1, 0.23, 0.5, 1.0, 2.0, 3.0, 3.62, 4.0, 8.0, 10.0, 48.0, 170.0,
    ] {
        for feather in [0.02, 0.08, 0.3, 0.6, 0.98] {
            let (outer, inner) = cone(angle, feather);
            for uniformity in [0.0, 0.55, 0.95, 1.0] {
                let actual = profile(1.0, outer, inner, uniformity, false);
                let axial_core = 2.75 * (1.0 - uniformity) + uniformity;
                assert!(
                    (actual - axial_core).abs() < 2e-6,
                    "{angle}/{feather}/{uniformity}: {actual}/{axial_core}"
                );
            }
        }
    }
    let (outer, inner) = cone(2.0, 0.08);
    assert!(
        profile(1.0, outer, inner, 1.0, true) < 0.23,
        "old 2-degree regression must remain demonstrated"
    );
}
#[test]
fn physical_span_and_tiny_f32_cones_have_finite_monotone_non_degenerate_profiles() {
    for outer in [
        f32::from_bits(1.0_f32.to_bits() - 1),
        f32::from_bits(1.0_f32.to_bits() - 2),
        cone(0.23, 0.02).0,
        cone(2.0, 0.02).0,
        cone(48.0, 0.3).0,
    ] {
        for inner in [outer, outer + (1.0 - outer) * 0.04, 1.0] {
            for uniformity in [0.0, 0.55, 1.0] {
                let mut previous = 0.0;
                for step in 0..=1000 {
                    let cosine = outer + (1.0 - outer) * step as f32 / 1000.0;
                    let value = profile(cosine, outer, inner, uniformity, false);
                    assert!(value.is_finite() && value >= 0.0 && value <= 2.750001);
                    assert!(
                        value + 1e-6 >= previous,
                        "profile decreases toward its axis"
                    );
                    previous = value;
                }
                assert_eq!(profile(outer, outer, inner, uniformity, false), 0.0);
                assert!(profile(1.0, outer, inner, uniformity, false) >= 1.0);
            }
        }
    }
}
#[test]
fn ordinary_authored_fresnel_par_and_acl_profiles_keep_existing_values() {
    for (angle, feather, uniformity) in [
        (19.0, 0.3, 0.55),
        (48.0, 0.58, 0.55),
        (28.0, 0.6, 0.45),
        (10.0, 0.6, 0.6),
    ] {
        let (outer, inner) = cone(angle, feather);
        assert!(inner - outer >= 0.0005);
        for step in 0..=1000 {
            let cosine = outer + (1.0 - outer) * step as f32 / 1000.0;
            assert_eq!(
                profile(cosine, outer, inner, uniformity, false),
                profile(cosine, outer, inner, uniformity, true)
            );
        }
    }
}
#[test]
fn authored_sharpness_survives_and_axial_gain_has_no_old_3_point_62_degree_boundary() {
    let (outer, sharp) = cone(2.0, 0.08);
    let (_, soft) = cone(2.0, 0.60);
    let middle = outer + (1.0 - outer) * 0.5;
    assert!(
        profile(middle, outer, sharp, 1.0, false) > profile(middle, outer, soft, 1.0, false) + 0.2
    );
    for angle in [3.60, 3.61, 3.62, 3.63, 3.64] {
        let (outer, inner) = cone(angle, 0.08);
        assert_eq!(profile(1.0, outer, inner, 1.0, false), 1.0);
    }
}

#[test]
fn unrepresentable_cones_are_dark_and_zoom_has_no_fixed_cosine_threshold_jump() {
    assert_eq!(profile(1.0, 1.0, 1.0, 0.5, false), 0.0);
    let mut previous: Option<f32> = None;
    for step in 0..=250 {
        let (outer, inner) = cone(3.5 + step as f32 * 0.001, 0.08);
        let value = profile(outer + (1.0 - outer) * 0.5, outer, inner, 0.55, false);
        if let Some(previous) = previous {
            assert!(
                (value - previous).abs() < 0.01,
                "fixed-cosine boundary changed field shape"
            );
        }
        previous = Some(value);
    }
}
