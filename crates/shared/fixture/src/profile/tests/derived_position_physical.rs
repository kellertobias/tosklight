//! TL-552 Position fallback: modes with Pan/Tilt channels but no authored Position physical graph
//! get a nominal, estimated one at runtime compile time, fitted by the existing compiler.
use super::*;
use crate::forward::PositionInstallation;
use light_core::spatial::RigidTransform as R;

fn package(name: &str) -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    crate::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

/// The shipped profile reduced to one mode, after the runtime projection.
fn runtime(name: &str, mode: &str) -> FixtureProfile {
    let mut profile = package(name);
    profile.modes.retain(|m| m.name == mode);
    assert_eq!(profile.modes.len(), 1, "{name} has mode {mode}");
    apply_runtime_profile_compatibility(&mut profile);
    profile.validate().unwrap();
    profile
}

/// One Angles request through the existing fitter: (raw of each written channel, achieved).
fn fit(
    profile: &FixtureProfile,
    install: PositionInstallation<'_>,
    angles: [f64; 2],
) -> (Vec<(usize, u32)>, [f64; 2], PositionFitResult) {
    let model = CompiledPositionFitting::compile(profile, profile.modes[0].id, install)
        .unwrap()
        .expect("the derived model compiles into the existing fitter");
    assert_eq!(model.emitters().len(), 1, "one derived lens");
    let mode = &profile.modes[0];
    let raw: Vec<u32> = mode.channels.iter().map(|c| c.default_raw).collect();
    let mut workspace = model.create_workspace();
    let mut output = model.create_output();
    model
        .fit(
            PositionFitInput {
                current_raw: &raw,
                available: &vec![true; raw.len()],
                requests: &[Some(PositionFitRequest::Angles {
                    pan: angles[0],
                    tilt: angles[1],
                })],
                previous: &vec![None; model.axes().len()],
                mount: R::IDENTITY,
            },
            &mut workspace,
            &mut output,
        )
        .unwrap();
    let result = output[0].clone();
    assert_eq!(result.status, PositionFitStatus::Fitted, "{result:?}");
    let writes = result
        .writes
        .iter()
        .flatten()
        .map(|w| (w.channel_index as usize, w.raw))
        .collect();
    (writes, result.achieved.unwrap(), result)
}

fn channel_index(profile: &FixtureProfile, attribute: &str) -> usize {
    profile.modes[0]
        .channels
        .iter()
        .position(|c| &*c.attribute.0 == attribute)
        .unwrap()
}

fn raw_of(writes: &[(usize, u32)], index: usize) -> u32 {
    writes.iter().find(|(i, _)| *i == index).unwrap().1
}

#[test]
fn sixteen_bit_declared_travel_is_centred_and_drives_both_bytes() {
    // Robin 300 LEDWash declares Pan 0..450° and Tilt 0..300° on 16-bit coarse/fine channels.
    let profile = runtime("robe--robin-300-ledwash", "Mode 1");
    let mode = &profile.modes[0];
    let stored = package("robe--robin-300-ledwash");
    assert_eq!(
        position_derivation(&stored, stored.modes[0].id),
        PositionDerivation::Derived
    );
    let graph = profile.mode_geometry(mode);
    assert!(is_derived_position_geometry(&graph));
    let contract = graph.physical_contract.as_ref().unwrap();
    assert_eq!(contract.provenance.quality, PhysicalDataQuality::Estimated);
    // The Stage's proxy reads the motion nodes, so their travel is the derived mapping's.
    let travel = |attribute: &str| {
        graph
            .nodes
            .iter()
            .filter_map(|n| n.motion.as_ref())
            .filter(|m| m.attribute.as_ref().is_some_and(|a| &*a.0 == attribute))
            .map(|m| (m.physical_min, m.physical_max))
            .collect::<Vec<_>>()
    };
    assert_eq!(travel("pan"), vec![(-225.0, 225.0)]);
    assert_eq!(
        travel("tilt"),
        vec![(-150.0, 150.0)],
        "one physical Tilt axis"
    );

    let (pan, tilt) = (
        channel_index(&profile, "pan"),
        channel_index(&profile, "tilt"),
    );
    let (writes, achieved, result) = fit(&profile, PositionInstallation::default(), [0., 0.]);
    assert!((raw_of(&writes, pan) as i64 - 32768).abs() <= 1);
    assert!((raw_of(&writes, tilt) as i64 - 32768).abs() <= 1);
    assert!(achieved.iter().all(|a| a.abs() < 0.01), "{achieved:?}");
    // Mapping quality is unknown, so nothing reads as calibrated.
    assert_eq!(result.quality, PhysicalDataQuality::Unknown);

    let (writes, achieved, _) = fit(&profile, PositionInstallation::default(), [67.5, -75.]);
    let expected = |degrees: f64, travel: f64| ((degrees / travel + 0.5) * 65535.).round() as i64;
    assert!((raw_of(&writes, pan) as i64 - expected(67.5, 450.)).abs() <= 1);
    assert!((raw_of(&writes, tilt) as i64 - expected(-75., 300.)).abs() <= 1);
    assert!((achieved[0] - 67.5).abs() < 0.01 && (achieved[1] + 75.).abs() < 0.01);
    // Fine bytes are part of the word: 16-bit precision, not 8-bit steps.
    assert_ne!(raw_of(&writes, pan) & 0xff, 0);
}

#[test]
fn eight_bit_channels_without_degrees_use_the_nominal_centred_travel() {
    let profile = runtime("generic--pan-tilt", "Pan Tilt 8-bit");
    let (pan, tilt) = (
        channel_index(&profile, "pan"),
        channel_index(&profile, "tilt"),
    );
    let (writes, achieved, _) = fit(&profile, PositionInstallation::default(), [135., -67.5]);
    // Nominal Pan 540° and Tilt 270°, centred: +135° is 75 % and −67.5° is 25 % of the channel.
    assert!((raw_of(&writes, pan) as i64 - 191).abs() <= 1);
    assert!((raw_of(&writes, tilt) as i64 - 64).abs() <= 1);
    // An 8-bit step is 540/255 ≈ 2.1°; the fitter reports the quantised achieved angle.
    assert!((achieved[0] - 135.).abs() < 2.2 && (achieved[1] + 67.5).abs() < 1.1);
    let mode = &profile.modes[0];
    let function = &mode.channels[pan].functions[0];
    assert!(matches!(
        function.behavior,
        ChannelFunctionBehavior::Continuous { physical_min, physical_max, .. }
            if physical_min == -270.0 && physical_max == 270.0
    ));
    assert_eq!(
        function.physical_mapping.as_ref().unwrap().quality,
        PhysicalDataQuality::Unknown
    );
}

#[test]
fn inverted_mounting_mirrors_only_the_wire_word() {
    let profile = runtime("robe--robin-300-ledwash", "Mode 2");
    let (pan, tilt) = (
        channel_index(&profile, "pan"),
        channel_index(&profile, "tilt"),
    );
    let request = [100., 40.];
    let (upright, upright_achieved, _) = fit(&profile, PositionInstallation::default(), request);
    let inverted_install = PositionInstallation {
        invert_pan: true,
        invert_tilt: true,
        ..Default::default()
    };
    let (inverted, inverted_achieved, _) = fit(&profile, inverted_install, request);
    for index in [pan, tilt] {
        let sum = raw_of(&upright, index) + raw_of(&inverted, index);
        assert!((65534..=65536).contains(&sum), "mirrored word: {sum}");
    }
    // The commanded and reported Angles are the same on the inverted lamp.
    for axis in 0..2 {
        assert!((upright_achieved[axis] - request[axis]).abs() < 0.01);
        assert!((inverted_achieved[axis] - request[axis]).abs() < 0.01);
    }
}

#[test]
fn a_built_in_profile_without_functions_gets_the_nominal_model() {
    // The Default Stage's "Profile Moving Light": 8-bit pan/tilt, no functions, no travel,
    // one master-shared head.
    let mut profile = FixtureProfile::blank();
    profile.fixture_type = "moving profile".into();
    profile.manufacturer = "ToskLight Built-in".into();
    profile.name = "Profile Moving Light".into();
    profile.short_name = profile.name.clone();
    let mode = &mut profile.modes[0];
    mode.heads[0].master_shared = true;
    let head = mode.heads[0].id;
    mode.channels = ["intensity", "pan", "tilt"]
        .into_iter()
        .map(|attribute| {
            let mut c = channel(head, ChannelResolution::U8, vec![]);
            c.attribute = AttributeKey(attribute.into());
            c.fixture_attribute = c.attribute.clone();
            c.default_raw = if attribute == "intensity" { 0 } else { 128 };
            if attribute != "intensity" {
                // The seed's built-in channels declare no functions and no travel at all.
                c.functions.clear();
                (c.physical_min, c.physical_max, c.unit) = (None, None, None);
            }
            c
        })
        .collect();
    mode.splits[0].footprint = 3;
    profile.validate().unwrap();
    let stored = serde_json::to_vec(&profile).unwrap();
    assert_eq!(
        position_derivation(&profile, profile.modes[0].id),
        PositionDerivation::Derived
    );
    let mut runtime = profile.clone();
    apply_runtime_profile_compatibility(&mut runtime);
    runtime.validate().unwrap();
    assert_eq!(serde_json::to_vec(&profile).unwrap(), stored);
    let (pan, tilt) = (
        channel_index(&runtime, "pan"),
        channel_index(&runtime, "tilt"),
    );
    let (writes, _, _) = fit(&runtime, PositionInstallation::default(), [0., 0.]);
    assert!((raw_of(&writes, pan) as i64 - 128).abs() <= 1);
    assert!((raw_of(&writes, tilt) as i64 - 128).abs() <= 1);
    let (writes, _, _) = fit(&runtime, PositionInstallation::default(), [-270., 135.]);
    assert_eq!((raw_of(&writes, pan), raw_of(&writes, tilt)), (0, 255));
    // Deterministic: a second projection has the same Position identity.
    let mut again = profile.clone();
    apply_runtime_profile_compatibility(&mut again);
    let id = runtime.modes[0].id;
    assert_eq!(
        runtime.position_calibration_identity(id).unwrap(),
        again.position_calibration_identity(id).unwrap()
    );
}

#[test]
fn authored_models_stored_bytes_and_excluded_modes_are_unchanged() {
    // Negative control: an authored Position model is never replaced.
    let raw = package("cameo--auro-spot-z300");
    let mut projected = raw.clone();
    apply_runtime_profile_compatibility(&mut projected);
    for (before, after) in raw.modes.iter().zip(&projected.modes) {
        assert_eq!(before.position_physical, after.position_physical);
        assert_eq!(
            serde_json::to_value(raw.mode_geometry(before)).unwrap(),
            serde_json::to_value(projected.mode_geometry(after)).unwrap()
        );
        assert_eq!(
            position_derivation(&raw, before.id),
            PositionDerivation::Authored
        );
    }
    // Excluded modes keep no Position model and keep their channels untouched.
    for (name, mode, reason) in [
        (
            "glp--jdc1",
            "Normal 23-channel",
            PositionDerivationExclusion::SingleAxis,
        ),
        (
            "generic--endless-pan-tilt",
            "Endless Pan/Tilt 16-bit",
            PositionDerivationExclusion::EndlessRotation,
        ),
        (
            "tosklight--media-server",
            "2 layers",
            PositionDerivationExclusion::MediaModelRotation,
        ),
        (
            "tosklight--visualizer-laser",
            "12 Channel",
            PositionDerivationExclusion::LaserScanEngine,
        ),
    ] {
        let raw = package(name);
        let id = raw.modes.iter().find(|m| m.name == mode).unwrap().id;
        assert_eq!(
            position_derivation(&raw, id),
            PositionDerivation::Excluded(reason),
            "{name} {mode}"
        );
        let projected = runtime(name, mode);
        assert!(projected.modes[0].position_physical.is_none());
        let original = raw.modes.iter().find(|m| m.id == id).unwrap();
        assert_eq!(
            serde_json::to_value(&original.channels).unwrap(),
            serde_json::to_value(&projected.modes[0].channels).unwrap(),
            "{name} {mode}"
        );
    }
    // A mode without Pan/Tilt is not a candidate at all.
    let par = package("cameo--root-par-6");
    assert_eq!(
        position_derivation(&par, par.modes[0].id),
        PositionDerivation::NotApplicable
    );
}

/// Every shipped mode with a Pan/Tilt-like channel either has a Position graph after the runtime
/// projection or a named, reasoned exclusion. None is silent.
#[test]
fn every_shipped_pan_tilt_mode_has_a_position_graph_or_a_named_exclusion() {
    use PositionDerivationExclusion as X;
    let expected_exclusions = [
        ("glp--jdc1", X::SingleAxis),
        ("generic--endless-pan-tilt", X::EndlessRotation),
        ("tosklight--media-server", X::MediaModelRotation),
        ("tosklight--visualizer-laser", X::LaserScanEngine),
    ];
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/fixture-library");
    let (mut authored, mut derived, mut excluded) = (0, 0, 0);
    let mut paths: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    for path in paths {
        let bytes = std::fs::read(&path).unwrap();
        let raw = crate::read_fixture_package(&bytes).unwrap();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let mut projected = raw.clone();
        apply_runtime_profile_compatibility(&mut projected);
        projected
            .validate()
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        for (before, after) in raw.modes.iter().zip(&projected.modes) {
            let classification = position_derivation(&raw, before.id);
            let label = format!("{name} / {}", before.name);
            match classification {
                PositionDerivation::NotApplicable => {
                    assert!(after.position_physical.is_none(), "{label}");
                    continue;
                }
                PositionDerivation::Excluded(reason) => {
                    assert!(
                        expected_exclusions.contains(&(name.as_str(), reason)),
                        "{label}: unexpected exclusion {reason:?}"
                    );
                    assert!(after.position_physical.is_none(), "{label}");
                    excluded += 1;
                    continue;
                }
                PositionDerivation::Authored => authored += 1,
                PositionDerivation::Derived => derived += 1,
            }
            CompiledPositionFitting::compile(&projected, after.id, Default::default())
                .unwrap_or_else(|e| panic!("{label}: {e}"))
                .unwrap_or_else(|| panic!("{label}: no Position model"));
        }
        // Stored package bytes are never touched by the projection.
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "{name}");
    }
    assert_eq!(authored, 10, "authored Position modes");
    assert_eq!(derived, 32, "derived Position modes");
    assert_eq!(excluded, 10, "named exclusions");
}
