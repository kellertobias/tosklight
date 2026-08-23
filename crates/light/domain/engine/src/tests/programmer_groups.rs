use super::*;

#[test]
fn group_ltp_uses_operator_edit_time_not_render_time() {
    let programmers = ProgrammerRegistry::default();
    let group_session = light_core::SessionId::new();
    let direct_session = light_core::SessionId::new();
    programmers.start(group_session, light_core::UserId::new());
    programmers.start(direct_session, light_core::UserId::new());
    let (mut fixture, logical) = fixture();
    retarget_only_channel(&mut fixture, "pan");
    programmers.set_group(
        group_session,
        "position".into(),
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.2),
    );
    programmers.set(
        direct_session,
        logical,
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.8),
    );
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            groups: vec![GroupDefinition {
                id: "position".into(),
                name: "Position".into(),
                fixtures: vec![logical],
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        204
    );
    programmers.set_group(
        group_session,
        "position".into(),
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.1),
    );
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        26
    );
}

#[test]
fn programmer_intensity_is_ltp_however_many_surfaces_program_it() {
    let programmers = ProgrammerRegistry::default();
    let main_window = light_core::SessionId::new();
    let second_screen = light_core::SessionId::new();
    programmers.start(main_window, light_core::UserId::new());
    programmers.start(second_screen, light_core::UserId::new());
    let (fixture, logical) = fixture();
    programmers.set_group(
        main_window,
        "wash".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    programmers.set(
        main_window,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.3),
    );
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            groups: vec![GroupDefinition {
                id: "wash".into(),
                name: "Wash".into(),
                fixtures: vec![logical],
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        77,
        "the fixture value programmed after the Group value wins: the Programmer is LTP",
    );

    // A second screen is a surface of the same Programmer, not a second one bidding against it.
    // Its later value therefore replaces the earlier one rather than being merged with it.
    programmers.set(
        second_screen,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.6),
    );
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        153,
    );
    programmers.set(
        main_window,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    );
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        51,
        "the most recent value wins wherever it was typed, with no HTP between surfaces",
    );
}

#[test]
fn empty_group_programming_becomes_effective_when_members_are_added() {
    let programmers = ProgrammerRegistry::default();
    let (fixture, logical) = fixture();
    let engine = Engine::new(programmers);
    let group = GroupDefinition {
        id: "template".into(),
        name: "Template".into(),
        programming: HashMap::from([(AttributeKey::intensity(), AttributeValue::Normalized(0.6))]),
        fixtures: vec![],
        ..Default::default()
    };
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture.clone()].into(),
            groups: vec![group.clone()].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        0
    );
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            groups: vec![GroupDefinition {
                fixtures: vec![logical],
                ..group
            }]
            .into(),
            revision: 2,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        153
    );
}
#[test]
fn a_live_group_selection_remains_live_across_membership_changes() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    programmers.select_expression(
        session,
        vec![],
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "template".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );
    programmers.set_group(
        session,
        "template".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.6),
    );
    let (fixture, logical) = fixture();
    let observed = programmers.clone();
    let engine = Engine::new(programmers);
    let group = GroupDefinition {
        id: "template".into(),
        name: "Template".into(),
        fixtures: vec![],
        ..Default::default()
    };
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture.clone()].into(),
            groups: vec![group.clone()].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        0
    );
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            groups: vec![GroupDefinition {
                fixtures: vec![logical],
                ..group
            }]
            .into(),
            revision: 2,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        153
    );
    assert_eq!(observed.get(session).unwrap().selected, vec![logical]);

    // A selection frozen to explicit sources does not follow the same membership change.
    observed.select_expression(
        session,
        vec![],
        light_programmer::SelectionExpression::Sources { items: vec![] },
    );
    assert!(observed.get(session).unwrap().selected.is_empty());
}
#[test]
fn explicit_cue_change_wins_when_group_expansion_targets_same_attribute() {
    let programmers = ProgrammerRegistry::default();
    let (fixture, logical) = fixture();
    let mut cue = light_playback::Cue::new(1_u16.into());
    cue.changes.push(light_playback::CueChange::set(
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    ));
    cue.group_changes.push(light_playback::GroupCueChange {
        group_id: "group".into(),
        attribute: AttributeKey::intensity(),
        value: Some(AttributeValue::Normalized(0.5)),
        fade_millis: None,
        delay_millis: None,
        automatic_restore: false,
    });
    let cue_list = light_playback::CueList {
        id: light_core::CueListId::new(),
        name: "Deduplicated".into(),
        priority: 10,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_step_millis: 1_000,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_group: None,
        speed_multiplier: 1.0,
        cues: vec![cue],
    };
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![cue_list].into(),
            groups: vec![GroupDefinition {
                id: "group".into(),
                name: "Group".into(),
                fixtures: vec![logical],
                source: None,
                mapping: None,
                programming: Default::default(),
                derived_from: None,
                frozen_from: None,
                color: None,
                icon: None,
            }]
            .into(),
            revision: 1,
            ..Default::default()
        })
        .expect("overlapping group and fixture cue values must compile");
}
