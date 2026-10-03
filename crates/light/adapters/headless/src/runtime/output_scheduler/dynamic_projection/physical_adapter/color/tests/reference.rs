//! TL-557 AC4 executable reference cases on real captured frames: white temperature and tint,
//! dim recipes and black, and constrained wheels. White Blend 0/50/100 and UV black/dim live
//! in `tests.rs`; Dynamics arbitration in `programming_projection/tests/color_arbitration.rs`.
use super::super::profiles::*;
use super::super::tests::intent;
use super::super::*;
use super::destinations::Rig;
use light_core::NativeColorValue;
use light_core::programming::{ColorWheelConstraint, WhiteTarget};
use light_fixture::forward::{ColorConstraintStatus, white_target_xyz};

fn uv_prime(xyz: Xyz) -> (f64, f64) {
    let [x, y, z] = [xyz.x, xyz.y, xyz.z].map(f64::from);
    let d = x + 15. * y + 3. * z;
    (4. * x / d, 9. * y / d)
}

fn delta(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

#[test]
fn white_temperature_and_tint_are_fitted_and_their_error_is_reported_honestly() {
    let rig = Rig::new(patched(&rgbal(), FixtureId::new(), 1));
    for kelvin in [2700., 3200., 4000., 5600., 6500.] {
        let mut tint_v = Vec::new();
        for duv in [-0.006, 0., 0.006] {
            let target = WhiteTarget { kelvin, duv };
            let mut request = intent([1., 1., 1.], 0.);
            request.white_blend = 1.;
            request.white_target = target;
            let resolved = rig.resolve(&request, None);
            resolved.verify_every_head();
            let quality = &resolved.result.quality;
            let achieved = resolved
                .result
                .achieved
                .visible
                .expect("known visible output");
            let error = delta(uv_prime(achieved), uv_prime(white_target_xyz(target)));
            let reported = quality.delta_uv.expect("a chromatic error is reported");
            assert!(
                (error - reported).abs() < 1e-4,
                "{kelvin} K {duv}: reported {reported} vs forward {error}"
            );
            if quality.color_match == ColorMatch::Exact {
                assert!(error < 0.002, "{kelvin} K {duv}: exact within Δu'v' 0.002");
            }
            assert_eq!(
                resolved.result.requested.semantic().unwrap().white_target,
                target
            );
            tint_v.push(uv_prime(achieved).1);
        }
        assert!(
            tint_v[0] < tint_v[1] && tint_v[1] < tint_v[2],
            "{kelvin} K: positive Duv is greener (higher v'): {tint_v:?}"
        );
    }
}

#[test]
fn dim_recipes_scale_luminance_once_keep_chromaticity_and_reach_black() {
    let rig = Rig::new(patched(&rgb(), FixtureId::new(), 1));
    let full = rig.resolve(&intent([1., 0., 1.], 0.), None);
    let full_xyz = full.result.achieved.visible.unwrap();
    for relative in [0.5, 0.25, 0.1] {
        let mut request = intent([1., 0., 1.], 0.);
        request.relative_output = relative;
        let dim = rig.resolve(&request, None);
        dim.verify_every_head();
        let xyz = dim.result.achieved.visible.unwrap();
        let ratio = f64::from(xyz.y / full_xyz.y);
        assert!(
            (ratio - f64::from(relative)).abs() < 0.02,
            "relativeOutput {relative}: Y ratio {ratio}"
        );
        assert!(delta(uv_prime(xyz), uv_prime(full_xyz)) < 0.003);
        assert_eq!(
            dim.result.requested.semantic().unwrap().relative_output,
            relative
        );
    }
    let mut black = intent([1., 0., 1.], 0.);
    black.relative_output = 0.;
    let black = rig.resolve(&black, None);
    assert_eq!(black.raws(rig.target), [0, 0, 0]);
    assert_eq!(black.result.quality.color_match, ColorMatch::Exact);
    assert_eq!(
        black.result.requested.semantic().unwrap().recipe.rgb,
        [1., 0., 1.],
        "black keeps the authored recipe"
    );
}

#[test]
fn a_matching_wheel_constraint_pins_its_slot_and_a_foreign_one_stays_passive() {
    let profile = wheel_only();
    let rig = Rig::new(patched(&profile, FixtureId::new(), 1));
    let mode = &profile.modes[0];
    let channel = &mode.channels[1];
    let constraint = |source| ColorWheelConstraint {
        source,
        value: NativeColorValue {
            channel_id: channel.id,
            function_id: channel.functions[0].id,
            raw: 20,
        },
    };
    let own = profile
        .native_color_identity(mode.id, mode.heads[0].id)
        .unwrap();
    // Blue is requested; the operator pinned the red slot of this exact fixture.
    let mut pinned = intent([0., 0., 1.], 0.);
    pinned.wheel_constraints = vec![constraint(own)];
    let resolved = rig.resolve(&pinned, None);
    resolved.verify_every_head();
    assert_eq!(resolved.raws(rig.target), [20], "the constraint wins");
    assert_eq!(
        resolved.result.quality.constraints[0].status,
        ColorConstraintStatus::Applied
    );
    assert_eq!(
        resolved.result.requested, pinned,
        "the constraint is retained"
    );

    // A constraint recorded on another fixture is reported, never reinterpreted.
    let other = wheel_only();
    let foreign = other
        .native_color_identity(other.modes[0].id, other.modes[0].heads[0].id)
        .unwrap();
    let mut moved = intent([0., 0., 1.], 0.);
    moved.wheel_constraints = vec![constraint(foreign)];
    let resolved = rig.resolve(&moved, None);
    assert_eq!(
        resolved.raws(rig.target),
        [39],
        "fitted blue, not the foreign slot"
    );
    assert_eq!(
        resolved.result.quality.constraints[0].status,
        ColorConstraintStatus::SourceMismatch
    );
    assert!(
        resolved
            .result
            .quality
            .limitations
            .contains(ColorFitLimitations::CONSTRAINT_REJECTED)
    );
    assert_eq!(resolved.result.requested, moved);
}
