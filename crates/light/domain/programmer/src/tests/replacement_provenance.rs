use super::*;

fn projection(source: FixtureId) -> light_core::ReplacementProgramProjection {
    let profile = light_core::ReplacementProfileContext {
        profile_id: FixtureId::new(),
        profile_revision: 1,
        mode_id: uuid::Uuid::new_v4(),
    };
    light_core::ReplacementProgramProjection {
        source_owner: source,
        source_profile: profile.clone(),
        source_head_id: uuid::Uuid::new_v4(),
        target_profile: profile,
        targets: vec![light_core::ReplacementHeadTarget {
            profile_head_id: uuid::Uuid::new_v4(),
            fixture_id: FixtureId::new(),
        }],
    }
}

fn physical_origins(
    snapshot: &crate::ProgrammerSnapshot,
    owner: FixtureId,
    metadata: &light_core::ReplacementProgramProjection,
) -> Vec<(u64, light_core::ReplacementProjectionMap)> {
    snapshot
        .values
        .iter()
        .chain(snapshot.preload_pending.iter())
        .chain(snapshot.preload_active.iter())
        .filter(|value| value.fixture_id == owner)
        .map(|value| {
            (
                value.programmer_order,
                HashMap::from([(owner, metadata.clone())]),
            )
        })
        .collect()
}

#[test]
fn replacement_physical_history_only_clear_and_preload_preserve_routing_across_fresh_edit_undo() {
    for preload in [false, true] {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let owner = FixtureId::new();
        registry.start(session);
        if preload {
            assert!(registry.arm_preload(session, true));
        }
        registry.set(
            session,
            owner,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.2),
        );
        registry.set(
            session,
            owner,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.4),
        );
        if !preload {
            assert!(registry.clear_normal_values(session));
        }
        let before = registry.get(session).unwrap();
        let metadata = projection(owner);
        let live = physical_origins(&before.snapshot(), owner, &metadata);
        let undo = before
            .undo
            .iter()
            .map(|snapshot| physical_origins(snapshot, owner, &metadata))
            .collect::<Vec<_>>();
        let redo = before
            .redo
            .iter()
            .map(|snapshot| physical_origins(snapshot, owner, &metadata))
            .collect::<Vec<_>>();
        assert!(
            registry
                .apply_replacement_migration_with_history(before.id, &live, &undo, &redo)
                .unwrap()
        );
        assert_eq!(registry.get(session).unwrap().undo.len(), before.undo.len());
        assert!(registry.undo(session));
        let restored = registry.get(session).unwrap();
        let values = if preload {
            restored.preload_pending.as_slice()
        } else {
            restored.values.as_slice()
        };
        assert_eq!(
            values[0].value,
            AttributeValue::Normalized(if preload { 0.2 } else { 0.4 })
        );
        assert_eq!(
            restored.replacement_provenance[&values[0].programmer_order][&owner],
            metadata
        );
        // A fresh explicit master edit detaches routing, but Undo of that edit restores it.
        registry.set(
            session,
            owner,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.4),
        );
        let fresh = registry.get(session).unwrap();
        let values = if preload {
            fresh.preload_pending.as_slice()
        } else {
            fresh.values.as_slice()
        };
        assert!(
            !fresh
                .replacement_provenance
                .contains_key(&values[0].programmer_order)
        );
        assert!(registry.undo(session));
        let restored = registry.get(session).unwrap();
        let values = if preload {
            restored.preload_pending.as_slice()
        } else {
            restored.values.as_slice()
        };
        assert_eq!(
            restored.replacement_provenance[&values[0].programmer_order][&owner],
            metadata
        );
    }
}

#[test]
fn replacement_history_preflight_rejects_stale_or_malformed_capture_atomically_and_keeps_untouched_arcs()
 {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let owner = FixtureId::new();
    let other = FixtureId::new();
    registry.start(session);
    registry.set(
        session,
        other,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.6),
    );
    registry.set(
        session,
        owner,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    );
    registry.set(
        session,
        owner,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.4),
    );
    assert!(registry.undo(session)); // Real redo content must be migrated too.
    let before = registry.get(session).unwrap();
    let metadata = projection(owner);
    let live = physical_origins(&before.snapshot(), owner, &metadata);
    let undo = before
        .undo
        .iter()
        .map(|snapshot| physical_origins(snapshot, owner, &metadata))
        .collect::<Vec<_>>();
    let redo = before
        .redo
        .iter()
        .map(|snapshot| physical_origins(snapshot, owner, &metadata))
        .collect::<Vec<_>>();
    let exact = serde_json::to_value(&before).unwrap();
    for (id, live, undo) in [
        (ProgrammerId::new(), live.clone(), undo.clone()),
        (before.id, live.clone(), Vec::new()),
        (
            before.id,
            vec![(u64::MAX, HashMap::from([(owner, metadata.clone())]))],
            undo.clone(),
        ),
        (
            before.id,
            vec![(live[0].0, HashMap::from([(other, metadata.clone())]))],
            undo.clone(),
        ),
    ] {
        assert!(
            registry
                .validate_replacement_migration_with_history(id, &live, &undo, &redo)
                .is_err()
        );
        assert!(
            registry
                .apply_replacement_migration_with_history(id, &live, &undo, &redo)
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(registry.get(session).unwrap()).unwrap(),
            exact
        );
    }
    assert!(
        registry
            .apply_replacement_migration_with_history(before.id, &live, &undo, &redo)
            .unwrap()
    );
    let after = registry.get(session).unwrap();
    assert_eq!(after.undo.len(), before.undo.len());
    assert_eq!(after.redo.len(), before.redo.len());
    for (index, _origins) in undo
        .iter()
        .enumerate()
        .filter(|(_, origins)| origins.is_empty())
    {
        assert!(Arc::ptr_eq(&before.undo[index], &after.undo[index]));
    }
    assert_eq!(after.values, before.values);
    assert!(registry.redo(session));
    let restored = registry.get(session).unwrap();
    let value = restored
        .values
        .iter()
        .find(|value| value.fixture_id == owner)
        .unwrap();
    assert_eq!(value.value, AttributeValue::Normalized(0.4));
    assert_eq!(
        restored.replacement_provenance[&value.programmer_order][&owner],
        metadata
    );
}

#[test]
fn replacement_metadata_only_undo_redo_invalidates_both_value_generations() {
    for preload in [false, true] {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let owner = FixtureId::new();
        registry.start(session);
        if preload {
            assert!(registry.arm_preload(session, true));
        }
        registry.set(
            session,
            owner,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.4),
        );
        let before = registry.get(session).unwrap();
        let value = if preload {
            &before.preload_pending[0]
        } else {
            &before.values[0]
        };
        assert!(registry.attach_replacement_provenance(
            session,
            &[(
                value.programmer_order,
                HashMap::from([(owner, projection(owner))])
            )],
            preload,
            true
        ));
        let normal = registry.normal_values_generation(session).unwrap();
        let pending = registry.preload_values_generation(session).unwrap();
        assert!(registry.undo(session));
        let restored = registry.get(session).unwrap();
        let values = if preload {
            restored.preload_pending.as_slice()
        } else {
            restored.values.as_slice()
        };
        assert_eq!(values[0], *value);
        assert!(restored.replacement_provenance.is_empty());
        assert!(registry.normal_values_generation(session).unwrap() > normal);
        assert!(registry.preload_values_generation(session).unwrap() > pending);
        let normal = registry.normal_values_generation(session).unwrap();
        let pending = registry.preload_values_generation(session).unwrap();
        assert!(registry.redo(session));
        assert!(registry.normal_values_generation(session).unwrap() > normal);
        assert!(registry.preload_values_generation(session).unwrap() > pending);
    }
}

#[test]
fn replacement_physical_history_rebase_preserves_operator_undo_redo_without_extra_step() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let owner = FixtureId::new();
    registry.start(session);
    registry.set(
        session,
        owner,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    );
    registry.set(
        session,
        owner,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.4),
    );
    let before = registry.get(session).unwrap();
    let metadata = projection(owner);
    let origins = before
        .values
        .iter()
        .map(|value| {
            (
                value.programmer_order,
                HashMap::from([(owner, metadata.clone())]),
            )
        })
        .collect::<Vec<_>>();
    let historical = |snapshot: &crate::ProgrammerSnapshot| {
        snapshot
            .values
            .iter()
            .map(|value| {
                (
                    value.programmer_order,
                    HashMap::from([(owner, metadata.clone())]),
                )
            })
            .collect::<Vec<_>>()
    };
    let undo = before
        .undo
        .iter()
        .map(|snapshot| historical(snapshot))
        .collect::<Vec<_>>();
    let redo = before
        .redo
        .iter()
        .map(|snapshot| historical(snapshot))
        .collect::<Vec<_>>();
    registry
        .validate_replacement_migration_with_history(before.id, &origins, &undo, &redo)
        .unwrap();
    assert!(
        registry
            .apply_replacement_migration_with_history(before.id, &origins, &undo, &redo)
            .unwrap()
    );
    assert_eq!(
        registry.get(session).unwrap().undo.len(),
        before.undo.len(),
        "physical replacement must not become a Programmer edit"
    );
    assert!(registry.undo(session));
    let restored = registry.get(session).unwrap();
    assert_eq!(restored.values[0].value, AttributeValue::Normalized(0.2));
    assert_eq!(
        restored.replacement_provenance[&restored.values[0].programmer_order][&owner],
        metadata
    );
    assert!(registry.redo(session));
    let restored = registry.get(session).unwrap();
    assert_eq!(restored.values[0].value, AttributeValue::Normalized(0.4));
    assert_eq!(
        restored.replacement_provenance[&restored.values[0].programmer_order][&owner],
        metadata
    );
}

#[test]
fn replacement_legacy_equal_setters_detach_only_replaced_orders_and_undo_restores_envelopes() {
    for preload in [false, true] {
        for batch in [false, true] {
            let registry = ProgrammerRegistry::default();
            let session = SessionId::new();
            registry.start(session);
            if preload {
                assert!(registry.arm_preload(session, true));
            }
            let owner = FixtureId::new();
            let other = FixtureId::new();
            let attribute = AttributeKey::intensity();
            let scalar = AttributeValue::Normalized(0.4);
            registry.set_many(
                session,
                [
                    (owner, attribute.clone(), scalar.clone()),
                    (other, attribute.clone(), scalar.clone()),
                ],
            );
            registry.set_group(session, "front".into(), attribute.clone(), scalar.clone());
            let before = registry.get(session).unwrap();
            let fixtures = if preload {
                &before.preload_pending
            } else {
                before.values.as_ref()
            };
            let groups = if preload {
                &before.preload_group_pending
            } else {
                before.group_values.as_ref()
            };
            let old_order = fixtures
                .iter()
                .find(|value| value.fixture_id == owner)
                .unwrap()
                .programmer_order;
            let other_order = fixtures
                .iter()
                .find(|value| value.fixture_id == other)
                .unwrap()
                .programmer_order;
            let group_order = groups["front"][&attribute].programmer_order;
            let maps = vec![
                (old_order, HashMap::from([(owner, projection(owner))])),
                (other_order, HashMap::from([(other, projection(other))])),
                (group_order, HashMap::from([(owner, projection(owner))])),
            ];
            assert!(registry.attach_replacement_provenance(session, &maps, preload, false));
            let projected = registry.get(session).unwrap();
            if batch {
                registry.set_many(session, [(owner, attribute.clone(), scalar.clone())]);
            } else {
                registry.set(session, owner, attribute.clone(), scalar.clone());
            }
            let current = registry.get(session).unwrap();
            assert!(!current.replacement_provenance.contains_key(&old_order));
            assert_eq!(
                current.replacement_provenance.get(&other_order),
                projected.replacement_provenance.get(&other_order)
            );
            assert_eq!(
                current.replacement_provenance.get(&group_order),
                projected.replacement_provenance.get(&group_order)
            );
            registry.undo(session);
            assert_eq!(
                registry.get(session).unwrap().replacement_provenance,
                projected.replacement_provenance
            );
            registry.set_group(session, "front".into(), attribute.clone(), scalar);
            let current = registry.get(session).unwrap();
            assert!(!current.replacement_provenance.contains_key(&group_order));
            assert_eq!(
                current.replacement_provenance.get(&old_order),
                projected.replacement_provenance.get(&old_order)
            );
            assert_eq!(
                current.replacement_provenance.get(&other_order),
                projected.replacement_provenance.get(&other_order)
            );
            registry.undo(session);
            assert_eq!(
                registry.get(session).unwrap().replacement_provenance,
                projected.replacement_provenance
            );
        }
    }
}

#[test]
fn replacement_projection_capture_same_value_detachment_undo_and_stale_order() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let other = FixtureId::new();
    let attribute = AttributeKey::intensity();
    let value = AttributeValue::Normalized(0.4);
    registry.start(session);
    registry.set_many(
        session,
        [
            (fixture, attribute.clone(), value.clone()),
            (other, attribute.clone(), value.clone()),
        ],
    );
    let before = registry.get(session).unwrap();
    let maps = before
        .values
        .iter()
        .map(|value| {
            (
                value.programmer_order,
                HashMap::from([(value.fixture_id, projection(value.fixture_id))]),
            )
        })
        .collect::<Vec<_>>();
    assert!(registry.attach_replacement_provenance(session, &maps, false, true));
    let preset = registry
        .capture_normal_preset(
            session,
            PresetAddress::new(PresetFamily::Intensity, 1).unwrap(),
            "Projected".into(),
        )
        .unwrap();
    assert_eq!(preset.fixture_replacement_projections.len(), 2);
    assert_eq!(preset.required_programming_contract(), 4);
    assert!(!registry.attach_replacement_provenance(session, &maps, false, true));
    let capture = registry
        .capture_cue_recording(session, CueRecordingSource::CurrentCapture)
        .unwrap();
    assert!(
        capture
            .fixture_values
            .iter()
            .all(|value| value.replacement_projection.is_some())
    );
    assert_eq!(
        registry.active_output_states()[0]
            .replacement_provenance
            .len(),
        2
    );
    assert!(registry.apply_normal_values(
        session,
        &[NormalProgrammerValueMutation::SetFixture {
            fixture_id: fixture,
            attribute: attribute.clone(),
            value: value.clone(),
            timing: NormalProgrammerValueTiming::default()
        }]
    ));
    let capture = registry
        .capture_cue_recording(session, CueRecordingSource::CurrentCapture)
        .unwrap();
    assert!(
        capture
            .fixture_values
            .iter()
            .find(|value| value.fixture_id == fixture)
            .unwrap()
            .replacement_projection
            .is_none()
    );
    assert!(
        capture
            .fixture_values
            .iter()
            .find(|value| value.fixture_id == other)
            .unwrap()
            .replacement_projection
            .is_some()
    );
    assert!(!registry.attach_replacement_provenance(session, &maps[..1], false, true));
    assert!(registry.undo(session));
    assert!(
        registry
            .capture_cue_recording(session, CueRecordingSource::CurrentCapture)
            .unwrap()
            .fixture_values
            .iter()
            .all(|value| value.replacement_projection.is_some())
    );
    let restored: ProgrammerState =
        serde_json::from_value(serde_json::to_value(registry.get(session).unwrap()).unwrap())
            .unwrap();
    assert_eq!(restored.replacement_provenance.len(), 2);
    assert_eq!(restored.required_programming_contract(), 4);
}

#[test]
fn replacement_projection_preload_commit_preserves_member_map_and_preset_reference() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let attribute = AttributeKey::intensity();
    registry.start(session);
    registry.arm_preload(session, true);
    registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetGroup {
            group_id: "Front".into(),
            attribute: attribute.clone(),
            value: AttributeValue::Normalized(0.5),
            timing: PreloadProgrammerValueTiming::default(),
        }],
    );
    let order =
        registry.get(session).unwrap().preload_group_pending["Front"][&attribute].programmer_order;
    let map = HashMap::from([(fixture, projection(fixture))]);
    assert!(registry.attach_replacement_provenance(session, &[(order, map.clone())], true, false));
    let reference = light_core::PresetValueReference {
        preset_instance_id: uuid::Uuid::new_v4(),
        source_owner: light_core::PresetValueOwner::Group {
            group_id: "Front".into(),
        },
        source_attribute: attribute.clone(),
        sample_rank: None,
        member_fixture: None,
    };
    registry.attach_preset_provenance(
        session,
        &[(
            light_core::PresetValueOwner::Group {
                group_id: "Front".into(),
            },
            attribute.clone(),
            reference.clone(),
        )],
        true,
        false,
    );
    registry.activate_preload(session);
    let capture = registry
        .capture_cue_recording(session, CueRecordingSource::PreloadPendingOrActive)
        .unwrap();
    assert_ne!(capture.group_values[0].programmer_order, order);
    assert_eq!(capture.group_values[0].replacement_projections, map);
    assert_eq!(capture.group_values[0].preset_reference, Some(reference));
    assert!(registry.undo(session));
    assert_eq!(
        registry.get(session).unwrap().preload_group_pending["Front"][&attribute].programmer_order,
        order
    );
}

#[test]
fn preset_store_projection_is_address_scoped_and_legacy_absence_is_literal() {
    let fixture = FixtureId::new();
    let other = FixtureId::new();
    let attribute = AttributeKey::intensity();
    let value = AttributeValue::Normalized(0.5);
    let mut preset = Preset {
        family: PresetFamily::Intensity,
        values: HashMap::from([
            (fixture, HashMap::from([(attribute.clone(), value.clone())])),
            (other, HashMap::from([(attribute.clone(), value.clone())])),
        ]),
        fixture_replacement_projections: HashMap::from([
            (
                fixture,
                HashMap::from([(attribute.clone(), projection(fixture))]),
            ),
            (
                other,
                HashMap::from([(attribute.clone(), projection(other))]),
            ),
        ]),
        ..Preset::default()
    };
    preset.validate_programming().unwrap();
    let incoming = Preset {
        family: PresetFamily::Intensity,
        values: HashMap::from([(fixture, HashMap::from([(attribute.clone(), value)]))]),
        ..Preset::default()
    };
    let mut add_missing = preset.clone();
    add_missing.store(incoming.clone(), PresetStoreMode::AddMissingFixtures);
    assert_eq!(
        add_missing.fixture_replacement_projections,
        preset.fixture_replacement_projections
    );
    preset.store(incoming, PresetStoreMode::Merge);
    assert!(
        !preset
            .fixture_replacement_projections
            .contains_key(&fixture)
    );
    assert!(preset.fixture_replacement_projections.contains_key(&other));
    let literal: Preset = serde_json::from_value(
        serde_json::json!({ "family":"Intensity", "number":1, "name":"Literal", "values": {} }),
    )
    .unwrap();
    assert!(literal.fixture_replacement_projections.is_empty());
    assert_eq!(literal.required_programming_contract(), 0);
}

#[test]
fn replacement_projection_migration_uses_exact_programmer_and_normal_pending_active_orders() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let root = FixtureId::new();
    let unrelated = FixtureId::new();
    let attribute = AttributeKey::intensity();
    registry.start(session);
    registry.set_many(
        session,
        [(root, attribute.clone(), AttributeValue::Normalized(0.4))],
    );
    registry.arm_preload(session, true);
    assert!(registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetGroup {
            group_id: "Active".into(),
            attribute: attribute.clone(),
            value: AttributeValue::Normalized(0.6),
            timing: PreloadProgrammerValueTiming::default(),
        }]
    ));
    registry.activate_preload(session);
    registry.arm_preload(session, true);
    registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetGroup {
            group_id: "Pending".into(),
            attribute: attribute.clone(),
            value: AttributeValue::Normalized(0.8),
            timing: PreloadProgrammerValueTiming::default(),
        }],
    );
    let before = registry.get(session).unwrap();
    let normal_order = before.values[0].programmer_order;
    let active_order = before.preload_group_active["Active"][&attribute].programmer_order;
    let pending_order = before.preload_group_pending["Pending"][&attribute].programmer_order;
    let origins = [normal_order, active_order, pending_order]
        .map(|order| (order, HashMap::from([(root, projection(root))])));
    assert!(!registry.apply_replacement_migration(light_core::ProgrammerId::new(), &origins));
    assert!(
        registry
            .get(session)
            .unwrap()
            .replacement_provenance
            .is_empty()
    );
    assert!(registry.apply_replacement_migration(before.id, &origins));
    assert!(!registry.apply_replacement_migration(before.id, &origins));
    let extra = projection(unrelated);
    assert!(registry.apply_replacement_migration(
        before.id,
        &[(active_order, HashMap::from([(unrelated, extra.clone())]))]
    ));
    let after = registry.get(session).unwrap();
    assert_eq!(after.values, before.values);
    assert_eq!(after.preload_pending, before.preload_pending);
    assert_eq!(after.preload_active, before.preload_active);
    assert_eq!(
        after.replacement_provenance[&active_order].len(),
        2,
        "member migration merges instead of replacing other captured members"
    );
    assert_eq!(
        after.replacement_provenance[&active_order][&unrelated],
        extra
    );
    assert!(!registry.apply_replacement_migration(
        before.id,
        &[(u64::MAX, HashMap::from([(root, projection(root))]))]
    ));
    assert!(registry.undo(session));
    assert_eq!(
        registry.get(session).unwrap().replacement_provenance[&active_order].len(),
        1
    );
    assert!(registry.undo(session));
    assert!(
        registry
            .get(session)
            .unwrap()
            .replacement_provenance
            .is_empty()
    );
}
