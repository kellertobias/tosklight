use super::*;

struct HighlightScenario {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    physical: FixtureId,
    fixture_template: PatchedFixture,
}

#[test]
fn transient_highlight_wins_over_group_master_while_grand_master_and_blackout_win() {
    let HighlightScenario {
        engine,
        programmers,
        session,
        physical,
        fixture_template,
    } = highlight_scenario();

    engine.set_highlighted_fixtures([physical]);
    assert_eq!(render_slot(&engine, 0, 0.5, false), 100);
    assert_eq!(render_slot(&engine, 9, 0.5, false), 100);
    assert_eq!(render_slot(&engine, 0, 1.0, true), 0);

    engine.clear_highlighted_fixtures();
    assert_eq!(render_slot(&engine, 0, 1.0, false), 64);
    assert_eq!(render_slot(&engine, 9, 1.0, false), 64);
    let programmer = programmers.get(session).unwrap();
    assert_eq!(programmer.values.len(), 1);
    assert_eq!(programmer.values[0].fixture_id, physical);
    assert_eq!(programmer.values[0].attribute, AttributeKey::intensity());
    assert_eq!(programmer.values[0].value, AttributeValue::Normalized(0.5));

    programmers.clear_values(session);
    assert_eq!(render_slot(&engine, 0, 1.0, false), 128);
    engine.set_highlighted_fixtures([physical]);
    assert_eq!(render_slot(&engine, 0, 1.0, false), 200);

    let mut second = fixture_template.clone();
    let second_id = FixtureId::new();
    second.fixture_id = second_id;
    second.fixture_number = Some(2);
    second.name = "Second Highlight fixture".into();
    second.address = Some(20);
    second.multipatch.clear();
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture_template, second],
            revision: 2,
            ..Default::default()
        })
        .unwrap();
    engine.set_highlighted_fixtures([physical]);
    let first_step = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(first_step.universes[&1][0], 200);
    assert_eq!(first_step.universes[&1][19], 0);
    engine.set_highlighted_fixtures([second_id]);
    let next_step = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(
        next_step.universes[&1][0], 0,
        "the old step must lose Highlight on the first frame after NEXT"
    );
    assert_eq!(
        next_step.universes[&1][19], 200,
        "the new step must receive Highlight on that same first frame"
    );
}

fn highlight_scenario() -> HighlightScenario {
    let physical = FixtureId::new();
    let mut fixture = highlight_fixture(physical);
    fixture.multipatch.push(MultiPatchInstance {
        id: uuid::Uuid::new_v4(),
        name: "Second physical copy".into(),
        universe: Some(1),
        address: Some(10),
        split_patches: vec![],
        location: Default::default(),
        rotation: Default::default(),
    });
    let fixture_template = fixture.clone();
    let cue_list = test_cue_list(
        "Highlight playback source",
        vec![CueChange::set(
            physical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        )],
    );
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    programmers.set(
        session,
        physical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            groups: vec![GroupDefinition {
                id: "1".into(),
                name: "Master".into(),
                fixtures: vec![physical],
                master: 0.5,
                playback_fader: Some(1),
                ..Default::default()
            }],
            cue_lists: vec![cue_list.clone()],
            playbacks: vec![test_playback(1, cue_list.id)],
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine.playback().write().go(cue_list.id).unwrap();
    HighlightScenario {
        engine,
        programmers,
        session,
        physical,
        fixture_template,
    }
}

fn highlight_fixture(physical: FixtureId) -> PatchedFixture {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Highlight fixture".into();
    profile.short_name = "Highlight".into();
    profile.revision = 1;
    let head = profile.modes[0].heads[0].id;
    profile.modes[0].channels = vec![FixtureChannel {
        id: uuid::Uuid::new_v4(),
        head_id: head,
        split: 1,
        attribute: AttributeKey::intensity(),
        resolution: ChannelResolution::U8,
        secondary_slots: vec![],
        default_raw: 0,
        highlight_raw: 200,
        physical_min: Some(0.0),
        physical_max: Some(1.0),
        unit: Some("percent".into()),
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        reacts_to_sequence_master: false,
        reacts_to_group_master: true,
        reacts_to_grand_master: true,
        behavior: ChannelBehavior::Controlled,
        functions: vec![ChannelFunction::continuous(
            "Dimmer",
            AttributeKey::intensity(),
            255,
        )],
    }];
    profile.modes[0].geometry = GeometryGraph::default();
    PatchedFixture {
        fixture_id: physical,
        fixture_number: Some(1),
        virtual_fixture_number: None,
        name: "Highlight fixture".into(),
        definition: profile.resolved_definition(profile.modes[0].id).unwrap(),
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

fn render_slot(engine: &Engine, slot: usize, grand_master: f32, blackout: bool) -> u8 {
    engine
        .render(RenderOptions {
            grand_master,
            blackout,
            control_loss_progress: None,
        })
        .unwrap()
        .universes[&1][slot]
}
