//! Interpret Beam (50%) / Field (10%) angles in the renderer's explicitly nominal light profile.
//! This does not invent measured photometry or a Focus-to-distance relation.

/// Resolve the outer cone once when native optics change, rather than fitting every display tick.
/// The nominal profile matches the shared light shader's edge and hotspot functions.
pub fn physical_zoom_outer_half_angle(
    degrees: f32,
    field: bool,
    sharpness: f32,
    uniformity: f32,
    focus: f32,
) -> Option<f32> {
    if !degrees.is_finite() || !(0. ..180.).contains(&degrees) {
        return None;
    }
    let reference = f64::from(degrees).to_radians() * 0.5;
    let feather = f64::from(
        (1. - sharpness.clamp(0., 1.) + (focus.clamp(0., 1.) - 0.5).abs() * 1.1).clamp(0.02, 0.98),
    );
    let uniformity = f64::from(uniformity.clamp(0., 1.));
    let threshold = if field { 0.1 } else { 0.5 };
    let mut lo = reference;
    let mut hi = 1.55_f64;
    if lo >= hi || nominal_profile(reference, hi, feather, uniformity) < threshold {
        return None;
    }
    for _ in 0..24 {
        let middle = (lo + hi) * 0.5;
        if nominal_profile(reference, middle, feather, uniformity) < threshold {
            lo = middle
        } else {
            hi = middle
        }
    }
    Some(((lo + hi) * 0.5) as f32)
}
fn nominal_profile(angle: f64, outer: f64, feather: f64, uniformity: f64) -> f64 {
    let cos_outer = outer.cos();
    let cos_inner = (outer * (1. - feather).clamp(0.05, 1.))
        .cos()
        .max(cos_outer + 0.0005);
    let edge = |cos: f64| {
        let t = ((cos - cos_outer) / (cos_inner - cos_outer)).clamp(0., 1.);
        t * t * (3. - 2. * t)
    };
    let radial = ((1. - angle.cos()) / (1. - cos_outer).max(1e-5)).clamp(0., 1.);
    let core = (0.35 + 2.4 * (1. - radial).powf(2.5)) * (1. - uniformity) + uniformity;
    let centre = 2.75 * (1. - uniformity) + uniformity;
    edge(angle.cos()) * core / (edge(1.) * centre).max(1e-12)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_opening_meets_its_own_beam_or_field_threshold() {
        for degrees in [1., 5., 20., 60.] {
            for uniformity in [0., 0.4, 1.] {
                for (field, threshold) in [(false, 0.5), (true, 0.1)] {
                    let outer =
                        physical_zoom_outer_half_angle(degrees, field, 0.7, uniformity, 0.5)
                            .unwrap();
                    let actual = nominal_profile(
                        f64::from(degrees).to_radians() * 0.5,
                        f64::from(outer),
                        0.3,
                        f64::from(uniformity),
                    );
                    assert!(
                        (actual - threshold).abs() < 1e-5,
                        "{degrees} {field} {actual}"
                    );
                }
            }
        }
    }
    #[test]
    fn same_number_with_different_convention_does_not_mean_same_cone() {
        let beam = physical_zoom_outer_half_angle(20., false, 0.5, 0.4, 0.5).unwrap();
        let field = physical_zoom_outer_half_angle(20., true, 0.5, 0.4, 0.5).unwrap();
        assert!(beam > field);
        assert!(physical_zoom_outer_half_angle(179., true, 0.5, 0.4, 0.5).is_none());
    }
}
