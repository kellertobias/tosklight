use super::*;

#[test]
fn derives_primary_slots_around_reserved_component_bytes() {
    let head_id = Uuid::new_v4();
    let first = channel(head_id, ChannelResolution::U16, vec![2]);
    let second = channel(head_id, ChannelResolution::U24, vec![5, 6]);
    let third = channel(head_id, ChannelResolution::U8, vec![]);
    let mode = FixtureMode {
        id: Uuid::new_v4(),
        name: "Mode".into(),
        notes: String::new(),
        splits: vec![FixtureSplit {
            number: 1,
            footprint: 6,
        }],
        heads: vec![FixtureHead {
            id: head_id,
            name: "Main".into(),
            master_shared: true,
        }],
        channels: vec![first.clone(), second.clone(), third.clone()],
        color_systems: vec![],
        control_actions: vec![],
        geometry: GeometryGraph::default(),
    };
    let slots = mode.primary_slots().unwrap();
    assert_eq!(slots[&first.id], 1);
    assert_eq!(slots[&second.id], 3);
    assert_eq!(slots[&third.id], 4);
}

#[test]
fn rejects_duplicate_components_and_overlapping_functions() {
    let head_id = Uuid::new_v4();
    let mut first = channel(head_id, ChannelResolution::U16, vec![2]);
    let second = channel(head_id, ChannelResolution::U16, vec![2]);
    first.functions.push(ChannelFunction {
        id: Uuid::new_v4(),
        name: "Conflict".into(),
        dmx_from: 100,
        dmx_to: 200,
        attribute: AttributeKey("strobe".into()),
        priority: 100,
        behavior: ChannelFunctionBehavior::Fixed {
            semantic_id: "strobe".into(),
            label: "Strobe".into(),
            raw_value: 150,
        },
    });
    assert!(
        matches!(first.validate(), Err(ProfileError::Invalid(message)) if message.contains("overlap"))
    );
    first.functions.pop();
    let mode = FixtureMode {
        id: Uuid::new_v4(),
        name: "Mode".into(),
        notes: String::new(),
        splits: vec![FixtureSplit {
            number: 1,
            footprint: 4,
        }],
        heads: vec![FixtureHead {
            id: head_id,
            name: "Main".into(),
            master_shared: true,
        }],
        channels: vec![first, second],
        color_systems: vec![],
        control_actions: vec![],
        geometry: GeometryGraph::default(),
    };
    assert!(
        matches!(mode.primary_slots(), Err(ProfileError::Invalid(message)) if message.contains("duplicated"))
    );
}

#[test]
fn blank_profile_has_one_default_mode_and_head() {
    let draft = FixtureProfile::blank();
    assert_eq!(draft.modes.len(), 1);
    assert_eq!(draft.modes[0].name, "Default");
    assert_eq!(draft.modes[0].heads.len(), 1);
}

#[test]
fn mode_rejects_more_than_one_master_shared_head() {
    let mut mode = FixtureProfile::blank().modes.remove(0);
    mode.heads.push(FixtureHead {
        id: Uuid::new_v4(),
        name: "Shared 2".into(),
        master_shared: true,
    });

    assert!(matches!(
        mode.validate(),
        Err(ProfileError::Invalid(message))
            if message == "at most one head can be master/shared"
    ));
}

#[test]
fn mode_rejects_a_channel_that_references_a_missing_split() {
    let mut mode = FixtureProfile::blank().modes.remove(0);
    mode.channels
        .push(channel(mode.heads[0].id, ChannelResolution::U8, vec![]));
    mode.channels[0].split = 2;

    assert!(matches!(
        mode.validate(),
        Err(ProfileError::Invalid(message))
            if message == "channel references a missing split"
    ));
}

#[test]
fn legacy_migration_derives_invert_aware_full_white_and_open_wheel_highlight() {
    let attributes = [
        ("intensity", 0.0, false),
        ("color.red", 0.0, false),
        ("color.green", 0.0, false),
        ("color.blue", 0.0, false),
        ("color.white", 0.0, false),
        ("color.cyan", 0.0, true),
        ("color.magenta", 0.0, false),
        ("color.yellow", 0.0, false),
        ("color.emitter.red", 0.0, false),
        ("color.emitter.green", 0.0, false),
        ("color.emitter.blue", 0.0, false),
        ("color.wheel.1", 7.0 / 255.0, false),
        ("pan", 0.5, false),
    ];
    let parameters = attributes
        .iter()
        .enumerate()
        .map(|(offset, (attribute, default, invert))| Parameter {
            attribute: AttributeKey((*attribute).into()),
            components: vec![ChannelComponent {
                offset: offset as u16,
                byte_order: ByteOrder::MsbFirst,
            }],
            default: *default,
            virtual_dimmer: false,
            metadata: ParameterMetadata {
                invert: *invert,
                ..Default::default()
            },
            capabilities: if *attribute == "color.wheel.1" {
                vec![Capability {
                    name: "Open / White".into(),
                    dmx_from: 12,
                    dmx_to: 18,
                    preset_family: Some("color".into()),
                }]
            } else {
                Vec::new()
            },
        })
        .collect::<Vec<_>>();
    let definition = FixtureDefinition {
        schema_version: 1,
        id: FixtureId::new(),
        revision: 1,
        manufacturer: "Test".into(),
        device_type: "wash".into(),
        name: "Semantic Highlight".into(),
        model: "Semantic Highlight".into(),
        mode: "Default".into(),
        footprint: parameters.len() as u16,
        heads: vec![LogicalHead {
            index: 0,
            name: "Main".into(),
            shared: true,
            parameters,
        }],
        color_calibration: Some(ColorCalibration {
            emitters: ["red", "green", "blue"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| EmitterCalibration {
                    name: name.into(),
                    xyz: match index {
                        0 => Xyz {
                            x: 1.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        1 => Xyz {
                            x: 0.0,
                            y: 1.0,
                            z: 0.0,
                        },
                        _ => Xyz {
                            x: 0.0,
                            y: 0.0,
                            z: 1.0,
                        },
                    },
                    limit: 1.0,
                })
                .collect(),
            correction_matrix: identity_color_correction(),
        }),
        physical: FixturePhysicalProperties::default(),
        model_asset: None,
        icon_asset: None,
        hazardous: false,
        direct_control_protocols: Vec::new(),
        signal_loss_policy: SignalLossPolicy::HoldLast,
        safe_values: BTreeMap::new(),
        profile_id: None,
        mode_id: None,
        profile_snapshot: None,
    };

    let profile = FixtureProfile::from_legacy_modes(&[definition]).unwrap();
    let mode = &profile.modes[0];
    let highlights = mode
        .channels
        .iter()
        .map(|channel| (channel.attribute.0.as_str(), channel.highlight_raw))
        .collect::<HashMap<_, _>>();
    assert_eq!(highlights["intensity"], 255);
    assert_eq!(highlights["color.red"], 255);
    assert_eq!(highlights["color.green"], 255);
    assert_eq!(highlights["color.blue"], 255);
    assert_eq!(highlights["color.white"], 255);
    assert_eq!(highlights["color.cyan"], 255, "inverted no-filter endpoint");
    assert_eq!(highlights["color.magenta"], 0);
    assert_eq!(highlights["color.yellow"], 0);
    assert_eq!(highlights["color.wheel.1"], 15);
    assert_eq!(highlights["pan"], 128);
    let calibrated_white = mode
        .resolve_color(mode.heads[0].id, SEMANTIC_WHITE_XYZ)
        .unwrap();
    for attribute in [
        "color.emitter.red",
        "color.emitter.green",
        "color.emitter.blue",
    ] {
        let channel = mode
            .channels
            .iter()
            .find(|channel| channel.attribute.0 == attribute)
            .unwrap();
        assert_eq!(channel.highlight_raw, calibrated_white[&channel.id]);
    }
}

#[test]
fn authored_schema_v2_highlight_raw_is_not_rederived() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Authored Highlight".into();
    profile.revision = 4;
    let mode = &mut profile.modes[0];
    let mut authored = channel(mode.heads[0].id, ChannelResolution::U8, Vec::new());
    authored.attribute = AttributeKey("color.cyan".into());
    authored.highlight_raw = 73;
    mode.channels = vec![authored.clone()];

    let encoded = serde_json::to_string(&profile).unwrap();
    let decoded: FixtureProfile = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.modes[0].channels[0].highlight_raw, 73);
    let definition = decoded.resolved_definition(decoded.modes[0].id).unwrap();
    assert_eq!(
        definition.profile_snapshot.unwrap().modes[0].channels[0].highlight_raw,
        73
    );
}

#[test]
fn complete_physical_metadata_round_trips_and_older_profiles_receive_safe_defaults() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Complete physical metadata".into();
    profile.physical.connectors = "powerCON TRUE1 TOP; 5-pin XLR in/out".into();
    profile.physical.light_source = "600 W LED engine".into();
    profile.physical.color_temperature_kelvin = Some(6_500.0);
    profile.physical.color_rendering_index = Some(92.0);
    profile.physical.luminous_output_lumens = Some(18_500.0);
    profile.physical.lens = "Fresnel zoom".into();
    profile.physical.beam_angle_degrees = Some(36.0);

    let encoded = serde_json::to_value(&profile).unwrap();
    let decoded: FixtureProfile = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(decoded.physical, profile.physical);
    decoded.validate().unwrap();

    let mut legacy = encoded;
    let physical = legacy["physical"].as_object_mut().unwrap();
    for field in [
        "connectors",
        "light_source",
        "color_temperature_kelvin",
        "color_rendering_index",
        "luminous_output_lumens",
        "lens",
        "beam_angle_degrees",
    ] {
        physical.remove(field);
    }
    let migrated: FixtureProfile = serde_json::from_value(legacy).unwrap();
    assert_eq!(migrated.physical, ProfilePhysicalProperties::default());
    migrated.validate().unwrap();
}

#[test]
fn legacy_geometry_emitters_default_to_directional_and_explicit_broad_sources_round_trip() {
    let mut profile = FixtureProfile::blank();
    let head_id = profile.modes[0].heads[0].id;
    profile.modes[0].geometry = GeometryGraph::template(GeometryTemplate::Fixed, &[head_id]);
    let node_id = profile.modes[0].geometry.nodes[0].id;
    profile.modes[0].geometry.emitters.push(GeometryEmitter {
        id: Uuid::new_v4(),
        name: "Beam".into(),
        node_id,
        head_id,
        origin: Vector3::default(),
        orientation_degrees: Vector3::default(),
        beam_angle_degrees: 20.0,
        field_angle_degrees: 24.0,
        feather: 0.0,
        focus: 1.0,
        directional: true,
        layout: EmitterLayout::Point,
    });
    let mut legacy = serde_json::to_value(&profile).unwrap();
    legacy["modes"][0]["geometry"]["emitters"][0]
        .as_object_mut()
        .unwrap()
        .remove("directional");
    let mut migrated: FixtureProfile = serde_json::from_value(legacy).unwrap();
    assert!(migrated.modes[0].geometry.emitters[0].directional);
    migrated.modes[0].geometry.emitters[0].directional = false;
    let restored: FixtureProfile =
        serde_json::from_value(serde_json::to_value(migrated).unwrap()).unwrap();
    assert!(!restored.modes[0].geometry.emitters[0].directional);
}

#[test]
fn legacy_head_split_migrates_to_channels_and_serializes_canonically() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Legacy split".into();
    let mode = &mut profile.modes[0];
    mode.channels = vec![channel(mode.heads[0].id, ChannelResolution::U8, vec![])];
    let mut value = serde_json::to_value(&profile).unwrap();
    let mode = &mut value["modes"][0];
    mode["heads"][0]["split"] = serde_json::json!(1);
    mode["channels"][0].as_object_mut().unwrap().remove("split");

    let migrated: FixtureProfile = serde_json::from_value(value).unwrap();
    assert_eq!(migrated.modes[0].channels[0].split, 1);
    let canonical = serde_json::to_value(migrated).unwrap();
    assert!(canonical["modes"][0]["heads"][0].get("split").is_none());
    assert_eq!(canonical["modes"][0]["channels"][0]["split"], 1);
}
