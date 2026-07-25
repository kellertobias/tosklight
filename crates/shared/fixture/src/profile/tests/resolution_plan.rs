use super::*;

#[test]
fn compiled_resolution_matches_dynamic_resolution_and_active_ownership() {
    let head_id = Uuid::new_v4();
    let mut fixture_channel = channel(head_id, ChannelResolution::U8, vec![]);
    fixture_channel.default_raw = 7;
    fixture_channel.highlight_raw = 240;
    fixture_channel.reacts_to_virtual_intensity = true;
    fixture_channel.functions = vec![
        ChannelFunction {
            id: Uuid::new_v4(),
            name: "Dimmer".into(),
            dmx_from: 0,
            dmx_to: 127,
            attribute: AttributeKey::intensity(),
            priority: 0,
            behavior: ChannelFunctionBehavior::Continuous {
                physical_min: 0.0,
                physical_max: 1.0,
                unit: None,
            },
        },
        ChannelFunction {
            id: Uuid::new_v4(),
            name: "Open".into(),
            dmx_from: 128,
            dmx_to: 255,
            attribute: AttributeKey("shutter".into()),
            priority: 10,
            behavior: ChannelFunctionBehavior::Fixed {
                semantic_id: "open".into(),
                label: "Open".into(),
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
        channels: vec![fixture_channel],
        color_systems: vec![],
        control_actions: vec![],
        geometry: GeometryGraph::default(),
    };
    let control = FixtureMode::control_action_attribute(mode.channels[0].id);
    let cases = [
        HashMap::from([
            (AttributeKey::intensity(), AttributeValue::Normalized(0.5)),
            (
                AttributeKey("shutter".into()),
                AttributeValue::Discrete("open".into()),
            ),
        ]),
        HashMap::from([(AttributeKey::intensity(), AttributeValue::Normalized(0.5))]),
        HashMap::from([(control.clone(), AttributeValue::RawDmxExact(173))]),
        HashMap::from([
            (control, AttributeValue::Discrete("invalid".into())),
            (
                AttributeKey("shutter".into()),
                AttributeValue::Discrete("open".into()),
            ),
        ]),
        HashMap::new(),
    ];
    let scales = ChannelScales {
        virtual_intensity: 0.8,
        sequence_master: 0.7,
        group_master: 0.6,
        grand_master: 0.5,
    };
    let plan = mode.compile_resolution_plan();
    let bound = plan.bind(&mode).unwrap();

    for values in cases {
        for highlighted in [false, true] {
            let expected_active = mode
                .active_attribute_for_channel(&mode.channels[0], &values)
                .cloned();
            let expected_raw = mode.resolve_channel_raw(
                &mode.channels[0],
                &values,
                highlighted,
                Some(220),
                scales,
            );
            let actual = bound.resolve_channel(0, &values, highlighted, Some(220), |active| {
                assert_eq!(active, expected_active.as_ref());
                scales
            });

            assert_eq!(actual.active_attribute, expected_active.as_ref());
            assert_eq!(actual.raw, expected_raw);
        }
    }
}

#[test]
fn compiled_resolution_rejects_a_different_mode() {
    let mode = additive_color_mode();
    let plan = mode.compile_resolution_plan();
    let mut other = mode.clone();
    other.id = Uuid::new_v4();

    assert!(plan.bind(&other).is_err());
}
