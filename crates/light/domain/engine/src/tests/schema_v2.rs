use super::*;

#[test]
fn single_patch_fast_path_preserves_profile_visualization_for_patched_and_unpatched_fixtures() {
    let (mut fixture, fixture_id) =
        schema_v2_fixture(&[("intensity", false, false, false, false, false)]);
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture.clone()].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    let patched = engine.render(RenderOptions::default()).unwrap();
    assert!(patched.universes.contains_key(&1));
    assert!(
        patched
            .profile_visualization_values
            .contains_key(&(fixture_id, AttributeKey::intensity()))
    );

    fixture.universe = None;
    fixture.address = None;
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 2,
            ..Default::default()
        })
        .unwrap();

    let unpatched = engine.render(RenderOptions::default()).unwrap();
    assert!(unpatched.universes.is_empty());
    assert!(
        unpatched
            .profile_visualization_values
            .contains_key(&(fixture_id, AttributeKey::intensity()))
    );
}

#[test]
fn physical_axis_inversion_is_independent_for_root_and_multipatch() {
    let (mut fixture, fixture_id) = schema_v2_fixture(&[
        ("pan", false, false, false, false, false),
        ("tilt", false, false, false, false, false),
    ]);
    fixture.invert_pan = true;
    fixture.multipatch = vec![MultiPatchInstance {
        id: uuid::Uuid::new_v4(),
        name: "Opposite hang".into(),
        universe: Some(1),
        address: Some(10),
        split_patches: vec![],
        location: Default::default(),
        rotation: Default::default(),
        invert_pan: false,
        invert_tilt: true,
        bracket_angle: 0.0,
        shaper_angle: None,
    }];
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    programmers.set(
        session,
        fixture_id,
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.25),
    );
    programmers.set(
        session,
        fixture_id,
        AttributeKey("tilt".into()),
        AttributeValue::Normalized(0.75),
    );
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(&rendered.universes[&1][0..2], &[191, 191]);
    assert_eq!(&rendered.universes[&1][9..11], &[64, 64]);
    assert_eq!(
        engine
            .resolved_values()
            .get(&(fixture_id, AttributeKey("pan".into()))),
        Some(&AttributeValue::Normalized(0.25)),
        "physical inversion must not rewrite the shared Programmer value"
    );

    programmers.set(
        session,
        fixture_id,
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.5),
    );
    programmers.set(
        session,
        fixture_id,
        AttributeKey("tilt".into()),
        AttributeValue::Normalized(0.5),
    );
    let midpoint = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(&midpoint.universes[&1][0..2], &[128, 128]);
    assert_eq!(&midpoint.universes[&1][9..11], &[128, 128]);

    programmers.set(
        session,
        fixture_id,
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.0),
    );
    programmers.set(
        session,
        fixture_id,
        AttributeKey("tilt".into()),
        AttributeValue::Normalized(0.0),
    );
    let low = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(&low.universes[&1][0..2], &[255, 0]);
    assert_eq!(&low.universes[&1][9..11], &[0, 255]);

    programmers.set(
        session,
        fixture_id,
        AttributeKey("pan".into()),
        AttributeValue::Normalized(1.0),
    );
    programmers.set(
        session,
        fixture_id,
        AttributeKey("tilt".into()),
        AttributeValue::Normalized(1.0),
    );
    let high = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(&high.universes[&1][0..2], &[0, 255]);
    assert_eq!(&high.universes[&1][9..11], &[255, 0]);
}

#[test]
fn patch_and_profile_axis_inversion_compose_exactly_once() {
    let (mut fixture, fixture_id) =
        schema_v2_fixture(&[("pan", false, false, false, false, false)]);
    fixture.invert_pan = true;
    fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].channels[0].invert = true;
    fixture.multipatch = vec![MultiPatchInstance {
        id: uuid::Uuid::new_v4(),
        name: "Profile inversion only".into(),
        universe: Some(1),
        address: Some(10),
        split_patches: vec![],
        location: Default::default(),
        rotation: Default::default(),
        invert_pan: false,
        invert_tilt: false,
        bracket_angle: 0.0,
        shaper_angle: None,
    }];
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    programmers.set(
        session,
        fixture_id,
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.25),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..Default::default()
        })
        .unwrap();
    let frame = &engine.render(RenderOptions::default()).unwrap().universes[&1];
    assert_eq!(frame[0], 64, "two inversions cancel physically");
    assert_eq!(frame[9], 191, "profile inversion applies once");
}

#[test]
fn patch_axis_inversion_preserves_exact_msb_first_encoding_at_every_resolution() {
    let cases = [
        (ChannelResolution::U8, vec![], vec![0xbf]),
        (ChannelResolution::U16, vec![2], vec![0xbf, 0xff]),
        (ChannelResolution::U24, vec![2, 3], vec![0xbf, 0xff, 0xff]),
        (
            ChannelResolution::U32,
            vec![2, 3, 4],
            vec![0xc0, 0x00, 0x00, 0x00],
        ),
    ];
    for (resolution, secondary_slots, expected) in cases {
        let (mut fixture, fixture_id) =
            schema_v2_fixture(&[("pan", false, false, false, false, false)]);
        fixture.invert_pan = true;
        let mode = &mut fixture.definition.profile_snapshot.as_mut().unwrap().modes[0];
        mode.splits[0].footprint = resolution.bytes() as u16;
        let channel = &mut mode.channels[0];
        channel.resolution = resolution;
        channel.secondary_slots = secondary_slots;
        channel.highlight_raw = resolution.max_raw();
        channel.functions = vec![ChannelFunction::continuous(
            "Pan",
            AttributeKey("pan".into()),
            resolution.max_raw(),
        )];

        let programmers = ProgrammerRegistry::default();
        let session = SessionId::new();
        programmers.start(session, UserId::new());
        programmers.set(
            session,
            fixture_id,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.25),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture].into(),
                ..Default::default()
            })
            .unwrap();
        let frame = &engine.render(RenderOptions::default()).unwrap().universes[&1];
        assert_eq!(&frame[..expected.len()], expected.as_slice());
    }
}

#[test]
fn borrowed_profile_lookup_matches_owned_hold_last_resolution() {
    let (fixture, fixture_id) = schema_v2_fixture(&[
        ("intensity", false, false, false, false, true),
        ("color.red", false, true, false, false, false),
        ("color.green", false, true, false, false, false),
        ("color.blue", false, true, false, false, false),
    ]);
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    programmers.set(
        session,
        fixture_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.65),
    );
    programmers.set(
        session,
        fixture_id,
        AttributeKey("color.red".into()),
        AttributeValue::Normalized(0.4),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    let borrowed = engine.render(RenderOptions::default()).unwrap();
    let owned = engine
        .render(RenderOptions {
            control_loss_progress: Some(0.5),
            ..RenderOptions::default()
        })
        .unwrap();

    assert_eq!(borrowed.universes, owned.universes);
    assert_eq!(
        borrowed.profile_visualization_values,
        owned.profile_visualization_values
    );
}

#[test]
fn schema_v2_renders_one_head_channels_to_independent_splits() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Split fixture".into();
    profile.short_name = "Split".into();
    profile.revision = 1;
    let first_head = profile.modes[0].heads[0].id;
    profile.modes[0].splits = vec![
        FixtureSplit {
            number: 1,
            footprint: 4,
        },
        FixtureSplit {
            number: 2,
            footprint: 1,
        },
    ];
    let exact_attribute = AttributeKey("control.exact".into());
    profile.modes[0].channels = vec![
        FixtureChannel {
            id: uuid::Uuid::new_v4(),
            head_id: first_head,
            split: 1,
            fixture_attribute: exact_attribute.clone(),
            attribute: exact_attribute.clone(),
            canonical_transform: light_fixture::CanonicalTransform::Identity,
            resolution: ChannelResolution::U32,
            secondary_slots: vec![2, 3, 4],
            default_raw: 0,
            highlight_raw: u32::MAX,
            physical_min: None,
            physical_max: None,
            unit: None,
            invert: false,
            snap: true,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: false,
            reacts_to_group_master: false,
            reacts_to_grand_master: false,
            behavior: ChannelBehavior::Controlled,
            functions: vec![ChannelFunction::continuous(
                "Exact",
                exact_attribute.clone(),
                u32::MAX,
            )],
        },
        FixtureChannel {
            id: uuid::Uuid::new_v4(),
            head_id: first_head,
            split: 2,
            fixture_attribute: AttributeKey("remote.static".into()),
            attribute: AttributeKey("remote.static".into()),
            canonical_transform: light_fixture::CanonicalTransform::Identity,
            resolution: ChannelResolution::U8,
            secondary_slots: vec![],
            default_raw: 0xaa,
            highlight_raw: 0xbb,
            physical_min: None,
            physical_max: None,
            unit: None,
            invert: false,
            snap: false,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: false,
            reacts_to_group_master: false,
            reacts_to_grand_master: false,
            behavior: ChannelBehavior::Static,
            functions: vec![],
        },
    ];
    profile.modes[0].geometry = GeometryGraph::default();
    let mode_id = profile.modes[0].id;
    let definition = profile.resolved_definition(mode_id).unwrap();
    let physical = FixtureId::new();
    let fixture = PatchedFixture {
        fixture_id: physical,
        fixture_number: Some(1),
        virtual_fixture_number: None,
        name: "Split fixture".into(),
        definition,
        universe: None,
        address: None,
        split_patches: vec![
            SplitPatch {
                split: 1,
                universe: Some(1),
                address: Some(10),
            },
            SplitPatch {
                split: 2,
                universe: Some(2),
                address: Some(20),
            },
        ],
        layer_id: "default".into(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        multipatch: vec![MultiPatchInstance {
            id: uuid::Uuid::new_v4(),
            name: "Mirror".into(),
            universe: None,
            address: None,
            split_patches: vec![
                SplitPatch {
                    split: 1,
                    universe: Some(3),
                    address: Some(30),
                },
                SplitPatch {
                    split: 2,
                    universe: Some(4),
                    address: Some(40),
                },
            ],
            location: Default::default(),
            rotation: Default::default(),
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.0,
            shaper_angle: None,
        }],
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
        bracket_angle: 0.0,
        shaper_angle: None,
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    programmers.set(
        session,
        physical,
        exact_attribute,
        AttributeValue::RawDmxExact(0x1234_5678),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(&rendered.universes[&1][9..13], &[0x12, 0x34, 0x56, 0x78]);
    assert_eq!(rendered.universes[&2][19], 0xaa);
    assert_eq!(&rendered.universes[&3][29..33], &[0x12, 0x34, 0x56, 0x78]);
    assert_eq!(rendered.universes[&4][39], 0xaa);
    assert_eq!(rendered.patched_slots[&1], 13);
    assert_eq!(rendered.patched_slots[&2], 20);
    assert_eq!(rendered.patched_slots[&3], 33);
    assert_eq!(rendered.patched_slots[&4], 40);
}

#[test]
fn schema_v2_snap_bypasses_programmer_fades_but_keeps_non_snap_timing() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    let (fixture, fixture_id) = schema_v2_fixture(&[
        ("pan", true, false, false, false, false),
        ("tilt", false, false, false, false, false),
    ]);
    let engine = Engine::new(programmers.clone());
    engine.set_control_timing([120.0; 5], 1_000, 0);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let generation = engine.generation.load();
    assert!(generation.attribute_is_snap(fixture_id, &AttributeKey("pan".into())));
    assert!(!generation.attribute_is_snap(fixture_id, &AttributeKey("tilt".into())));
    drop(generation);
    programmers.set_faded(
        session,
        fixture_id,
        AttributeKey("pan".into()),
        AttributeValue::Normalized(1.0),
    );
    programmers.set_faded(
        session,
        fixture_id,
        AttributeKey("tilt".into()),
        AttributeValue::Normalized(1.0),
    );

    let values = engine.resolved_values();
    assert_eq!(normalized(&values, fixture_id, "pan"), 1.0);
    assert_eq!(normalized(&values, fixture_id, "tilt"), 0.0);
    clock.set(started + ChronoDuration::milliseconds(500));
    let values = engine.resolved_values();
    assert_eq!(normalized(&values, fixture_id, "pan"), 1.0);
    assert!((normalized(&values, fixture_id, "tilt") - 0.5).abs() < 0.001);
}

#[test]
fn schema_v2_snap_bypasses_move_in_black_and_signal_loss_fades() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let (fixture, fixture_id) = schema_v2_fixture(&[
        ("intensity", false, false, false, false, false),
        ("pan", true, false, false, false, false),
    ]);
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(mib_snapshot(vec![fixture], &[fixture_id]))
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    clock.set(started + ChronoDuration::milliseconds(1_999));
    assert_eq!(
        normalized(&engine.resolved_values(), fixture_id, "pan"),
        0.2
    );
    clock.set(started + ChronoDuration::milliseconds(2_000));
    assert_eq!(
        normalized(&engine.resolved_values(), fixture_id, "pan"),
        0.8
    );
    assert_eq!(
        engine.move_in_black_runtime()[0].state,
        MoveInBlackState::Completed
    );

    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    let (mut fixture, fixture_id) = schema_v2_fixture(&[
        ("pan", true, false, false, false, false),
        ("tilt", false, false, false, false, false),
    ]);
    fixture.definition.signal_loss_policy = SignalLossPolicy::FadeToSafe {
        duration_millis: 1_000,
    };
    fixture
        .definition
        .safe_values
        .insert(AttributeKey("pan".into()), AttributeValue::Normalized(0.0));
    fixture
        .definition
        .safe_values
        .insert(AttributeKey("tilt".into()), AttributeValue::Normalized(0.0));
    programmers.set(
        session,
        fixture_id,
        AttributeKey("pan".into()),
        AttributeValue::Normalized(1.0),
    );
    programmers.set(
        session,
        fixture_id,
        AttributeKey("tilt".into()),
        AttributeValue::Normalized(1.0),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let frame = engine
        .render(RenderOptions {
            control_loss_progress: Some(0.5),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(frame.universes[&1][0], 0);
    assert_eq!(frame.universes[&1][1], 128);
}

#[test]
fn schema_v2_master_reactions_use_only_the_winning_sources_and_scale_once() {
    let (fixture, fixture_id) = schema_v2_fixture(&[
        ("intensity", false, false, true, true, true),
        ("color.red", false, true, true, true, true),
        ("beam.rate", false, false, true, true, true),
        ("beam.other", false, false, true, true, true),
    ]);
    let main = test_cue_list(
        "Main",
        ["intensity", "color.red", "beam.rate"]
            .into_iter()
            .map(|attribute| {
                CueChange::set(
                    fixture_id,
                    AttributeKey(attribute.into()),
                    AttributeValue::Normalized(1.0),
                )
            })
            .collect(),
    );
    let unrelated = test_cue_list(
        "Unrelated",
        vec![CueChange::set(
            fixture_id,
            AttributeKey("beam.other".into()),
            AttributeValue::Normalized(1.0),
        )],
    );
    let playbacks = vec![
        test_playback(1, main.id),
        test_playback(2, unrelated.id),
        test_group_playback_with_master(3, "front", 0.5),
    ];
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![main, unrelated].into(),
            playbacks: playbacks.into(),
            groups: vec![GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                fixtures: vec![fixture_id],
                ..Default::default()
            }]
            .into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    execute_pool(&engine, 2, PoolPlaybackAction::Go);
    execute_pool(&engine, 1, PoolPlaybackAction::SetVirtualMaster(0.5));
    execute_pool(&engine, 2, PoolPlaybackAction::SetVirtualMaster(0.1));

    let frame = engine
        .render(RenderOptions {
            grand_master: 0.5,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        &frame.universes[&1][0..4],
        &[32, 32, 32, 6],
        "intensity and virtual intensity already contain their sequence master; a separate semantic source receives only its own master"
    );
}

#[test]
fn inverted_intensity_masters_and_blackout_move_to_physical_off() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    let (mut fixture, fixture_id) =
        schema_v2_fixture(&[("intensity", false, false, false, false, true)]);
    fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].channels[0].invert = true;
    programmers.set(
        session,
        fixture_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        engine
            .render(RenderOptions {
                grand_master: 0.5,
                ..Default::default()
            })
            .unwrap()
            .universes[&1][0],
        191
    );
    assert_eq!(
        engine
            .render(RenderOptions {
                grand_master: 0.0,
                ..Default::default()
            })
            .unwrap()
            .universes[&1][0],
        255
    );
    assert_eq!(
        engine
            .render(RenderOptions {
                blackout: true,
                ..Default::default()
            })
            .unwrap()
            .universes[&1][0],
        255
    );
}

/// D3 (derive-only) guardrail: a virtual-dimmer head has no physical dimmer channel; the
/// operator's independent intensity multiplies onto the `reacts_to_virtual_intensity` colour
/// channels at DMX output only. Neither stored value derives from the other — changing intensity
/// must not rewrite the colour value, and the derived definition exposes the abstract intensity
/// parameter re-derived from the channels.
#[test]
fn virtual_dimmer_intensity_multiplies_reacting_channels_one_way() {
    let (fixture, fixture_id) = schema_v2_fixture(&[
        ("color.red", false, true, false, false, false),
        ("color.green", false, true, false, false, false),
    ]);
    let intensity = fixture
        .definition
        .heads
        .iter()
        .flat_map(|head| &head.parameters)
        .find(|parameter| parameter.attribute.is_intensity())
        .expect("the derived definition re-derives the abstract virtual-dimmer intensity");
    assert!(intensity.virtual_dimmer);
    assert!(intensity.components.is_empty());

    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    programmers.set(
        session,
        fixture_id,
        AttributeKey("color.red".into()),
        AttributeValue::Normalized(0.8),
    );
    programmers.set(
        session,
        fixture_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let observed = programmers.clone();
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(
        rendered.universes[&1][0],
        (0.8f32 * 0.5 * 255.0).round() as u8
    );
    assert_eq!(rendered.universes[&1][1], 0);

    // One-way: the multiply happens at output; the stored colour value stays untouched.
    let stored = observed.get(session).unwrap();
    let red = stored
        .values
        .iter()
        .find(|value| value.attribute.0 == "color.red")
        .unwrap();
    assert_eq!(red.value, AttributeValue::Normalized(0.8));
}
