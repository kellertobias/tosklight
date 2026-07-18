use super::*;

#[test]
fn geometry_motion_uses_physical_range_without_changing_profile_data() {
    let node_id = Uuid::new_v4();
    let graph = GeometryGraph {
        nodes: vec![GeometryNode {
            id: node_id,
            name: "Yoke".into(),
            parent_id: None,
            transform: Transform3::default(),
            pivot: Vector3::default(),
            glb_node: None,
            motion: Some(GeometryMotion {
                attribute: AttributeKey("pan".into()),
                kind: GeometryMotionKind::Rotation,
                axis: Vector3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
                physical_min: -270.0,
                physical_max: 270.0,
            }),
        }],
        emitters: vec![],
    };
    let values = HashMap::from([(AttributeKey("pan".into()), AttributeValue::Normalized(0.75))]);
    assert_eq!(
        graph.resolved_transforms(&values)[&node_id]
            .rotation_degrees
            .y,
        135.0
    );
    assert_eq!(graph.nodes[0].transform.rotation_degrees.y, 0.0);
}

#[test]
fn additive_color_applies_response_drive_limit_inversion_and_gamut_clipping() {
    let mut profile = FixtureProfile::blank();
    let mode = &mut profile.modes[0];
    let head_id = mode.heads[0].id;
    mode.splits[0].footprint = 3;
    mode.channels = ["red", "green", "blue"]
        .into_iter()
        .map(|name| {
            let mut channel = channel(head_id, ChannelResolution::U8, vec![]);
            channel.attribute = AttributeKey(format!("color.{name}"));
            channel
        })
        .collect();
    mode.channels[0].invert = true;
    mode.color_systems = vec![HeadColorSystem {
        head_id,
        correction_matrix: identity_color_correction(),
        system: ColorSystem::Additive {
            emitters: mode
                .channels
                .iter()
                .enumerate()
                .map(|(index, channel)| EmitterBinding {
                    channel_id: channel.id,
                    name: channel.attribute.0.clone(),
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
                    maximum_level: if index == 0 { 0.5 } else { 1.0 },
                    response_curve: if index == 0 { 2.0 } else { 1.0 },
                    visible: true,
                })
                .collect(),
        },
    }];
    mode.validate().unwrap();

    let resolved = mode
        .resolve_color(
            head_id,
            Xyz {
                x: 0.25,
                y: 0.0,
                z: 0.0,
            },
        )
        .unwrap();
    assert_eq!(resolved[&mode.channels[0].id], 127);
    assert_eq!(resolved[&mode.channels[1].id], 0);
    assert_eq!(resolved[&mode.channels[2].id], 0);

    let clipped = mode
        .resolve_color(
            head_id,
            Xyz {
                x: 2.0,
                y: 0.0,
                z: 0.5,
            },
        )
        .unwrap();
    assert_eq!(clipped[&mode.channels[0].id], 127);
    assert_eq!(clipped[&mode.channels[1].id], 0);
    assert_eq!(clipped[&mode.channels[2].id], 128);
}

#[test]
fn subtractive_color_uses_cmy_fallback_and_honors_continuous_inversion() {
    let mut profile = FixtureProfile::blank();
    let mode = &mut profile.modes[0];
    let head_id = mode.heads[0].id;
    mode.splits[0].footprint = 3;
    mode.channels = ["cyan", "magenta", "yellow"]
        .into_iter()
        .map(|name| {
            let mut channel = channel(head_id, ChannelResolution::U8, vec![]);
            channel.attribute = AttributeKey(format!("color.{name}"));
            channel
        })
        .collect();
    mode.channels[1].invert = true;
    mode.color_systems = vec![HeadColorSystem {
        head_id,
        correction_matrix: identity_color_correction(),
        system: ColorSystem::Subtractive {
            cyan_channel_id: mode.channels[0].id,
            magenta_channel_id: mode.channels[1].id,
            yellow_channel_id: mode.channels[2].id,
        },
    }];
    mode.validate().unwrap();

    let resolved = mode
        .resolve_color(head_id, crate::srgb_to_xyz(1.0, 0.0, 0.0))
        .unwrap();
    assert_eq!(resolved[&mode.channels[0].id], 0);
    assert_eq!(resolved[&mode.channels[1].id], 0);
    assert_eq!(resolved[&mode.channels[2].id], 255);
}

#[test]
fn discrete_color_wheel_selects_measured_slot_as_an_exact_fixture_raw_value() {
    let mut profile = FixtureProfile::blank();
    let mode = &mut profile.modes[0];
    let head_id = mode.heads[0].id;
    let mut wheel = channel(head_id, ChannelResolution::U8, vec![]);
    wheel.attribute = AttributeKey("color.wheel.1".into());
    wheel.invert = true;
    let wheel_id = wheel.id;
    mode.channels = vec![wheel];
    let red = crate::srgb_to_xyz(1.0, 0.0, 0.0);
    let blue = crate::srgb_to_xyz(0.0, 0.0, 1.0);
    mode.color_systems = vec![HeadColorSystem {
        head_id,
        correction_matrix: identity_color_correction(),
        system: ColorSystem::DiscreteWheel {
            channel_id: wheel_id,
            slots: vec![
                ColorWheelSlot {
                    semantic_id: "red".into(),
                    label: "Red".into(),
                    dmx_from: 10,
                    dmx_to: 40,
                    measured_xyz: Some(red),
                },
                ColorWheelSlot {
                    semantic_id: "blue".into(),
                    label: "Blue".into(),
                    dmx_from: 100,
                    dmx_to: 140,
                    measured_xyz: Some(blue),
                },
            ],
        },
    }];
    mode.validate().unwrap();

    assert_eq!(mode.resolve_color(head_id, blue).unwrap()[&wheel_id], 120);
}

#[test]
fn rejects_non_finite_and_negative_additive_calibration() {
    let valid = additive_color_mode();
    valid.validate().unwrap();

    for invalid in [f32::NAN, f32::INFINITY, -0.1] {
        let mut mode = valid.clone();
        additive_emitter(&mut mode).xyz.x = invalid;
        assert!(matches!(
            mode.validate(),
            Err(ProfileError::Invalid(message))
                if message.contains("additive emitter calibration")
        ));

        let mut mode = valid.clone();
        additive_emitter(&mut mode).maximum_level = invalid;
        assert!(matches!(
            mode.validate(),
            Err(ProfileError::Invalid(message))
                if message.contains("additive emitter calibration")
        ));

        let mut mode = valid.clone();
        additive_emitter(&mut mode).response_curve = invalid;
        assert!(matches!(
            mode.validate(),
            Err(ProfileError::Invalid(message))
                if message.contains("additive emitter calibration")
        ));
    }
}

#[test]
fn rejects_invalid_discrete_wheel_slot_metadata_and_ranges() {
    let valid = discrete_color_mode();
    valid.validate().unwrap();

    let mut empty = valid.clone();
    wheel_slots(&mut empty).clear();
    assert!(empty.validate().is_err());

    let mut empty_semantic_id = valid.clone();
    wheel_slots(&mut empty_semantic_id)[0].semantic_id = "  ".into();
    assert!(empty_semantic_id.validate().is_err());

    let mut duplicate_semantic_id = valid.clone();
    wheel_slots(&mut duplicate_semantic_id)[1].semantic_id = "red".into();
    assert!(duplicate_semantic_id.validate().is_err());

    let mut empty_label = valid.clone();
    wheel_slots(&mut empty_label)[0].label = "  ".into();
    assert!(empty_label.validate().is_err());

    let mut reversed_range = valid.clone();
    wheel_slots(&mut reversed_range)[0].dmx_from = 41;
    assert!(reversed_range.validate().is_err());

    let mut unsorted = valid.clone();
    wheel_slots(&mut unsorted).swap(0, 1);
    assert!(matches!(
        unsorted.validate(),
        Err(ProfileError::Invalid(message)) if message.contains("sorted")
    ));

    let mut overlapping = valid.clone();
    wheel_slots(&mut overlapping)[1].dmx_from = 40;
    assert!(matches!(
        overlapping.validate(),
        Err(ProfileError::Invalid(message)) if message.contains("non-overlapping")
    ));

    let mut out_of_range = valid.clone();
    wheel_slots(&mut out_of_range)[1].dmx_to = 256;
    assert!(out_of_range.validate().is_err());

    for invalid in [f32::NAN, f32::INFINITY, -0.1] {
        let mut invalid_measurement = valid.clone();
        wheel_slots(&mut invalid_measurement)[0]
            .measured_xyz
            .as_mut()
            .unwrap()
            .y = invalid;
        assert!(invalid_measurement.validate().is_err());
    }
}

#[test]
fn visual_only_profiles_require_zero_footprint_and_no_dmx_behavior() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Venue".into();
    profile.name = "Stage Element".into();
    profile.patch_policy = PatchPolicy::VisualOnly;
    profile.modes[0].splits[0].footprint = 0;
    profile.validate().unwrap();
    assert_eq!(
        profile
            .resolved_definition(profile.modes[0].id)
            .unwrap()
            .footprint,
        0
    );

    profile.modes[0].splits[0].footprint = 1;
    assert!(profile.validate().is_err());
    profile.modes[0].splits[0].footprint = 0;
    let head = profile.modes[0].heads[0].id;
    profile.modes[0].channels.push(FixtureChannel {
        id: Uuid::new_v4(),
        head_id: head,
        split: 1,
        attribute: AttributeKey::intensity(),
        resolution: ChannelResolution::U8,
        secondary_slots: vec![],
        default_raw: 0,
        highlight_raw: 255,
        physical_min: None,
        physical_max: None,
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        reacts_to_sequence_master: true,
        reacts_to_group_master: true,
        reacts_to_grand_master: true,
        behavior: ChannelBehavior::Controlled,
        functions: vec![],
    });
    assert!(profile.validate().is_err());
}

#[test]
fn missing_patch_and_model_policy_fields_decode_to_legacy_defaults() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Generic".into();
    profile.name = "Dimmer".into();
    let mut value = serde_json::to_value(profile).unwrap();
    value.as_object_mut().unwrap().remove("patch_policy");
    value.as_object_mut().unwrap().remove("model_units");
    let decoded: FixtureProfile = serde_json::from_value(value).unwrap();
    assert_eq!(decoded.patch_policy, PatchPolicy::Dmx);
    assert_eq!(decoded.model_units, ModelUnits::Auto);
}

#[test]
fn control_action_semantics_are_portable_and_legacy_actions_default_to_custom() {
    let channel_id = Uuid::new_v4();
    let mut value = serde_json::json!({
        "id": Uuid::new_v4(),
        "name": "Lamp On",
        "semantic": "lamp_on",
        "kind": "timed_pulse",
        "duration_millis": 1000,
        "assignments": [{
            "channel_id": channel_id,
            "active_raw": 255,
            "inactive_raw": 0
        }]
    });
    let action: ControlAction = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(action.semantic, ControlActionSemantic::LampOn);
    assert_eq!(serde_json::to_value(action).unwrap()["semantic"], "lamp_on");

    value.as_object_mut().unwrap().remove("semantic");
    let legacy: ControlAction = serde_json::from_value(value).unwrap();
    assert_eq!(legacy.semantic, ControlActionSemantic::Custom);
}
