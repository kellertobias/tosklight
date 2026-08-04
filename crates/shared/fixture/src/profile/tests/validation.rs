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
        .map(|channel| (channel.fixture_attribute.0.as_str(), channel.highlight_raw))
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
fn authored_profile_highlight_raw_is_not_rederived() {
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
fn schema_v2_channels_migrate_to_explicit_identity_mappings() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Legacy mapping".into();
    let mode = &mut profile.modes[0];
    mode.channels = vec![channel(mode.heads[0].id, ChannelResolution::U8, vec![])];
    let mut value = serde_json::to_value(&profile).unwrap();
    value["schema_version"] = serde_json::json!(2);
    value["modes"][0]["channels"][0]
        .as_object_mut()
        .unwrap()
        .remove("fixture_attribute");
    value["modes"][0]["channels"][0]
        .as_object_mut()
        .unwrap()
        .remove("canonical_transform");

    let migrated: FixtureProfile = serde_json::from_value(value).unwrap();
    assert_eq!(migrated.schema_version, FIXTURE_PROFILE_SCHEMA_VERSION);
    let channel = &migrated.modes[0].channels[0];
    assert_eq!(channel.fixture_attribute, channel.attribute);
    assert_eq!(channel.canonical_transform, CanonicalTransform::Identity);
    migrated.validate().unwrap();

    let canonical = serde_json::to_value(migrated).unwrap();
    assert_eq!(canonical["schema_version"], FIXTURE_PROFILE_SCHEMA_VERSION);
    assert_eq!(
        canonical["modes"][0]["channels"][0]["fixture_attribute"],
        canonical["modes"][0]["channels"][0]["attribute"]
    );
    assert_eq!(
        canonical["modes"][0]["channels"][0]["canonical_transform"],
        "identity"
    );
}

#[test]
fn schema_v2_cmy_channels_migrate_to_inverted_canonical_rgb_without_losing_identity() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Legacy CMY mapping".into();
    let mode = &mut profile.modes[0];
    let mut cyan = channel(mode.heads[0].id, ChannelResolution::U8, vec![]);
    cyan.fixture_attribute = AttributeKey("color.cyan".into());
    cyan.attribute = AttributeKey("color.cyan".into());
    cyan.functions = vec![ChannelFunction::continuous(
        "Cyan filtration",
        AttributeKey("color.cyan".into()),
        255,
    )];
    mode.channels = vec![cyan];
    let mut value = serde_json::to_value(&profile).unwrap();
    value["schema_version"] = serde_json::json!(2);
    let channel = value["modes"][0]["channels"][0].as_object_mut().unwrap();
    channel.remove("fixture_attribute");
    channel.remove("canonical_transform");

    let migrated: FixtureProfile = serde_json::from_value(value).unwrap();
    let channel = &migrated.modes[0].channels[0];
    assert_eq!(channel.fixture_attribute, AttributeKey("color.cyan".into()));
    assert_eq!(channel.attribute, AttributeKey("color.red".into()));
    assert_eq!(
        channel.canonical_transform,
        CanonicalTransform::InvertNormalized
    );
    assert_eq!(
        channel.functions[0].attribute,
        AttributeKey("color.red".into())
    );
    migrated.validate().unwrap();
}

#[test]
fn schema_v2_cct_emitters_migrate_to_white_and_amber_without_losing_identity() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Legacy CCT mapping".into();
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 2;
    let head_id = mode.heads[0].id;
    mode.channels = [
        ("color.cold_white", "Cold White"),
        ("color.warm_white", "Warm White"),
    ]
    .into_iter()
    .map(|(attribute, name)| {
        let mut emitter = channel(head_id, ChannelResolution::U8, vec![]);
        emitter.fixture_attribute = AttributeKey(attribute.into());
        emitter.attribute = AttributeKey(attribute.into());
        emitter.functions = vec![ChannelFunction::continuous(
            name,
            AttributeKey(attribute.into()),
            255,
        )];
        emitter
    })
    .collect();
    let mut value = serde_json::to_value(&profile).unwrap();
    value["schema_version"] = serde_json::json!(2);
    for channel in value["modes"][0]["channels"].as_array_mut().unwrap() {
        let channel = channel.as_object_mut().unwrap();
        channel.remove("fixture_attribute");
        channel.remove("canonical_transform");
    }

    let migrated: FixtureProfile = serde_json::from_value(value).unwrap();
    for (channel, fixture_attribute, canonical) in [
        (
            &migrated.modes[0].channels[0],
            "color.cold_white",
            "color.white",
        ),
        (
            &migrated.modes[0].channels[1],
            "color.warm_white",
            "color.amber",
        ),
    ] {
        assert_eq!(
            channel.fixture_attribute,
            AttributeKey(fixture_attribute.into())
        );
        assert_eq!(channel.attribute, AttributeKey(canonical.into()));
        assert_eq!(channel.canonical_transform, CanonicalTransform::Identity);
        assert_eq!(
            channel.functions[0].attribute,
            AttributeKey(canonical.into())
        );
    }
    migrated.validate().unwrap();
}

#[test]
fn schema_v3_cct_channels_from_existing_installations_migrate_idempotently() {
    let mut profile = FixtureProfile::blank();
    let mode = &mut profile.modes[0];
    let mut emitter = channel(mode.heads[0].id, ChannelResolution::U8, vec![]);
    emitter.fixture_attribute = AttributeKey("Imported:ColdWhite".into());
    emitter.attribute = AttributeKey("color.cold_white".into());
    emitter.functions[0].attribute = AttributeKey("color.cold_white".into());
    mode.channels = vec![emitter];

    let migrated: FixtureProfile =
        serde_json::from_value(serde_json::to_value(profile).unwrap()).unwrap();
    let channel = &migrated.modes[0].channels[0];
    assert_eq!(channel.fixture_attribute.0, "Imported:ColdWhite");
    assert_eq!(channel.attribute.0, "color.white");
    assert_eq!(channel.functions[0].attribute.0, "color.white");
    assert_eq!(channel.canonical_transform, CanonicalTransform::Identity);

    let second: FixtureProfile =
        serde_json::from_value(serde_json::to_value(&migrated).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(second).unwrap(),
        serde_json::to_value(migrated).unwrap()
    );
}

#[test]
fn canonical_cct_migration_rejects_two_physical_channels_for_the_same_control() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Ambiguous white emitters".into();
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 2;
    let head_id = mode.heads[0].id;
    mode.channels = ["color.white", "color.cold_white"]
        .into_iter()
        .map(|attribute| {
            let mut emitter = channel(head_id, ChannelResolution::U8, vec![]);
            emitter.fixture_attribute = AttributeKey(attribute.into());
            emitter.attribute = AttributeKey(attribute.into());
            emitter.functions[0].attribute = AttributeKey(attribute.into());
            emitter
        })
        .collect();

    let migrated: FixtureProfile =
        serde_json::from_value(serde_json::to_value(profile).unwrap()).unwrap();
    let error = migrated.validate().unwrap_err();
    assert!(error.to_string().contains(
        "split 1 maps more than one channel on the same head to canonical attribute `color.white`"
    ));
}

#[test]
fn primary_softness_migration_rejects_an_independent_second_mechanism() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Independent frost and edge".into();
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 2;
    let head_id = mode.heads[0].id;
    mode.channels = ["frost.1", "beam.edge"]
        .into_iter()
        .map(|attribute| {
            let mut channel = channel(head_id, ChannelResolution::U8, vec![]);
            channel.fixture_attribute = AttributeKey(attribute.into());
            channel.attribute = AttributeKey(attribute.into());
            channel.functions[0].attribute = AttributeKey(attribute.into());
            channel
        })
        .collect();

    let migrated: FixtureProfile =
        serde_json::from_value(serde_json::to_value(profile).unwrap()).unwrap();
    let error = migrated.validate().unwrap_err();
    assert!(error.to_string().contains(
        "split 1 maps more than one channel on the same head to canonical attribute `softness`"
    ));
}

#[test]
fn legacy_media_channels_migrate_to_shared_canonical_controls_without_losing_identity() {
    for (source, target) in [
        ("media.opacity", "intensity"),
        ("media.rotation", "position.rotation"),
        ("media.tint", "color"),
    ] {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = format!("Legacy {source}");
        let mode = &mut profile.modes[0];
        let mut legacy = channel(mode.heads[0].id, ChannelResolution::U8, vec![]);
        legacy.fixture_attribute = AttributeKey(format!("Imported:{source}"));
        legacy.attribute = AttributeKey(source.into());
        legacy.functions[0].attribute = AttributeKey(source.into());
        mode.channels = vec![legacy];

        let migrated: FixtureProfile =
            serde_json::from_value(serde_json::to_value(profile).unwrap()).unwrap();
        let channel = &migrated.modes[0].channels[0];
        assert_eq!(channel.fixture_attribute.0, format!("Imported:{source}"));
        assert_eq!(channel.attribute.0, target);
        assert_eq!(channel.functions[0].attribute.0, target);
        assert_eq!(channel.canonical_transform, CanonicalTransform::Identity);
        migrated.validate().unwrap();
    }
}

#[test]
fn legacy_media_aliases_reject_same_head_collisions_but_allow_distinct_heads() {
    for (source, target) in [
        ("media.opacity", "intensity"),
        ("media.rotation", "position.rotation"),
        ("media.tint", "color"),
    ] {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = format!("Ambiguous {source}");
        let mode = &mut profile.modes[0];
        mode.splits[0].footprint = 2;
        let head_id = mode.heads[0].id;
        mode.channels = [target, source]
            .into_iter()
            .map(|attribute| {
                let mut channel = channel(head_id, ChannelResolution::U8, vec![]);
                channel.fixture_attribute = AttributeKey(attribute.into());
                channel.attribute = AttributeKey(attribute.into());
                channel.functions[0].attribute = AttributeKey(attribute.into());
                channel
            })
            .collect();
        let migrated: FixtureProfile =
            serde_json::from_value(serde_json::to_value(profile).unwrap()).unwrap();
        let error = migrated.validate().unwrap_err();
        assert!(error.to_string().contains(&format!(
            "split 1 maps more than one channel on the same head to canonical attribute `{target}`"
        )));

        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = format!("Independent {source} heads");
        let mode = &mut profile.modes[0];
        mode.splits[0].footprint = 2;
        let primary_head = mode.heads[0].id;
        let secondary_head = Uuid::new_v4();
        mode.heads.push(FixtureHead {
            id: secondary_head,
            name: "Layer 2".into(),
            master_shared: false,
        });
        mode.channels = [(primary_head, target), (secondary_head, source)]
            .into_iter()
            .map(|(head_id, attribute)| {
                let mut channel = channel(head_id, ChannelResolution::U8, vec![]);
                channel.fixture_attribute = AttributeKey(attribute.into());
                channel.attribute = AttributeKey(attribute.into());
                channel.functions[0].attribute = AttributeKey(attribute.into());
                channel
            })
            .collect();
        let migrated: FixtureProfile =
            serde_json::from_value(serde_json::to_value(profile).unwrap()).unwrap();
        migrated.validate().unwrap();
        assert!(
            migrated.modes[0]
                .channels
                .iter()
                .all(|channel| channel.attribute.0 == target)
        );
    }
}

#[test]
fn schema_v2_strobe_migrates_to_canonical_shutter_without_losing_identity() {
    let mut profile = FixtureProfile::blank();
    profile.schema_version = 2;
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 1;
    let mut strobe = channel(mode.heads[0].id, ChannelResolution::U8, vec![]);
    strobe.fixture_attribute = AttributeKey("strobe".into());
    strobe.attribute = AttributeKey("strobe".into());
    strobe.functions[0].attribute = AttributeKey("strobe".into());
    mode.channels = vec![strobe];
    let mut encoded = serde_json::to_value(profile).unwrap();
    encoded["modes"][0]["channels"][0]
        .as_object_mut()
        .unwrap()
        .remove("fixture_attribute");
    let migrated: FixtureProfile = serde_json::from_value(encoded).unwrap();
    let channel = &migrated.modes[0].channels[0];
    assert_eq!(channel.fixture_attribute.0, "strobe");
    assert_eq!(channel.attribute.0, "shutter");
    assert_eq!(channel.canonical_transform, CanonicalTransform::Identity);
    assert_eq!(channel.functions[0].attribute.0, "shutter");
}

#[test]
fn schema_v2_named_aliases_are_limited_to_documented_unambiguous_mappings() {
    for (legacy, canonical) in [
        ("fog", "intensity"),
        ("media.volume", "volume"),
        ("fixture.tint", "color.tint"),
        ("fixture.pan_tilt_time", "position.movement"),
        ("fixture.pan_tilt_speed", "position.movement"),
        ("fixture.pan_tilt_speed_time", "position.movement"),
        ("fixture.mspeed", "position.movement"),
        ("prism.prism", "prism.1"),
        ("prism.prism_insertion", "prism.1"),
        ("prism.prism_rotation", "prism.1.rotation"),
        ("fixture.blade_1", "shaper.blade.1.position"),
        ("fixture.blade_2", "shaper.blade.2.position"),
        ("fixture.blade_3", "shaper.blade.3.position"),
        ("fixture.blade_4", "shaper.blade.4.position"),
        ("fixture.framing_module_rotation", "shaper.rotation"),
        ("fixture.barndoor_module_rotation", "shaper.rotation"),
        ("frost", "softness"),
    ] {
        assert_eq!(
            legacy_canonical_mapping(&AttributeKey(legacy.into())),
            Some((AttributeKey(canonical.into()), CanonicalTransform::Identity))
        );
    }
    assert_eq!(
        legacy_canonical_mapping(&AttributeKey("fixture.effect_1".into())),
        None,
        "an ambiguous numbered effect remains a preserved custom identity"
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

/// A library that has never been told anything about a lantern's optics has to keep working. The
/// block is optional, absent means "ask the fixture type", and reading a file that predates it
/// must not fail or invent numbers.
#[test]
fn a_profile_without_an_optics_block_reads_and_declares_nothing() {
    let profile = FixtureProfile::blank();
    let json = serde_json::to_value(&profile).expect("serialises");
    let optics = json.get("optics").expect("the block is always written");
    assert!(
        optics.as_object().is_some_and(|block| block.is_empty()),
        "an undeclared block stays empty rather than writing guesses: {optics}"
    );
    let round_tripped: FixtureProfile = serde_json::from_value(json).expect("reads back");
    assert_eq!(round_tripped.optics, ProfileOptics::default());
    assert_eq!(round_tripped.optics.sharpness, None);
}

/// What a profile does declare has to survive the archive, because this is the whole point: the
/// library, not the renderer, decides what a fixture's light looks like.
#[test]
fn declared_optics_round_trip_through_the_package_format() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Generic".into();
    profile.name = "Test".into();
    profile.optics = ProfileOptics {
        output: Some(1.8),
        sharpness: Some(0.9),
        uniformity: Some(0.25),
        light_source: Some(ProfileLightSource {
            form: LightSourceForm::Oval,
            width_millimetres: 200.0,
            height_millimetres: 160.0,
        }),
    };
    profile.validate().expect("valid");
    let json = serde_json::to_string(&profile).expect("serialises");
    let round_tripped: FixtureProfile = serde_json::from_str(&json).expect("reads back");
    assert_eq!(round_tripped.optics, profile.optics);
}

/// An out-of-range figure is a transcription error, and the package validator is where an author
/// finds out — not the renderer, silently clamping it three layers away.
#[test]
fn optical_figures_outside_their_range_are_refused() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Generic".into();
    profile.name = "Test".into();
    for optics in [
        ProfileOptics {
            sharpness: Some(40.0),
            ..ProfileOptics::default()
        },
        ProfileOptics {
            uniformity: Some(-0.5),
            ..ProfileOptics::default()
        },
        ProfileOptics {
            output: Some(0.0),
            ..ProfileOptics::default()
        },
        ProfileOptics {
            light_source: Some(ProfileLightSource {
                form: LightSourceForm::Round,
                width_millimetres: 0.0,
                height_millimetres: 120.0,
            }),
            ..ProfileOptics::default()
        },
    ] {
        profile.optics = optics;
        assert!(
            profile.validate().is_err(),
            "an impossible figure must be reported: {optics:?}"
        );
    }
}
