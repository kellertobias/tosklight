use super::*;
use light_programmer::{HighlightOutputLayer, HighlightOutputRole};
use std::collections::HashSet;

#[test]
fn semantic_highlight_applies_only_authored_identification_attributes() {
    let (mut fixture, fixture_id) = schema_v2_fixture(&[
        ("intensity", false, false, false, false, false),
        ("shutter", true, false, false, false, false),
        ("gobo", true, false, false, false, false),
    ]);
    let mode = &mut fixture.definition.profile_snapshot.as_mut().unwrap().modes[0];
    mode.channels[0].default_raw = 9;
    mode.channels[0].highlight_raw = 255;
    mode.channels[1].default_raw = 7;
    mode.channels[1].highlight_raw = 244;
    mode.channels[1].functions = vec![ChannelFunction {
        id: uuid::Uuid::new_v4(),
        name: "Open".into(),
        dmx_from: 80,
        dmx_to: 110,
        attribute: AttributeKey("shutter".into()),
        priority: 0,
        angular_motion: None,
        behavior: light_fixture::ChannelFunctionBehavior::Fixed {
            semantic_id: "open".into(),
            label: "Open".into(),
            raw_value: 96,
        },
    }];
    mode.channels[2].default_raw = 42;
    mode.channels[2].highlight_raw = 233;

    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine
        .set_highlight_look(light_fixture::HighlightLook {
            intensity: 0.4,
            ..Default::default()
        })
        .unwrap();
    engine.set_highlighted_fixtures([fixture_id]);

    assert_eq!(
        &engine.render(RenderOptions::default()).unwrap().universes[&1][0..3],
        &[102, 96, 42],
        "semantic Highlight must use the configured intensity and authored Open function without changing Gobo"
    );
}

#[test]
fn semantic_highlight_does_not_guess_an_unauthored_shutter_open_value() {
    let (mut fixture, fixture_id) =
        schema_v2_fixture(&[("shutter", true, false, false, false, false)]);
    let channel = &mut fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].channels[0];
    channel.default_raw = 31;
    channel.highlight_raw = 255;
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine
        .set_highlight_look(light_fixture::HighlightLook::default())
        .unwrap();
    engine.set_highlighted_fixtures([fixture_id]);

    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        31,
        "a profile without semantic Open must retain its ordinary resolved value"
    );
}

#[test]
fn unsupported_semantic_highlight_color_leaves_the_fixture_value_unchanged() {
    let (mut fixture, fixture_id) =
        schema_v2_fixture(&[("color.wheel.1", true, false, false, false, false)]);
    let channel = &mut fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].channels[0];
    channel.default_raw = 57;
    channel.highlight_raw = 255;
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine
        .set_highlight_look(light_fixture::HighlightLook {
            color: Some(light_fixture::HighlightColor::Blue),
            ..Default::default()
        })
        .unwrap();
    assert!(
        engine
            .highlight_look_warnings(&engine.highlight_look())
            .iter()
            .any(|warning| warning.contains("Color is unavailable"))
    );
    engine.set_highlighted_fixtures([fixture_id]);

    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        57,
        "an unrepresentable optional color must retain the ordinary resolved value"
    );
}

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
            fixture_attribute: AttributeKey(attribute.into()),
            attribute: AttributeKey(attribute.into()),
            canonical_transform: light_fixture::CanonicalTransform::Identity,
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
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::from([(red_id, 0), (green_id, 0), (blue_id, 255)]),
    };
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
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
            fixtures: vec![fixture].into(),
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
            fixture_attribute: AttributeKey::intensity(),
            attribute: AttributeKey::intensity(),
            canonical_transform: light_fixture::CanonicalTransform::Identity,
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
                profile_head_id: None,
                head_index: 0,
                fixture_id: first,
            },
            PatchedHead {
                profile_head_id: None,
                head_index: 1,
                fixture_id: second,
            },
        ],
        multipatch: vec![],
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
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
            fixtures: vec![fixture].into(),
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

#[test]
fn high_low_and_explicit_attribute_suppression_are_temporary_and_exact() {
    let (mut fixture, fixture_id) = schema_v2_fixture(&[
        ("intensity", false, false, false, false, false),
        ("shutter", true, false, false, false, false),
    ]);
    let mode = &mut fixture.definition.profile_snapshot.as_mut().unwrap().modes[0];
    mode.channels[0].default_raw = 64;
    mode.channels[1].default_raw = 7;
    mode.channels[1].functions = vec![ChannelFunction {
        id: uuid::Uuid::new_v4(),
        name: "Open".into(),
        dmx_from: 80,
        dmx_to: 110,
        attribute: AttributeKey("shutter".into()),
        priority: 0,
        angular_motion: None,
        behavior: light_fixture::ChannelFunctionBehavior::Fixed {
            semantic_id: "open".into(),
            label: "Open".into(),
            raw_value: 96,
        },
    }];
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine
        .set_highlight_look(light_fixture::HighlightLook::default())
        .unwrap();
    let ordinary = engine.render(RenderOptions::default()).unwrap().universes[&1];
    assert_eq!(&ordinary[0..2], &[64, 7]);

    engine.set_highlight_layers([HighlightOutputLayer {
        fixture_id,
        role: HighlightOutputRole::LowLight,
        suppressed_attributes: HashSet::new(),
    }]);
    assert_eq!(
        &engine.render(RenderOptions::default()).unwrap().universes[&1][0..2],
        &[26, 96],
        "Low Light is 10% with the same authored safe Open identification"
    );

    engine.set_highlight_layers([HighlightOutputLayer {
        fixture_id,
        role: HighlightOutputRole::Highlight,
        suppressed_attributes: HashSet::from([AttributeKey::intensity()]),
    }]);
    assert_eq!(
        &engine.render(RenderOptions::default()).unwrap().universes[&1][0..2],
        &[64, 96],
        "the explicit intensity falls through while untouched shutter remains temporary"
    );

    engine.clear_highlighted_fixtures();
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1],
        ordinary,
        "removing the temporary layer restores the exact ordinary render"
    );
}
