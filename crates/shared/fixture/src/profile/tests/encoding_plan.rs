use super::*;

#[test]
fn batch_encoding_matches_channel_encoding_for_mixed_resolutions_and_splits() {
    let head_id = Uuid::new_v4();
    let first = channel(head_id, ChannelResolution::U16, vec![2]);
    let second = channel(head_id, ChannelResolution::U8, vec![]);
    let mut third = channel(head_id, ChannelResolution::U24, vec![3, 4]);
    third.split = 2;
    let mut fourth = channel(head_id, ChannelResolution::U32, vec![5, 6, 7]);
    fourth.split = 2;
    let mode = FixtureMode {
        id: Uuid::new_v4(),
        name: "Mixed".into(),
        notes: String::new(),
        splits: vec![
            FixtureSplit {
                number: 1,
                footprint: 3,
            },
            FixtureSplit {
                number: 2,
                footprint: 7,
            },
        ],
        heads: vec![FixtureHead {
            id: head_id,
            name: "Main".into(),
            master_shared: true,
        }],
        channels: vec![first.clone(), second.clone(), third.clone(), fourth.clone()],
        color_systems: vec![],
        control_actions: vec![],
        geometry: GeometryGraph::default(),
    };
    let values = vec![
        (first.id, 0xabcd),
        (second.id, 0x12),
        (third.id, 0x34_5678),
        (fourth.id, 0x9abc_def0),
    ];
    let plan = mode.compile_encoding_plan().unwrap();

    for split in [1, 2] {
        let mut expected = [0x55; 512];
        for (channel_id, raw) in &values {
            let fixture_channel = mode
                .channels
                .iter()
                .find(|channel| channel.id == *channel_id)
                .unwrap();
            if fixture_channel.split == split {
                mode.encode_channel(&mut expected, 37, fixture_channel, *raw)
                    .unwrap();
            }
        }
        let mut actual = [0x55; 512];
        plan.encode_split(&mut actual, 37, split, &values).unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn batch_encoding_validates_the_whole_write_before_mutating_the_frame() {
    let head_id = Uuid::new_v4();
    let channel = channel(head_id, ChannelResolution::U16, vec![2]);
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
        channels: vec![channel.clone()],
        color_systems: vec![],
        control_actions: vec![],
        geometry: GeometryGraph::default(),
    };
    let plan = mode.compile_encoding_plan().unwrap();
    let mut frame = [0x55; 512];

    let error = plan
        .encode_split(&mut frame, 512, 1, &[(channel.id, 0xabcd)])
        .unwrap_err();

    assert!(error.to_string().contains("exceeds its universe"));
    assert_eq!(frame, [0x55; 512]);

    let error = plan
        .encode_split(
            &mut frame,
            1,
            1,
            &[(channel.id, 0xabcd), (Uuid::new_v4(), 0x12)],
        )
        .unwrap_err();

    assert!(error.to_string().contains("channel is missing"));
    assert_eq!(frame, [0x55; 512]);
}
