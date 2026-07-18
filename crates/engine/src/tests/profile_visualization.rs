use super::*;

#[test]
fn profile_visualization_uses_the_exact_calibrated_and_mastered_channel_result() {
    let (engine, fixture_id) = calibrated_visual_engine();
    let options = RenderOptions {
        grand_master: 0.5,
        ..Default::default()
    };
    let rendered = engine.render(options).unwrap();
    let frame = &rendered.universes[&1];
    let projected = engine
        .profile_visualization_values(&engine.resolved_values(), options)
        .unwrap();
    let AttributeValue::Normalized(intensity) = projected[&(fixture_id, AttributeKey::intensity())]
    else {
        panic!("profile intensity projection is missing");
    };
    let AttributeValue::ColorXyz(color) = projected[&(fixture_id, AttributeKey("color".into()))]
    else {
        panic!("profile color projection is missing");
    };

    assert_eq!(frame[0], 64);
    assert!((intensity - frame[0] as f32 / 255.0).abs() < 0.000_001);
    assert_eq!(frame[1], 210);
    assert!((color.x - ((255 - frame[1]) as f32 / 255.0).powi(2)).abs() < 0.000_001);
    assert!((color.y - frame[2] as f32 / 255.0).abs() < 0.000_001);
    assert!((color.z - frame[3] as f32 / 255.0).abs() < 0.000_001);
    assert!(
        color.x < 0.6,
        "the correction matrix must affect visual color"
    );
    assert_eq!(
        &engine
            .render(RenderOptions {
                blackout: true,
                ..Default::default()
            })
            .unwrap()
            .universes[&1][0..4],
        &[0, 255, 0, 0],
        "blackout must drive inverted additive emitters to their physical off endpoint"
    );
}

fn calibrated_visual_engine() -> (Engine, FixtureId) {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    let fixture_id = FixtureId::new();
    let fixture = calibrated_visual_fixture(fixture_id);
    programmers.set(
        session,
        fixture_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    );
    programmers.set(
        session,
        fixture_id,
        AttributeKey("color".into()),
        AttributeValue::ColorXyz(Xyz {
            x: 1.0,
            y: 0.25,
            z: 0.1,
        }),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            groups: vec![GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                fixtures: vec![fixture_id],
                master: 0.5,
                playback_fader: Some(1),
                ..Default::default()
            }],
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    (engine, fixture_id)
}

fn calibrated_visual_fixture(fixture_id: FixtureId) -> PatchedFixture {
    PatchedFixture {
        fixture_id,
        fixture_number: Some(1),
        virtual_fixture_number: None,
        name: "Calibrated visual".into(),
        definition: calibrated_visual_definition(),
        universe: Some(1),
        address: Some(1),
        split_patches: vec![],
        layer_id: "default".into(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        multipatch: vec![],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    }
}

fn calibrated_visual_definition() -> FixtureDefinition {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Calibrated visual".into();
    profile.short_name = "Visual".into();
    profile.revision = 1;
    let mode = &mut profile.modes[0];
    let head_id = mode.heads[0].id;
    mode.splits[0].footprint = 4;
    let intensity_id = uuid::Uuid::new_v4();
    let red_id = uuid::Uuid::new_v4();
    let green_id = uuid::Uuid::new_v4();
    let blue_id = uuid::Uuid::new_v4();
    mode.channels = vec![
        calibrated_channel(intensity_id, head_id, "intensity", true, true, false),
        calibrated_channel(red_id, head_id, "color.red", true, true, true),
        calibrated_channel(green_id, head_id, "color.green", false, false, false),
        calibrated_channel(blue_id, head_id, "color.blue", false, false, false),
    ];
    mode.color_systems = vec![light_fixture::HeadColorSystem {
        head_id,
        correction_matrix: [[0.5, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        system: ColorSystem::Additive {
            emitters: vec![
                emitter(
                    red_id,
                    "Red",
                    Xyz {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    2.0,
                ),
                emitter(
                    green_id,
                    "Green",
                    Xyz {
                        x: 0.0,
                        y: 1.0,
                        z: 0.0,
                    },
                    1.0,
                ),
                emitter(
                    blue_id,
                    "Blue",
                    Xyz {
                        x: 0.0,
                        y: 0.0,
                        z: 1.0,
                    },
                    1.0,
                ),
            ],
        },
    }];
    let mode_id = mode.id;
    profile.resolved_definition(mode_id).unwrap()
}

fn calibrated_channel(
    id: uuid::Uuid,
    head_id: uuid::Uuid,
    attribute: &str,
    group: bool,
    grand: bool,
    invert: bool,
) -> FixtureChannel {
    FixtureChannel {
        id,
        head_id,
        split: 1,
        attribute: AttributeKey(attribute.into()),
        resolution: ChannelResolution::U8,
        secondary_slots: vec![],
        default_raw: 0,
        highlight_raw: 255,
        physical_min: Some(0.0),
        physical_max: Some(1.0),
        unit: None,
        invert,
        snap: false,
        reacts_to_virtual_intensity: false,
        reacts_to_sequence_master: false,
        reacts_to_group_master: group,
        reacts_to_grand_master: grand,
        behavior: ChannelBehavior::Controlled,
        functions: vec![ChannelFunction::continuous(
            attribute,
            AttributeKey(attribute.into()),
            255,
        )],
    }
}

fn emitter(
    channel_id: uuid::Uuid,
    name: &str,
    xyz: Xyz,
    response_curve: f32,
) -> light_fixture::EmitterBinding {
    light_fixture::EmitterBinding {
        channel_id,
        name: name.into(),
        xyz,
        maximum_level: 1.0,
        response_curve,
        visible: true,
    }
}
