//! TL-544 G8: mirror scanners, Tilt-only fixtures and endless axes through the one Position graph,
//! forward model and fitter.
use super::*;
use crate::forward::{CompiledPositionForward, PositionInstallation};
use light_core::spatial::RigidTransform as R;

fn package(name: &str) -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    crate::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

fn runtime(name: &str, mode: &str) -> FixtureProfile {
    let mut profile = package(name);
    profile.modes.retain(|m| m.name == mode);
    assert_eq!(profile.modes.len(), 1, "{name} has mode {mode}");
    apply_runtime_profile_compatibility(&mut profile);
    profile.validate().unwrap();
    profile
}

/// The forward beam (origin, direction) for joint Angles by role.
fn beam(profile: &FixtureProfile, pan: f64, tilt: f64) -> ([f64; 3], [f64; 3]) {
    let forward =
        CompiledPositionForward::compile(profile, profile.modes[0].id, Default::default())
            .unwrap()
            .unwrap();
    let axes: Vec<_> = forward
        .create_commands()
        .iter()
        .map(|c| match c.role {
            Some(PositionAxisRole::Pan) => Some(pan),
            Some(PositionAxisRole::Tilt) => Some(tilt),
            None => Some(0.),
        })
        .collect();
    let mut workspace = forward.create_workspace();
    let mut output = forward.create_output();
    forward
        .evaluate_pose(&axes, R::IDENTITY, &mut workspace, &mut output)
        .unwrap();
    let pose = output[0].world.expect("known pose");
    (pose.point([0.; 3]), pose.direction([0., -1., 0.]))
}

struct Fit {
    result: PositionFitResult,
    raw: Vec<u32>,
}

fn fit_request(
    profile: &FixtureProfile,
    request: PositionFitRequest,
    previous: Option<[f64; 2]>,
) -> Fit {
    let model = CompiledPositionFitting::compile(
        profile,
        profile.modes[0].id,
        PositionInstallation::default(),
    )
    .unwrap()
    .expect("a Position model");
    let raw: Vec<u32> = profile.modes[0]
        .channels
        .iter()
        .map(|c| c.default_raw)
        .collect();
    let previous_joints: Vec<_> = model
        .axes()
        .iter()
        .map(|a| match (a.role, previous) {
            (Some(PositionAxisRole::Pan), Some(p)) => Some(p[0]),
            (Some(PositionAxisRole::Tilt), Some(p)) => Some(p[1]),
            _ => None,
        })
        .collect();
    let mut workspace = model.create_workspace();
    let mut output = model.create_output();
    model
        .fit(
            PositionFitInput {
                current_raw: &raw,
                available: &vec![true; raw.len()],
                requests: &[Some(request)],
                previous: &previous_joints,
                mount: R::IDENTITY,
            },
            &mut workspace,
            &mut output,
        )
        .unwrap();
    Fit {
        result: output[0].clone(),
        raw: workspace.proposed_raw().to_vec(),
    }
}

fn angle_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dot: f64 = (0..3).map(|i| a[i] * b[i]).sum();
    let len = |v: [f64; 3]| v.iter().map(|c| c * c).sum::<f64>().sqrt();
    (dot / (len(a) * len(b))).clamp(-1., 1.).acos().to_degrees()
}

/// Independent reference: a lamp shining down onto a mirror whose rest normal is halfway between
/// up and forward, panned about the lamp axis and tilted by half the beam angle.
fn reflected(pan: f64, tilt: f64) -> [f64; 3] {
    let n0 = [
        0.,
        std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
    ];
    let n = R::axis_angle([0., 1., 0.], pan)
        .unwrap()
        .compose(R::axis_angle([1., 0., 0.], tilt / 2.).unwrap())
        .direction(n0);
    let d = [0., -1., 0.];
    let k = 2. * (0..3).map(|i| d[i] * n[i]).sum::<f64>();
    std::array::from_fn(|i| d[i] - k * n[i])
}

#[test]
fn shipped_scanners_derive_mirror_kinematics() {
    for (name, mode) in [
        ("eurolite--ts-255-dmx-scan", "6-Channel"),
        ("high-end-systems--trackspot", "DMX Low Resolution"),
        ("generic--mirror-mover-scanner", "Dimmer, Pan, Tilt"),
    ] {
        let stored = package(name);
        let id = stored.modes.iter().find(|m| m.name == mode).unwrap().id;
        assert_eq!(
            position_derivation(&stored, id),
            PositionDerivation::Derived
        );
        let profile = runtime(name, mode);
        let model = profile.modes[0].position_physical.as_ref().unwrap();
        let mirror = model.kinematics.mirror.as_ref().expect("{name}: mirror");
        assert_eq!(mirror.axis_ratios.len(), 1, "{name}: only Tilt is halved");
        let graph = profile.mode_geometry(&profile.modes[0]);
        assert!(is_derived_position_geometry(&graph));
        assert_eq!(
            graph
                .physical_contract
                .as_ref()
                .unwrap()
                .provenance
                .source
                .as_deref(),
            Some(DERIVED_SCANNER_SOURCE)
        );
        // At rest the beam leaves the mirror centre forward, not down like a moving head.
        let (origin, direction) = beam(&profile, 0., 0.);
        assert!(origin.iter().all(|v| v.abs() < 1e-9), "{name} {origin:?}");
        assert!(angle_between(direction, [0., 0., 1.]) < 1e-6, "{name}");
    }
    // Without declared degrees a scanner gets the nominal beam deflection, not a moving head's.
    let trackspot = runtime("high-end-systems--trackspot", "DMX Low Resolution");
    let travel: Vec<_> = trackspot
        .mode_geometry(&trackspot.modes[0])
        .nodes
        .iter()
        .filter_map(|n| n.motion.as_ref())
        .map(|m| (m.physical_min, m.physical_max))
        .collect();
    assert_eq!(travel, vec![(-90.0, 90.0), (-45.0, 45.0)]);
}

#[test]
fn a_scanner_beam_is_the_reflection_of_its_lamp_by_the_moving_mirror() {
    let profile = runtime("eurolite--ts-255-dmx-scan", "6-Channel");
    for (pan, tilt) in [(30., 0.), (0., 20.), (-60., -35.), (75., 40.)] {
        let (origin, direction) = beam(&profile, pan, tilt);
        assert!(
            origin.iter().all(|v| v.abs() < 1e-9),
            "the mirror centre stays put"
        );
        let expected = reflected(pan, tilt);
        assert!(
            angle_between(direction, expected) < 1e-6,
            "{pan}/{tilt}: {direction:?} != {expected:?}"
        );
    }
    // In the plane of incidence the mirror turns half as far as the beam it deflects.
    let (_, tilted) = beam(&profile, 0., 30.);
    assert!((angle_between(tilted, [0., 0., 1.]) - 30.).abs() < 1e-6);
    // Negative control: without the mirror the same lens is a rigid head pointing along the
    // mirror normal, 45° off the real beam, and its Tilt turns the beam only one to one.
    let mut rigid = profile.clone();
    let model = rigid.modes[0].position_physical.as_mut().unwrap();
    model.kinematics = PositionKinematics::default();
    let (_, normal) = beam(&rigid, 0., 0.);
    assert!((angle_between(normal, [0., 0., 1.]) - 45.).abs() < 1e-6);
    let (_, rigid_tilt) = beam(&rigid, 0., 30.);
    assert!((angle_between(rigid_tilt, normal) - 30.).abs() < 1e-6);
}

#[test]
fn a_scanner_target_is_aimed_through_the_mirror() {
    let profile = runtime("eurolite--ts-255-dmx-scan", "6-Channel");
    let (pan, tilt) = (25., -15.);
    let direction = reflected(pan, tilt);
    let target = direction.map(|v| v * 6.);
    let fit = fit_request(
        &profile,
        PositionFitRequest::Target {
            world: Some(target),
        },
        None,
    );
    let result = &fit.result;
    assert_eq!(result.status, PositionFitStatus::Fitted, "{result:?}");
    let achieved = result.achieved.unwrap();
    // 8-bit steps: Pan 180°/255 and Tilt 90°/255.
    assert!((achieved[0] - pan).abs() < 0.75 && (achieved[1] - tilt).abs() < 0.4);
    assert!(result.angular_error_degrees.unwrap() < 0.75);
    assert!(result.writes.iter().all(Option::is_some));
    let pan_channel = profile.modes[0]
        .channels
        .iter()
        .position(|c| &*c.attribute.0 == "pan")
        .unwrap();
    assert_eq!(
        fit.raw[pan_channel],
        ((pan / 180. + 0.5) * 255.).round() as u32
    );
}

#[test]
fn reflected_beam_derivatives_match_finite_differences() {
    // A mirror whose lamp rides the Pan node too, so Pan turns lamp and mirror together.
    let mut profile = runtime("eurolite--ts-255-dmx-scan", "6-Channel");
    let model = profile.modes[0].position_physical.as_mut().unwrap();
    let pan_node = model
        .bindings
        .iter()
        .find(|b| b.role == PositionAxisRole::Pan)
        .unwrap()
        .node_id;
    let mirror = model.kinematics.mirror.as_mut().unwrap();
    mirror.source_node_id = pan_node;
    mirror.incident = Vector3 {
        x: 0.3,
        y: -1.0,
        z: 0.2,
    };
    profile.validate().unwrap();
    for profile in [runtime("eurolite--ts-255-dmx-scan", "6-Channel"), profile] {
        let forward =
            CompiledPositionForward::compile(&profile, profile.modes[0].id, Default::default())
                .unwrap()
                .unwrap();
        let pair = [0, 1];
        let chain = forward.fitting_ancestry(0);
        let at = |pan: f64, tilt: f64| {
            forward
                .fitting_lens_geometry(0, &chain, &[Some(pan), Some(tilt)], R::IDENTITY, pair)
                .unwrap()
        };
        let (pan, tilt, h) = (17., -11., 1e-4);
        let (_, tangents) = at(pan, tilt);
        for (j, delta) in [[h, 0.], [0., h]].into_iter().enumerate() {
            let (plus, _) = at(pan + delta[0], tilt + delta[1]);
            let (minus, _) = at(pan - delta[0], tilt - delta[1]);
            let numeric: [f64; 3] = std::array::from_fn(|i| {
                (plus.direction([0., -1., 0.])[i] - minus.direction([0., -1., 0.])[i]) / (2. * h)
            });
            for i in 0..3 {
                assert!(
                    (numeric[i] - tangents[j].direction[i]).abs() < 1e-6,
                    "axis {j}: {numeric:?} vs {:?}",
                    tangents[j].direction
                );
            }
        }
    }
}

#[test]
fn a_tilt_only_fixture_gets_a_fixed_pan_and_aims_as_closely_as_its_tilt_allows() {
    let stored = package("glp--jdc1");
    for mode in &stored.modes {
        assert_eq!(
            position_derivation(&stored, mode.id),
            PositionDerivation::Derived,
            "{}",
            mode.name
        );
    }
    let profile = runtime("glp--jdc1", "Normal 23-channel");
    let model = profile.modes[0].position_physical.as_ref().unwrap();
    assert_eq!(model.bindings.len(), 1);
    assert_eq!(model.kinematics.fixed_axes.len(), 1);
    assert_eq!(model.kinematics.fixed_axes[0].role, PositionAxisRole::Pan);
    let tilt_channel = profile.modes[0]
        .channels
        .iter()
        .position(|c| &*c.attribute.0 == "tilt")
        .unwrap();
    // Angles: Tilt is exact (0..185° re-centred), the fixed Pan is reported, never written.
    let fit = fit_request(
        &profile,
        PositionFitRequest::Angles {
            pan: 0.,
            tilt: 46.25,
        },
        None,
    );
    assert_eq!(fit.result.status, PositionFitStatus::Fitted);
    assert!(!fit.result.clipped);
    assert!(fit.result.writes[0].is_none() && fit.result.writes[1].is_some());
    let achieved = fit.result.achieved.unwrap();
    assert!(achieved[0] == 0. && (achieved[1] - 46.25).abs() < 0.01);
    assert!((fit.raw[tilt_channel] as i64 - 49151).abs() <= 1);
    let panned = fit_request(
        &profile,
        PositionFitRequest::Angles {
            pan: 30.,
            tilt: 46.25,
        },
        None,
    );
    assert_eq!(panned.result.status, PositionFitStatus::Fitted);
    assert!(panned.result.clipped, "the fixed Pan cannot follow");
    assert_eq!(panned.result.achieved.unwrap()[0], 0.);
    // A Target in the Tilt plane is reached exactly.
    let tilt = 40f64.to_radians();
    let reachable = [0., -tilt.cos() * 5., tilt.sin() * 5.];
    let fit = fit_request(
        &profile,
        PositionFitRequest::Target {
            world: Some(reachable),
        },
        None,
    );
    assert_eq!(
        fit.result.status,
        PositionFitStatus::Fitted,
        "{:?}",
        fit.result
    );
    assert!(fit.result.angular_error_degrees.unwrap() < 0.01);
    assert!(!fit.result.clipped);
    // A Target to the side is approached as closely as Tilt allows and the shortfall reported.
    let aside = [3., -4., 3.];
    let fit = fit_request(
        &profile,
        PositionFitRequest::Target { world: Some(aside) },
        None,
    );
    assert_eq!(
        fit.result.status,
        PositionFitStatus::Fitted,
        "{:?}",
        fit.result
    );
    assert!(fit.result.clipped);
    let error = fit.result.angular_error_degrees.unwrap();
    // The closest beam lies in the Tilt plane: the error is the target's angle out of it.
    let out_of_plane = (3f64 / (9f64 + 16. + 9.).sqrt()).asin().to_degrees();
    assert!(
        (error - out_of_plane).abs() < 0.05,
        "{error} vs {out_of_plane}"
    );
}

#[test]
fn an_endless_axis_is_a_signed_multi_turn_absolute_axis() {
    let stored = package("generic--endless-pan-tilt");
    assert_eq!(
        position_derivation(&stored, stored.modes[0].id),
        PositionDerivation::Derived
    );
    let profile = runtime("generic--endless-pan-tilt", "Endless Pan/Tilt 16-bit");
    let pan = profile.modes[0]
        .channels
        .iter()
        .position(|c| &*c.attribute.0 == "pan")
        .unwrap();
    let fit = fit_request(
        &profile,
        PositionFitRequest::Angles {
            pan: 600.,
            tilt: -400.,
        },
        None,
    );
    assert_eq!(fit.result.status, PositionFitStatus::Fitted);
    let achieved = fit.result.achieved.unwrap();
    assert!((achieved[0] - 600.).abs() < 0.05 && (achieved[1] + 400.).abs() < 0.05);
    let expected = ((600_f64 / 1440. + 0.5) * 65535.).round() as i64;
    assert!((fit.raw[pan] as i64 - expected).abs() <= 1);
    // A Target keeps the accumulated turns nearest the previous pose rather than unwinding.
    let tilted = 30f64.to_radians();
    let ahead = [0., -tilted.cos(), -tilted.sin()];
    let fit = fit_request(
        &profile,
        PositionFitRequest::Target { world: Some(ahead) },
        Some([700., 0.]),
    );
    assert_eq!(
        fit.result.status,
        PositionFitStatus::Fitted,
        "{:?}",
        fit.result
    );
    let achieved = fit.result.achieved.unwrap();
    assert!((achieved[0] - 720.).abs() < 0.05, "{achieved:?}");
    assert!((achieved[1] - 30.).abs() < 0.05, "{achieved:?}");
}

#[test]
fn moving_head_identity_is_unchanged_and_kinematics_enter_the_scanner_identity() {
    // Mirror and fixed axes are physics: they belong to the calibration identity. A moving head
    // (no kinematics) keeps exactly the identity it had before kinematics existed.
    let scanner = runtime("eurolite--ts-255-dmx-scan", "6-Channel");
    let mut without = scanner.clone();
    without.modes[0]
        .position_physical
        .as_mut()
        .unwrap()
        .kinematics = PositionKinematics::default();
    let id = scanner.modes[0].id;
    assert_ne!(
        scanner.position_calibration_identity(id).unwrap(),
        without.position_calibration_identity(id).unwrap()
    );
    let json = serde_json::to_value(without.modes[0].position_physical.as_ref().unwrap()).unwrap();
    assert!(
        json.get("kinematics").is_none(),
        "a moving head serialises as before"
    );
    // Validation: a fixed axis may not also be driven.
    let mut jdc1 = runtime("glp--jdc1", "Normal 23-channel");
    let model = jdc1.modes[0].position_physical.as_mut().unwrap();
    model.kinematics.fixed_axes[0].node_id = model.bindings[0].node_id;
    assert!(jdc1.validate().is_err());
}
