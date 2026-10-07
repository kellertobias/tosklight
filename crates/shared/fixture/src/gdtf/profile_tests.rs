use super::*;
use crate::{ChannelFunction, ChannelFunctionBehavior, FixtureHead};
use light_core::AttributeKey;

fn channel(head: Uuid, attribute: &str, resolution: ChannelResolution) -> FixtureChannel {
    FixtureChannel {
        id: Uuid::new_v4(),
        head_id: head,
        split: 1,
        fixture_attribute: AttributeKey(attribute.into()),
        attribute: AttributeKey(attribute.into()),
        canonical_transform: Default::default(),
        resolution,
        secondary_slots: Vec::new(),
        default_raw: 0,
        highlight_raw: resolution.max_raw(),
        physical_min: None,
        physical_max: None,
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        virtual_intensity_inverted: false,
        behavior: Default::default(),
        functions: Vec::new(),
    }
}

fn head(name: &str) -> FixtureHead {
    FixtureHead {
        id: Uuid::new_v4(),
        name: name.into(),
        master_shared: false,
    }
}

/// A two-cell bar: a 16-bit master dimmer with a fine byte, a red channel per cell, a second red
/// on the second cell, and a gobo rotation with named ranges.
fn bar() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Acme".into();
    profile.name = "Cell Bar".into();
    profile.short_name = "Bar".into();
    profile.physical.width_millimetres = Some(1000.0);
    let mode = &mut profile.modes[0];
    mode.name = "8 bit – 2 cells".into();
    let master = mode.heads[0].id;
    let (first, second) = (head("Cell"), head("Cell"));
    let mut dimmer = channel(master, "intensity", ChannelResolution::U16);
    dimmer.secondary_slots = vec![2];
    dimmer.default_raw = 65_535;
    let mut rotation = channel(master, "gobo.1.rotation", ChannelResolution::U8);
    let mut stopped = ChannelFunction::continuous("Stop", rotation.attribute.clone(), 9);
    stopped.behavior = ChannelFunctionBehavior::Fixed {
        semantic_id: "stop".into(),
        label: "Stopped".into(),
        raw_value: 0,
    };
    let mut spin = ChannelFunction::continuous("Spin", rotation.attribute.clone(), 255);
    spin.dmx_from = 10;
    rotation.functions = vec![spin, stopped];
    mode.channels = vec![
        dimmer,
        channel(first.id, "color.red", ChannelResolution::U8),
        channel(second.id, "color.red", ChannelResolution::U8),
        channel(second.id, "color.red", ChannelResolution::U8),
        rotation,
    ];
    mode.heads.extend([first, second]);
    mode.splits[0].footprint = 6;
    profile
}

#[test]
fn every_channel_keeps_its_exact_slots_head_and_standard_attribute() {
    let fixture = fixture_type(&bar()).expect("a valid profile describes");
    let mode = &fixture.modes[0];
    let described: Vec<_> = mode
        .channels
        .iter()
        .map(|channel| {
            (
                channel.attribute.as_str(),
                channel.offsets(),
                channel.geometry.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        described,
        vec![
            ("Dimmer", vec![1, 2], None),
            ("ColorAdd_R", vec![3], Some("Cell")),
            ("ColorAdd_R", vec![4], Some("Cell 2")),
            ("ColorAdd_R_2", vec![5], Some("Cell 2")),
            ("Gobo1PosRotate", vec![6], None),
        ]
    );
    assert_eq!(mode.footprint(), 6, "the footprint an MVR patches");
    assert_eq!(mode.channels[0].default, 65_535);
    assert_eq!(mode.channels[0].highlight, Some(65_535));
    assert_eq!(fixture.beams, vec!["Cell", "Cell 2"]);
    assert_eq!(fixture.body_size, Some([1.0, 0.3, 0.3]));
}

#[test]
fn mode_names_are_valid_gdtf_names_and_unique() {
    let mut profile = bar();
    let copy = profile.modes[0].clone();
    profile.modes.push(copy);
    assert_eq!(
        mode_names(&profile),
        vec!["8 bit - 2 cells", "8 bit - 2 cells 2"]
    );
    let xml = super::super::description_xml(&fixture_type(&profile).unwrap());
    assert!(xml.contains("<DMXMode Name=\"8 bit - 2 cells 2\""), "{xml}");
}

#[test]
fn named_ranges_start_where_their_functions_start_and_never_overlap() {
    let xml = super::super::description_xml(&fixture_type(&bar()).unwrap());
    assert!(
        xml.contains("<ChannelSet Name=\"Stopped\" DMXFrom=\"0/1\"/>"),
        "{xml}"
    );
    assert!(xml.contains("<ChannelSet Name=\"Spin\" DMXFrom=\"10/1\"/>"));
    assert!(xml.contains("Feature=\"Dimmer.Dimmer\""));
    assert!(xml.contains("Feature=\"Gobo.Gobo\""));
}

#[test]
fn a_fixture_with_only_the_shared_master_still_has_a_beam() {
    let mut profile = FixtureProfile::blank();
    profile.name = "Par".into();
    let master = profile.modes[0].heads[0].id;
    profile.modes[0].channels = vec![channel(master, "intensity", ChannelResolution::U8)];
    let fixture = fixture_type(&profile).unwrap();
    assert_eq!(fixture.beams, vec!["Beam"]);
    assert_eq!(
        fixture.manufacturer, "Generic",
        "GDTF requires a manufacturer"
    );
    assert_eq!(
        fixture.modes[0].channels[0].geometry, None,
        "the body controls the beam below it"
    );
}

#[test]
fn a_profile_whose_slots_cannot_be_derived_is_refused() {
    let mut profile = bar();
    profile.modes[0].splits[0].footprint = 3;
    assert!(fixture_type(&profile).is_err());
}

/// Read XML independently from the writer model: these assertions verify the actual interchange
/// attributes, including full-width integer text, rather than a private conversion helper.
fn xml_nodes(xml: &str, tag: &[u8]) -> Vec<HashMap<String, String>> {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_str(xml);
    let mut result = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(node) | Event::Empty(node) if node.name().as_ref() == tag => {
                result.push(
                    node.attributes()
                        .map(|attribute| {
                            let attribute = attribute.unwrap();
                            (
                                String::from_utf8(attribute.key.as_ref().to_vec()).unwrap(),
                                attribute
                                    .decoded_and_normalized_value(
                                        quick_xml::XmlVersion::Implicit1_0,
                                        reader.decoder(),
                                    )
                                    .unwrap()
                                    .into_owned(),
                            )
                        })
                        .collect(),
                );
            }
            Event::Eof => break,
            _ => {}
        }
    }
    result
}

fn physical_profile(resolution: ChannelResolution) -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.name = "Physical fixture".into();
    let mode = &mut profile.modes[0];
    let mut pan = channel(mode.heads[0].id, "pan", resolution);
    pan.secondary_slots = (1..resolution.bytes())
        .map(|index| 2 * index as u16 + 1)
        .collect();
    pan.default_raw = resolution.max_raw() / 2;
    let mut function =
        ChannelFunction::continuous("Position", pan.attribute.clone(), resolution.max_raw());
    function.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: -720.0,
        physical_max: 720.0,
        unit: Some("deg".into()),
    };
    pan.functions = vec![function];
    mode.splits[0].footprint = (2 * resolution.bytes() - 1) as u16;
    mode.channels = vec![pan];
    profile
}

#[test]
fn all_four_widths_keep_raw_precision_separate_fine_slots_and_signed_endpoints() {
    for resolution in [
        ChannelResolution::U8,
        ChannelResolution::U16,
        ChannelResolution::U24,
        ChannelResolution::U32,
    ] {
        let profile = physical_profile(resolution);
        let xml = super::super::description_xml(&fixture_type(&profile).unwrap());
        let functions = xml_nodes(&xml, b"ChannelFunction");
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0]["DMXFrom"], format!("0/{}", resolution.bytes()));
        assert_eq!(
            functions[0]["Default"],
            format!("{}/{}", resolution.max_raw() / 2, resolution.bytes())
        );
        assert_eq!(functions[0]["PhysicalFrom"], "-720");
        assert_eq!(functions[0]["PhysicalTo"], "720");
        let attributes = xml_nodes(&xml, b"Attribute");
        assert_eq!(attributes[0]["PhysicalUnit"], "Angle");
        let channels = xml_nodes(&xml, b"DMXChannel");
        assert_eq!(
            channels[0]["Offset"],
            (0..resolution.bytes())
                .map(|index| (index * 2 + 1).to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(
            channels[0]["Highlight"],
            format!("{}/{}", resolution.max_raw(), resolution.bytes())
        );
        assert_eq!(channels[0]["InitialFunction"], "Body_Pan.Pan.Position");
    }
}

#[test]
fn function_ranges_preserve_descending_zoom_and_explicit_gaps() {
    let mut profile = physical_profile(ChannelResolution::U16);
    let channel = &mut profile.modes[0].channels[0];
    channel.attribute = AttributeKey("zoom".into());
    channel.physical_min = Some(5.0);
    channel.physical_max = Some(60.0);
    channel.default_raw = 130;
    let first = &mut channel.functions[0];
    first.name = "Narrowing".into();
    first.attribute = channel.attribute.clone();
    first.dmx_from = 100;
    first.dmx_to = 200;
    first.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 60.0,
        physical_max: 5.0,
        unit: Some("deg".into()),
    };
    let mut last = first.clone();
    last.id = Uuid::new_v4();
    last.name = "Wide".into();
    last.dmx_from = 300;
    last.dmx_to = 60_000;
    last.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 5.0,
        physical_max: 45.0,
        unit: Some("deg".into()),
    };
    channel.functions.push(last);
    let xml = super::super::description_xml(&fixture_type(&profile).unwrap());
    let functions = xml_nodes(&xml, b"ChannelFunction");
    let starts: Vec<_> = functions.iter().map(|f| f["DMXFrom"].as_str()).collect();
    assert_eq!(starts, ["0/2", "100/2", "201/2", "300/2", "60001/2"]);
    assert_eq!(functions[1]["PhysicalFrom"], "60");
    assert_eq!(functions[1]["PhysicalTo"], "5");
    assert_eq!(functions[1]["Default"], "130/2");
    assert_eq!(functions[3]["Default"], "300/2");
    for index in [0, 2, 4] {
        assert_eq!(functions[index]["Attribute"], "NoFeature");
        assert!(!functions[index].contains_key("PhysicalFrom"));
    }
    assert_eq!(
        xml_nodes(&xml, b"DMXChannel")[0]["InitialFunction"],
        "Body_Zoom.Zoom.Narrowing"
    );
}

#[test]
fn a_default_inside_a_gap_keeps_its_exact_raw_value_and_initial_function() {
    let mut profile = physical_profile(ChannelResolution::U8);
    let channel = &mut profile.modes[0].channels[0];
    channel.default_raw = 240;
    channel.functions[0].dmx_to = 200;
    let xml = super::super::description_xml(&fixture_type(&profile).unwrap());
    let functions = xml_nodes(&xml, b"ChannelFunction");
    assert_eq!(functions[1]["Attribute"], "NoFeature");
    assert_eq!(functions[1]["Default"], "240/1");
    assert_eq!(
        xml_nodes(&xml, b"DMXChannel")[0]["InitialFunction"],
        "Body_Pan.Pan.Unused"
    );
}

#[test]
fn position_and_rotation_functions_keep_their_own_attributes_and_units() {
    let mut profile = physical_profile(ChannelResolution::U8);
    let channel = &mut profile.modes[0].channels[0];
    channel.functions[0].dmx_to = 127;
    let mut velocity = channel.functions[0].clone();
    velocity.id = Uuid::new_v4();
    velocity.name = "Rotate".into();
    velocity.dmx_from = 128;
    velocity.dmx_to = 255;
    velocity.angular_motion = Some(crate::AngularMotion {
        kind: crate::AngularMotionKind::AngularVelocity,
        max_speed_degrees_per_second: None,
        acceleration_degrees_per_second_squared: None,
        deceleration_degrees_per_second_squared: None,
    });
    velocity.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: -180.0,
        physical_max: 180.0,
        unit: Some("deg/s".into()),
    };
    channel.functions.push(velocity);
    let xml = super::super::description_xml(&fixture_type(&profile).unwrap());
    let functions = xml_nodes(&xml, b"ChannelFunction");
    assert_eq!(functions[0]["Attribute"], "Pan");
    assert_eq!(functions[1]["Attribute"], "PanRotate");
    let attributes = xml_nodes(&xml, b"Attribute");
    assert!(
        attributes
            .iter()
            .any(|a| a["Name"] == "Pan" && a["PhysicalUnit"] == "Angle")
    );
    assert!(
        attributes
            .iter()
            .any(|a| a["Name"] == "PanRotate" && a["PhysicalUnit"] == "AngularSpeed")
    );
}

#[test]
fn physical_units_are_function_local_without_changing_numeric_conventions() {
    let mut profile = physical_profile(ChannelResolution::U8);
    let channel = &mut profile.modes[0].channels[0];
    channel.attribute = AttributeKey("focus".into());
    channel.functions[0].attribute = channel.attribute.clone();
    channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 0.0,
        physical_max: 100.0,
        unit: Some("%".into()),
    };
    let mut second = profile.modes[0].clone();
    second.name = "Unknown units".into();
    second.channels[0].functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 0.0,
        physical_max: 1.0,
        unit: None,
    };
    profile.modes.push(second);
    let xml = super::super::description_xml(&fixture_type(&profile).unwrap());
    let attributes = xml_nodes(&xml, b"Attribute");
    assert!(
        attributes
            .iter()
            .any(|a| a["Name"] == "Focus1" && a["PhysicalUnit"] == "Percent")
    );
    assert!(
        attributes
            .iter()
            .any(|a| a["Name"] == "Focus1_Unknown" && !a.contains_key("PhysicalUnit"))
    );
    let functions = xml_nodes(&xml, b"ChannelFunction");
    assert_eq!(functions[0]["PhysicalTo"], "100");
    assert_eq!(functions[1]["PhysicalTo"], "1");
    assert_eq!(functions[1]["Attribute"], "Focus1_Unknown");
}

#[test]
fn piecewise_calibration_is_refused_instead_of_exporting_a_false_linear_curve() {
    let mut profile = physical_profile(ChannelResolution::U8);
    profile.modes[0].channels[0].functions[0].physical_mapping =
        Some(crate::PhysicalMappingCalibration {
            samples: vec![
                crate::PhysicalMappingPoint {
                    raw: 0,
                    physical: -720.0,
                },
                crate::PhysicalMappingPoint {
                    raw: 127,
                    physical: -100.0,
                },
                crate::PhysicalMappingPoint {
                    raw: 255,
                    physical: 720.0,
                },
            ],
            ..Default::default()
        });
    let error = fixture_type(&profile).unwrap_err().to_string();
    assert!(error.contains("piecewise physical curve"), "{error}");
    assert!(error.contains("native fixture package"), "{error}");
}

#[test]
fn invalid_intervals_units_and_endpoints_are_refused_without_clamping_or_deduplicating() {
    let mut profile = physical_profile(ChannelResolution::U8);
    let mut duplicate = profile.modes[0].channels[0].functions[0].clone();
    duplicate.id = Uuid::new_v4();
    profile.modes[0].channels[0].functions.push(duplicate);
    assert!(
        fixture_type(&profile)
            .unwrap_err()
            .to_string()
            .contains("overlapping")
    );
    profile.modes[0].channels[0].functions.pop();
    profile.modes[0].channels[0].functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 0.0,
        physical_max: 1.0,
        unit: Some("rpm".into()),
    };
    assert!(
        fixture_type(&profile)
            .unwrap_err()
            .to_string()
            .contains("physical unit")
    );
    profile.modes[0].channels[0].functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: f32::NAN,
        physical_max: 1.0,
        unit: None,
    };
    assert!(
        fixture_type(&profile)
            .unwrap_err()
            .to_string()
            .contains("non-finite")
    );
}

#[test]
fn tiny_physical_ranges_survive_archive_xml_without_decimal_truncation() {
    use std::io::Read;
    let mut profile = physical_profile(ChannelResolution::U32);
    profile.modes[0].channels[0].functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: -0.000000123,
        physical_max: 0.000000567,
        unit: Some("m".into()),
    };
    let bytes = package_profile(&profile).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("description.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    let functions = xml_nodes(&xml, b"ChannelFunction");
    assert_eq!(
        functions[0]["PhysicalFrom"].parse::<f32>().unwrap(),
        -0.000000123_f32
    );
    assert_eq!(
        functions[0]["PhysicalTo"].parse::<f32>().unwrap(),
        0.000000567_f32
    );
}

#[test]
fn native_cmy_filters_do_not_export_as_their_canonical_rgb_aliases() {
    for (native, canonical, expected) in [
        ("color.cyan", "color.red", "ColorSub_C"),
        ("color.magenta", "color.green", "ColorSub_M"),
        ("color.yellow", "color.blue", "ColorSub_Y"),
        ("media.layer.cyan", "color.red", "ColorSub_C"),
        ("media.master.master.magenta", "color.green", "ColorSub_M"),
    ] {
        let mut profile = physical_profile(ChannelResolution::U16);
        let channel = &mut profile.modes[0].channels[0];
        channel.fixture_attribute = AttributeKey(native.into());
        channel.attribute = AttributeKey(canonical.into());
        channel.canonical_transform = crate::CanonicalTransform::InvertNormalized;
        channel.default_raw = 321;
        channel.highlight_raw = 42;
        channel.functions[0].attribute = channel.attribute.clone();
        channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
            physical_min: 100.0,
            physical_max: 0.0,
            unit: Some("percent".into()),
        };
        let xml = super::super::description_xml(&fixture_type(&profile).unwrap());
        assert_eq!(xml_nodes(&xml, b"LogicalChannel")[0]["Attribute"], expected);
        let functions = xml_nodes(&xml, b"ChannelFunction");
        assert_eq!(functions[0]["Attribute"], expected);
        assert_eq!(functions[0]["OriginalAttribute"], native);
        assert_eq!(functions[0]["Default"], "321/2");
        assert_eq!(functions[0]["PhysicalFrom"], "100");
        assert_eq!(functions[0]["PhysicalTo"], "0");
        assert_eq!(xml_nodes(&xml, b"DMXChannel")[0]["Highlight"], "42/2");
        assert!(!xml.contains("ColorAdd_"));
    }
}

#[test]
fn a_different_function_on_a_cmy_channel_keeps_its_own_attribute() {
    let mut profile = physical_profile(ChannelResolution::U8);
    let channel = &mut profile.modes[0].channels[0];
    channel.fixture_attribute = AttributeKey("color.cyan".into());
    channel.attribute = AttributeKey("color.red".into());
    channel.canonical_transform = crate::CanonicalTransform::InvertNormalized;
    channel.functions[0].attribute = AttributeKey("shutter".into());
    let xml = super::super::description_xml(&fixture_type(&profile).unwrap());
    assert_eq!(
        xml_nodes(&xml, b"LogicalChannel")[0]["Attribute"],
        "ColorSub_C"
    );
    assert_eq!(
        xml_nodes(&xml, b"ChannelFunction")[0]["Attribute"],
        "Shutter1"
    );
}
