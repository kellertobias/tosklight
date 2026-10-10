use super::*;
use light_core::{ReplacementHeadTarget, ReplacementProfileContext, ReplacementProgramProjection};

fn replacement_fixture() -> (PatchedFixture, ReplacementProgramProjection) {
    let (mut fixture, _) = fixture();
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let mode = &mut profile.modes[0];
    let mut head = mode.heads[0].clone();
    head.id = uuid::Uuid::new_v4();
    head.name = "Second emitter".into();
    let mut channel = mode.channels[0].clone();
    channel.id = uuid::Uuid::new_v4();
    channel.head_id = head.id;
    mode.heads.push(head);
    mode.channels.push(channel);
    mode.splits[0].footprint = 2;
    let mode_id = mode.id;
    fixture.logical_heads.push(PatchedHead {
        fixture_id: FixtureId::new(),
        head_index: 1,
        profile_head_id: Some(mode.heads[1].id),
    });
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    let context = ReplacementProfileContext {
        profile_id: profile.id,
        profile_revision: profile.revision.into(),
        mode_id,
    };
    let projection = ReplacementProgramProjection {
        source_owner: fixture.fixture_id,
        source_profile: ReplacementProfileContext {
            profile_id: FixtureId::new(),
            profile_revision: 1,
            mode_id: uuid::Uuid::new_v4(),
        },
        source_head_id: uuid::Uuid::new_v4(),
        target_profile: context,
        targets: fixture
            .logical_heads
            .iter()
            .map(|head| ReplacementHeadTarget {
                profile_head_id: head.profile_head_id.unwrap(),
                fixture_id: head.fixture_id,
            })
            .collect(),
    };
    (fixture, projection)
}

fn projection_engine(programmers: ProgrammerRegistry) -> Engine {
    Engine::with_programming_contract_support(
        programmers,
        light_core::programming::REPLACEMENT_PROGRAM_PROJECTION_CONTRACT,
    )
}

#[test]
fn replacement_projection_stored_cue_routes_two_heads_but_new_master_is_independent() {
    let (fixture, projection) = replacement_fixture();
    let root = fixture.fixture_id;
    let mut old = CueChange::set(
        root,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    old.replacement_projection = Some(projection.clone());
    let mut list = test_cue_list("Stored root source", vec![old]);
    let mut master = Cue::new(2_u16.into());
    master.changes.push(CueChange::set(
        root,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    ));
    list.cues.push(master);
    let id = list.id;
    let engine = projection_engine(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            required_programming_contract: 4,
            fixtures: vec![fixture].into(),
            cue_lists: vec![list].into(),
            ..Default::default()
        })
        .unwrap();
    execute_cue_list(
        &engine,
        id,
        CueListPlaybackAction::GoAt(Utc::now() - ChronoDuration::milliseconds(1)),
    );
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..2],
        &[128, 128]
    );
    execute_cue_list(
        &engine,
        id,
        CueListPlaybackAction::GoAt(Utc::now() - ChronoDuration::milliseconds(1)),
    );
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..2],
        &[128, 128],
        "fresh root master does not replace independently tracked emitters"
    );
    let held = engine.playback_contributions_at(Utc::now());
    assert!(held.iter().any(|value| value.value.fixture_id == root));
    assert!(projection.targets.iter().all(|head| {
        held.iter()
            .any(|value| value.value.fixture_id == head.fixture_id)
    }));
}

#[test]
fn replacement_projection_live_group_preserves_original_rank_and_membership_refresh() {
    let (fixture, projection) = replacement_fixture();
    let (mut peer, peer_head) = super::fixture();
    peer.address = Some(3);
    let root = fixture.fixture_id;
    let mut list = test_cue_list("Live replacement Group", vec![]);
    list.cues[0]
        .group_changes
        .push(light_playback::GroupCueChange {
            replacement_projections: HashMap::from([(root, projection)]),
            preset_reference: None,
            group_id: "live".into(),
            attribute: AttributeKey::intensity(),
            value: Some(AttributeValue::Spread(vec![0.2, 0.8])),
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        });
    let id = list.id;
    let snapshot = |members| EngineSnapshot {
        required_programming_contract: 4,
        fixtures: vec![fixture.clone(), peer.clone()].into(),
        cue_lists: vec![list.clone()].into(),
        groups: vec![GroupDefinition {
            id: "live".into(),
            fixtures: members,
            ..Default::default()
        }]
        .into(),
        ..Default::default()
    };
    let engine = projection_engine(ProgrammerRegistry::default());
    engine
        .replace_snapshot(snapshot(vec![root, peer_head]))
        .unwrap();
    execute_cue_list(
        &engine,
        id,
        CueListPlaybackAction::GoAt(Utc::now() - ChronoDuration::milliseconds(1)),
    );
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..3],
        &[51, 51, 204]
    );
    engine.replace_snapshot(snapshot(vec![peer_head])).unwrap();
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..3],
        &[0, 0, 51]
    );
    engine
        .replace_snapshot(snapshot(vec![peer_head, root]))
        .unwrap();
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..3],
        &[204, 204, 51]
    );
    assert_eq!(engine.snapshot().groups[0].fixtures, vec![peer_head, root]);
}

#[test]
fn replacement_projection_programmer_priority_provenance_and_identical_master_edit() {
    let (fixture, projection) = replacement_fixture();
    let root = fixture.fixture_id;
    let first = projection.targets[0].fixture_id;
    let second = projection.targets[1].fixture_id;
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    registry.set(
        session,
        root,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.4),
    );
    let order = registry.get(session).unwrap().values[0].programmer_order;
    assert!(registry.attach_replacement_provenance(
        session,
        &[(order, HashMap::from([(root, projection)]))],
        false,
        false
    ));
    let engine = projection_engine(registry.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            required_programming_contract: 4,
            fixtures: vec![fixture].into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..2],
        &[102, 102]
    );
    let captured = engine.prepare_observer_frame(Default::default());
    let observed = engine.observe_prepared_frame(&captured, &[]);
    let origin = observed
        .values()
        .contribution_origin(second, &AttributeKey::intensity())
        .unwrap();
    assert_eq!(origin.authored_fixture_id(), root);
    assert_eq!(origin.stamp().programmer_order, order);
    registry.set(
        session,
        first,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..2],
        &[204, 102]
    );
    registry.set(
        session,
        root,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.4),
    );
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..2],
        &[204, 0],
        "identical fresh master edit detaches the old projection"
    );
}

#[test]
fn replacement_projection_release_uses_authored_history_and_cue_only_effective_addresses() {
    let (fixture, projection) = replacement_fixture();
    let root = fixture.fixture_id;
    let mut original = CueChange::set(
        root,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    original.replacement_projection = Some(projection.clone());
    let mut list = test_cue_list("Projected source lifecycle", vec![original]);
    let mut master = Cue::new(2_u16.into());
    master.cue_only = true;
    master.changes.push(CueChange::set(
        root,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    ));
    list.cues.push(master);
    list.cues.push(Cue::new(3_u16.into()));
    let mut release = Cue::new(4_u16.into());
    let mut released = CueChange::set(
        root,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.0),
    );
    released.value = None;
    release.changes.push(released);
    list.cues.push(release);
    let plan = ReplacementDestinationPlan::compile(&[fixture.clone()]).unwrap();
    let expanded = crate::lifecycle::expand_group_references_for_preview(
        &list,
        &HashMap::new(),
        &HashMap::new(),
        Some(&plan),
    );
    assert!(
        expanded.cues[2]
            .changes
            .iter()
            .any(|change| change.fixture_id == root
                && change.value.is_none()
                && change.automatic_restore)
    );
    assert!(!expanded.cues[2].changes.iter().any(|change| {
        projection
            .targets
            .iter()
            .any(|target| target.fixture_id == change.fixture_id)
    }));
    assert_eq!(expanded.cues[3].changes.len(), 1);
    assert_eq!(
        expanded.cues[3].changes[0].fixture_id, root,
        "fresh root value breaks projected source history"
    );
    let mut only_projected =
        test_cue_list("Projected release", vec![list.cues[0].changes[0].clone()]);
    let mut release = Cue::new(2_u16.into());
    let mut released = CueChange::set(
        root,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.0),
    );
    released.value = None;
    release.changes.push(released);
    only_projected.cues.push(release);
    let expanded = crate::lifecycle::expand_group_references_for_preview(
        &only_projected,
        &HashMap::new(),
        &HashMap::new(),
        Some(&plan),
    );
    assert_eq!(expanded.cues[1].changes.len(), 2);
    assert!(
        expanded.cues[1]
            .changes
            .iter()
            .all(|change| change.value.is_none()
                && change.replacement_projection.as_ref() == Some(&projection))
    );
    let id = only_projected.id;
    let engine = projection_engine(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            required_programming_contract: 4,
            fixtures: vec![fixture].into(),
            cue_lists: vec![only_projected].into(),
            ..Default::default()
        })
        .unwrap();
    execute_cue_list(
        &engine,
        id,
        CueListPlaybackAction::GoAt(Utc::now() - ChronoDuration::milliseconds(1)),
    );
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..2],
        &[128, 128]
    );
    execute_cue_list(
        &engine,
        id,
        CueListPlaybackAction::GoAt(Utc::now() - ChronoDuration::milliseconds(1)),
    );
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..2],
        &[0, 0]
    );
}

fn shared_source_fixture(
    next: &PatchedFixture,
    projection: &mut ReplacementProgramProjection,
) -> PatchedFixture {
    let mut old = next.clone();
    let mut profile = next.definition.profile_snapshot.as_deref().unwrap().clone();
    profile.id = FixtureId::new();
    let mode = &mut profile.modes[0];
    mode.id = uuid::Uuid::new_v4();
    mode.heads.truncate(1);
    mode.heads[0].id = uuid::Uuid::new_v4();
    mode.heads[0].master_shared = true;
    mode.channels.truncate(1);
    mode.channels[0].head_id = mode.heads[0].id;
    mode.splits[0].footprint = 1;
    let id = mode.id;
    projection.source_profile = ReplacementProfileContext {
        profile_id: profile.id,
        profile_revision: profile.revision.into(),
        mode_id: id,
    };
    projection.source_head_id = mode.heads[0].id;
    old.definition = profile.resolved_definition(id).unwrap();
    old.logical_heads.clear();
    old
}

#[test]
fn replacement_projection_running_shared_source_recompiles_without_restart() {
    let (next, mut projection) = replacement_fixture();
    let old = shared_source_fixture(&next, &mut projection);
    let mut list = test_cue_list(
        "Running old shared root",
        vec![CueChange::set(
            old.fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        )],
    );
    let id = list.id;
    let engine = projection_engine(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            revision: 1,
            fixtures: vec![old].into(),
            cue_lists: vec![list.clone()].into(),
            ..Default::default()
        })
        .unwrap();
    execute_cue_list(
        &engine,
        id,
        CueListPlaybackAction::GoAt(Utc::now() - ChronoDuration::milliseconds(1)),
    );
    assert_eq!(
        engine.render(Default::default()).unwrap().universes[&1][0],
        128
    );
    list.cues[0].changes[0].replacement_projection = Some(projection);
    engine
        .replace_snapshot(EngineSnapshot {
            revision: 2,
            required_programming_contract: 4,
            fixtures: vec![next].into(),
            cue_lists: vec![list].into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..2],
        &[128, 128]
    );
}

#[test]
fn replacement_projection_interrupted_transition_migrates_atomically_without_advancing_clock() {
    let (next, mut projection) = replacement_fixture();
    let old = shared_source_fixture(&next, &mut projection);
    let root = old.fixture_id;
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 10, 9, 9, 0, 0).unwrap(),
    ));
    let engine = projection_engine(ProgrammerRegistry::with_clock(clock.clone()));
    let mut list = test_cue_list(
        "Interrupted stored root",
        vec![CueChange::set(
            root,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.2),
        )],
    );
    let mut second = Cue::new(2_u16.into());
    let mut change = CueChange::set(
        root,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    change.fade_millis = Some(1000);
    second.changes.push(change);
    list.cues.push(second);
    let mut third = Cue::new(3_u16.into());
    let mut change = CueChange::set(
        root,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.0),
    );
    change.fade_millis = Some(1000);
    third.changes.push(change);
    list.cues.push(third);
    let id = list.id;
    engine
        .replace_snapshot(EngineSnapshot {
            revision: 1,
            fixtures: vec![old].into(),
            cue_lists: vec![list.clone()].into(),
            ..Default::default()
        })
        .unwrap();
    execute_cue_list(&engine, id, CueListPlaybackAction::GoAt(clock.now()));
    clock.advance_millis(1);
    execute_cue_list(&engine, id, CueListPlaybackAction::GoAt(clock.now()));
    clock.advance_millis(500);
    assert_eq!(
        engine.render(Default::default()).unwrap().universes[&1][0],
        128
    );
    execute_cue_list(&engine, id, CueListPlaybackAction::GoAt(clock.now()));
    let before = engine.render(Default::default()).unwrap().universes[&1][0];
    for cue in &mut list.cues {
        cue.changes[0].replacement_projection = Some(projection.clone());
    }
    let candidate = EngineSnapshot {
        revision: 2,
        required_programming_contract: 4,
        fixtures: vec![next].into(),
        cue_lists: vec![list].into(),
        ..Default::default()
    };
    let mut plan = light_core::ReplacementRuntimeMigration {
        source_owner: root,
        source_profile: projection.source_profile.clone(),
        target_profile: projection.target_profile.clone(),
        root_attributes: [AttributeKey::intensity()].into(),
        root_projections: HashMap::new(),
        head_targets: HashMap::new(),
        source_head_owners: [(projection.source_head_id, root)].into(),
    };
    let refused = engine.finalize_snapshot_playback_with_replacement_migrations(
        engine.prepare_snapshot(candidate.clone()).unwrap(),
        &[plan.clone()],
    );
    assert!(refused.is_err());
    assert_eq!(engine.snapshot().revision, 1);
    assert_eq!(
        engine.render(Default::default()).unwrap().universes[&1][0],
        before
    );
    plan.root_projections
        .insert(AttributeKey::intensity(), projection);
    let finalized = engine
        .finalize_snapshot_playback_with_replacement_migrations(
            engine.prepare_snapshot(candidate).unwrap(),
            &[plan],
        )
        .unwrap();
    // Finalization remains detached: a failed persistence callback must never install it.
    assert_eq!(engine.snapshot().revision, 1);
    assert_eq!(
        engine.render(Default::default()).unwrap().universes[&1][0],
        before
    );
    engine.install_finalized_snapshot(finalized);
    assert_eq!(
        &engine.render(Default::default()).unwrap().universes[&1][..2],
        &[before, before]
    );
    clock.advance_millis(500);
    let frame = engine.render(Default::default()).unwrap();
    assert_eq!(&frame.universes[&1][..2], &[64, 64]);
}
