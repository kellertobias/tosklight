use super::*;

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
            attribute: exact_attribute.clone(),
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
            attribute: AttributeKey("remote.static".into()),
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
        multipatch: vec![],
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
            fixtures: vec![fixture],
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(&rendered.universes[&1][9..13], &[0x12, 0x34, 0x56, 0x78]);
    assert_eq!(rendered.universes[&2][19], 0xaa);
    assert_eq!(rendered.patched_slots[&1], 13);
    assert_eq!(rendered.patched_slots[&2], 20);
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
            fixtures: vec![fixture],
            revision: 1,
            ..Default::default()
        })
        .unwrap();
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
    engine.playback().write().go_playback(1).unwrap();
    engine.playback().write().go_playback(1).unwrap();
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
            fixtures: vec![fixture],
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
    let playbacks = vec![test_playback(1, main.id), test_playback(2, unrelated.id)];
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            cue_lists: vec![main, unrelated],
            playbacks,
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
    engine.playback().write().go_playback(1).unwrap();
    engine.playback().write().go_playback(2).unwrap();
    engine.playback().write().set_master(1, 0.5).unwrap();
    engine.playback().write().set_master(2, 0.1).unwrap();

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
            fixtures: vec![fixture],
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
