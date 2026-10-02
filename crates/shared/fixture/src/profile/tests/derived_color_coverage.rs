//! TL-552 colour coverage: every shipped mode with a colour control gets a derived model through
//! the existing fitter, or an explicit reason. Auxiliary colour controls (CTC, tint, colour
//! point, a wheel in front of emitters, a colour macro) are parked at their neutral state and the
//! forward model knows the output exactly only while they stay there.
use super::derived_color_physical::{Fit, intent, runtime};
use super::*;
use crate::forward::*;

const LIBRARY: &str = "../../../assets/fixture-library";

/// The only shipped modes whose colour is honestly not describable, with the reason's gist.
const EXCLUDED: [(&str, &str, &str); 2] = [
    (
        "etc--source-four-led-series-2-lustr",
        "HSI Plus 7",
        "layered over the hue/saturation engine",
    ),
    (
        "etc--source-four-led-series-2-lustr",
        "HSIC Plus 7",
        "layered over the hue/saturation engine",
    ),
];

fn current_defaults(profile: &FixtureProfile) -> Vec<u32> {
    profile.modes[0]
        .channels
        .iter()
        .map(|c| c.default_raw.min(c.resolution.max_raw()))
        .collect()
}

fn set(profile: &FixtureProfile, current: &mut [u32], fixture_attribute: &str, raw: u32) {
    let index = profile.modes[0]
        .channels
        .iter()
        .position(|c| &*c.fixture_attribute.0 == fixture_attribute)
        .unwrap_or_else(|| panic!("{fixture_attribute}"));
    current[index] = raw;
}

#[test]
fn every_shipped_mode_with_a_colour_control_derives_a_model_or_names_why() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(LIBRARY);
    let mut paths: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    let (mut derived, mut excluded, mut media) = (0, Vec::new(), 0);
    for path in paths {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let source = crate::read_fixture_package(&std::fs::read(&path).unwrap()).unwrap();
        for mode in &source.modes {
            match mode.derived_color_outcome() {
                DerivedColorOutcome::Authored | DerivedColorOutcome::NoColorControls => {}
                DerivedColorOutcome::MediaColor => media += 1,
                DerivedColorOutcome::Excluded(reason) => {
                    assert!(!reason.trim().is_empty(), "{name} {}", mode.name);
                    excluded.push((name.clone(), mode.name.clone(), reason));
                }
                DerivedColorOutcome::Derived(_) => {
                    derived += 1;
                    let mut profile = source.clone();
                    profile.modes.retain(|m| m.id == mode.id);
                    apply_runtime_profile_compatibility(&mut profile);
                    every_head_shows_a_colour(&name, &profile);
                }
            }
        }
    }
    assert!(derived > 110, "derived modes: {derived}");
    assert_eq!(
        media, 2,
        "the Media Server personalities keep their own path"
    );
    assert_eq!(
        excluded.len(),
        EXCLUDED.len(),
        "no new silent or explained exclusion: {excluded:#?}"
    );
    for ((package, mode, reason), (want_package, want_mode, gist)) in excluded.iter().zip(EXCLUDED)
    {
        assert_eq!((package.as_str(), mode.as_str()), (want_package, want_mode));
        assert!(reason.contains(gist), "{package} {mode}: {reason}");
    }
}

/// Every derived head fits primaries and white to a complete forward prediction, starting from
/// the profile defaults, and writes every Color-owned control once.
fn every_head_shows_a_colour(name: &str, profile: &FixtureProfile) {
    let mode = &profile.modes[0];
    let fitting = CompiledColorFitting::compile(profile, mode.id, None)
        .unwrap_or_else(|e| panic!("{name} {}: {e}", mode.name))
        .expect("a derived model");
    let mut workspace = fitting.create_workspace();
    let current = current_defaults(profile);
    for head in 0..fitting.head_count() {
        let mut output = fitting.create_output(head).unwrap();
        for rgb in [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [1., 1., 1.]] {
            fitting
                .fit(head, &current, &intent(rgb), &mut workspace, &mut output)
                .unwrap();
            let context = format!("{name} {} head {head} {rgb:?}", mode.name);
            assert_eq!(output.status, ColorFitStatus::Fitted, "{context}");
            assert!(
                output.visible.achieved.is_some(),
                "{context}: incomplete prediction {:?} {:?}",
                output.visible.status,
                output.flags
            );
            assert_eq!(
                output.writes.len() + output.retained.len(),
                fitting.controls(head).unwrap().len(),
                "{context}"
            );
        }
    }
}

#[test]
fn jbled_a7_drives_rgb_and_parks_ctc_and_the_colour_macro_neutral() {
    for mode in ["Standard RGB 8 Bit (S8)", "Compressed RGB 8 Bit (C8)"] {
        let profile = runtime("jb-lighting--jbled-a7", mode);
        let mut current = current_defaults(&profile);
        set(&profile, &mut current, "color.temperature", 200);
        let wheel = profile.modes[0]
            .channels
            .iter()
            .any(|c| &*c.fixture_attribute.0 == "color.wheel.1");
        if wheel {
            set(&profile, &mut current, "color.wheel.1", 40);
        }
        let mut fit = Fit::new(profile);
        let red = fit.run_from(&current, &intent([1., 0., 0.]));
        assert_eq!(
            fit.raws(&[
                "color.red",
                "color.green",
                "color.blue",
                "color.temperature"
            ]),
            [Some(255), Some(0), Some(0), Some(0)],
            "{mode}"
        );
        if wheel {
            // "Colour mixing with RGB" [0-1]: the wheel macro is off.
            assert!(fit.raw("color.wheel.1").unwrap() <= 1, "{mode}");
        }
        assert_eq!(red.visible.color_match, ColorMatch::Exact, "{mode}");
        assert_eq!(red.visible.data_quality, PhysicalDataQuality::Unknown);
        let note = fit.profile.modes[0]
            .derived_color_note(fit.profile.modes[0].heads[0].id)
            .unwrap();
        assert!(
            note.contains("parked at neutral") && note.contains("CTC"),
            "{note}"
        );
    }
}

#[test]
fn robe_led_washes_and_beams_drive_their_emitters_with_ctc_and_macro_wheel_parked() {
    for (package, mode) in [
        ("robe--robin-300-ledwash", "Mode 3"),
        ("robe--robin-600x-ledwash", "Mode 2"),
        ("robe--robin-dlf-wash", "Mode 2"),
        ("robe--robin-dls-profile", "Mode 2"),
        ("robe--robin-ledbeam-150", "Mode 2 – Reduced 8-bit"),
    ] {
        let profile = runtime(package, mode);
        let mut current = current_defaults(&profile);
        set(&profile, &mut current, "color.temperature", 128);
        set(&profile, &mut current, "color.wheel.1", 175);
        let mut fit = Fit::new(profile);
        let blue = fit.run_from(&current, &intent([0., 0., 1.]));
        let rgbw = ["color.red", "color.green", "color.blue", "color.white"];
        let max = fit.raw("color.blue").unwrap();
        assert_eq!(
            fit.raws(&rgbw),
            [Some(0), Some(0), Some(max), Some(0)],
            "{package} {mode}"
        );
        assert!(max > 0);
        assert_eq!(
            fit.raws(&["color.temperature", "color.wheel.1"]),
            [Some(0), Some(0)],
            "{package} {mode}: CTC and the colour macro park at their neutral default"
        );
        assert_eq!(blue.visible.color_match, ColorMatch::Exact);
    }
}

#[test]
fn zoned_led_wash_fits_each_zone_and_leaves_the_shared_ctc_and_macro_at_default() {
    let profile = runtime("robe--robin-600x-ledwash", "Mode 1");
    let mode = &profile.modes[0];
    let model = mode.color_physical.as_ref().expect("a derived model");
    let shared = mode.heads.iter().find(|h| h.master_shared).unwrap();
    assert_eq!(model.paths.len(), 3, "one path per zone");
    assert!(model.paths.iter().all(|p| p.head_id != shared.id));
    for path in &model.paths {
        let note = mode.derived_color_note(path.head_id).unwrap();
        assert!(note.contains("shared, left at default"), "{note}");
    }
    assert_eq!(
        mode.color_model_exclusion(shared.id).as_deref(),
        Some("shared colour controls stay at their defaults")
    );
}

#[test]
fn prolights_ecl_and_mac_300_mode_4_park_their_extra_colour_controls() {
    let profile = runtime("prolights--ecl-fresnel-ct-plus-m", "STANDARD");
    let mut current = current_defaults(&profile);
    set(&profile, &mut current, "color.tint", 90);
    let mut fit = Fit::new(profile);
    fit.run_from(&current, &intent([1., 0., 0.]));
    assert_eq!(
        fit.raws(&[
            "color.red",
            "color.green",
            "color.blue",
            "color.temperature",
            "color.tint"
        ]),
        [Some(255), Some(0), Some(0), Some(0), Some(0)]
    );

    // CMY flags with a continuous colour wheel in front: the wheel parks open (its default).
    let profile = runtime("martin--mac-300", "Mode 4");
    let mut current = current_defaults(&profile);
    set(&profile, &mut current, "color.wheel.1", 120);
    let mut fit = Fit::new(profile);
    let red = fit.run_from(&current, &intent([1., 0., 0.]));
    assert_eq!(
        fit.raws(&[
            "color.cyan",
            "color.magenta",
            "color.yellow",
            "color.wheel.1"
        ]),
        [Some(0), Some(255), Some(255), Some(0)]
    );
    assert_eq!(red.visible.color_match, ColorMatch::Exact);
}

#[test]
fn hue_saturation_engines_fit_a_nominal_srgb_grid_with_estimated_data() {
    let profile = runtime("etc--source-four-led-series-2-lustr", "HSI");
    let path = &profile.modes[0].color_physical.as_ref().unwrap().paths[0];
    assert!(path.measurements.len() > 800, "a dense HSV grid");
    // Brightness stays Intensity's: the fixture's Intensity channel is not a Color control.
    assert_eq!(path.controls.len(), 2);
    let mut fit = Fit::new(profile);
    let red = fit.run(&intent([1., 0., 0.]));
    assert_eq!(
        fit.raws(&["color.hue", "color.saturation", "intensity"]),
        [Some(0), Some(255), None]
    );
    assert_eq!(red.visible.color_match, ColorMatch::Exact);
    assert_eq!(red.visible.data_quality, PhysicalDataQuality::Estimated);
    let cyan = fit.run(&intent([0., 1., 1.]));
    assert_eq!(cyan.visible.color_match, ColorMatch::Exact);
    assert_eq!(
        fit.raws(&["color.hue", "color.saturation"]),
        [Some(32768), Some(255)]
    );
    // Between grid points the nearest one is shown and the match says so.
    let orange = fit.run(&intent([1., 0.47, 0.]));
    assert!(matches!(
        orange.visible.color_match,
        ColorMatch::Exact | ColorMatch::Approximate
    ));

    // A colour macro channel next to the engine parks on its "No function" range.
    let profile = runtime("chauvet-professional--colorado-1-solo", "HSIC");
    let mut current = current_defaults(&profile);
    set(&profile, &mut current, "color.white", 200);
    let mut fit = Fit::new(profile);
    let green = fit.run_from(&current, &intent([0., 1., 0.]));
    assert!(fit.raw("color.white").unwrap() <= 10);
    assert_eq!(green.visible.color_match, ColorMatch::Exact);
}

#[test]
fn white_only_heads_show_white_and_report_a_colour_request_out_of_gamut() {
    for (package, mode, parked) in [
        (
            "etc--source-four-led-series-2-lustr",
            "Studio",
            "color.temperature",
        ),
        ("cameo--q-spot-40-tw", "3-CHANNEL 1", "color.temperature"),
    ] {
        let profile = runtime(package, mode);
        let mut current = current_defaults(&profile);
        set(&profile, &mut current, parked, 77);
        let mut fit = Fit::new(profile);
        let red = fit.run_from(&current, &intent([1., 0., 0.]));
        assert_eq!(fit.raw(parked), Some(0), "{package} {mode}");
        assert!(red.visible.achieved.is_some(), "{package} {mode}");
        assert_eq!(red.visible.color_match, ColorMatch::OutOfGamut);
        assert_eq!(red.visible.data_quality, PhysicalDataQuality::Unknown);
        let white = fit.run_from(&current, &intent([1., 1., 1.]));
        assert_eq!(white.visible.color_match, ColorMatch::Exact);
    }
}

#[test]
fn a_parked_control_moved_off_its_neutral_state_makes_the_prediction_unknown() {
    // Negative control: only the neutral state is known; a CTC at 200 is not invented as neutral.
    let profile = runtime("jb-lighting--jbled-a7", "Compressed RGB 8 Bit (C8)");
    let mode = &profile.modes[0];
    let forward = CompiledColorForward::compile(&profile, mode.id, None)
        .unwrap()
        .unwrap();
    let mut output = forward.create_output();
    let mut raw = current_defaults(&profile);
    set(&profile, &mut raw, "color.red", 255);
    set(&profile, &mut raw, "color.green", 0);
    set(&profile, &mut raw, "color.blue", 0);
    set(&profile, &mut raw, "color.temperature", 0);
    forward.evaluate(&raw, &mut output).unwrap();
    assert!(output[0].visible_complete, "neutral CTC: known red");
    set(&profile, &mut raw, "color.temperature", 200);
    forward.evaluate(&raw, &mut output).unwrap();
    assert!(!output[0].visible_complete, "CTC moved: unknown");
    assert!(
        output[0]
            .flags
            .contains(ColorForwardFlags::FILTER_SAMPLE_GAP)
    );
}

#[test]
fn only_a_unit_transmission_is_treated_as_no_filter() {
    // Negative control for the identity rule: a known flat half transmission still needs the
    // source spectrum, so an emitter without one stays unknown behind it.
    let mut profile = runtime("jb-lighting--jbled-a7", "Compressed RGB 8 Bit (C8)");
    let mode = &mut profile.modes[0];
    let path = &mut mode.color_physical.as_mut().unwrap().paths[0];
    let filter = path
        .filters
        .iter_mut()
        .find(|f| f.provenance.source.as_deref() == Some(DERIVED_PARKED_SOURCE))
        .unwrap();
    let OpticalTransmission::Spectral { samples } = &mut filter.transmission else {
        panic!("parked filters are spectral")
    };
    for value in &mut samples[0].spectrum {
        value.value = 0.5;
    }
    let forward = CompiledColorForward::compile(&profile, profile.modes[0].id, None)
        .unwrap()
        .unwrap();
    let mut output = forward.create_output();
    let mut raw = current_defaults(&profile);
    set(&profile, &mut raw, "color.temperature", 0);
    forward.evaluate(&raw, &mut output).unwrap();
    assert!(!output[0].visible_complete);
}
