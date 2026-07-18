use super::*;

#[test]
fn group_ltp_uses_operator_edit_time_not_render_time() {
    let programmers = ProgrammerRegistry::default();
    let group_session = light_core::SessionId::new();
    let direct_session = light_core::SessionId::new();
    programmers.start(group_session, light_core::UserId::new());
    programmers.start(direct_session, light_core::UserId::new());
    let (mut fixture, logical) = fixture();
    fixture.definition.heads[0].parameters[0].attribute = AttributeKey("pan".into());
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
            fixtures: vec![fixture],
            groups: vec![GroupDefinition {
                id: "position".into(),
                name: "Position".into(),
                fixtures: vec![logical],
                ..Default::default()
            }],
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
fn programmer_intensity_is_ltp_within_one_programmer_and_htp_between_programmers() {
    let programmers = ProgrammerRegistry::default();
    let first = light_core::SessionId::new();
    let second = light_core::SessionId::new();
    programmers.start(first, light_core::UserId::new());
    programmers.start(second, light_core::UserId::new());
    let (fixture, logical) = fixture();
    programmers.set_group(
        first,
        "wash".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    programmers.set(
        first,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.3),
    );
    programmers.set(
        second,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.6),
    );
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            groups: vec![GroupDefinition {
                id: "wash".into(),
                name: "Wash".into(),
                fixtures: vec![logical],
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        153,
        "the first programmer resolves its newer 30% fixture value before cross-source HTP chooses the second programmer's 60%",
    );
    assert!(programmers.set_priority(second, 110));
    programmers.set(
        second,
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    );
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        51,
        "numeric priority resolves before HTP magnitude",
    );
    assert!(programmers.set_priority(second, 90));
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        77,
        "changing programmer priority retags its existing values and reveals the higher-priority programmer",
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
            fixtures: vec![fixture.clone()],
            groups: vec![group.clone()],
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
            fixtures: vec![fixture],
            groups: vec![GroupDefinition {
                fixtures: vec![logical],
                ..group
            }],
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
fn session_group_programmer_remains_live_across_membership_changes() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    let frozen_session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    programmers.start(frozen_session, light_core::UserId::new());
    programmers.select_expression(
        session,
        vec![],
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "template".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );
    programmers.select_expression(
        frozen_session,
        vec![],
        light_programmer::SelectionExpression::FrozenGroup {
            group_id: "template".into(),
            source_revision: 0,
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
            fixtures: vec![fixture.clone()],
            groups: vec![group.clone()],
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
            fixtures: vec![fixture],
            groups: vec![GroupDefinition {
                fixtures: vec![logical],
                ..group
            }],
            revision: 2,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        153
    );
    assert_eq!(observed.get(session).unwrap().selected, vec![logical]);
    assert!(observed.get(frozen_session).unwrap().selected.is_empty());
}
#[test]
fn explicit_cue_change_wins_when_group_expansion_targets_same_attribute() {
    let programmers = ProgrammerRegistry::default();
    let (fixture, logical) = fixture();
    let mut cue = light_playback::Cue::new(1.0);
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
            fixtures: vec![fixture],
            cue_lists: vec![cue_list],
            groups: vec![GroupDefinition {
                id: "group".into(),
                name: "Group".into(),
                fixtures: vec![logical],
                master: 1.0,
                playback_fader: None,
                programming: Default::default(),
                derived_from: None,
                frozen_from: None,
                color: None,
                icon: None,
            }],
            revision: 1,
            ..Default::default()
        })
        .expect("overlapping group and fixture cue values must compile");
}
