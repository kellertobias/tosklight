//! Independent sampled geometry checks for the conservative Draft receiver cone bound.
use glam::DVec3;

fn bounds(origin: DVec3, axis: DVec3, range: f64, cosine: f64) -> (DVec3, DVec3) {
    let along = range / (2.0 * cosine);
    let centre = origin + axis * along;
    let end = origin + axis * range;
    let radial = range * (1.0 - cosine * cosine).sqrt() / cosine;
    let extent = DVec3::new(
        (1.0 - axis.x * axis.x).max(0.0).sqrt(),
        (1.0 - axis.y * axis.y).max(0.0).sqrt(),
        (1.0 - axis.z * axis.z).max(0.0).sqrt(),
    ) * radial;
    let padding = (range * 1e-4).max(1e-4);
    (
        (centre - DVec3::splat(along)).max(origin.min(end - extent) - DVec3::splat(padding)),
        (centre + DVec3::splat(along)).min(origin.max(end + extent) + DVec3::splat(padding)),
    )
}
#[test]
fn finite_cone_box_contains_all_sampled_range_and_angle_eligible_receivers() {
    // Generate points independently by spherical rays and physical lens distance, not by
    // evaluating the bounding formula. Include axis-aligned/diagonal axes and edge rays.
    for axis in [
        DVec3::X,
        -DVec3::Y,
        DVec3::Z,
        DVec3::new(1.0, 2.0, -3.0).normalize(),
        DVec3::new(-7.0, 1.0, 2.0).normalize(),
    ] {
        let reference = if axis.y.abs() < 0.9 {
            DVec3::Y
        } else {
            DVec3::X
        };
        let tangent = axis.cross(reference).normalize();
        let bitangent = axis.cross(tangent);
        for cosine in [0.5_f64, 0.7, 0.95, 0.99999] {
            for range in [0.001_f64, 1.0, 100.0] {
                let origin = DVec3::new(3.25, -1.75, 9.0);
                let (low, high) = bounds(origin, axis, range, cosine);
                for ring in 0..=16 {
                    let angle = cosine.acos() * f64::from(ring) / 16.0;
                    for turn in 0..64 {
                        let azimuth = std::f64::consts::TAU * f64::from(turn) / 64.0;
                        let ray = axis * angle.cos()
                            + (tangent * azimuth.cos() + bitangent * azimuth.sin()) * angle.sin();
                        for depth in [0.0, 0.1, 0.5, 0.999999, 1.0] {
                            let point = origin + ray * (range * depth);
                            let tolerance = DVec3::splat(1e-9);
                            assert!(
                                point.cmpge(low - tolerance).all()
                                    && point.cmple(high + tolerance).all(),
                                "receiver outside bound: axis={axis:?}, cos={cosine}, range={range}, point={point:?}, low={low:?}, high={high:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn narrow_downward_cone_tightens_lateral_bounds_without_shortening_its_range() {
    let range = 20.0;
    let cosine = 10.0_f64.to_radians().cos();
    let (low, high) = bounds(DVec3::ZERO, -DVec3::Y, range, cosine);
    assert!(high.x - low.x < range * 0.4);
    assert!(low.y <= -range && high.y >= 0.0);
}
