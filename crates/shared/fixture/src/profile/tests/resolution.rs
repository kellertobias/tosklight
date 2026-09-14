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
            emitter_heads: Vec::new(),
            motion_attributes: Vec::new(),
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
            angular_motion: None,
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
            angular_motion: None,
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
        emitter_heads: Vec::new(),
        motion_attributes: Vec::new(),
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
                virtual_intensity: Some(0.0),
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
    fixture_channel.highlight_raw = 211;
    fixture_channel.invert = true;
    fixture_channel.reacts_to_virtual_intensity = true;
    fixture_channel.reacts_to_sequence_master = true;
    fixture_channel.reacts_to_group_master = true;
    fixture_channel.reacts_to_grand_master = true;
    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &values,
            false,
            None,
            ChannelScales {
                virtual_intensity: Some(0.0),
                sequence_master: 0.0,
                group_master: 0.0,
                grand_master: 0.0,
            },
        ),
        37,
        "a static physical slot ignores semantic values, inversion, and every master"
    );
    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &values,
            true,
            None,
            ChannelScales::default(),
        ),
        211,
        "a static physical slot may have an authored Highlight value"
    );
    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &values,
            true,
            Some(220),
            ChannelScales::default(),
        ),
        220,
        "a static physical slot honors its per-instance Highlight override"
    );
}

/// A channel reacting to virtual intensity inversely closes as the head opens: it is scaled by
/// `1 - virtual intensity`, inside the function's own range and after inversion as usual.
/// Highlight bypasses it exactly as it bypasses the ordinary reaction.
#[test]
fn inverted_virtual_intensity_reaction_scales_by_the_complement() {
    let mut profile = FixtureProfile::blank();
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 1;
    let mut fixture_channel = channel(mode.heads[0].id, ChannelResolution::U8, vec![]);
    fixture_channel.attribute = AttributeKey("color.red".into());
    fixture_channel.fixture_attribute = AttributeKey("color.red".into());
    fixture_channel.functions = vec![ChannelFunction::continuous(
        "Red",
        AttributeKey("color.red".into()),
        u8::MAX.into(),
    )];
    fixture_channel.reacts_to_virtual_intensity = true;
    fixture_channel.virtual_intensity_inverted = true;
    fixture_channel.reacts_to_sequence_master = false;
    fixture_channel.reacts_to_group_master = false;
    fixture_channel.reacts_to_grand_master = false;
    mode.channels = vec![fixture_channel.clone()];
    let mode = mode.clone();
    let values = HashMap::from([(
        AttributeKey("color.red".into()),
        AttributeValue::Normalized(1.0),
    )]);
    let scales = |virtual_intensity| ChannelScales {
        virtual_intensity,
        ..Default::default()
    };
    let resolve = |channel: &FixtureChannel, highlighted, scales| {
        mode.resolve_channel_raw(channel, &values, highlighted, None, scales)
    };

    assert_eq!(resolve(&fixture_channel, false, scales(Some(0.25))), 191);
    assert_eq!(resolve(&fixture_channel, false, scales(Some(1.0))), 0);
    assert_eq!(resolve(&fixture_channel, false, scales(Some(0.0))), 255);
    assert_eq!(
        resolve(&fixture_channel, false, scales(None)),
        255,
        "no virtual intensity applies, so nothing scales an inverse reaction either"
    );

    let mut ordinary = fixture_channel.clone();
    ordinary.virtual_intensity_inverted = false;
    assert_eq!(resolve(&ordinary, false, scales(Some(0.25))), 64);
    assert_eq!(resolve(&ordinary, false, scales(None)), 255);

    let mut not_reacting = fixture_channel.clone();
    not_reacting.reacts_to_virtual_intensity = false;
    assert_eq!(
        resolve(&not_reacting, false, scales(Some(0.25))),
        255,
        "the inverse flag means nothing unless the channel reacts"
    );

    let mut inverted_range = fixture_channel.clone();
    inverted_range.invert = true;
    assert_eq!(resolve(&inverted_range, false, scales(Some(0.25))), 64);

    assert_eq!(
        resolve(&fixture_channel, true, scales(Some(1.0))),
        255,
        "Highlight bypasses an inverse reaction to virtual intensity"
    );
    assert_eq!(
        mode.resolve_channel_raw(
            &fixture_channel,
            &values,
            true,
            Some(220),
            scales(Some(1.0))
        ),
        220
    );
}

/// Every channel written before the inverse reaction existed reacts the ordinary way, and the flag
/// is always written alongside its sibling flags.
#[test]
fn virtual_intensity_inversion_defaults_off_and_is_always_written() {
    let fixture_channel = channel(Uuid::new_v4(), ChannelResolution::U8, vec![]);
    let mut encoded = serde_json::to_value(&fixture_channel).unwrap();
    assert_eq!(encoded["virtual_intensity_inverted"], false);
    encoded
        .as_object_mut()
        .unwrap()
        .remove("virtual_intensity_inverted");
    let legacy: FixtureChannel = serde_json::from_value(encoded).unwrap();
    assert!(!legacy.virtual_intensity_inverted);
}
