use super::*;
use light_core::{ReplacementHeadTarget, ReplacementProfileContext, ReplacementProgramProjection};

fn envelope(root: FixtureId, child: FixtureId) -> ReplacementProgramProjection {
    let profile = ReplacementProfileContext {
        profile_id: FixtureId::new(),
        profile_revision: 1,
        mode_id: Uuid::new_v4(),
    };
    ReplacementProgramProjection {
        source_owner: root,
        source_profile: profile.clone(),
        source_head_id: Uuid::new_v4(),
        target_profile: profile,
        targets: vec![ReplacementHeadTarget {
            profile_head_id: Uuid::new_v4(),
            fixture_id: child,
        }],
    }
}

#[test]
fn replacement_projection_tracks_effective_child_and_keeps_fresh_root_independent() {
    let root = FixtureId::new();
    let child = FixtureId::new();
    let projection = envelope(root, child);
    let mut first = Cue::new(cue_number(1.0));
    let mut projected = value(child, "intensity", 0.8);
    projected.replacement_projection = Some(projection.clone());
    first.changes.push(projected);
    let mut second = Cue::new(cue_number(2.0));
    second.changes.push(value(root, "intensity", 0.3));
    let mut third = Cue::new(cue_number(3.0));
    third.changes.push(value(child, "intensity", 0.8));
    let cues = list(vec![first, second, third]);
    let id = cues.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cues).unwrap();
    let start = Utc::now();
    engine.go_at(id, start).unwrap();
    engine.go_at(id, start).unwrap();
    let contributions = engine.contributions_with_context(start, None);
    let child_sample = contributions
        .iter()
        .find(|v| v.value.fixture_id == child)
        .unwrap();
    assert_eq!(
        child_sample.replacement_projection,
        Some(projection.clone())
    );
    assert_eq!(child_sample.value.value.normalized(), Some(0.8));
    let root_sample = contributions
        .iter()
        .find(|v| v.value.fixture_id == root)
        .unwrap();
    assert_eq!(root_sample.replacement_projection, None);
    assert_eq!(root_sample.value.value.normalized(), Some(0.3));
    let retained = PlaybackRetainedValue::from(child_sample.clone());
    assert_eq!(
        serde_json::from_str::<PlaybackRetainedValue>(&serde_json::to_string(&retained).unwrap())
            .unwrap(),
        retained
    );
    engine.go_at(id, start).unwrap();
    let explicit = engine
        .contributions_with_context(start, None)
        .into_iter()
        .find(|v| v.value.fixture_id == child)
        .unwrap();
    assert_eq!(
        explicit.replacement_projection, None,
        "equal fresh child edit detaches old projection"
    );
}

#[test]
fn replacement_projection_status_transfer_preserves_authored_root_and_group_envelopes() {
    let root = FixtureId::new();
    let projection = envelope(root, FixtureId::new());
    let mut first = Cue::new(cue_number(1.0));
    let mut change = value(root, "intensity", 0.4);
    change.replacement_projection = Some(projection.clone());
    first.changes.push(change);
    first.group_changes.push(GroupCueChange {
        group_id: "1".into(),
        attribute: AttributeKey::intensity(),
        value: Some(AttributeValue::Normalized(0.6)),
        preset_reference: None,
        replacement_projections: HashMap::from([(root, projection.clone())]),
        automatic_restore: false,
        fade_millis: Some(700),
        delay_millis: None,
    });
    let source = list(vec![first, Cue::new(cue_number(2.0))]);
    let status = transferred_cue(&source, 1, cue_number(9.0), CueTransferMode::Status).unwrap();
    assert_eq!(status.changes[0].fixture_id, root);
    assert_eq!(
        status.changes[0].replacement_projection,
        Some(projection.clone())
    );
    assert_eq!(
        status.group_changes[0].replacement_projections.get(&root),
        Some(&projection)
    );
    let plain = transferred_cue(&source, 0, cue_number(9.0), CueTransferMode::Plain).unwrap();
    assert_eq!(plain.group_changes[0].fade_millis, Some(700));
    assert_eq!(plain.changes[0].replacement_projection, Some(projection));
}

#[test]
fn replacement_projection_record_merge_and_add_missing_preserve_address_ownership() {
    let roots = [FixtureId::new(), FixtureId::new()];
    let mut first = Cue::new(cue_number(1.0));
    for root in roots {
        let mut change = value(root, "intensity", 0.4);
        change.replacement_projection = Some(envelope(root, FixtureId::new()));
        first.changes.push(change);
    }
    let original = list(vec![first]);
    let fresh = CueRecordingContent {
        changes: vec![value(roots[0], "intensity", 0.4)],
        ..Default::default()
    };
    let added = original
        .plan_recording(
            fresh.clone(),
            CueRecordOperation::AddMissing {
                cue_number: cue_number(1.0),
            },
        )
        .unwrap();
    assert!(!added.changed);
    assert_eq!(added.cue_list, original);
    let merged = original
        .plan_recording(
            fresh.clone(),
            CueRecordOperation::Merge {
                cue_number: cue_number(1.0),
            },
        )
        .unwrap();
    assert!(
        merged.changed,
        "equal scalar explicit edit must detach old source envelope"
    );
    assert_eq!(merged.cue_list.cues[0].id, original.cues[0].id);
    assert!(
        merged.cue_list.cues[0]
            .changes
            .iter()
            .find(|change| change.fixture_id == roots[0])
            .unwrap()
            .replacement_projection
            .is_none()
    );
    assert_eq!(
        merged.cue_list.cues[0]
            .changes
            .iter()
            .find(|change| change.fixture_id == roots[1]),
        original.cues[0]
            .changes
            .iter()
            .find(|change| change.fixture_id == roots[1])
    );
    let replaced = original
        .plan_recording(
            fresh,
            CueRecordOperation::Overwrite {
                cue_number: cue_number(1.0),
            },
        )
        .unwrap();
    assert_eq!(replaced.cue_list.cues[0].changes.len(), 1);
    assert!(
        replaced.cue_list.cues[0].changes[0]
            .replacement_projection
            .is_none()
    );
}

#[test]
fn replacement_projection_cue_only_restoration_carries_baseline_and_introduced_release() {
    let root = FixtureId::new();
    let projection = envelope(root, FixtureId::new());
    let mut baseline = Cue::new(cue_number(1.0));
    let mut change = value(root, "intensity", 0.4);
    change.replacement_projection = Some(projection.clone());
    baseline.changes.push(change);
    let mut temporary = Cue::new(cue_number(2.0));
    temporary.cue_only = true;
    let mut changed = value(root, "intensity", 0.8);
    changed.replacement_projection = Some(projection.clone());
    temporary.changes.push(changed);
    for has_baseline in [true, false] {
        let mut cues = list(if has_baseline {
            vec![
                baseline.clone(),
                temporary.clone(),
                Cue::new(cue_number(3.0)),
            ]
        } else {
            vec![temporary.clone(), Cue::new(cue_number(3.0))]
        });
        refresh_cue_only_restorations(&mut cues);
        let restored = &cues.cues.last().unwrap().changes[0];
        assert_eq!(restored.replacement_projection, Some(projection.clone()));
        assert_eq!(
            restored.value,
            has_baseline.then(|| AttributeValue::Normalized(0.4))
        );
        let once = cues.clone();
        refresh_cue_only_restorations(&mut cues);
        assert_eq!(cues, once);
    }
}

fn migration(root: FixtureId, heads: &[FixtureId]) -> light_core::ReplacementRuntimeMigration {
    let projection = envelope(root, heads[0]);
    let mut projected = projection.clone();
    projected.target_profile.profile_revision += 1;
    projected.target_profile.mode_id = Uuid::new_v4();
    projected.targets = heads
        .iter()
        .map(|fixture| ReplacementHeadTarget {
            profile_head_id: Uuid::new_v4(),
            fixture_id: *fixture,
        })
        .collect();
    light_core::ReplacementRuntimeMigration {
        source_head_owners: HashMap::from([(projection.source_head_id, root)]),
        source_owner: root,
        source_profile: projection.source_profile,
        target_profile: projected.target_profile.clone(),
        root_attributes: HashSet::from([AttributeKey("focus".into())]),
        root_projections: HashMap::from([(AttributeKey("focus".into()), projected)]),
        head_targets: HashMap::new(),
    }
}

#[test]
fn replacement_projection_runtime_migration_keeps_interrupted_samples_clocks_and_unknown_seed() {
    let root = FixtureId::new();
    let heads = [FixtureId::new(), FixtureId::new()];
    let mut cues = Vec::new();
    for (number, level) in [(1.0, 0.2), (2.0, 0.8), (3.0, 0.4)] {
        let mut cue = Cue::new(cue_number(number));
        cue.fade_millis = if number == 1.0 { 0 } else { 1000 };
        cue.changes.push(value(root, "focus", level));
        cues.push(cue);
    }
    let list = list(cues);
    let id = list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(list).unwrap();
    let start = Utc::now();
    engine.go_at(id, start).unwrap();
    engine.go_at(id, start).unwrap();
    engine
        .go_at(id, start + ChronoDuration::milliseconds(500))
        .unwrap();
    let before = engine.active.values().next().unwrap().clone();
    let sample = before.deleted_cue_transition_source.as_ref().unwrap()[0].clone();
    let plan = migration(root, &heads);
    engine.apply_replacement_runtime_migration(&plan).unwrap();
    let after = engine.active.values().next().unwrap();
    assert_eq!(after.transition_ordinal, before.transition_ordinal);
    assert_eq!(after.activated_at, before.activated_at);
    assert_eq!(
        after.source_history.as_ref().unwrap().action_ordinal(),
        before.source_history.as_ref().unwrap().action_ordinal()
    );
    let rows = after.deleted_cue_transition_source.as_ref().unwrap();
    assert_eq!(rows.len(), 2);
    for (row, head) in rows.iter().zip(heads) {
        assert_eq!(row.timed.fixture_id, head);
        assert_eq!(row.timed.value, sample.timed.value);
        assert_eq!(row.timed.changed_at, sample.timed.changed_at);
        assert_eq!(row.family_evidence, sample.family_evidence);
        assert_eq!(
            row.replacement_projection.as_ref().unwrap().source_owner,
            root
        );
    }
    // Captured physical start values have intentionally unknown Cue evidence.
    let active = engine.active.values_mut().next().unwrap();
    let mut unknown = sample.clone();
    unknown.family_evidence = None;
    active.deleted_cue_transition_source = Some(vec![unknown]);
    engine.apply_replacement_runtime_migration(&plan).unwrap();
    assert!(
        engine
            .active
            .values()
            .next()
            .unwrap()
            .deleted_cue_transition_source
            .as_ref()
            .unwrap()
            .iter()
            .all(|row| row.family_evidence.is_none())
    );
}

#[test]
fn replacement_projection_runtime_migration_is_atomic_and_does_not_fan_out_effective_children() {
    let root = FixtureId::new();
    let old_child = FixtureId::new();
    let heads = [FixtureId::new(), FixtureId::new()];
    let mut cue = Cue::new(cue_number(1.0));
    cue.changes.push(value(root, "focus", 0.5));
    let list = list(vec![cue]);
    let id = list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(list).unwrap();
    let now = Utc::now();
    engine.go_at(id, now).unwrap();
    let sample =
        PlaybackRetainedValue::from(engine.contributions_with_context(now, None).remove(0));
    engine
        .active
        .values_mut()
        .next()
        .unwrap()
        .deleted_cue_transition_source = Some(vec![sample.clone()]);
    let mut plan = migration(root, &heads);
    let valid = plan.clone();
    plan.root_projections.clear();
    let before = engine.active.values().next().unwrap().clone();
    assert!(engine.apply_replacement_runtime_migration(&plan).is_err());
    assert_eq!(engine.active.values().next().unwrap(), &before);
    let mut existing = valid.root_projections[&AttributeKey("focus".into())].clone();
    existing.target_profile = valid.source_profile.clone();
    existing.targets = vec![ReplacementHeadTarget {
        profile_head_id: Uuid::new_v4(),
        fixture_id: old_child,
    }];
    let old_head = existing.targets[0].profile_head_id;
    let mut child = sample;
    child.timed.fixture_id = old_child;
    child.replacement_projection = Some(existing);
    engine
        .active
        .values_mut()
        .next()
        .unwrap()
        .deleted_cue_transition_source = Some(vec![child]);
    plan = valid;
    plan.head_targets.insert(
        old_child,
        ReplacementHeadTarget {
            profile_head_id: Uuid::new_v4(),
            fixture_id: heads[1],
        },
    );
    plan.source_head_owners.insert(old_head, old_child);
    engine.apply_replacement_runtime_migration(&plan).unwrap();
    let rows = engine
        .active
        .values()
        .next()
        .unwrap()
        .deleted_cue_transition_source
        .as_ref()
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].timed.fixture_id, heads[1]);
    assert_eq!(
        rows[0]
            .replacement_projection
            .as_ref()
            .unwrap()
            .target_profile,
        plan.target_profile
    );
    engine.apply_replacement_runtime_migration(&plan).unwrap();
    assert_eq!(
        engine
            .active
            .values()
            .next()
            .unwrap()
            .deleted_cue_transition_source
            .as_ref()
            .unwrap()
            .len(),
        1
    );
}
