//! Export → import equivalence of physical colour models and Position/Zoom physical ranges.
//!
//! Identities are regenerated from GDTF names on import, so models are compared through
//! identity-free signatures: channels by split and primary slot, functions by raw interval.

use super::gdtf_name;
use super::profile::{GdtfExportDiagnostic, package_profile_with_diagnostics};
use super::read::import_profile;
use crate::{
    ChannelFunction, ChannelFunctionBehavior, ChannelResolution, FilterSpectrum, FixtureChannel,
    FixtureMode, FixtureProfile, HeadOpticalPath, NativeColorBinding, OpticalEmitter,
    OpticalEmitterBand, OpticalFilter, OpticalProvenance, OpticalSource, OpticalTransmission,
    PhysicalUnit, SpectrumSample,
};
use light_core::{AttributeKey, Xyz};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

type Key = (u16, u16, u32, u32);

fn package(name: &str) -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    crate::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

fn roundtrip(profile: &FixtureProfile) -> (FixtureProfile, Vec<GdtfExportDiagnostic>) {
    let (bytes, diagnostics) = package_profile_with_diagnostics(profile).unwrap();
    (import_profile(&bytes).unwrap().profile, diagnostics)
}

/// Five significant digits: xyY conversion is exact to f32 rounding, never to a wrong colour.
fn number(value: f32) -> String {
    format!("{value:.4e}")
}

fn xyz(value: Option<Xyz>) -> Option<[String; 3]> {
    value.map(|v| [v.x, v.y, v.z].map(number))
}

fn spectrum(samples: &[SpectrumSample]) -> Vec<(String, String)> {
    samples
        .iter()
        .map(|s| (number(s.wavelength_nm), number(s.value)))
        .collect()
}

fn function_key(mode: &FixtureMode, binding: NativeColorBinding) -> Key {
    let slots = mode.primary_slots().unwrap();
    let channel = mode
        .channels
        .iter()
        .find(|c| c.id == binding.channel_id)
        .unwrap();
    let function = channel
        .functions
        .iter()
        .find(|f| f.id == binding.function_id)
        .unwrap();
    (
        channel.split,
        slots[&channel.id],
        function.dmx_from,
        function.dmx_to,
    )
}

#[derive(Debug, PartialEq)]
enum Source {
    Unknown,
    Fixed(Option<[String; 3]>, Vec<(String, String)>),
    Additive(BTreeMap<Key, String>),
}

#[derive(Debug, PartialEq)]
struct Path {
    source: Source,
    filters: BTreeMap<Key, String>,
}

/// Paths keyed by the slots of their controls. Provenance, ids and revisions are excluded:
/// GDTF has no place for them (they are reported as export diagnostics instead).
fn signature(mode: &FixtureMode) -> BTreeMap<Vec<(u16, u16)>, Path> {
    let slots = mode.primary_slots().unwrap();
    let Some(model) = &mode.color_physical else {
        return BTreeMap::new();
    };
    model
        .paths
        .iter()
        .map(|path| {
            let channel = |id: &Uuid| mode.channels.iter().find(|c| c.id == *id).unwrap();
            let controls = path
                .controls
                .iter()
                .map(|id| (channel(id).split, slots[id]))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let source = match &path.source {
                OpticalSource::Unknown => Source::Unknown,
                OpticalSource::Fixed {
                    xyz: v,
                    spectrum: s,
                    ..
                } => Source::Fixed(xyz(*v), spectrum(s)),
                OpticalSource::Additive { emitters } => Source::Additive(
                    emitters
                        .iter()
                        .map(|e| {
                            let data = format!(
                                "{} {:?} {:?} {:?} {} {} {}",
                                gdtf_name(&e.name),
                                xyz(e.xyz),
                                spectrum(&e.spectrum),
                                e.band,
                                e.native_reversed,
                                e.maximum_level,
                                e.response_exponent
                            );
                            (function_key(mode, e.binding), data)
                        })
                        .collect(),
                ),
            };
            let filters = path
                .filters
                .iter()
                .map(|f| {
                    let transmission = match &f.transmission {
                        OpticalTransmission::Unknown => None,
                        OpticalTransmission::Spectral { samples } => Some(
                            samples
                                .iter()
                                .map(|s| (s.raw_from, s.raw_to, spectrum(&s.spectrum)))
                                .collect::<Vec<_>>(),
                        ),
                    };
                    let data = format!("{} {transmission:?}", gdtf_name(&f.name));
                    (function_key(mode, f.binding), data)
                })
                .collect();
            (controls, Path { source, filters })
        })
        .collect()
}

fn assert_equivalent(original: &FixtureProfile, imported: &FixtureProfile) {
    assert_eq!(original.modes.len(), imported.modes.len());
    for (before, after) in original.modes.iter().zip(&imported.modes) {
        assert!(
            before.color_physical.is_some() == after.color_physical.is_some(),
            "{}: model presence",
            before.name
        );
        assert_eq!(signature(before), signature(after), "mode {}", before.name);
    }
}

fn channel(head: Uuid, attribute: &str, functions: Vec<ChannelFunction>) -> FixtureChannel {
    FixtureChannel {
        id: Uuid::new_v4(),
        head_id: head,
        split: 1,
        fixture_attribute: AttributeKey(attribute.into()),
        attribute: AttributeKey(attribute.into()),
        canonical_transform: Default::default(),
        resolution: ChannelResolution::U8,
        secondary_slots: Vec::new(),
        default_raw: 0,
        highlight_raw: 255,
        physical_min: None,
        physical_max: None,
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        virtual_intensity_inverted: false,
        reacts_to_sequence_master: false,
        reacts_to_group_master: false,
        reacts_to_grand_master: false,
        behavior: Default::default(),
        functions,
    }
}

fn continuous(attribute: &str, from: f32, to: f32, unit: Option<&str>) -> ChannelFunction {
    let mut function = ChannelFunction::continuous(attribute, AttributeKey(attribute.into()), 255);
    function.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: from,
        physical_max: to,
        unit: unit.map(str::to_owned),
    };
    function
}

fn slot(name: &str, from: u32, to: u32) -> ChannelFunction {
    let mut function = ChannelFunction::continuous(name, AttributeKey("color.wheel.1".into()), 255);
    function.dmx_from = from;
    function.dmx_to = to;
    function.behavior = ChannelFunctionBehavior::Indexed {
        semantic_id: name.to_lowercase(),
        label: name.into(),
        raw_value: from,
    };
    function
}

fn binding(channel: &FixtureChannel, function: usize) -> NativeColorBinding {
    NativeColorBinding {
        channel_id: channel.id,
        function_id: channel.functions[function].id,
    }
}

fn samples(points: &[(f32, f32)]) -> Vec<SpectrumSample> {
    points
        .iter()
        .map(|(wavelength_nm, value)| SpectrumSample {
            wavelength_nm: *wavelength_nm,
            value: *value,
        })
        .collect()
}

fn emitter(name: &str, channel: &FixtureChannel, xyz: Option<Xyz>) -> OpticalEmitter {
    OpticalEmitter {
        id: Uuid::new_v4(),
        name: name.into(),
        binding: binding(channel, 0),
        xyz,
        spectrum: Vec::new(),
        band: OpticalEmitterBand::Visible,
        native_reversed: false,
        maximum_level: 1.0,
        response_exponent: 1.0,
        provenance: OpticalProvenance::default(),
    }
}

fn path(mode: &FixtureMode, source: OpticalSource, filters: Vec<OpticalFilter>) -> HeadOpticalPath {
    HeadOpticalPath {
        id: Uuid::new_v4(),
        head_id: mode.heads[0].id,
        controls: mode.channels.iter().map(|c| c.id).collect(),
        source,
        filters,
        measurements: Vec::new(),
    }
}

fn single_mode(channels: Vec<FixtureChannel>) -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Physical".into();
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = channels.len() as u16;
    mode.channels = channels;
    profile
}

/// RGBW + UV: measured-style colours, one spectrum, a reversed emitter and a UV band.
fn additive() -> FixtureProfile {
    let mut profile = single_mode(Vec::new());
    let head = profile.modes[0].heads[0].id;
    let mut channels = [
        "color.red",
        "color.green",
        "color.blue",
        "color.white",
        "color.uv",
    ]
    .map(|attribute| channel(head, attribute, vec![continuous(attribute, 0.0, 1.0, None)]))
    .to_vec();
    channels[2].functions[0] = continuous("color.blue", 1.0, 0.0, None);
    let mut emitters = vec![
        emitter(
            "Red",
            &channels[0],
            Some(Xyz {
                x: 0.4124,
                y: 0.2126,
                z: 0.0193,
            }),
        ),
        emitter(
            "Green",
            &channels[1],
            Some(Xyz {
                x: 0.3576,
                y: 0.7152,
                z: 0.1192,
            }),
        ),
        emitter(
            "Blue",
            &channels[2],
            Some(Xyz {
                x: 0.1805,
                y: 0.0722,
                z: 0.9505,
            }),
        ),
        emitter("White", &channels[3], None),
        emitter(
            "UV",
            &channels[4],
            Some(Xyz {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            }),
        ),
    ];
    emitters[2].native_reversed = true;
    emitters[3].spectrum = samples(&[(400.0, 0.2), (550.0, 1.0), (700.0, 0.6)]);
    emitters[4].band = OpticalEmitterBand::Ultraviolet;
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = channels.len() as u16;
    mode.channels = channels;
    let model = path(mode, OpticalSource::Additive { emitters }, Vec::new());
    mode.color_physical = Some(crate::ColorPhysicalModel {
        version: 1,
        revision: 3,
        paths: vec![model],
    });
    profile.validate().unwrap();
    profile
}

/// CMY with point-measured cyan, and a colour wheel whose slots carry stepped transmission.
fn wheel() -> FixtureProfile {
    let mut profile = single_mode(Vec::new());
    let head = profile.modes[0].heads[0].id;
    let cyan = channel(
        head,
        "color.cyan",
        vec![continuous("color.cyan", 0.0, 1.0, None)],
    );
    let wheel = channel(
        head,
        "color.wheel.1",
        vec![
            slot("Open", 0, 9),
            slot("Deep Red", 10, 19),
            slot("Congo Blue", 20, 29),
        ],
    );
    let transmission = |samples| OpticalTransmission::Spectral { samples };
    let filter = |name: &str, binding, transmission| OpticalFilter {
        id: Uuid::new_v4(),
        name: name.into(),
        binding,
        transmission,
        provenance: OpticalProvenance::default(),
    };
    let red = samples(&[(450.0, 0.01), (600.0, 0.4), (700.0, 0.9)]);
    let blue = samples(&[(420.0, 0.5), (500.0, 0.2), (650.0, 0.0)]);
    let filters = vec![
        filter(
            "Cyan",
            binding(&cyan, 0),
            transmission(vec![
                FilterSpectrum {
                    raw_from: 0,
                    raw_to: 0,
                    spectrum: samples(&[(400.0, 1.0), (700.0, 1.0)]),
                },
                FilterSpectrum {
                    raw_from: 128,
                    raw_to: 128,
                    spectrum: samples(&[(400.0, 0.9), (700.0, 0.4)]),
                },
                FilterSpectrum {
                    raw_from: 255,
                    raw_to: 255,
                    spectrum: samples(&[(400.0, 0.8), (700.0, 0.0)]),
                },
            ]),
        ),
        filter("Open", binding(&wheel, 0), OpticalTransmission::Unknown),
        filter(
            "Deep Red",
            binding(&wheel, 1),
            transmission(vec![FilterSpectrum {
                raw_from: 10,
                raw_to: 19,
                spectrum: red,
            }]),
        ),
        filter(
            "Congo Blue",
            binding(&wheel, 2),
            transmission(vec![
                FilterSpectrum {
                    raw_from: 20,
                    raw_to: 24,
                    spectrum: blue.clone(),
                },
                FilterSpectrum {
                    raw_from: 25,
                    raw_to: 29,
                    spectrum: blue,
                },
            ]),
        ),
    ];
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 2;
    mode.channels = vec![cyan, wheel];
    let model = path(mode, OpticalSource::Unknown, filters);
    mode.color_physical = Some(crate::ColorPhysicalModel {
        version: 1,
        revision: 1,
        paths: vec![model],
    });
    profile.validate().unwrap();
    profile
}

#[test]
fn an_additive_rgbw_uv_model_survives_export_and_import() {
    let original = additive();
    let (imported, diagnostics) = roundtrip(&original);
    assert_eq!(diagnostics, Vec::new(), "nothing authored is dropped");
    assert_equivalent(&original, &imported);
    let model = imported.modes[0].color_physical.as_ref().unwrap();
    let OpticalSource::Additive { emitters } = &model.paths[0].source else {
        panic!("additive source");
    };
    assert!(
        emitters
            .iter()
            .all(|e| e.provenance.quality == crate::PhysicalDataQuality::Unknown)
    );
    assert_eq!(emitters[4].band, OpticalEmitterBand::Ultraviolet);
    assert_eq!(
        emitters[4].xyz,
        Some(Xyz {
            x: 0.0,
            y: 0.0,
            z: 0.0
        }),
        "black stays black"
    );
    assert_eq!(emitters[3].xyz, None, "unknown never becomes black");
    // Identities are deterministic, so a second import yields the same model ids.
    assert_eq!(
        roundtrip(&original).0.modes[0]
            .color_physical
            .as_ref()
            .unwrap()
            .paths[0]
            .id,
        model.paths[0].id
    );
}

#[test]
fn a_wheel_and_cmy_filter_model_survives_export_and_import() {
    let original = wheel();
    let (bytes, diagnostics) = package_profile_with_diagnostics(&original).unwrap();
    assert_eq!(diagnostics, Vec::new());
    let xml = super::read::archive_xml(&bytes).unwrap();
    assert!(xml.contains("<Wheel Name=\"Color1\">"), "{xml}");
    assert!(xml.contains("<Slot Name=\"Deep Red\" Filter=\"Deep Red\"/>"));
    assert!(xml.contains("Filter=\"Cyan\""));
    assert!(xml.contains("Wheel=\"Color1\""));
    assert!(xml.contains("WheelSlotIndex=\"3\""));
    assert!(xml.contains("InterpolationTo=\"Step\""));
    let imported = import_profile(&bytes).unwrap().profile;
    assert_equivalent(&original, &imported);
}

/// Drops controls a GDTF reader cannot recover: neither linked nor a Color channel.
fn without_unlinked_controls(mut profile: FixtureProfile) -> FixtureProfile {
    for mode in &mut profile.modes {
        let channels = mode.channels.clone();
        let Some(model) = &mut mode.color_physical else {
            continue;
        };
        for path in &mut model.paths {
            let mut linked = path
                .filters
                .iter()
                .map(|f| f.binding.channel_id)
                .collect::<Vec<_>>();
            if let OpticalSource::Additive { emitters } = &path.source {
                linked.extend(emitters.iter().map(|e| e.binding.channel_id));
            }
            path.controls.retain(|id| {
                linked.contains(id)
                    || channels
                        .iter()
                        .any(|c| c.id == *id && super::physical::is_color(c))
            });
        }
    }
    profile
}

/// Export diagnostics that mean colour data, rather than its provenance, was not exported.
const COLOUR_LOSSES: [&str; 8] = [
    "whole-path",
    "emitter direction",
    "maximum level",
    "CIE xyY",
    "filter transmission",
    "fixed source",
    "optical path has no",
    "another head",
];

#[test]
fn shipped_physical_colour_models_survive_export_and_import() {
    for name in [
        "cameo--auro-spot-z300",
        "claypaky--stage-zoom-1200",
        "claypaky--stage-zoom-1200-sv",
        "cameo--root-par-6",
        "martin--elp-cl-profile",
        "martin--elp-ww-profile",
        "etc--source-four-led-series-2-lustr",
    ] {
        let mut original = package(name);
        assert!(
            original.modes.iter().any(|m| m.color_physical.is_some()),
            "{name}"
        );
        // GDTF export refuses rpm by design (no guessed conversion to degrees per second); the
        // gobo/prism rotation that uses it is unrelated to Color, so drop only that unit here.
        for function in original
            .modes
            .iter_mut()
            .flat_map(|mode| &mut mode.channels)
            .flat_map(|channel| &mut channel.functions)
        {
            if let ChannelFunctionBehavior::Continuous { unit, .. } = &mut function.behavior
                && unit.as_deref() == Some("rpm")
            {
                *unit = None;
            }
        }
        let (imported, diagnostics) = roundtrip(&original);
        let ownership = diagnostics
            .iter()
            .filter(|d| d.message.starts_with("Colour control"))
            .count();
        // ELP CL lists its Color Scene macro as a Color control; GDTF links cannot say so. The
        // export reports it, and every linked or Color channel is still equivalent.
        assert_eq!(
            ownership,
            usize::from(name == "martin--elp-cl-profile"),
            "{name}"
        );
        assert_equivalent(&without_unlinked_controls(original), &imported);
        imported.validate().unwrap();
        // Provenance and Position/geometry losses are reported; no colour data is lost.
        for diagnostic in diagnostics {
            assert!(
                !COLOUR_LOSSES
                    .iter()
                    .any(|loss| diagnostic.message.contains(loss)),
                "{name}: unexpected colour loss {diagnostic:?}"
            );
        }
    }
}

/// Every continuous Position/Zoom function as (slot, raw interval, endpoints, unit, motion).
fn ranges(mode: &FixtureMode) -> Vec<String> {
    let slots = mode.primary_slots().unwrap();
    let mut output = Vec::new();
    for channel in &mode.channels {
        let native = channel.fixture_attribute.0.as_ref();
        if !["pan", "tilt", "zoom"].contains(&native) {
            continue;
        }
        for function in &channel.functions {
            if let ChannelFunctionBehavior::Continuous {
                physical_min,
                physical_max,
                unit,
            } = &function.behavior
            {
                output.push(format!(
                    "{native}@{} {}..{} {physical_min}..{physical_max} {:?} {:?}",
                    slots[&channel.id],
                    function.dmx_from,
                    function.dmx_to,
                    PhysicalUnit::parse(unit.as_deref()),
                    function.angular_motion.map(|m| m.kind),
                ));
            }
        }
    }
    output
}

#[test]
fn pan_tilt_and_zoom_degree_ranges_survive_export_and_import() {
    let mut profile = single_mode(Vec::new());
    let head = profile.modes[0].heads[0].id;
    let mut pan = channel(
        head,
        "pan",
        vec![continuous("pan", -270.0, 270.0, Some("degrees"))],
    );
    pan.functions[0].angular_motion = Some(crate::AngularMotion {
        kind: crate::AngularMotionKind::AbsolutePosition,
        max_speed_degrees_per_second: None,
        acceleration_degrees_per_second_squared: None,
        deceleration_degrees_per_second_squared: None,
    });
    let mut tilt = channel(
        head,
        "tilt",
        vec![continuous("tilt", 135.0, -135.0, Some("degrees"))],
    );
    tilt.functions[0].angular_motion = pan.functions[0].angular_motion;
    let zoom = channel(
        head,
        "zoom",
        vec![continuous("zoom", 4.0, 50.0, Some("degrees"))],
    );
    profile.modes[0].splits[0].footprint = 3;
    profile.modes[0].channels = vec![pan, tilt, zoom];
    profile.validate().unwrap();
    let (imported, diagnostics) = roundtrip(&profile);
    assert_eq!(diagnostics, Vec::new());
    let expected = ranges(&profile.modes[0]);
    assert_eq!(expected.len(), 3);
    assert_eq!(ranges(&imported.modes[0]), expected);

    for name in [
        "martin--mac-300",
        "robe--robin-dls-profile",
        "jb-lighting--jbled-a7",
        "cameo--auro-spot-z300",
    ] {
        let original = package(name);
        let (imported, diagnostics) = roundtrip(&original);
        for (before, after) in original.modes.iter().zip(&imported.modes) {
            assert!(!ranges(before).is_empty(), "{name}/{}", before.name);
            assert_eq!(ranges(after), ranges(before), "{name}/{}", before.name);
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.node == before.name && d.message.contains("Axis geometry")),
                "{name}: the unexported Position binding is reported"
            );
        }
    }
}

#[test]
fn data_gdtf_cannot_carry_is_reported_not_silently_dropped() {
    let mut profile = additive();
    let mode = &mut profile.modes[0];
    let model = mode.color_physical.as_mut().unwrap();
    let OpticalSource::Additive { emitters } = &mut model.paths[0].source else {
        unreachable!()
    };
    emitters[0].maximum_level = 0.8;
    emitters[1].provenance = OpticalProvenance {
        quality: crate::PhysicalDataQuality::Measured,
        source: Some("Lab sheet".into()),
        revision: 2,
    };
    emitters[2].native_reversed = false;
    emitters[3].band = OpticalEmitterBand::Infrared;
    let mut zoom = channel(
        mode.heads[0].id,
        "zoom",
        vec![continuous("zoom", 4.0, 50.0, Some("degrees"))],
    );
    zoom.functions[0].physical_mapping = Some(crate::PhysicalMappingCalibration {
        opening_convention: Some(crate::OpeningConvention::Beam),
        ..Default::default()
    });
    mode.channels.push(zoom);
    mode.splits[0].footprint += 1;
    profile.validate().unwrap();
    let (imported, diagnostics) = roundtrip(&profile);
    let messages = diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect::<Vec<_>>();
    for expected in [
        "maximum level",
        "Optical provenance",
        "emitter direction",
        "opening convention",
    ] {
        assert!(
            messages.iter().any(|m| m.contains(expected)),
            "{expected}: {messages:#?}"
        );
    }
    // The IR band is not lost: DiodePart records it.
    let model = imported.modes[0].color_physical.as_ref().unwrap();
    let OpticalSource::Additive { emitters } = &model.paths[0].source else {
        unreachable!()
    };
    assert_eq!(emitters[3].band, OpticalEmitterBand::Infrared);
}

#[test]
fn a_mutated_model_is_not_equivalent_to_the_export() {
    let original = wheel();
    let (imported, _) = roundtrip(&original);
    assert_equivalent(&original, &imported);

    let mut swapped = original.clone();
    let model = swapped.modes[0].color_physical.as_mut().unwrap();
    let (first, second) = (
        model.paths[0].filters[2].binding,
        model.paths[0].filters[3].binding,
    );
    model.paths[0].filters[2].binding = second;
    model.paths[0].filters[3].binding = first;
    assert_ne!(
        signature(&swapped.modes[0]),
        signature(&imported.modes[0]),
        "swapped slots"
    );

    let mut recoloured = additive();
    let (imported, _) = roundtrip(&recoloured);
    let model = recoloured.modes[0].color_physical.as_mut().unwrap();
    let OpticalSource::Additive { emitters } = &mut model.paths[0].source else {
        unreachable!()
    };
    emitters[0].xyz.as_mut().unwrap().x += 0.01;
    assert_ne!(
        signature(&recoloured.modes[0]),
        signature(&imported.modes[0]),
        "changed colour"
    );

    // A tampered archive is detected too: the slot links decide the binding.
    let (bytes, _) = package_profile_with_diagnostics(&original).unwrap();
    let xml = super::read::archive_xml(&bytes)
        .unwrap()
        .replace("WheelSlotIndex=\"2\"", "WheelSlotIndex=\"9\"");
    let tampered = super::read::import_xml_for_tests(&xml).profile;
    assert_ne!(signature(&tampered.modes[0]), signature(&original.modes[0]));
}

#[test]
fn an_unusable_link_in_a_foreign_gdtf_drops_only_the_colour_model() {
    let xml = r#"<GDTF DataVersion="1.2"><FixtureType Name="Foreign" Manufacturer="Test" FixtureTypeID="9f4cbbd1-3c8a-4c2b-9e75-0a0e2f6c1d10"><AttributeDefinitions><Attributes><Attribute Name="ColorAdd_R"/></Attributes></AttributeDefinitions><PhysicalDescriptions><Emitters><Emitter Name="Red" Color="0.7,0.3,20"/></Emitters></PhysicalDescriptions><DMXModes><DMXMode Name="M"><DMXChannels><DMXChannel Geometry="Head" Offset="1"><LogicalChannel Attribute="ColorAdd_R"><ChannelFunction Name="Fixed" Attribute="ColorAdd_R" DMXFrom="0/1" Default="0/1" PhysicalFrom="1" PhysicalTo="1" Emitter="Red"/></LogicalChannel></DMXChannel></DMXChannels></DMXMode></DMXModes></FixtureType></GDTF>"#;
    let imported = super::read::import_xml_for_tests(xml);
    assert!(imported.profile.modes[0].color_physical.is_none());
    assert!(imported.diagnostics.iter().any(|d| {
        d.message
            .contains("do not form a valid physical colour model")
    }));
}
