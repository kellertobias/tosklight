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
