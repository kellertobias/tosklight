use super::*;

#[test]
fn exact_raw_values_encode_msb_first_at_every_supported_resolution() {
    let cases = [
        (ChannelResolution::U8, 0x0000_00ab, vec![], vec![0xab]),
        (
            ChannelResolution::U16,
            0x0000_abcd,
            vec![2],
            vec![0xab, 0xcd],
        ),
        (
            ChannelResolution::U24,
            0x00ab_cdef,
            vec![2, 3],
            vec![0xab, 0xcd, 0xef],
        ),
        (
            ChannelResolution::U32,
            0xabcd_ef12,
            vec![2, 3, 4],
            vec![0xab, 0xcd, 0xef, 0x12],
        ),
    ];
    for (resolution, expected_raw, secondary_slots, expected_bytes) in cases {
        let head_id = Uuid::new_v4();
        let fixture_channel = channel(head_id, resolution, secondary_slots);
        let mode = FixtureMode {
            id: Uuid::new_v4(),
            name: "Mode".into(),
            notes: String::new(),
            splits: vec![FixtureSplit {
                number: 1,
                footprint: resolution.bytes() as u16,
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
        let values = HashMap::from([(
            AttributeKey::intensity(),
            AttributeValue::RawDmxExact(expected_raw),
        )]);
        let raw = mode.resolve_channel_raw(
            &fixture_channel,
            &values,
            false,
            None,
            ChannelScales::default(),
        );
        assert_eq!(raw, expected_raw);
        let mut frame = [0_u8; 512];
        mode.encode_channel(&mut frame, 5, &fixture_channel, raw)
            .unwrap();
        assert_eq!(
            &frame[4..4 + expected_bytes.len()],
            expected_bytes.as_slice()
        );
    }
}

#[test]
fn multi_function_priority_release_static_and_highlight_are_deterministic() {
    let head_id = Uuid::new_v4();
    let mut fixture_channel = channel(head_id, ChannelResolution::U8, vec![]);
    fixture_channel.highlight_raw = 240;
    fixture_channel.reacts_to_group_master = true;
    fixture_channel.reacts_to_grand_master = true;
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
        channels: vec![fixture_channel.clone()],
        color_systems: vec![],
        control_actions: vec![],
        geometry: GeometryGraph::default(),
    };
    mode.validate().unwrap();
    let mut values = HashMap::from([
        (AttributeKey::intensity(), AttributeValue::Normalized(0.5)),
        (
            AttributeKey("shutter".into()),
            AttributeValue::Discrete("open".into()),
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
        200
    );
    values.remove(&AttributeKey("shutter".into()));
    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &values,
            false,
            None,
            ChannelScales::default(),
        ),
        64
    );
    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &values,
            true,
            Some(220),
            ChannelScales {
                virtual_intensity: 0.0,
                sequence_master: 0.0,
                group_master: 0.5,
                grand_master: 0.5,
            },
        ),
        110,
        "Highlight bypasses virtual intensity, sequence masters, and Group Masters; Grand Master remains above it"
    );

    fixture_channel.behavior = ChannelBehavior::Static;
    fixture_channel.default_raw = 37;
    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &values,
            false,
            None,
            ChannelScales::default(),
        ),
        37
    );
}
