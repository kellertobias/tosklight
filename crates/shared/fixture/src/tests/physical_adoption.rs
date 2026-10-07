use crate::*;
use std::fs;
use std::path::PathBuf;

fn package(name: &str) -> FixtureProfile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    let profile = read_fixture_package(&fs::read(path).unwrap()).unwrap();
    profile.validate().unwrap();
    let round_trip = read_fixture_package(&write_fixture_package(&profile).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&profile).unwrap(),
        serde_json::to_value(&round_trip).unwrap(),
        "package round trip lost IDs or physical data: {name}"
    );
    profile
}

fn color_channels(mode: &FixtureMode, head: uuid::Uuid) -> Vec<uuid::Uuid> {
    mode.channels
        .iter()
        .filter(|channel| {
            channel.head_id == head
                && (channel.fixture_attribute.0.starts_with("color.")
                    || channel.attribute.0.starts_with("color."))
        })
        .map(|channel| channel.id)
        .collect()
}

#[test]
fn root_par_preserves_six_native_emitters_and_keeps_uv_appearance_unknown() {
    let p = package("cameo--root-par-6");
    assert_eq!(p.revision, 5);
    let m = &p.modes[0];
    let path = &m.color_physical.as_ref().unwrap().paths[0];
    assert_eq!(path.controls, color_channels(m, path.head_id));
    let OpticalSource::Additive { emitters } = &path.source else {
        panic!("ROOT PAR must retain the six additive native functions")
    };
    assert_eq!(emitters.len(), 6);
    assert!(emitters.iter().all(|emitter| emitter.spectrum.is_empty()));
    assert_eq!(
        emitters
            .iter()
            .filter(|emitter| emitter.band == OpticalEmitterBand::Ultraviolet)
            .count(),
        1
    );
    assert!(
        emitters
            .iter()
            .find(|emitter| emitter.band == OpticalEmitterBand::Ultraviolet)
            .unwrap()
            .xyz
            .is_none()
    );
    assert!(
        emitters
            .iter()
            .filter(|emitter| emitter.band == OpticalEmitterBand::Visible)
            .all(|emitter| emitter.provenance.quality == PhysicalDataQuality::Estimated)
    );
}

#[test]
fn auro_wheel_slots_are_alternative_unknown_states_and_focus_excludes_raw_zero() {
    let p = package("cameo--auro-spot-z300");
    assert_eq!(p.modes.len(), 2);
    for m in &p.modes {
        let path = &m.color_physical.as_ref().unwrap().paths[0];
        assert_eq!(path.controls, color_channels(m, path.head_id));
        assert_eq!(path.filters.len(), 9);
        assert!(matches!(path.source, OpticalSource::Unknown));
        let wheel_id = path.controls[0];
        assert!(path.filters.iter().all(|f| f.binding.channel_id == wheel_id
            && matches!(f.transmission, OpticalTransmission::Unknown)));
        let wheel = m.channels.iter().find(|c| c.id == wheel_id).unwrap();
        let range: Vec<_> = path
            .filters
            .iter()
            .map(|f| {
                let function = wheel
                    .functions
                    .iter()
                    .find(|v| v.id == f.binding.function_id)
                    .unwrap();
                (function.dmx_from, function.dmx_to)
            })
            .collect();
        assert_eq!(
            range,
            vec![
                (0, 5),
                (6, 11),
                (12, 17),
                (18, 23),
                (24, 29),
                (30, 35),
                (36, 41),
                (42, 47),
                (48, 53)
            ]
        );
        let focus = m
            .channels
            .iter()
            .find(|c| c.fixture_attribute.0.as_ref() == "focus")
            .unwrap();
        assert_eq!(focus.functions[0].dmx_from, 6);
        assert_eq!(focus.functions[0].dmx_to, 255);
        assert!(focus.functions[0].physical_mapping.is_some());
        let zoom = m
            .channels
            .iter()
            .find(|c| c.fixture_attribute.0.as_ref() == "zoom")
            .unwrap();
        // TL-637: the user manual names the zoom range a beam angle; the range conflict stays
        // recorded and the mapping keeps unknown quality.
        let mapping = zoom.functions[0].physical_mapping.as_ref().unwrap();
        assert_eq!(mapping.opening_convention, Some(OpeningConvention::Beam));
        assert_eq!(mapping.quality, PhysicalDataQuality::Unknown);
        assert!(m.position_physical.is_some());
    }
}

/// TL-637: every shipped mover whose profile already documents Pan/Tilt travel in degrees and
/// owns lens geometry binds that travel to its yoke and head axes, on an explicitly nominal
/// (estimated) geometry contract, and compiles the shared Position fitting in every mode.
#[test]
fn documented_movers_bind_centred_travel_to_a_nominal_position_graph() {
    for (name, pan, tilt) in [
        ("cameo--auro-spot-z300", 540.0, 270.0),
        ("jb-lighting--jbled-a7", 430.0, 300.0),
        ("robe--robin-dls-profile", 540.0, 280.0),
        ("martin--mac-300", 540.0, 265.0),
    ] {
        let p = package(name);
        let contract = p.geometry.physical_contract.as_ref().unwrap();
        assert_eq!(contract.version, 1);
        assert_eq!(
            contract.provenance.quality,
            PhysicalDataQuality::Estimated,
            "{name}"
        );
        assert!(
            contract
                .provenance
                .source
                .as_deref()
                .unwrap()
                .contains("unverified")
        );
        assert_eq!(contract.bracket, GeometryBracket::Fixed);
        for m in &p.modes {
            let model = m.position_physical.as_ref().unwrap();
            assert_eq!(model.bindings.len(), 2, "{name}/{}", m.name);
            for binding in &model.bindings {
                let (role, travel) = match binding.role {
                    PositionAxisRole::Pan => ("pan", pan),
                    PositionAxisRole::Tilt => ("tilt", tilt),
                };
                let channel = m
                    .channels
                    .iter()
                    .find(|c| c.id == binding.channel_id)
                    .unwrap();
                assert_eq!(channel.attribute.0.as_ref(), role);
                let function = channel
                    .functions
                    .iter()
                    .find(|f| f.id == binding.function_id)
                    .unwrap();
                assert_eq!(
                    function.angular_motion.unwrap().kind,
                    AngularMotionKind::AbsolutePosition
                );
                let ChannelFunctionBehavior::Continuous {
                    physical_min,
                    physical_max,
                    unit,
                } = &function.behavior
                else {
                    panic!("{name}: {role} must stay continuous")
                };
                // The documented travel is retained exactly, centred on the neutral pose.
                assert_eq!(
                    (*physical_min, *physical_max),
                    (-travel / 2.0, travel / 2.0)
                );
                assert_eq!(unit.as_deref(), Some("degrees"));
                assert_eq!(
                    (function.dmx_from, function.dmx_to),
                    (0, channel.resolution.max_raw())
                );
                let node = p
                    .geometry
                    .nodes
                    .iter()
                    .find(|n| n.id == binding.node_id)
                    .unwrap();
                let motion = node.motion.as_ref().unwrap();
                assert_eq!(
                    (motion.physical_min, motion.physical_max),
                    (-travel / 2.0, travel / 2.0)
                );
            }
            let fitting = CompiledPositionFitting::compile(
                &p,
                m.id,
                crate::forward::PositionInstallation::default(),
            )
            .unwrap()
            .unwrap_or_else(|| panic!("{name}/{} compiles no Position fitting", m.name));
            let roles: Vec<_> = fitting.axes().iter().map(|axis| axis.role).collect();
            assert_eq!(
                roles,
                vec![Some(PositionAxisRole::Pan), Some(PositionAxisRole::Tilt)]
            );
            assert!(
                fitting.emitters().all(|e| e.command_indices.is_some()),
                "{name}/{}: every lens solves Pan and Tilt",
                m.name
            );
            assert!(p.position_calibration_identity(m.id).unwrap().is_some());
        }
    }
}

/// Movers without degree travel or without lens geometry stay explicitly unbound rather than
/// acquiring invented travel, pivots or beam angles.
#[test]
fn movers_without_documented_travel_or_lens_geometry_stay_unbound() {
    for name in [
        "claypaky--sharpy",
        "martin--mac-250-entour",
        "robe--robin-300-ledwash",
        "robe--robin-600x-ledwash",
        "robe--robin-dlf-wash",
        "robe--robin-ledbeam-150",
        "claypaky--stage-zoom-1200",
        "claypaky--stage-zoom-1200-sv",
        "eurolite--ts-255-dmx-scan",
        "glp--jdc1",
    ] {
        let p = package(name);
        assert!(p.geometry.physical_contract.is_none(), "{name}");
        assert!(
            p.modes.iter().all(|m| m.position_physical.is_none()),
            "{name}"
        );
    }
}

#[test]
fn martin_and_etc_keep_exact_native_sources_without_invented_spectra() {
    let cl = package("martin--elp-cl-profile");
    let m = &cl.modes[0];
    let path = &m.color_physical.as_ref().unwrap().paths[0];
    assert_eq!(path.controls.len(), 7);
    let OpticalSource::Additive { emitters } = &path.source else {
        panic!("CL is RGBAL")
    };
    assert_eq!(emitters.len(), 5);
    assert!(
        emitters
            .iter()
            .all(|e| e.xyz.is_none() && e.spectrum.is_empty())
    );
    assert!(
        path.controls.contains(
            &m.channels
                .iter()
                .find(|c| c.fixture_attribute.0.as_ref() == "fixture.color_scene")
                .unwrap()
                .id
        )
    );

    let ww = package("martin--elp-ww-profile");
    let path = &ww.modes[0].color_physical.as_ref().unwrap().paths[0];
    assert!(path.controls.is_empty());
    assert!(matches!(
        path.source,
        OpticalSource::Fixed { xyz: None, .. }
    ));

    let etc = package("etc--source-four-led-series-2-lustr");
    assert_eq!(etc.modes.len(), 8);
    let direct = etc.modes.iter().find(|m| m.name == "Direct").unwrap();
    let path = &direct.color_physical.as_ref().unwrap().paths[0];
    assert_eq!(path.controls, color_channels(direct, path.head_id));
    let OpticalSource::Additive { emitters } = &path.source else {
        panic!("Direct is additive")
    };
    assert_eq!(emitters.len(), 7);
    assert!(
        emitters
            .iter()
            .all(|e| e.xyz.is_none() && e.spectrum.is_empty() && !e.native_reversed)
    );
    assert!(
        etc.modes
            .iter()
            .filter(|m| m.name != "Direct")
            .all(|m| m.color_physical.is_none())
    );
}

#[test]
fn stage_zoom_repairs_u16_domain_and_keeps_unknown_cmy_wheel_appearance() {
    for name in ["claypaky--stage-zoom-1200", "claypaky--stage-zoom-1200-sv"] {
        let p = package(name);
        assert_eq!(p.modes.len(), 3);
        for m in &p.modes {
            let path = &m.color_physical.as_ref().unwrap().paths[0];
            let mut owned = path.controls.clone();
            let mut native = color_channels(m, path.head_id);
            owned.sort();
            native.sort();
            assert_eq!(owned, native);
            assert!(matches!(path.source, OpticalSource::Unknown));
            assert!(
                path.filters
                    .iter()
                    .all(|f| matches!(f.transmission, OpticalTransmission::Unknown))
            );
            assert!(
                path.filters
                    .iter()
                    .all(|f| !f.name.to_ascii_lowercase().contains("rotation"))
            );
            for c in m.channels.iter().filter(|c| {
                c.resolution == ChannelResolution::U16
                    && matches!(c.fixture_attribute.0.as_ref(), "pan" | "tilt")
            }) {
                assert_eq!(c.functions[0].dmx_to, 65535);
                assert_eq!(c.default_raw, 32768);
            }
            assert!(m.position_physical.is_none());
        }
    }
}

#[test]
fn mixed_jbled_control_and_unsourced_zone_emitters_stay_unopted() {
    let jb = package("jb-lighting--jbled-a7");
    assert!(jb.modes.iter().all(|m| m.color_physical.is_none()));
    for m in &jb.modes {
        let control = m
            .channels
            .iter()
            .find(|c| c.fixture_attribute.0.as_ref() == "fixture.control")
            .unwrap();
        let reset = control
            .functions
            .iter()
            .find(|f| f.name.starts_with("reset"))
            .unwrap();
        assert_eq!((reset.dmx_from, reset.dmx_to), (240, 247));
        assert!(!native_color_function_allowed(control, reset));
    }
    for name in ["glp--jdc1", "robe--robin-600x-ledwash"] {
        let profile = package(name);
        assert!(profile.modes.iter().all(|m| m.color_physical.is_none()));
    }
}
