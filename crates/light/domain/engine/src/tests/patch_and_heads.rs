use super::*;

#[test]
fn visual_only_profile_renders_without_a_dmx_encoding_plan() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Venue".into();
    profile.name = "Stage element".into();
    profile.patch_policy = light_fixture::PatchPolicy::VisualOnly;
    profile.modes[0].splits[0].footprint = 0;
    let mode_id = profile.modes[0].id;
    let definition = profile.resolved_definition(mode_id).unwrap();
    let fixture = PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: None,
        virtual_fixture_number: Some(1),
        name: "Stage element".into(),
        definition,
        universe: None,
        address: None,
        split_patches: vec![],
        layer_id: "default".into(),
        note: None,
        position_master: None,
        direct_control: None,
        internal_bindings: Default::default(),
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
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
        freeze: Default::default(),
    };
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..Default::default()
        })
        .unwrap();

    let rendered = engine.render(RenderOptions::default()).unwrap();

    assert!(rendered.universes.is_empty());
}

#[test]
fn patched_multipatch_instances_duplicate_output_while_visual_only_instances_do_not() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session);
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
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.0,
            shaper_angle: None,
            installed_appearance: Default::default(),
        },
        MultiPatchInstance {
            id: FixtureId::new().0,
            name: "Visualizer clone".into(),
            universe: None,
            address: None,
            split_patches: Vec::new(),
            location: Default::default(),
            rotation: Default::default(),
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.0,
            shaper_angle: None,
            installed_appearance: Default::default(),
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
            fixtures: vec![fixture].into(),
            cue_lists: vec![].into(),
            dynamics: vec![].into(),
            dynamic_stage_positions: Default::default(),
            playbacks: vec![].into(),
            playback_pages: vec![].into(),
            routes: vec![].into(),
            control_mappings: vec![].into(),
            groups: vec![].into(),
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
    programmers.start(session);
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
            fixtures: vec![fixture].into(),
            cue_lists: vec![].into(),
            dynamics: vec![].into(),
            dynamic_stage_positions: Default::default(),
            playbacks: vec![].into(),
            playback_pages: vec![].into(),
            routes: vec![].into(),
            control_mappings: vec![].into(),
            groups: vec![].into(),
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
    programmers.start(session);
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
            fixtures: vec![fixture].into(),
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
    programmers.start(session);
    let (mut fixture, child) = fixture();
    // A shared master head alongside the child cell: the child's dimmer takes slot 1, the
    // master's slot 2, which is what the byte assertions below read.
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .expect("the test fixture carries its profile")
        .clone();
    let mode_id = {
        let mode = &mut profile.modes[0];
        mode.splits[0].footprint = 2;
        let child_head = mode.heads[0].id;
        let master_head = uuid::Uuid::new_v4();
        mode.heads.insert(
            0,
            light_fixture::FixtureHead {
                id: master_head,
                name: "Master".into(),
                master_shared: true,
            },
        );
        let mut master_channel = mode.channels[0].clone();
        master_channel.id = uuid::Uuid::new_v4();
        master_channel.head_id = master_head;
        mode.channels[0].head_id = child_head;
        mode.channels.push(master_channel);
        mode.id
    };
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    // The child head is now the second head of the mode.
    fixture.logical_heads[0].head_index = 1;
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
            fixtures: vec![fixture].into(),
            playbacks: vec![test_group_playback_with_master(1, "master", 0.5)].into(),
            groups: vec![GroupDefinition {
                id: "master".into(),
                name: "Master only".into(),
                fixtures: vec![master],
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(rendered.universes[&1][0], 204);
    assert_eq!(rendered.universes[&1][1], 102);
}
