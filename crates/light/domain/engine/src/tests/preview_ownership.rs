use super::*;
use light_fixture::FixtureMode;
use std::collections::HashSet;

fn engine(profile: FixtureProfile) -> (Engine, FixtureId) {
    let id = FixtureId::new();
    let mut fixture = calibrated_visual_fixture(id);
    fixture.definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..Default::default()
        })
        .unwrap();
    (engine, id)
}
fn mask(engine: &Engine, id: FixtureId, name: &str, values: &ResolvedValues) -> Vec<bool> {
    let keys = HashSet::from([(id, AttributeKey(name.into()))]);
    engine
        .profile_preload_projection_at(
            values,
            Default::default(),
            Some(&engine.snapshot()),
            &keys,
            &keys,
        )
        .unwrap()
        .native_ownership
        .get(&id)
        .map(|v| v.to_vec())
        .unwrap_or_default()
}
fn profile() -> FixtureProfile {
    calibrated_visual_definition()
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone()
}

#[test]
fn preview_ownership_maps_aliases_functions_controls_and_virtual_intensity_without_static_channels()
{
    let mut profile = profile();
    let mode = &mut profile.modes[0];
    mode.channels[1].fixture_attribute = AttributeKey("manufacturer.red".into());
    mode.channels[1].functions[0].attribute = AttributeKey("red.alternate".into());
    mode.channels[1].reacts_to_virtual_intensity = true;
    mode.channels[3].behavior = ChannelBehavior::Static;
    let control = FixtureMode::control_action_attribute(mode.channels[1].id);
    let (engine, id) = engine(profile);
    let values = ResolvedValues::default();
    for name in ["color.red", "manufacturer.red", "red.alternate", &control.0] {
        assert_eq!(
            mask(&engine, id, name, &values),
            [false, true, false, false],
            "{name}"
        );
    }
    assert_eq!(
        mask(&engine, id, "intensity", &values),
        [true, true, false, false]
    );
    assert!(mask(&engine, id, "color.blue", &values).is_empty());
    assert!(mask(&engine, FixtureId::new(), "color.red", &values).is_empty());
}

#[test]
fn preview_ownership_uses_actual_direct_color_consumers_including_aliases_and_priority() {
    let mut profile = profile();
    let mode = &mut profile.modes[0];
    let mut duplicate = mode.channels[1].clone();
    duplicate.id = uuid::Uuid::new_v4();
    duplicate.functions[0].id = uuid::Uuid::new_v4();
    mode.channels.push(duplicate);
    mode.splits[0].footprint = 5;
    let red_control = FixtureMode::control_action_attribute(mode.channels[1].id);
    let (engine, id) = engine(profile);
    let mut values = [(
        (id, AttributeKey::color()),
        AttributeValue::ColorXyz(Xyz {
            x: 0.3,
            y: 0.4,
            z: 0.2,
        }),
    )]
    .into_iter()
    .collect::<ResolvedValues>();
    assert_eq!(
        mask(&engine, id, "color", &values),
        [false, true, true, true, true]
    );
    values.insert((id, red_control), AttributeValue::RawDmxExact(17));
    assert_eq!(
        mask(&engine, id, "color", &values),
        [false, false, true, true, true]
    );
    values.insert(
        (id, AttributeKey("color.red".into())),
        AttributeValue::Normalized(0.4),
    );
    assert_eq!(
        mask(&engine, id, "color", &values),
        [false, false, true, true, false]
    );
}

#[test]
fn preview_ownership_direct_leaves_uv_and_wheel_live_while_intent_owns_its_parked_channels() {
    let mut profile = profile();
    let mode = &mut profile.modes[0];
    let head = mode.heads[0].id;
    let uv = uuid::Uuid::new_v4();
    let wheel = uuid::Uuid::new_v4();
    mode.channels.push(calibrated_channel(
        uv, head, "color.uv", false, false, false,
    ));
    mode.channels.push(calibrated_channel(
        wheel,
        head,
        "color.wheel",
        false,
        false,
        false,
    ));
    mode.splits[0].footprint = 6;
    if let ColorSystem::Additive { emitters } = &mut mode.color_systems[0].system {
        let mut e = emitter(
            uv,
            "UV",
            Xyz {
                x: 0.,
                y: 0.,
                z: 0.,
            },
            1.,
        );
        e.visible = false;
        emitters.push(e);
    }
    mode.color_systems.push(light_fixture::HeadColorSystem {
        calibration: Default::default(),
        head_id: head,
        correction_matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        system: ColorSystem::DiscreteWheel {
            channel_id: wheel,
            slots: vec![light_fixture::ColorWheelSlot {
                semantic_id: "open".into(),
                label: "Open".into(),
                dmx_from: 0,
                dmx_to: 10,
                measured_xyz: Some(Xyz {
                    x: 0.95,
                    y: 1.,
                    z: 1.09,
                }),
                steady: Some(true),
            }],
        },
    });
    let (engine, id) = engine(profile);
    let values = [(
        (id, AttributeKey::color()),
        AttributeValue::ColorXyz(Xyz {
            x: 0.3,
            y: 0.4,
            z: 0.2,
        }),
    )]
    .into_iter()
    .collect::<ResolvedValues>();
    assert_eq!(
        mask(&engine, id, "color", &values),
        [false, true, true, true, false, false]
    );
    engine.set_color_model(light_core::ColorProgrammingModel::Intent);
    assert_eq!(
        mask(&engine, id, "color", &values),
        [false, true, true, true, true, true]
    );
}

#[test]
fn preview_ownership_keeps_logical_heads_and_shared_master_separate() {
    let mut fixture = calibrated_visual_fixture(FixtureId::new());
    let child = FixtureId::new();
    let mut profile = profile();
    let mode = &mut profile.modes[0];
    let head = mode.heads[0].id;
    let master = uuid::Uuid::new_v4();
    mode.heads[0].master_shared = false;
    mode.heads.push(light_fixture::FixtureHead {
        id: master,
        name: "Master".into(),
        master_shared: true,
    });
    mode.channels[0].head_id = master;
    let mode_id = mode.id;
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    fixture.logical_heads = vec![light_fixture::PatchedHead {
        profile_head_id: Some(head),
        head_index: 0,
        fixture_id: child,
    }];
    let root = fixture.fixture_id;
    let snapshot = EngineSnapshot {
        fixtures: vec![fixture].into(),
        ..Default::default()
    };
    let index = crate::profile_projection_plan::ProfileProjectionIndex::compile(&snapshot).unwrap();
    let plan = index.fixture(root).unwrap();
    assert_eq!(
        &*plan
            .native_ownership(
                &HashSet::from([(child, AttributeKey("color.red".into()))]),
                &[],
                &Default::default(),
                &[]
            )
            .unwrap(),
        &[false, true, false, false]
    );
    assert!(
        plan.native_ownership(
            &HashSet::from([(root, AttributeKey("color.red".into()))]),
            &[],
            &Default::default(),
            &[]
        )
        .is_none()
    );
    assert_eq!(
        &*plan
            .native_ownership(
                &HashSet::from([(root, AttributeKey::intensity())]),
                &[],
                &Default::default(),
                &[]
            )
            .unwrap(),
        &[true, false, false, false]
    );
}

#[test]
fn preview_ownership_does_not_claim_shadowed_or_frozen_addresses() {
    let mut profile = profile();
    profile.modes[0].channels[1].fixture_attribute = AttributeKey("manufacturer.red".into());
    profile.modes[0].channels[1].functions[0].attribute = AttributeKey("red.secondary".into());
    let (engine, id) = engine(profile);
    let mut values = ResolvedValues::default();
    values.insert(
        (id, AttributeKey("color.red".into())),
        AttributeValue::Normalized(0.3),
    );
    values.insert(
        (id, AttributeKey("manufacturer.red".into())),
        AttributeValue::Normalized(0.9),
    );
    assert!(mask(&engine, id, "manufacturer.red", &values).is_empty());
    assert_eq!(
        mask(&engine, id, "color.red", &values),
        [false, true, false, false]
    );
    let mut snapshot = (*engine.snapshot()).clone();
    let mut fixtures = snapshot.fixtures.to_vec();
    fixtures[0].freeze = FixtureFreezeState {
        targets: HashMap::from([(
            id,
            FrozenFixtureTarget {
                position_native: None,
                full: false,
                families: vec![FreezeFamily::Color],
                values: HashMap::from([
                    (
                        AttributeKey("color.red".into()),
                        AttributeValue::Normalized(0.25),
                    ),
                    (
                        AttributeKey::color(),
                        AttributeValue::ColorXyz(Xyz {
                            x: 0.2,
                            y: 0.3,
                            z: 0.4,
                        }),
                    ),
                ]),
            },
        )]),
    };
    snapshot.fixtures = fixtures.into();
    engine.replace_snapshot(snapshot).unwrap();
    assert!(mask(&engine, id, "color.red", &values).is_empty());
    values.insert(
        (id, AttributeKey::color()),
        AttributeValue::ColorXyz(Xyz {
            x: 0.8,
            y: 0.1,
            z: 0.1,
        }),
    );
    assert!(mask(&engine, id, "color", &values).is_empty());
}

#[test]
fn preview_ownership_includes_inferred_intent_rgb_without_authored_color_system() {
    let mut profile = profile();
    profile.modes[0].color_systems.clear();
    let (engine, id) = engine(profile);
    engine.set_color_model(light_core::ColorProgrammingModel::Intent);
    let mut values = ResolvedValues::default();
    values.insert(
        (id, AttributeKey::color()),
        AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(1., 0., 1.)),
    );
    assert_eq!(
        mask(&engine, id, "color", &values),
        [false, true, true, true]
    );
}
