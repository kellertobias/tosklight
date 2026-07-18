use super::*;

#[test]
fn grand_master_and_blackout_affect_intensity() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    let (fixture, logical) = fixture();
    programmers.set(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
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
    assert_eq!(
        engine
            .render(RenderOptions {
                grand_master: 0.5,
                blackout: false,
                control_loss_progress: None,
            })
            .unwrap()
            .universes[&1][0],
        128
    );
    assert_eq!(
        engine
            .render(RenderOptions {
                grand_master: 1.0,
                blackout: true,
                control_loss_progress: None,
            })
            .unwrap()
            .universes[&1][0],
        0
    );
}

#[test]
fn group_masters_scale_before_encoding_and_use_highest_master() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    let (fixture, logical) = fixture();
    programmers.set(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
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
            groups: vec![
                GroupDefinition {
                    id: "a".into(),
                    name: "A".into(),
                    fixtures: vec![logical],
                    master: 0.5,
                    playback_fader: Some(1),
                    ..Default::default()
                },
                GroupDefinition {
                    id: "b".into(),
                    name: "B".into(),
                    fixtures: vec![logical],
                    master: 0.75,
                    playback_fader: Some(2),
                    ..Default::default()
                },
                GroupDefinition {
                    id: "unassigned".into(),
                    name: "Unassigned".into(),
                    fixtures: vec![logical],
                    master: 1.0,
                    playback_fader: None,
                    ..Default::default()
                },
            ],
            revision: 1,
        })
        .unwrap();
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        153
    );
}

#[test]
fn group_master_flash_is_temporary_and_does_not_move_the_fader() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    let (fixture, logical) = fixture();
    programmers.set(
        session,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            groups: vec![GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                fixtures: vec![logical],
                master: 0.25,
                playback_fader: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        51
    );
    engine.set_group_master_flash("front".into(), 1.0);
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        204
    );
    assert_eq!(engine.snapshot().groups[0].master, 0.25);
    engine.set_group_master_flash("front".into(), 0.0);
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        51
    );
}
#[test]
fn logical_head_master_does_not_limit_sibling_heads() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    let physical = FixtureId::new();
    let first = FixtureId::new();
    let second = FixtureId::new();
    let parameter = |offset| Parameter {
        attribute: AttributeKey::intensity(),
        components: vec![ChannelComponent {
            offset,
            byte_order: light_fixture::ByteOrder::MsbFirst,
        }],
        default: 0.0,
        virtual_dimmer: false,
        metadata: light_fixture::ParameterMetadata::default(),
        capabilities: vec![],
    };
    let fixture = PatchedFixture {
        fixture_id: physical,
        fixture_number: None,
        virtual_fixture_number: None,
        name: "Two cell".into(),
        layer_id: "default".into(),
        definition: FixtureDefinition {
            schema_version: 1,
            id: FixtureId::new(),
            revision: 1,
            manufacturer: "Test".into(),
            device_type: "other".into(),
            name: "Two cell".into(),
            model: "Two cell".into(),
            mode: "2ch".into(),
            footprint: 2,
            heads: vec![
                LogicalHead {
                    index: 1,
                    name: "One".into(),
                    shared: false,
                    parameters: vec![parameter(0)],
                },
                LogicalHead {
                    index: 2,
                    name: "Two".into(),
                    shared: false,
                    parameters: vec![parameter(1)],
                },
            ],
            color_calibration: None,
            physical: Default::default(),
            model_asset: None,
            icon_asset: None,
            hazardous: false,
            direct_control_protocols: vec![],
            signal_loss_policy: SignalLossPolicy::HoldLast,
            safe_values: BTreeMap::new(),
            profile_id: None,
            mode_id: None,
            profile_snapshot: None,
        },
        universe: Some(1),
        address: Some(1),
        split_patches: Vec::new(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![
            PatchedHead {
                head_index: 1,
                fixture_id: first,
            },
            PatchedHead {
                head_index: 2,
                fixture_id: second,
            },
        ],
        multipatch: vec![],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    for fixture_id in [first, second] {
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
                id: "first".into(),
                name: "First".into(),
                fixtures: vec![first],
                master: 0.5,
                playback_fader: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let rendered = engine.render(RenderOptions::default()).unwrap();
    let frame = &rendered.universes[&1];
    assert_eq!(frame[0], 102);
    assert_eq!(frame[1], 204);
}
