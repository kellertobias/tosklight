//! Live-Group multi-point spread recall across ordered-membership edits (plan-50, coverage 7).
//!
//! A live Group retains its `AttributeValue::Spread` control points; the engine re-resolves them
//! against the group's *current* ordered membership through the shared deterministic anchor rule
//! (`light_core::resolve_spread`) every time the snapshot changes. These tests prove the
//! normalized values first, then the quantized DMX bytes, for the direct programmer path and the
//! Cue playback path.

use super::*;

/// One-channel dimmers patched to universe 1, addresses 1..=count, in patch order.
fn dimmer_rig(count: u16) -> (Vec<PatchedFixture>, Vec<FixtureId>) {
    (1..=count)
        .map(|address| {
            let (mut patched, logical) = fixture();
            patched.address = Some(address);
            (patched, logical)
        })
        .unzip()
}

fn assert_resolved_intensities(engine: &Engine, members: &[FixtureId], expected: &[f32]) {
    let resolved = engine.resolved_values();
    assert_eq!(members.len(), expected.len());
    for (fixture_id, expected) in members.iter().zip(expected) {
        assert_eq!(
            normalized(&resolved, *fixture_id, "intensity"),
            *expected,
            "normalized value for ordered member {fixture_id:?}"
        );
    }
}

#[test]
fn live_group_spread_re_resolves_after_membership_add_remove_and_reorder() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    // The stored value keeps its control points; only membership changes between renders.
    programmers.set_group(
        session,
        "wave".into(),
        AttributeKey::intensity(),
        AttributeValue::Spread(vec![1.0, 0.0, 1.0]),
    );
    let (patched, logical) = dimmer_rig(6);
    let engine = Engine::new(programmers);
    let snapshot = |members: Vec<FixtureId>, revision| EngineSnapshot {
        fixtures: patched.clone().into(),
        groups: vec![GroupDefinition {
            id: "wave".into(),
            name: "Wave".into(),
            fixtures: members,
            ..Default::default()
        }]
        .into(),
        revision,
        ..Default::default()
    };

    // Five ordered members: 100, 50, 0, 50, 100 (normative table).
    engine
        .replace_snapshot(snapshot(logical[..5].to_vec(), 1))
        .unwrap();
    assert_resolved_intensities(&engine, &logical[..5], &[1.0, 0.5, 0.0, 0.5, 1.0]);
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(rendered.universes[&1][..6], [255, 128, 0, 128, 255, 0]);

    // Grow to six members: the interior anchor lands exactly between items 2 and 3, so the
    // stored control points re-resolve to 100, 50, 0, 0, 50, 100 against the new membership.
    engine
        .replace_snapshot(snapshot(logical.clone(), 2))
        .unwrap();
    assert_resolved_intensities(&engine, &logical, &[1.0, 0.5, 0.0, 0.0, 0.5, 1.0]);
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(rendered.universes[&1][..6], [255, 128, 0, 0, 128, 255]);

    // Reorder the same members: the spread follows the stored ordered membership, not the patch.
    let reordered = vec![logical[2], logical[0], logical[4], logical[1], logical[3]];
    engine
        .replace_snapshot(snapshot(reordered.clone(), 3))
        .unwrap();
    assert_resolved_intensities(&engine, &reordered, &[1.0, 0.5, 0.0, 0.5, 1.0]);
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(rendered.universes[&1][..6], [128, 128, 255, 255, 0, 0]);

    // Remove down to four members: the interior anchor expands across both middle items.
    engine
        .replace_snapshot(snapshot(logical[..4].to_vec(), 4))
        .unwrap();
    assert_resolved_intensities(&engine, &logical[..4], &[1.0, 0.0, 0.0, 1.0]);
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(rendered.universes[&1][..6], [255, 0, 0, 255, 0, 0]);
}

#[test]
fn cue_group_spread_re_resolves_against_current_membership_on_recall() {
    let programmers = ProgrammerRegistry::default();
    let (patched, logical) = dimmer_rig(6);
    let list_id = light_core::CueListId::new();
    let mut cue = light_playback::Cue::new(1.0);
    cue.group_changes.push(light_playback::GroupCueChange {
        group_id: "wave".into(),
        attribute: AttributeKey::intensity(),
        value: Some(AttributeValue::Spread(vec![1.0, 0.0, 1.0])),
        fade_millis: None,
        delay_millis: None,
        automatic_restore: false,
    });
    let list = light_playback::CueList {
        id: list_id,
        name: "Wave".into(),
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
    let snapshot = |members: Vec<FixtureId>, revision| EngineSnapshot {
        fixtures: patched.clone().into(),
        cue_lists: vec![list.clone()].into(),
        groups: vec![GroupDefinition {
            id: "wave".into(),
            name: "Wave".into(),
            fixtures: members,
            ..Default::default()
        }]
        .into(),
        revision,
        ..Default::default()
    };
    engine
        .replace_snapshot(snapshot(logical[..5].to_vec(), 1))
        .unwrap();
    execute_cue_list(
        &engine,
        list_id,
        CueListPlaybackAction::GoAt(Utc::now() - ChronoDuration::milliseconds(1)),
    );
    // Recall before the membership edit: five ordered members resolve 100, 50, 0, 50, 100.
    assert_resolved_intensities(&engine, &logical[..5], &[1.0, 0.5, 0.0, 0.5, 1.0]);
    let before = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(before.universes[&1][..6], [255, 128, 0, 128, 255, 0]);

    // Membership grows while the cue stays active: the stored control points re-resolve against
    // the new six-member ordered membership without re-recording the cue.
    engine
        .replace_snapshot(snapshot(logical.clone(), 2))
        .unwrap();
    assert_resolved_intensities(&engine, &logical, &[1.0, 0.5, 0.0, 0.0, 0.5, 1.0]);
    let after = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(after.universes[&1][..6], [255, 128, 0, 0, 128, 255]);
}
