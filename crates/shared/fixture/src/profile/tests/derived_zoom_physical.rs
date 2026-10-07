//! G8c Zoom conventions: every shipped mode with a Zoom function is authored, derived (documented
//! or nominal, with its convention and quality) or excluded with a named reason. The projection
//! is transient: package bytes and the decoded source profile never change.
use super::derived_color_physical::{package, runtime};
use super::*;

const LIBRARY: &str = "../../../assets/fixture-library";

#[derive(Clone, Copy, Debug, PartialEq)]
enum Want {
    Authored,
    Derived(OpeningConvention, PhysicalDataQuality, (f32, f32)),
    Excluded(ZoomDerivationExclusion),
}

use OpeningConvention::{Beam, Field};
use PhysicalDataQuality::{Estimated, Manufacturer};
use ZoomDerivationExclusion as X;

/// Every shipped package with a Zoom function and what each of its modes gets. Degrees are at
/// the function's `physical_min` and `physical_max` (the first and last DMX value).
const EXPECTED: [(&str, Want); 15] = [
    ("cameo--auro-spot-z300", Want::Authored),
    (
        "chauvet-professional--colorado-1-solo",
        Want::Derived(Beam, Estimated, (4.0, 40.0)),
    ),
    (
        "claypaky--stage-zoom-1200",
        Want::Derived(Beam, Estimated, (24.0, 16.0)),
    ),
    (
        "claypaky--stage-zoom-1200-sv",
        Want::Derived(Beam, Estimated, (24.0, 16.0)),
    ),
    // A generic profile without a manual: the Stage's nominal Profile cone.
    (
        "generic--beam-size-edge",
        Want::Derived(Beam, Estimated, (10.0, 32.0)),
    ),
    ("generic--cold-spark", Want::Excluded(X::NotABeamOpening)),
    (
        "generic--five-nozzle-flame",
        Want::Excluded(X::NotABeamOpening),
    ),
    ("generic--flame-jet", Want::Excluded(X::NotABeamOpening)),
    (
        "jb-lighting--jbled-a7",
        Want::Derived(Field, Estimated, (12.0, 36.0)),
    ),
    (
        "robe--robin-300-ledwash",
        Want::Derived(Beam, Estimated, (15.0, 60.0)),
    ),
    (
        "robe--robin-600x-ledwash",
        Want::Derived(Beam, Estimated, (8.0, 63.0)),
    ),
    (
        "robe--robin-dlf-wash",
        Want::Derived(Beam, Manufacturer, (5.5, 60.0)),
    ),
    (
        "robe--robin-dls-profile",
        Want::Derived(Beam, Manufacturer, (45.0, 10.0)),
    ),
    (
        "robe--robin-ledbeam-150",
        Want::Derived(Beam, Manufacturer, (60.0, 3.8)),
    ),
    (
        "tosklight--visualizer-laser",
        Want::Excluded(X::LaserPatternSize),
    ),
];

fn zoom_functions(mode: &FixtureMode) -> Vec<&ChannelFunction> {
    mode.channels
        .iter()
        .flat_map(|c| &c.functions)
        .filter(|f| f.attribute.0.as_ref() == "zoom")
        .collect()
}

/// The Zoom functions as stored, for an exact "untouched" comparison.
fn zoom_json(mode: &FixtureMode) -> String {
    serde_json::to_string(&zoom_functions(mode)).unwrap()
}

fn fit_zoom(mode: &FixtureMode, degrees: f64) -> OpticsFit {
    let fitting = CompiledOpticsFitting::compile(mode).unwrap();
    let mut workspace = fitting.create_workspace();
    let mut output = fitting.create_output();
    let current: Vec<u32> = mode.channels.iter().map(|c| c.default_raw).collect();
    let request = OpticsFitRequest {
        focus: None,
        zoom: Some(ZoomFitRequest {
            degrees,
            convention: None,
            function_id: None,
        }),
    };
    let requests = vec![request; fitting.head_count()];
    fitting
        .fit(&current, &requests, &mut workspace, &mut output)
        .unwrap();
    output
        .iter()
        .map(|r| r.zoom)
        .find(|z| z.status != OpticsFitStatus::Unsupported)
        .expect("a head owns the Zoom")
}

#[test]
fn every_shipped_zoom_mode_is_authored_derived_or_excluded_with_a_reason() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(LIBRARY);
    let mut paths: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    let mut seen = Vec::new();
    let mut modes = 0;
    for path in paths {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let bytes = std::fs::read(&path).unwrap();
        let source = crate::read_fixture_package(&bytes).unwrap();
        let before = serde_json::to_string(&source).unwrap();
        let mut projected = source.clone();
        apply_runtime_profile_compatibility(&mut projected);
        let zoom_modes: Vec<_> = source
            .modes
            .iter()
            .filter(|m| !zoom_functions(m).is_empty())
            .collect();
        if zoom_modes.is_empty() {
            continue;
        }
        let want = EXPECTED
            .iter()
            .find(|(package, _)| *package == name)
            .unwrap_or_else(|| panic!("{name}: a Zoom package missing from EXPECTED (silent)"))
            .1;
        seen.push(name.clone());
        for mode in zoom_modes {
            modes += 1;
            let context = format!("{name} {}", mode.name);
            let outcome = zoom_derivation(&source, mode);
            let after = projected.mode(mode.id).unwrap();
            match want {
                Want::Authored => {
                    assert_eq!(outcome, ZoomDerivation::Authored, "{context}");
                    assert_eq!(zoom_json(after), zoom_json(mode), "{context}");
                }
                Want::Excluded(reason) => {
                    assert_eq!(outcome, ZoomDerivation::Excluded(reason), "{context}");
                    assert!(!reason.reason().is_empty());
                    // Untouched: still no degrees, so a degree request stays unsupported.
                    assert_eq!(zoom_json(after), zoom_json(mode), "{context}");
                }
                Want::Derived(convention, quality, degrees) => {
                    let ZoomDerivation::Derived(derived) = outcome else {
                        panic!("{context}: {outcome:?}");
                    };
                    for zoom in &derived {
                        assert_eq!(
                            (zoom.convention, zoom.quality, zoom.degrees),
                            (convention, quality, degrees),
                            "{context}"
                        );
                        assert!(!zoom.source.trim().is_empty(), "{context}");
                    }
                    for function in zoom_functions(after) {
                        let ChannelFunctionBehavior::Continuous {
                            physical_min,
                            physical_max,
                            unit,
                        } = &function.behavior
                        else {
                            panic!("{context}")
                        };
                        assert_eq!((*physical_min, *physical_max), degrees, "{context}");
                        assert_eq!(unit.as_deref(), Some("degrees"), "{context}");
                        let mapping = function.physical_mapping.as_ref().unwrap();
                        assert_eq!(mapping.opening_convention, Some(convention));
                        assert_eq!(mapping.quality, quality, "{context}");
                    }
                    // The existing fitter now fits a degree request inside the range.
                    let middle = f64::from(degrees.0 + degrees.1) / 2.0;
                    let fit = fit_zoom(after, middle);
                    assert_eq!(fit.status, OpticsFitStatus::Fitted, "{context}");
                    assert_eq!(fit.convention, Some(convention), "{context}");
                    assert_eq!(fit.quality, Some(quality), "{context}");
                    let achieved = fit.achieved.unwrap();
                    assert!((achieved - middle).abs() < 0.5, "{context}: {achieved}");
                }
            }
        }
        // The projection is transient: the decoded source and the package bytes are unchanged.
        assert_eq!(serde_json::to_string(&source).unwrap(), before, "{name}");
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "{name}");
        projected.validate().unwrap();
    }
    let mut expected: Vec<String> = EXPECTED.iter().map(|(p, _)| (*p).to_owned()).collect();
    seen.sort();
    expected.sort();
    assert_eq!(
        seen, expected,
        "every Zoom package is classified, none silently"
    );
    assert!(modes >= 40, "zoom modes: {modes}");
}

#[test]
fn robin_dls_zoom_stays_descending_from_maximum_to_minimum_beam_angle() {
    let profile = runtime("robe--robin-dls-profile", "Mode 2");
    let mode = &profile.modes[0];
    let fit = |degrees| fit_zoom(mode, degrees);
    assert_eq!(fit(45.0).write.unwrap().raw, 0, "DMX 0 is the widest beam");
    assert_eq!(
        fit(10.0).write.unwrap().raw,
        255,
        "DMX 255 is the narrowest"
    );
    let middle = fit(27.5);
    assert!((127..=128).contains(&middle.write.unwrap().raw));
    assert_eq!(middle.quality, Some(Manufacturer));
    assert_eq!(middle.convention, Some(Beam));
    // Outside the documented range the request is clipped, never extrapolated.
    let wide = fit(70.0);
    assert!(wide.clipped);
    assert_eq!(wide.achieved, Some(45.0));
    // The 16-bit mode keeps the same direction.
    let profile = runtime("robe--robin-dls-profile", "Mode 1");
    assert_eq!(fit_zoom(&profile.modes[0], 10.0).write.unwrap().raw, 65535);
}

#[test]
fn stage_zoom_percent_travel_becomes_degrees_wide_at_dmx_zero() {
    let profile = runtime("claypaky--stage-zoom-1200", "16-Channel");
    let mode = &profile.modes[0];
    let narrow = fit_zoom(mode, 16.0);
    assert_eq!(narrow.write.unwrap().raw, 255);
    assert_eq!(fit_zoom(mode, 24.0).write.unwrap().raw, 0);
    let twenty = fit_zoom(mode, 20.0);
    assert!((127..=128).contains(&twenty.write.unwrap().raw));
    assert_eq!(twenty.quality, Some(Estimated));
    let function = zoom_functions(mode)[0];
    let source = function
        .physical_mapping
        .as_ref()
        .unwrap()
        .source
        .as_deref();
    assert!(source.unwrap().contains("16°–24°"), "{source:?}");
}

#[test]
fn a_non_optical_zoom_attribute_is_excluded_and_keeps_its_travel() {
    for (name, reason) in [
        ("generic--cold-spark", X::NotABeamOpening),
        ("tosklight--visualizer-laser", X::LaserPatternSize),
    ] {
        let source = package(name);
        let mode = &source.modes[0];
        assert_eq!(
            zoom_derivation(&source, mode),
            ZoomDerivation::Excluded(reason)
        );
        let mut projected = source.clone();
        apply_runtime_profile_compatibility(&mut projected);
        let after = &projected.modes[0];
        assert_eq!(zoom_json(after), zoom_json(mode), "{name}");
        assert!(zoom_functions(after)[0].physical_mapping.is_none());
        // Negative control: a degree request is not invented for spark lifetime or pattern size.
        assert_eq!(
            fit_zoom(after, 20.0).status,
            OpticsFitStatus::UnknownPhysicalMapping,
            "{name}"
        );
    }
}

#[test]
fn nominal_zoom_keeps_the_authored_direction_and_is_labelled_estimated() {
    // A profile without a documented range: nominal travel 0 = narrow, so a descending
    // normalized travel stays descending.
    let mut profile = package("generic--beam-size-edge");
    let function = profile.modes[0]
        .channels
        .iter_mut()
        .flat_map(|c| c.functions.iter_mut())
        .find(|f| f.attribute.0.as_ref() == "zoom")
        .unwrap();
    function.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 1.0,
        physical_max: 0.0,
        unit: None,
    };
    apply_runtime_profile_compatibility(&mut profile);
    let function = zoom_functions(&profile.modes[0])[0];
    let ChannelFunctionBehavior::Continuous {
        physical_min,
        physical_max,
        unit,
    } = &function.behavior
    else {
        panic!("still continuous")
    };
    assert_eq!((*physical_min, *physical_max), (32.0, 10.0));
    assert_eq!(unit.as_deref(), Some("degrees"));
    let mapping = function.physical_mapping.as_ref().unwrap();
    assert_eq!(mapping.quality, Estimated);
    assert_eq!(mapping.source.as_deref(), Some(DERIVED_ZOOM_SOURCE));
    assert_eq!(mapping.opening_convention, Some(Beam));
}
