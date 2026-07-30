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
fn patch_master_opt_outs_are_independent_and_blackout_remains_authoritative() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    let (mut participating, participating_id) = fixture();
    let (mut ignores_grand, ignores_grand_id) = fixture();
    let (mut ignores_groups, ignores_groups_id) = fixture();
    participating.address = Some(1);
    ignores_grand.address = Some(2);
    ignores_grand.grand_master_enabled = false;
    ignores_groups.address = Some(3);
    ignores_groups.group_masters_enabled = false;
    for fixture_id in [participating_id, ignores_grand_id, ignores_groups_id] {
        programmers.set(
            session,
            fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
    }
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![participating, ignores_grand, ignores_groups].into(),
            playbacks: vec![test_group_playback(1, "all")].into(),
            groups: vec![GroupDefinition {
                id: "all".into(),
                fixtures: vec![participating_id, ignores_grand_id, ignores_groups_id],
                master: 0.25,
                playback_fader: Some(1),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();

    let ordinary = engine
        .render(RenderOptions {
            grand_master: 0.5,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(&ordinary.universes[&1][..3], &[32, 64, 128]);

    engine.set_group_master_flash("all".into(), 1.0);
    let flashed = engine
        .render(RenderOptions {
            grand_master: 0.5,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(&flashed.universes[&1][..3], &[128, 255, 128]);

    let blackout = engine
        .render(RenderOptions {
            grand_master: 0.5,
            blackout: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(&blackout.universes[&1][..3], &[0, 0, 0]);
}

#[test]
fn group_masters_follow_real_assignments_and_resolve_overlap_by_htp() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    let mut fixtures = Vec::new();
    let mut logical_ids = Vec::new();
    for address in 1..=6 {
        let (mut fixture, logical) = fixture();
        fixture.address = Some(address);
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
        fixtures.push(fixture);
        logical_ids.push(logical);
    }
    let engine = Engine::new(programmers);
    let groups = vec![
        GroupDefinition {
            id: "all".into(),
            name: "All".into(),
            fixtures: logical_ids.clone(),
            master: 0.0,
            // A stale legacy pointer is not an actual Group Master assignment.
            playback_fader: Some(99),
            ..Default::default()
        },
        GroupDefinition {
            id: "odd".into(),
            name: "Odd".into(),
            derived_from: Some(DerivedGroup {
                source_group_id: "all".into(),
                rule: SelectionRule::Odd,
            }),
            master: 0.25,
            ..Default::default()
        },
        GroupDefinition {
            id: "even".into(),
            name: "Even".into(),
            fixtures: logical_ids.iter().skip(1).step_by(2).copied().collect(),
            frozen_from: Some(FrozenGroup {
                source_group_id: "all".into(),
                source_revision: 1,
                captured_at: chrono::Utc::now(),
            }),
            master: 0.75,
            ..Default::default()
        },
    ];
    let mut snapshot = EngineSnapshot {
        fixtures: fixtures.into(),
        playbacks: vec![
            test_group_playback(1, "odd"),
            test_group_playback(2, "even"),
        ]
        .into(),
        groups: groups.into(),
        revision: 1,
        ..Default::default()
    };
    engine.replace_snapshot(snapshot.clone()).unwrap();
    assert_eq!(
        &engine
            .render(RenderOptions {
                grand_master: 0.5,
                ..Default::default()
            })
            .unwrap()
            .universes[&1][..6],
        &[32, 96, 32, 96, 32, 96]
    );
    for logical in &logical_ids {
        assert_eq!(
            normalized(&engine.resolved_values(), *logical, "intensity"),
            1.0
        );
    }

    Arc::make_mut(&mut snapshot.playbacks).push(test_group_playback(3, "all"));
    engine.replace_snapshot(snapshot).unwrap();
    assert!(engine.set_group_master("all", 0.9).unwrap());
    assert_eq!(
        &engine
            .render(RenderOptions {
                grand_master: 0.5,
                ..Default::default()
            })
            .unwrap()
            .universes[&1][..6],
        &[115; 6]
    );

    assert!(engine.set_group_master("all", 0.1).unwrap());
    assert_eq!(
        &engine
            .render(RenderOptions {
                grand_master: 0.5,
                ..Default::default()
            })
            .unwrap()
            .universes[&1][..6],
        &[32, 96, 32, 96, 32, 96]
    );
}

#[test]
fn group_master_runtime_update_is_targeted_idempotent_and_revision_neutral() {
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
            fixtures: vec![fixture].into(),
            playbacks: vec![test_group_playback(1, "front")].into(),
            groups: vec![GroupDefinition {
                id: "front".into(),
                fixtures: vec![logical],
                master: 0.25,
                playback_fader: Some(1),
                ..Default::default()
            }]
            .into(),
            revision: 7,
            ..Default::default()
        })
        .unwrap();

    assert!(engine.set_group_master("front", 0.75).unwrap());
    assert_eq!(engine.snapshot().revision, 7);
    assert_eq!(engine.snapshot().groups[0].master, 0.75);
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        153
    );
    assert!(!engine.set_group_master("front", 0.75).unwrap());
    let mut replacement = (*engine.snapshot()).clone();
    Arc::make_mut(&mut replacement.groups)[0].master = 1.0;
    replacement.revision += 1;
    engine.replace_snapshot(replacement).unwrap();
    assert_eq!(engine.snapshot().groups[0].master, 0.75);
    assert!(engine.set_group_master("missing", 0.5).is_err());
    assert!(engine.set_group_master("front", f32::NAN).is_err());
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
            fixtures: vec![fixture].into(),
            playbacks: vec![test_group_playback(1, "front")].into(),
            groups: vec![GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                fixtures: vec![logical],
                master: 0.25,
                playback_fader: Some(1),
                ..Default::default()
            }]
            .into(),
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
                profile_head_id: None,
                head_index: 1,
                fixture_id: first,
            },
            PatchedHead {
                profile_head_id: None,
                head_index: 2,
                fixture_id: second,
            },
        ],
        multipatch: vec![],
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
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
            fixtures: vec![fixture].into(),
            playbacks: vec![test_group_playback(1, "first")].into(),
            groups: vec![GroupDefinition {
                id: "first".into(),
                name: "First".into(),
                fixtures: vec![first],
                master: 0.5,
                playback_fader: Some(1),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    let rendered = engine.render(RenderOptions::default()).unwrap();
    let frame = &rendered.universes[&1];
    assert_eq!(frame[0], 102);
    assert_eq!(frame[1], 204);
}
