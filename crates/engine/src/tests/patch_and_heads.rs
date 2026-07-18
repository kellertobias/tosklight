use super::*;

#[test]
fn patched_multipatch_instances_duplicate_output_while_visual_only_instances_do_not() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    let (mut fixture, logical) = fixture();
    fixture.multipatch = vec![
        MultiPatchInstance {
            id: FixtureId::new().0,
            name: "Patched clone".into(),
            universe: Some(1),
            address: Some(8),
            split_patches: Vec::new(),
            location: Default::default(),
            rotation: Default::default(),
        },
        MultiPatchInstance {
            id: FixtureId::new().0,
            name: "Visualizer clone".into(),
            universe: None,
            address: None,
            split_patches: Vec::new(),
            location: Default::default(),
            rotation: Default::default(),
        },
    ];
    programmers.set(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            cue_lists: vec![],
            playbacks: vec![],
            playback_pages: vec![],
            routes: vec![],
            control_mappings: vec![],
            groups: vec![],
            revision: 1,
        })
        .unwrap();
    let result = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(result.universes[&1][0], 128);
    assert_eq!(result.universes[&1][7], 128);
    assert_eq!(
        result.universes[&1]
            .iter()
            .filter(|value| **value != 0)
            .count(),
        2
    );
}

#[test]
fn logical_head_programmer_value_renders_to_physical_patch() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    let (fixture, logical) = fixture();
    programmers.set(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            cue_lists: vec![],
            playbacks: vec![],
            playback_pages: vec![],
            routes: vec![],
            control_mappings: vec![],
            groups: vec![],
            revision: 7,
        })
        .unwrap();
    let result = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(result.universes[&1][0], 128);
    assert_eq!(result.revision, 7);
    assert_eq!(
        engine
            .resolved_values()
            .get(&(logical, AttributeKey::intensity())),
        Some(&AttributeValue::Normalized(0.5))
    );
}

#[test]
fn parent_programmer_value_does_not_fan_out_to_child_heads() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    let (fixture, _) = fixture();
    programmers.set(
        session,
        fixture.fixture_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        0
    );
}

#[test]
fn master_only_group_fader_does_not_scale_child_heads() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    let (mut fixture, child) = fixture();
    fixture.definition.footprint = 2;
    let mut master_parameter = fixture.definition.heads[0].parameters[0].clone();
    master_parameter.components[0].offset = 1;
    fixture.definition.heads.insert(
        0,
        LogicalHead {
            index: 0,
            name: "Master".into(),
            shared: true,
            parameters: vec![master_parameter],
        },
    );
    let master = fixture.fixture_id;
    for fixture_id in [master, child] {
        programmers.set(
            session,
            fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.8),
        );
    }
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            groups: vec![GroupDefinition {
                id: "master".into(),
                name: "Master only".into(),
                fixtures: vec![master],
                master: 0.5,
                playback_fader: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(rendered.universes[&1][0], 204);
    assert_eq!(rendered.universes[&1][1], 102);
}
