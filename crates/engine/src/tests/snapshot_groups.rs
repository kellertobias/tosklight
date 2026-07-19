use super::*;

#[test]
fn active_group_cue_survives_snapshot_swap_and_gains_new_members() {
    let programmers = ProgrammerRegistry::default();
    let (first, first_logical) = fixture();
    let (mut second, second_logical) = fixture();
    second.address = Some(2);
    let list_id = light_core::CueListId::new();
    let mut cue = light_playback::Cue::new(1.0);
    cue.group_changes.push(light_playback::GroupCueChange {
        group_id: "live".into(),
        attribute: AttributeKey::intensity(),
        value: Some(AttributeValue::Normalized(0.6)),
        fade_millis: None,
        delay_millis: None,
        automatic_restore: false,
    });
    let list = light_playback::CueList {
        id: list_id,
        name: "Live group".into(),
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
    let snapshot = |members| EngineSnapshot {
        fixtures: vec![first.clone(), second.clone()],
        cue_lists: vec![list.clone()],
        groups: vec![GroupDefinition {
            id: "live".into(),
            name: "Live".into(),
            fixtures: members,
            master: 0.5,
            playback_fader: Some(1),
            ..Default::default()
        }],
        ..Default::default()
    };
    engine
        .replace_snapshot(snapshot(vec![first_logical]))
        .unwrap();
    execute_cue_list(
        &engine,
        list_id,
        CueListPlaybackAction::GoAt(chrono::Utc::now() - chrono::Duration::milliseconds(1)),
    );
    let playback_values = engine
        .playback_contributions_at(chrono::Utc::now())
        .into_iter()
        .map(|contribution| contribution.value)
        .collect::<Vec<_>>();
    assert!(
        playback_values
            .iter()
            .any(|value| value.fixture_id == first_logical
                && value.attribute.is_intensity()
                && value.value.normalized().is_some_and(|level| level > 0.59))
    );
    let before = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(before.universes[&1][0], 77);
    assert_eq!(before.universes[&1][1], 0);
    engine
        .replace_snapshot(snapshot(vec![first_logical, second_logical]))
        .unwrap();
    assert_eq!(engine.active_playbacks().len(), 1);
    let after = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(after.universes[&1][0], 77);
    assert_eq!(after.universes[&1][1], 77);
    engine
        .replace_snapshot_releasing_playback(snapshot(vec![first_logical, second_logical]))
        .unwrap();
    assert!(engine.active_playbacks().is_empty());
}

#[test]
fn unpatched_group_member_keeps_programming_but_outputs_no_dmx() {
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session, light_core::UserId::new());
    programmers.set_group(
        session,
        "look".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let (patched, patched_logical) = fixture();
    let (mut unpatched, unpatched_logical) = fixture();
    unpatched.universe = None;
    unpatched.address = None;
    let group = GroupDefinition {
        id: "look".into(),
        name: "Look".into(),
        fixtures: vec![patched_logical, unpatched_logical],
        master: 1.0,
        playback_fader: None,
        ..Default::default()
    };
    let snapshot = |unpatched_fixture: PatchedFixture| EngineSnapshot {
        fixtures: vec![patched.clone(), unpatched_fixture],
        cue_lists: vec![],
        playbacks: vec![],
        playback_pages: vec![],
        routes: vec![],
        control_mappings: vec![],
        groups: vec![group.clone()],
        revision: 1,
    };
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(snapshot(unpatched.clone()))
        .unwrap();
    let resolved = engine.resolved_values();
    assert_eq!(
        resolved
            .get(&(unpatched_logical, AttributeKey::intensity()))
            .and_then(AttributeValue::normalized),
        Some(0.5),
    );
    assert_eq!(group.fixtures, vec![patched_logical, unpatched_logical]);
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(rendered.universes[&1][0], 128);
    assert_eq!(rendered.universes[&1][1], 0);

    unpatched.universe = Some(1);
    unpatched.address = Some(2);
    engine.replace_snapshot(snapshot(unpatched)).unwrap();
    let repatched = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(repatched.universes[&1][0], 128);
    assert_eq!(repatched.universes[&1][1], 128);
}
