use super::*;

#[test]
fn fixture_highlight_override_renders_an_individual_blue_identification_look() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "RGB Highlight".into();
    profile.short_name = "RGB Highlight".into();
    profile.revision = 1;
    let mode = &mut profile.modes[0];
    let head_id = mode.heads[0].id;
    mode.splits[0].footprint = 4;
    let channels = ["intensity", "color.red", "color.green", "color.blue"]
        .into_iter()
        .map(|attribute| FixtureChannel {
            id: uuid::Uuid::new_v4(),
            head_id,
            split: 1,
            attribute: AttributeKey(attribute.into()),
            resolution: ChannelResolution::U8,
            secondary_slots: Vec::new(),
            default_raw: 0,
            highlight_raw: 255,
            physical_min: Some(0.0),
            physical_max: Some(1.0),
            unit: None,
            invert: false,
            snap: false,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: false,
            reacts_to_group_master: attribute == "intensity",
            reacts_to_grand_master: attribute == "intensity",
            behavior: ChannelBehavior::Controlled,
            functions: vec![ChannelFunction::continuous(
                attribute,
                AttributeKey(attribute.into()),
                255,
            )],
        })
        .collect::<Vec<_>>();
    let red_id = channels[1].id;
    let green_id = channels[2].id;
    let blue_id = channels[3].id;
    mode.channels = channels;
    mode.color_systems = vec![light_fixture::HeadColorSystem {
        head_id,
        correction_matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        system: ColorSystem::Additive {
            emitters: [
                (
                    red_id,
                    "Red",
                    Xyz {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    green_id,
                    "Green",
                    Xyz {
                        x: 0.0,
                        y: 1.0,
                        z: 0.0,
                    },
                ),
                (
                    blue_id,
                    "Blue",
                    Xyz {
                        x: 0.0,
                        y: 0.0,
                        z: 1.0,
                    },
                ),
            ]
            .into_iter()
            .map(|(channel_id, name, xyz)| light_fixture::EmitterBinding {
                channel_id,
                name: name.into(),
                xyz,
                maximum_level: 1.0,
                response_curve: 1.0,
                visible: true,
            })
            .collect(),
        },
    }];
    let mode_id = mode.id;
    let definition = profile.resolved_definition(mode_id).unwrap();
    let fixture_id = FixtureId::new();
    let fixture = PatchedFixture {
        fixture_id,
        fixture_number: Some(41),
        virtual_fixture_number: None,
        name: "Blue identification".into(),
        definition,
        universe: Some(1),
        address: Some(1),
        split_patches: Vec::new(),
        layer_id: "default".into(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: Vec::new(),
        multipatch: Vec::new(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::from([(red_id, 0), (green_id, 0), (blue_id, 255)]),
    };
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine.set_highlighted_fixtures([fixture_id]);

    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(&rendered.universes[&1][0..4], &[255, 0, 0, 255]);
    let visual = engine
        .profile_visualization_values(&engine.resolved_values(), RenderOptions::default())
        .unwrap();
    let AttributeValue::ColorXyz(blue) = visual
        .get(&(fixture_id, AttributeKey("color".into())))
        .expect("configured blue Highlight color")
    else {
        panic!("configured Highlight must project a color")
    };
    assert!(blue.z > blue.x && blue.z > blue.y);
}

#[test]
fn fixture_without_intensity_uses_its_configured_non_intensity_highlight_look() {
    let (mut fixture, fixture_id) =
        schema_v2_fixture(&[("shutter", false, false, false, false, false)]);
    let mode = &mut fixture.definition.profile_snapshot.as_mut().unwrap().modes[0];
    mode.channels[0].default_raw = 17;
    mode.channels[0].highlight_raw = 211;
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        17,
        "without Highlight, a no-intensity fixture keeps its configured safe default"
    );
    engine.set_highlighted_fixtures([fixture_id]);
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        211,
        "a no-intensity fixture can still identify through a deliberately configured safe Highlight raw value"
    );
}

#[test]
fn selected_logical_head_highlights_independently_while_parent_identifies_all_heads() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Two-head fixture".into();
    profile.short_name = "Two-head".into();
    profile.revision = 1;
    let mode = &mut profile.modes[0];
    mode.heads[0].master_shared = false;
    let first_head = mode.heads[0].id;
    let second_head = uuid::Uuid::new_v4();
    mode.heads.push(FixtureHead {
        id: second_head,
        name: "Second".into(),
        master_shared: false,
    });
    mode.splits[0].footprint = 2;
    mode.channels = [(first_head, 10, 101), (second_head, 20, 202)]
        .into_iter()
        .map(|(head_id, default_raw, highlight_raw)| FixtureChannel {
            id: uuid::Uuid::new_v4(),
            head_id,
            split: 1,
            attribute: AttributeKey::intensity(),
            resolution: ChannelResolution::U8,
            secondary_slots: vec![],
            default_raw,
            highlight_raw,
            physical_min: Some(0.0),
            physical_max: Some(1.0),
            unit: Some("percent".into()),
            invert: false,
            snap: false,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: false,
            reacts_to_group_master: false,
            reacts_to_grand_master: false,
            behavior: ChannelBehavior::Controlled,
            functions: vec![ChannelFunction::continuous(
                "Dimmer",
                AttributeKey::intensity(),
                255,
            )],
        })
        .collect();
    mode.geometry = GeometryGraph::template(GeometryTemplate::Bar, &[first_head, second_head]);
    let mode_id = mode.id;
    let definition = profile.resolved_definition(mode_id).unwrap();
    let parent = FixtureId::new();
    let first = FixtureId::new();
    let second = FixtureId::new();
    let fixture = PatchedFixture {
        fixture_id: parent,
        fixture_number: Some(1),
        virtual_fixture_number: None,
        name: "Two-head fixture".into(),
        definition,
        universe: Some(1),
        address: Some(1),
        split_patches: vec![],
        layer_id: "default".into(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![
            PatchedHead {
                head_index: 0,
                fixture_id: first,
            },
            PatchedHead {
                head_index: 1,
                fixture_id: second,
            },
        ],
        multipatch: vec![],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    engine.set_highlighted_fixtures([second]);
    assert_eq!(
        &engine.render(RenderOptions::default()).unwrap().universes[&1][0..2],
        &[10, 202],
        "selecting one logical head must not highlight its sibling"
    );
    engine.set_highlighted_fixtures([parent]);
    assert_eq!(
        &engine.render(RenderOptions::default()).unwrap().universes[&1][0..2],
        &[101, 202],
        "selecting the physical parent identifies the complete compound fixture"
    );
}

#[test]
fn hazardous_blackout_safe_raw_value_wins_over_non_intensity_highlight() {
    let (mut fixture, fixture_id) =
        schema_v2_fixture(&[("control.reset", false, false, false, false, false)]);
    fixture.definition.hazardous = true;
    fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].channels[0].invert = true;
    fixture.definition.safe_values.insert(
        AttributeKey("control.reset".into()),
        AttributeValue::RawDmxExact(37),
    );
    let channel_id = fixture.definition.profile_snapshot.as_ref().unwrap().modes[0].channels[0].id;
    fixture.highlight_overrides.insert(channel_id, 211);
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine.set_highlighted_fixtures([fixture_id]);

    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        211
    );
    assert_eq!(
        engine
            .render(RenderOptions {
                blackout: true,
                ..Default::default()
            })
            .unwrap()
            .universes[&1][0],
        37
    );
}
