use super::*;

fn inverted_channel_mode() -> (FixtureMode, FixtureChannel) {
    let head_id = Uuid::new_v4();
    let mut fixture_channel = channel(head_id, ChannelResolution::U8, vec![]);
    fixture_channel.invert = true;
    fixture_channel.default_raw = 37;
    fixture_channel.highlight_raw = 211;
    fixture_channel.functions = vec![
        ChannelFunction {
            id: Uuid::new_v4(),
            name: "Dimmer range".into(),
            dmx_from: 10,
            dmx_to: 109,
            attribute: AttributeKey::intensity(),
            priority: 0,
            behavior: ChannelFunctionBehavior::Continuous {
                physical_min: 0.0,
                physical_max: 1.0,
                unit: Some("percent".into()),
            },
        },
        ChannelFunction {
            id: Uuid::new_v4(),
            name: "Open".into(),
            dmx_from: 110,
            dmx_to: 179,
            attribute: AttributeKey("shutter".into()),
            priority: 100,
            behavior: ChannelFunctionBehavior::Fixed {
                semantic_id: "open".into(),
                label: "Open".into(),
                raw_value: 150,
            },
        },
        ChannelFunction {
            id: Uuid::new_v4(),
            name: "Pattern".into(),
            dmx_from: 180,
            dmx_to: 255,
            attribute: AttributeKey("gobo".into()),
            priority: 100,
            behavior: ChannelFunctionBehavior::Indexed {
                semantic_id: "dots".into(),
                label: "Dots".into(),
                raw_value: 200,
            },
        },
    ];
    let mode = FixtureMode {
        id: Uuid::new_v4(),
        name: "Mode".into(),
        notes: String::new(),
        splits: vec![FixtureSplit {
            number: 1,
            footprint: 1,
        }],
        heads: vec![FixtureHead {
            id: head_id,
            name: "Main".into(),
            master_shared: true,
        }],
        channels: vec![fixture_channel.clone()],
        color_systems: vec![],
        control_actions: vec![],
        geometry: GeometryGraph::default(),
    };
    mode.validate().unwrap();
    (mode, fixture_channel)
}

#[test]
fn invert_scales_semantic_ranges_before_inversion_and_preserves_exact_raw_values() {
    let (mode, fixture_channel) = inverted_channel_mode();
    let semantic = HashMap::from([(AttributeKey::intensity(), AttributeValue::Normalized(0.5))]);
    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &semantic,
            false,
            None,
            ChannelScales {
                grand_master: 0.5,
                ..Default::default()
            },
        ),
        84,
        "the semantic value is scaled from 10 toward 109 before inversion inside that range"
    );
    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &semantic,
            false,
            None,
            ChannelScales {
                grand_master: 0.0,
                ..Default::default()
            },
        ),
        109
    );

    for (values, expected) in [
        (
            HashMap::from([(AttributeKey::intensity(), AttributeValue::RawDmxExact(17))]),
            17,
        ),
        (
            HashMap::from([(
                AttributeKey("shutter".into()),
                AttributeValue::Discrete("open".into()),
            )]),
            150,
        ),
        (
            HashMap::from([(
                AttributeKey("gobo".into()),
                AttributeValue::Discrete("dots".into()),
            )]),
            200,
        ),
        (
            HashMap::from([(
                FixtureMode::control_action_attribute(fixture_channel.id),
                AttributeValue::RawDmxExact(23),
            )]),
            23,
        ),
    ] {
        assert_eq!(
            mode.resolve_channel_raw(
                &fixture_channel,
                &values,
                false,
                None,
                ChannelScales::default(),
            ),
            expected
        );
    }
    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &HashMap::new(),
            true,
            Some(211),
            ChannelScales::default(),
        ),
        211
    );
    let mut static_channel = fixture_channel.clone();
    static_channel.behavior = ChannelBehavior::Static;
    assert_eq!(
        mode.resolve_channel_raw(
            &static_channel,
            &HashMap::new(),
            false,
            None,
            ChannelScales::default(),
        ),
        37
    );
    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &HashMap::from([(AttributeKey::intensity(), AttributeValue::RawDmxExact(17),)]),
            false,
            None,
            ChannelScales {
                grand_master: 0.5,
                ..Default::default()
            },
        ),
        136,
        "an exact raw value moves toward inverted physical off instead of being reinterpreted"
    );
}

#[test]
fn typed_control_action_owns_its_exact_channel_without_losing_function_precision() {
    let head_id = Uuid::new_v4();
    let mut fixture_channel = channel(head_id, ChannelResolution::U16, vec![2]);
    fixture_channel.functions = vec![ChannelFunction {
        id: Uuid::new_v4(),
        name: "High priority fixed value".into(),
        dmx_from: 0,
        dmx_to: 65_535,
        attribute: AttributeKey("shutter".into()),
        priority: 250,
        behavior: ChannelFunctionBehavior::Fixed {
            semantic_id: "open".into(),
            label: "Open".into(),
            raw_value: 40_000,
        },
    }];
    let mode = FixtureMode {
        id: Uuid::new_v4(),
        name: "Mode".into(),
        notes: String::new(),
        splits: vec![FixtureSplit {
            number: 1,
            footprint: 2,
        }],
        heads: vec![FixtureHead {
            id: head_id,
            name: "Main".into(),
            master_shared: true,
        }],
        channels: vec![fixture_channel.clone()],
        color_systems: vec![],
        control_actions: vec![],
        geometry: GeometryGraph::default(),
    };
    let action_attribute = FixtureMode::control_action_attribute(fixture_channel.id);
    let values = HashMap::from([
        (
            AttributeKey("shutter".into()),
            AttributeValue::Discrete("open".into()),
        ),
        (
            action_attribute.clone(),
            AttributeValue::RawDmxExact(0x1234),
        ),
    ]);

    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &values,
            false,
            None,
            ChannelScales::default(),
        ),
        0x1234
    );
    assert_eq!(
        mode.active_attribute_for_channel(&fixture_channel, &values),
        Some(&action_attribute)
    );
}

#[test]
fn fixture_facing_cmy_can_map_to_inverted_canonical_rgb_without_reinterpreting_exact_raw() {
    let head_id = Uuid::new_v4();
    let mut cyan = channel(head_id, ChannelResolution::U8, vec![]);
    cyan.fixture_attribute = AttributeKey("color.cyan".into());
    cyan.attribute = AttributeKey("color.red".into());
    cyan.canonical_transform = CanonicalTransform::InvertNormalized;
    cyan.functions = vec![ChannelFunction::continuous(
        "Cyan filtration",
        AttributeKey("color.red".into()),
        255,
    )];
    let mode = FixtureMode {
        id: Uuid::new_v4(),
        name: "CMY".into(),
        notes: String::new(),
        splits: vec![FixtureSplit {
            number: 1,
            footprint: 1,
        }],
        heads: vec![FixtureHead {
            id: head_id,
            name: "Main".into(),
            master_shared: true,
        }],
        channels: vec![cyan.clone()],
        color_systems: vec![],
        control_actions: vec![],
        geometry: GeometryGraph::default(),
    };
    mode.validate().unwrap();

    for (value, expected) in [(1.0, 0), (0.25, 191), (0.0, 255)] {
        assert_eq!(
            mode.resolve_channel_raw(
                &cyan,
                &HashMap::from([(
                    AttributeKey("color.red".into()),
                    AttributeValue::Normalized(value),
                )]),
                false,
                None,
                ChannelScales::default(),
            ),
            expected
        );
    }
    assert_eq!(
        mode.resolve_channel_raw(
            &cyan,
            &HashMap::from([(
                AttributeKey("color.red".into()),
                AttributeValue::RawDmxExact(37),
            )]),
            false,
            None,
            ChannelScales::default(),
        ),
        37
    );
    let legacy_value = HashMap::from([(
        AttributeKey("color.cyan".into()),
        AttributeValue::Normalized(0.75),
    )]);
    assert_eq!(
        mode.resolve_channel_raw(&cyan, &legacy_value, false, None, ChannelScales::default(),),
        191,
        "a stored schema-v2 Cyan value keeps its physical filtration meaning"
    );
    let plan = mode.compile_resolution_plan();
    let bound = plan.bind(&mode).unwrap();
    assert_eq!(
        bound
            .resolve_channel(0, &legacy_value, false, None, |_| ChannelScales::default(),)
            .raw,
        191,
        "the optimized runtime plan preserves the same legacy fallback"
    );
    let canonical_wins = HashMap::from([
        (
            AttributeKey("color.cyan".into()),
            AttributeValue::Normalized(0.75),
        ),
        (
            AttributeKey("color.red".into()),
            AttributeValue::Normalized(0.25),
        ),
    ]);
    assert_eq!(
        mode.resolve_channel_raw(
            &cyan,
            &canonical_wins,
            false,
            None,
            ChannelScales::default(),
        ),
        191,
        "canonical RGB wins when old and new values coexist"
    );
}
