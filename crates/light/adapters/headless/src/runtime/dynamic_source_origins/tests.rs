use super::*;

#[path = "tests/field_scopes.rs"]
mod field_scopes;

#[test]
fn engine_source_descriptors_round_trip_every_programmer_scope_and_playback_identity() {
    use light_engine::ContributionSourceId as Source;
    let programmer = ProgrammerId::new();
    let mut sources = vec![
        Source::programmer(programmer),
        Source::preload(programmer),
        Source::programmer_transient(programmer, "touch/α"),
        Source::programmer_group(programmer, "front/é"),
        Source::preload_group(programmer, "front/é"),
    ];
    for identity in [
        None,
        Some(PlaybackIdentity::physical(4).unwrap()),
        Some(PlaybackIdentity::virtual_playback(2, 1301).unwrap()),
    ] {
        for temporary in [false, true] {
            sources.push(Source::playback(SequenceMasterSource {
                playback_number: identity.map(|id| id.number()),
                playback_identity: identity,
                cue_list_id: CueListId::new(),
                temporary,
            }));
        }
    }
    for source in sources {
        let stored = DynamicStaticSource::from_contribution(&source);
        let restored: DynamicStaticSource =
            serde_json::from_value(serde_json::to_value(stored).unwrap()).unwrap();
        assert_eq!(restored.contribution_source(), source);
    }
}

#[test]
fn static_playback_actions_at_the_same_time_keep_distinct_occurrences_and_legacy_absence() {
    let authored_cue_id = Uuid::new_v4();
    let source = DynamicStaticSource::Playback {
        source: source(false),
    };
    let entry = light_engine::ContributionFamilyEntry::new(
        source.contribution_source(),
        light_core::ProgrammerEditStamp {
            changed_at: DateTime::from_timestamp(100, 123456789).unwrap(),
            programmer_order: 0,
        },
        light_engine::ContributionFamilyFootprint::Whole,
        light_engine::ContributionFamilyRole::Authored,
    )
    .with_transition_ordinal(Some(41))
    .with_authored_cue_id(Some(authored_cue_id));
    let first = DynamicStaticSourceEntry::from_evidence(source.clone(), &entry).unwrap();
    let second = DynamicStaticSourceEntry::from_evidence(
        source,
        &entry.clone().with_transition_ordinal(Some(42)),
    )
    .unwrap();
    let binding = DynamicSourceBinding::StaticBaseline {
        target: FixtureId::new(),
        owner: ProgrammingOwner::Color,
    };
    let mut origins = DynamicSourceOrigins::default();
    let a = origins
        .bind(
            binding,
            DynamicSourceOrigin::StaticBaseline {
                sources: vec![first.clone()],
            },
        )
        .unwrap();
    let b = origins
        .bind(
            binding,
            DynamicSourceOrigin::StaticBaseline {
                sources: vec![second],
            },
        )
        .unwrap();
    assert_ne!(a, b);
    assert_eq!(first.family_entry().transition_ordinal(), Some(41));
    assert_eq!(
        first.family_entry().authored_cue_id(),
        Some(authored_cue_id)
    );
    let mut legacy = serde_json::to_value(first).unwrap();
    legacy.as_object_mut().unwrap().remove("transition_ordinal");
    legacy.as_object_mut().unwrap().remove("authored_cue_id");
    let legacy: DynamicStaticSourceEntry = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.transition_ordinal, None);
    assert_eq!(legacy.family_entry().transition_ordinal(), None);
    assert_eq!(legacy.family_entry().authored_cue_id(), None);
}

fn binding(origin: &DynamicSourceOrigin) -> DynamicSourceBinding {
    DynamicSourceBinding::Authored {
        instance_id: Uuid::new_v4(),
        controller_id: origin.authored_controller_id().unwrap_or_else(Uuid::new_v4),
        target: FixtureId::new(),
        lane_id: Uuid::new_v4(),
    }
}

fn programmer() -> DynamicSourceOrigin {
    DynamicSourceOrigin::Programmer {
        programmer_id: ProgrammerId::new(),
        lane: DynamicProgrammerSourceLane::Live,
        instance_link: Uuid::new_v4(),
        changed_at_millis: 1_000,
        programmer_order: 1,
    }
}

fn changed_order(mut origin: DynamicSourceOrigin, order: u64) -> DynamicSourceOrigin {
    let DynamicSourceOrigin::Programmer {
        programmer_order, ..
    } = &mut origin
    else {
        panic!()
    };
    *programmer_order = order;
    origin
}

fn source(temporary: bool) -> DynamicSequenceSource {
    SequenceMasterSource {
        playback_number: Some(1_301),
        playback_identity: Some(PlaybackIdentity::virtual_playback(2, 1_301).unwrap()),
        cue_list_id: CueListId::new(),
        temporary,
    }
    .into()
}

fn cue(temporary: bool) -> DynamicSourceOrigin {
    DynamicSourceOrigin::Cue {
        source: source(temporary),
        temporary_kind: temporary.then_some(DynamicTemporarySourceKind::TempFader),
        cue_id: Uuid::new_v4(),
        instance_link: Uuid::new_v4(),
        changed_at: DateTime::from_timestamp(123, 456_789_123).unwrap(),
        transition_ordinal: 17,
    }
}

#[test]
fn unchanged_bind_reuses_id_and_storage_but_equal_valued_new_edit_gets_an_occurrence() {
    let mut catalogue = DynamicSourceOrigins::default();
    let origin = programmer();
    let key = binding(&origin);
    let first = catalogue.bind(key, origin.clone()).unwrap();
    let frozen = catalogue.clone();
    let record = Arc::clone(catalogue.get(first).unwrap());
    for _ in 0..100 {
        assert_eq!(catalogue.bind(key, origin.clone()).unwrap(), first);
    }
    assert!(Arc::ptr_eq(&catalogue.records, &frozen.records));
    assert!(Arc::ptr_eq(&catalogue.bindings, &frozen.bindings));
    assert!(Arc::ptr_eq(catalogue.get(first).unwrap(), &record));

    // Values deliberately are not a key: a new operator edit of the same value is a new source.
    let second = catalogue
        .bind(key, changed_order(origin.clone(), 2))
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(catalogue.records.len(), 2);
    assert_eq!(catalogue.binding(&key), Some(second));
    assert_eq!(frozen.binding(&key), Some(first));
    assert!(Arc::ptr_eq(catalogue.get(first).unwrap(), &record));
    assert_eq!(catalogue.get(first).unwrap().origin, origin);
}

#[test]
fn preview_forks_keep_independent_bindings_and_shared_immutable_history() {
    let mut live = DynamicSourceOrigins::default();
    let origin = programmer();
    let key = binding(&origin);
    let original = live.bind(key, origin.clone()).unwrap();
    let mut preview = live.clone();
    let mut pending = changed_order(origin, 2);
    let DynamicSourceOrigin::Programmer { lane, .. } = &mut pending else {
        panic!()
    };
    *lane = DynamicProgrammerSourceLane::Preload;
    let staged = preview.bind(key, pending).unwrap();
    assert_eq!(live.binding(&key), Some(original));
    assert_eq!(preview.binding(&key), Some(staged));
    assert!(live.get(staged).is_none());
    assert!(Arc::ptr_eq(
        live.get(original).unwrap(),
        preview.get(original).unwrap()
    ));
    assert!(preview.unbind(&key));
    assert_eq!(live.binding(&key), Some(original));
    assert!(preview.get(original).is_some());
    assert!(
        preview.get(staged).is_some(),
        "unbinding cannot discard held expression history"
    );
}

#[test]
fn retaining_active_bindings_keeps_history_and_skips_unchanged_map_copies() {
    let mut catalogue = DynamicSourceOrigins::default();
    let first_origin = programmer();
    let first_binding = binding(&first_origin);
    let first = catalogue.bind(first_binding, first_origin).unwrap();
    let second_origin = programmer();
    let second_binding = binding(&second_origin);
    let second = catalogue.bind(second_binding, second_origin).unwrap();
    let frozen = catalogue.clone();

    assert_eq!(catalogue.retain_bindings(|_| true), 0);
    assert!(catalogue.shares_storage(&frozen));
    assert_eq!(
        catalogue.retain_bindings(|record| record.binding == second_binding),
        1
    );
    assert_eq!(catalogue.binding(&first_binding), None);
    assert_eq!(catalogue.binding(&second_binding), Some(second));
    assert!(
        catalogue.get(first).is_some(),
        "held source remains addressable"
    );
    assert_eq!(frozen.binding(&first_binding), Some(first));
    assert!(Arc::ptr_eq(&catalogue.records, &frozen.records));
    assert!(!Arc::ptr_eq(&catalogue.bindings, &frozen.bindings));
}

#[test]
fn each_instance_controller_target_lane_and_role_is_a_distinct_binding() {
    let mut catalogue = DynamicSourceOrigins::default();
    let origin = programmer();
    let key = binding(&origin);
    let original = catalogue.bind(key, origin.clone()).unwrap();
    let DynamicSourceBinding::Authored {
        instance_id,
        controller_id,
        target,
        lane_id,
    } = key
    else {
        panic!()
    };
    let keys = [
        DynamicSourceBinding::Authored {
            instance_id: Uuid::new_v4(),
            controller_id,
            target,
            lane_id,
        },
        DynamicSourceBinding::Authored {
            instance_id,
            controller_id,
            target: FixtureId::new(),
            lane_id,
        },
        DynamicSourceBinding::Authored {
            instance_id,
            controller_id,
            target,
            lane_id: Uuid::new_v4(),
        },
    ];
    let mut ids = HashSet::from([original]);
    for key in keys {
        let id = catalogue.bind(key, origin.clone()).unwrap();
        assert!(ids.insert(id));
        assert_eq!(catalogue.get(id).unwrap().binding, key);
    }
    let other_controller = DynamicSourceBinding::Authored {
        instance_id,
        controller_id: Uuid::new_v4(),
        target,
        lane_id,
    };
    assert!(
        catalogue.bind(other_controller, origin.clone()).is_err(),
        "a source cannot be attached to another controller"
    );
    let other_origin = programmer();
    let other_controller = DynamicSourceBinding::Authored {
        instance_id,
        controller_id: other_origin.authored_controller_id().unwrap(),
        target,
        lane_id,
    };
    assert!(ids.insert(catalogue.bind(other_controller, other_origin).unwrap()));
    let baseline_key = DynamicSourceBinding::StaticBaseline {
        target,
        owner: ProgrammingOwner::Color,
    };
    assert!(catalogue.bind(baseline_key, origin).is_err());
    let baseline = DynamicSourceOrigin::StaticBaseline {
        sources: vec![DynamicStaticSourceEntry {
            source: DynamicStaticSource::Playback {
                source: source(false),
            },
            changed_at: DateTime::from_timestamp(500, 987_654_321).unwrap(),
            programmer_order: 0,
            transition_ordinal: Some(11),
            authored_cue_id: None,
            footprint: DynamicStaticFootprint::Whole,
            role: DynamicStaticRole::Authored,
            effective_fields: None,
        }],
    };
    assert!(catalogue.bind(key, baseline.clone()).is_err());
    let dependency = catalogue.bind(baseline_key, baseline).unwrap();
    assert!(ids.insert(dependency));
    assert_eq!(catalogue.binding(&key), Some(original));
    assert!(
        catalogue
            .bind(
                baseline_key,
                DynamicSourceOrigin::StaticBaseline { sources: vec![] }
            )
            .is_err()
    );
    assert_eq!(catalogue.binding(&baseline_key), Some(dependency));
}

#[test]
fn exact_cue_source_time_kind_and_standalone_playback_survive_roundtrip() {
    let mut catalogue = DynamicSourceOrigins::default();
    let origin = cue(true);
    let key = binding(&origin);
    let DynamicSourceOrigin::Cue {
        source: sequence,
        temporary_kind,
        changed_at,
        ..
    } = &origin
    else {
        panic!()
    };
    assert_eq!(
        sequence.sequence_master_source().playback_identity,
        Some(PlaybackIdentity::virtual_playback(2, 1_301).unwrap())
    );
    assert_eq!(
        temporary_kind.unwrap().playback_kind(),
        TemporaryPlaybackKind::TempFader
    );
    assert_eq!(changed_at.timestamp_subsec_nanos(), 456_789_123);
    let first = catalogue.bind(key, origin.clone()).unwrap();
    let mut another_kind = origin.clone();
    let DynamicSourceOrigin::Cue { temporary_kind, .. } = &mut another_kind else {
        panic!()
    };
    *temporary_kind = Some(DynamicTemporarySourceKind::Flash);
    let another_kind_key = binding(&another_kind);
    let second = catalogue.bind(another_kind_key, another_kind).unwrap();
    assert_ne!(first, second);

    let standalone = DynamicSourceOrigin::Playback {
        identity: PlaybackIdentity::physical(42).unwrap(),
        activated_at: DateTime::from_timestamp(300, 654_321_123).unwrap(),
    };
    let playback_key = binding(&standalone);
    let third = catalogue.bind(playback_key, standalone.clone()).unwrap();
    let encoded = serde_json::to_vec(&catalogue.snapshot()).unwrap();
    let restored = DynamicSourceOrigins::restore(
        serde_json::from_slice(&encoded).unwrap(),
        [first, second, third],
    )
    .unwrap();
    assert_eq!(restored.snapshot(), catalogue.snapshot());
    assert_eq!(restored.get(first).unwrap().origin, origin);
    assert_eq!(restored.get(third).unwrap().origin, standalone);
    let standalone_json = serde_json::to_value(&restored.get(third).unwrap().origin).unwrap();
    assert!(standalone_json.get("cue_id").is_none());
    assert!(standalone_json.get("source").is_none());
}

#[test]
fn prune_keeps_active_and_reachable_history_and_rejects_unknown_ids_atomically() {
    let mut catalogue = DynamicSourceOrigins::default();
    let origin = programmer();
    let key = binding(&origin);
    let mut ids = Vec::new();
    for order in 1..=120 {
        ids.push(
            catalogue
                .bind(key, changed_order(origin.clone(), order))
                .unwrap(),
        );
    }
    let frozen = catalogue.clone();
    let unknown = DynamicSourceOccurrenceId::new(Uuid::new_v4()).unwrap();
    assert!(catalogue.prune([ids[1], unknown]).is_err());
    assert!(Arc::ptr_eq(&catalogue.records, &frozen.records));
    assert!(Arc::ptr_eq(&catalogue.bindings, &frozen.bindings));
    assert_eq!(catalogue.prune([ids[1], ids[70], ids[70]]).unwrap(), 117);
    assert_eq!(catalogue.records.len(), 3);
    assert_eq!(catalogue.binding(&key), Some(ids[119]));
    assert_eq!(frozen.records.len(), 120);
    assert!(
        catalogue
            .validate_reachable([ids[1], ids[70], ids[119]])
            .is_ok()
    );
    assert!(catalogue.validate_reachable([ids[0]]).is_err());
    assert!(catalogue.unbind(&key));
    assert_eq!(catalogue.prune([]).unwrap(), 3);
    assert!(catalogue.records.is_empty());
}

#[test]
fn restore_rejects_duplicate_conflicting_dangling_and_cross_scope_metadata() {
    let mut catalogue = DynamicSourceOrigins::default();
    let origin = programmer();
    let key = binding(&origin);
    let id = catalogue.bind(key, origin).unwrap();
    let snapshot = catalogue.snapshot();
    let mut duplicate = snapshot.clone();
    duplicate.records.push(duplicate.records[0].clone());
    assert!(DynamicSourceOrigins::restore(duplicate, [id]).is_err());
    let mut conflict = snapshot.clone();
    let mut record = conflict.records[0].clone();
    record.origin = changed_order(record.origin, 2);
    conflict.records.push(record);
    assert!(DynamicSourceOrigins::restore(conflict, [id]).is_err());
    let mut duplicate_binding = snapshot.clone();
    duplicate_binding
        .bindings
        .push(duplicate_binding.bindings[0]);
    assert!(DynamicSourceOrigins::restore(duplicate_binding, [id]).is_err());
    let unknown = DynamicSourceOccurrenceId::new(Uuid::new_v4()).unwrap();
    let mut dangling = snapshot.clone();
    dangling.bindings[0].occurrence_id = unknown;
    assert!(DynamicSourceOrigins::restore(dangling, [id]).is_err());
    assert!(DynamicSourceOrigins::restore(snapshot.clone(), [unknown]).is_err());
    let mut wrong_scope = snapshot.clone();
    let DynamicSourceBinding::Authored { target, .. } = &mut wrong_scope.bindings[0].binding else {
        panic!()
    };
    *target = FixtureId::new();
    assert!(DynamicSourceOrigins::restore(wrong_scope, [id]).is_err());
    let mut wrong_role = snapshot.clone();
    let baseline = DynamicSourceBinding::StaticBaseline {
        target: FixtureId::new(),
        owner: ProgrammingOwner::Color,
    };
    wrong_role.records[0].binding = baseline;
    wrong_role.bindings[0].binding = baseline;
    assert!(DynamicSourceOrigins::restore(wrong_role, [id]).is_err());
    let mut wrong_version = snapshot.clone();
    wrong_version.version += 1;
    assert!(DynamicSourceOrigins::restore(wrong_version, [id]).is_err());
    assert_eq!(
        catalogue.snapshot(),
        snapshot,
        "invalid candidates never mutate live metadata"
    );
}

#[test]
fn restore_revalidates_ids_and_playback_domains_after_deserialization() {
    let mut catalogue = DynamicSourceOrigins::default();
    let origin = cue(true);
    let key = binding(&origin);
    let id = catalogue.bind(key, origin).unwrap();
    let mut json = serde_json::to_value(catalogue.snapshot()).unwrap();
    json["records"][0]["occurrence_id"] = serde_json::json!(Uuid::nil());
    assert!(
        serde_json::from_value::<DynamicSourceOriginsSnapshot>(json).is_err(),
        "nil opaque IDs are rejected at deserialization before restore can install them"
    );

    let mut invalid_binding = catalogue.snapshot();
    let DynamicSourceBinding::Authored { lane_id, .. } = &mut invalid_binding.records[0].binding
    else {
        panic!()
    };
    *lane_id = Uuid::nil();
    assert!(DynamicSourceOrigins::restore(invalid_binding, [id]).is_err());
    let mut invalid_source = catalogue.snapshot();
    let DynamicSourceOrigin::Cue {
        source,
        temporary_kind,
        ..
    } = &mut invalid_source.records[0].origin
    else {
        panic!()
    };
    *temporary_kind = None;
    assert!(source.temporary);
    assert!(DynamicSourceOrigins::restore(invalid_source, [id]).is_err());

    let mut invalid_page = catalogue.snapshot();
    let DynamicSourceOrigin::Cue { source, .. } = &mut invalid_page.records[0].origin else {
        panic!()
    };
    let mut identity = serde_json::to_value(source.playback_identity.unwrap()).unwrap();
    identity["page"] = serde_json::json!(3);
    source.playback_identity = Some(serde_json::from_value(identity).unwrap());
    assert!(DynamicSourceOrigins::restore(invalid_page, [id]).is_err());
    let mut mismatch = catalogue.snapshot();
    let DynamicSourceOrigin::Cue { source, .. } = &mut mismatch.records[0].origin else {
        panic!()
    };
    source.playback_number = Some(1_302);
    assert!(DynamicSourceOrigins::restore(mismatch, [id]).is_err());
}

#[test]
fn invalid_bind_does_not_change_an_existing_assignment() {
    let mut catalogue = DynamicSourceOrigins::default();
    let origin = programmer();
    let key = binding(&origin);
    let id = catalogue.bind(key, origin.clone()).unwrap();
    let frozen = catalogue.clone();
    let mut nil_source = origin;
    let DynamicSourceOrigin::Programmer { instance_link, .. } = &mut nil_source else {
        panic!()
    };
    *instance_link = Uuid::nil();
    assert!(catalogue.bind(key, nil_source).is_err());
    assert_eq!(catalogue.binding(&key), Some(id));
    assert!(Arc::ptr_eq(&catalogue.records, &frozen.records));
    assert!(Arc::ptr_eq(&catalogue.bindings, &frozen.bindings));
}

#[test]
fn static_evidence_order_and_duplicates_do_not_create_frame_occurrences() {
    let key = DynamicSourceBinding::StaticBaseline {
        target: FixtureId::new(),
        owner: ProgrammingOwner::Color,
    };
    let a = DynamicStaticSourceEntry {
        source: DynamicStaticSource::Programmer {
            programmer_id: ProgrammerId::new(),
            lane: DynamicStaticProgrammerLane::Group("stage-left".into()),
        },
        changed_at: DateTime::from_timestamp(1, 2).unwrap(),
        programmer_order: 5,
        transition_ordinal: None,
        authored_cue_id: None,
        footprint: DynamicStaticFootprint::Component(ProgrammingComponent::Color(
            light_core::programming::ColorComponent::Uv,
        )),
        role: DynamicStaticRole::Authored,
        effective_fields: None,
    };
    let b = DynamicStaticSourceEntry {
        source: DynamicStaticSource::Playback {
            source: source(false),
        },
        changed_at: DateTime::from_timestamp(3, 4).unwrap(),
        programmer_order: 0,
        transition_ordinal: Some(12),
        authored_cue_id: None,
        footprint: DynamicStaticFootprint::Whole,
        role: DynamicStaticRole::CalculationDependency,
        effective_fields: None,
    };
    let mut catalogue = DynamicSourceOrigins::default();
    let id = catalogue
        .bind(
            key,
            DynamicSourceOrigin::StaticBaseline {
                sources: vec![a.clone(), b.clone()],
            },
        )
        .unwrap();
    let frozen = catalogue.clone();
    assert_eq!(
        catalogue
            .bind(
                key,
                DynamicSourceOrigin::StaticBaseline {
                    sources: vec![b, a.clone(), a]
                }
            )
            .unwrap(),
        id
    );
    assert!(Arc::ptr_eq(&catalogue.records, &frozen.records));
    assert!(Arc::ptr_eq(&catalogue.bindings, &frozen.bindings));
    let restored = DynamicSourceOrigins::restore(
        serde_json::from_slice(&serde_json::to_vec(&catalogue.snapshot()).unwrap()).unwrap(),
        [id],
    )
    .unwrap();
    assert_eq!(restored.snapshot(), catalogue.snapshot());
}

#[test]
fn static_family_evidence_preserves_component_ownership_and_original_dependency_role() {
    use light_engine::{
        ContributionFamilyEntry, ContributionFamilyFootprint as Footprint,
        ContributionFamilyRole as Role,
    };
    let source = DynamicStaticSource::Programmer {
        programmer_id: ProgrammerId::new(),
        lane: DynamicStaticProgrammerLane::PreloadGroup("colors".into()),
    };
    let uv = ProgrammingComponent::Color(light_core::programming::ColorComponent::Uv);
    let white = ProgrammingComponent::Color(light_core::programming::ColorComponent::WhiteBlend);
    let stamp = light_core::ProgrammerEditStamp {
        changed_at: DateTime::from_timestamp(5, 123_456_789).unwrap(),
        programmer_order: 71,
    };
    let evidence = [
        ContributionFamilyEntry::new(
            source.contribution_source(),
            stamp,
            Footprint::Component(uv),
            Role::Authored,
        ),
        ContributionFamilyEntry::new(
            source.contribution_source(),
            stamp,
            Footprint::Component(white),
            Role::CalculationDependency,
        ),
    ];
    let entries = evidence
        .iter()
        .map(|entry| DynamicStaticSourceEntry::from_evidence(source.clone(), entry).unwrap())
        .collect::<Vec<_>>();
    let wrong_source = DynamicStaticSource::Programmer {
        programmer_id: ProgrammerId::new(),
        lane: DynamicStaticProgrammerLane::Live,
    };
    assert!(DynamicStaticSourceEntry::from_evidence(wrong_source, &evidence[0]).is_err());
    let mut catalogue = DynamicSourceOrigins::default();
    let target = FixtureId::new();
    let binding = DynamicSourceBinding::StaticBaseline {
        target,
        owner: ProgrammingOwner::Color,
    };
    let origin = DynamicSourceOrigin::StaticBaseline { sources: entries };
    assert!(
        catalogue
            .bind(
                DynamicSourceBinding::StaticBaseline {
                    target,
                    owner: ProgrammingOwner::Position
                },
                origin.clone()
            )
            .is_err()
    );
    let id = catalogue.bind(binding, origin).unwrap();
    let restored = DynamicSourceOrigins::restore(
        serde_json::from_slice(&serde_json::to_vec(&catalogue.snapshot()).unwrap()).unwrap(),
        [id],
    )
    .unwrap();
    let DynamicSourceOrigin::StaticBaseline { sources } = &restored.get(id).unwrap().origin else {
        panic!()
    };
    assert_eq!(
        sources.len(),
        2,
        "one source may own different components and roles"
    );
    for expected in &evidence {
        let actual = sources
            .iter()
            .map(DynamicStaticSourceEntry::family_entry)
            .find(|entry| entry.footprint() == expected.footprint())
            .unwrap();
        assert_eq!(actual.source(), expected.source());
        assert_eq!(actual.role(), expected.role());
        assert_eq!(actual.stamp().changed_at, expected.stamp().changed_at);
        assert_eq!(
            actual.stamp().programmer_order,
            expected.stamp().programmer_order
        );
    }
}

fn captured_static_evidence() -> (
    DynamicSourceBinding,
    Arc<light_engine::ContributionFamilyEvidence>,
) {
    use light_engine::{
        ContributionFamilyEntry, ContributionFamilyEvidence,
        ContributionFamilyFootprint as Footprint, ContributionFamilyRole as Role,
        ContributionSourceId,
    };
    let stamp = light_core::ProgrammerEditStamp {
        changed_at: DateTime::from_timestamp(12, 345).unwrap(),
        programmer_order: 9,
    };
    (
        DynamicSourceBinding::StaticBaseline {
            target: FixtureId::new(),
            owner: ProgrammingOwner::Position,
        },
        Arc::new(ContributionFamilyEvidence::new(vec![
            ContributionFamilyEntry::new(
                ContributionSourceId::programmer_group(ProgrammerId::new(), "front/é"),
                stamp,
                Footprint::Component(ProgrammingComponent::Pan),
                Role::Authored,
            ),
            ContributionFamilyEntry::new(
                ContributionSourceId::playback(source(false).sequence_master_source()),
                stamp,
                Footprint::Component(ProgrammingComponent::Tilt),
                Role::CalculationDependency,
            )
            .with_transition_ordinal(Some(17)),
        ])),
    )
}

#[test]
fn captured_static_evidence_reuses_storage_without_retaining_the_source_frame() {
    let (binding, evidence) = captured_static_evidence();
    let mut origins = DynamicSourceOrigins::default();
    let id = origins.bind_static_evidence(binding, &evidence).unwrap();
    let frozen = origins.clone();
    for _ in 0..3 {
        assert_eq!(
            origins.bind_static_evidence(binding, &evidence).unwrap(),
            id
        );
        assert!(origins.shares_storage(&frozen));
    }
    assert_eq!(Arc::strong_count(&evidence), 1);
    let weak = Arc::downgrade(&evidence);
    drop(evidence);
    assert!(weak.upgrade().is_none());
    assert!(
        origins.static_evidence[&binding]
            .evidence
            .upgrade()
            .is_none()
    );
    assert_eq!(origins.binding(&binding), Some(id));
    assert!(origins.get(id).is_some());
}

#[test]
fn captured_static_evidence_validates_new_allocations_and_canonicalizes_equivalent_sets() {
    use light_engine::{
        ContributionFamilyEntry, ContributionFamilyEvidence, ContributionFamilyFootprint,
    };
    let (binding, evidence) = captured_static_evidence();
    let mut origins = DynamicSourceOrigins::default();
    let id = origins.bind_static_evidence(binding, &evidence).unwrap();
    let frozen = origins.clone();
    let reordered = Arc::new(ContributionFamilyEvidence::new(vec![
        evidence.entries()[1].clone(),
        evidence.entries()[0].clone(),
        evidence.entries()[0].clone(),
    ]));
    assert_eq!(
        origins.bind_static_evidence(binding, &reordered).unwrap(),
        id
    );
    assert!(Arc::ptr_eq(&origins.records, &frozen.records));
    assert!(Arc::ptr_eq(&origins.bindings, &frozen.bindings));
    assert!(
        !origins.shares_storage(&frozen),
        "publish the newly warmed cache"
    );
    let warmed = origins.clone();
    assert_eq!(
        origins.bind_static_evidence(binding, &reordered).unwrap(),
        id
    );
    assert!(origins.shares_storage(&warmed));

    let changed = Arc::new(ContributionFamilyEvidence::new(vec![
        evidence.entries()[0].clone(),
        evidence.entries()[1]
            .clone()
            .with_transition_ordinal(Some(18)),
    ]));
    let next = origins.bind_static_evidence(binding, &changed).unwrap();
    assert_ne!(
        next, id,
        "a different action remains a different occurrence"
    );
    let frozen = origins.clone();
    let wrong = &evidence.entries()[1];
    let invalid = Arc::new(ContributionFamilyEvidence::new(vec![
        evidence.entries()[0].clone(),
        ContributionFamilyEntry::new(
            wrong.source().clone(),
            wrong.stamp(),
            ContributionFamilyFootprint::Component(ProgrammingComponent::Focus),
            wrong.role(),
        ),
    ]));
    assert!(origins.bind_static_evidence(binding, &invalid).is_err());
    assert!(origins.shares_storage(&frozen));
    assert_eq!(origins.binding(&binding), Some(next));
    let empty = Arc::new(ContributionFamilyEvidence::new(Vec::new()));
    assert!(origins.bind_static_evidence(binding, &empty).is_err());
    assert!(origins.shares_storage(&frozen));
}

#[test]
fn static_evidence_cache_is_scoped_to_its_branch_binding_and_current_occurrence() {
    let (binding, evidence) = captured_static_evidence();
    let mut live = DynamicSourceOrigins::default();
    let id = live.bind_static_evidence(binding, &evidence).unwrap();
    let frozen = live.clone();
    let mut preview = live.clone();
    assert!(preview.unbind(&binding));
    assert!(preview.static_evidence.is_empty());
    assert!(live.shares_storage(&frozen));
    let next = preview.bind_static_evidence(binding, &evidence).unwrap();
    assert_ne!(next, id, "rebinding cannot resurrect a retired occurrence");
    assert_eq!(live.bind_static_evidence(binding, &evidence).unwrap(), id);
    assert!(live.shares_storage(&frozen));

    let other = DynamicSourceBinding::StaticBaseline {
        target: FixtureId::new(),
        owner: ProgrammingOwner::Position,
    };
    let other_id = preview.bind_static_evidence(other, &evidence).unwrap();
    assert_ne!(
        other_id, next,
        "the same evidence Arc does not merge target scopes"
    );
    let mut replacement = preview.get(next).unwrap().origin.clone();
    let DynamicSourceOrigin::StaticBaseline { sources } = &mut replacement else {
        panic!()
    };
    sources[0].programmer_order += 1;
    let replacement_id = preview.bind(binding, replacement).unwrap();
    assert_ne!(replacement_id, next);
    assert!(!preview.static_evidence.contains_key(&binding));
    let rebound = preview.bind_static_evidence(binding, &evidence).unwrap();
    assert_ne!(rebound, replacement_id);
    assert_ne!(rebound, next);

    assert_eq!(preview.retain_bindings(|record| record.binding == other), 1);
    assert_eq!(preview.static_evidence.len(), 1);
    assert!(preview.static_evidence.contains_key(&other));
    assert!(
        preview.get(rebound).is_some(),
        "retirement preserves immutable history"
    );
    assert_eq!(preview.prune([]).unwrap(), 4);
    assert!(preview.get(other_id).is_some());
    assert_eq!(preview.static_evidence.len(), 1);
    assert!(preview.unbind(&other));
    assert!(preview.static_evidence.is_empty());
    assert_eq!(preview.prune([]).unwrap(), 1);
    assert!(live.shares_storage(&frozen));
}

#[test]
fn restored_static_evidence_cache_warms_lazily_and_never_enters_a_snapshot() {
    let (binding, evidence) = captured_static_evidence();
    let mut original = DynamicSourceOrigins::default();
    let id = original.bind_static_evidence(binding, &evidence).unwrap();
    let snapshot = original.snapshot();
    let wire = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(wire.as_object().unwrap().len(), 3);
    let mut restored =
        DynamicSourceOrigins::restore(serde_json::from_value(wire).unwrap(), [id]).unwrap();
    assert!(restored.static_evidence.is_empty());
    let cold = restored.clone();
    assert_eq!(
        restored.bind_static_evidence(binding, &evidence).unwrap(),
        id
    );
    assert_eq!(restored.snapshot(), snapshot);
    assert!(Arc::ptr_eq(&restored.records, &cold.records));
    assert!(Arc::ptr_eq(&restored.bindings, &cold.bindings));
    assert!(!restored.shares_storage(&cold));
    let warmed = restored.clone();
    assert_eq!(
        restored.bind_static_evidence(binding, &evidence).unwrap(),
        id
    );
    assert!(restored.shares_storage(&warmed));
    assert!(
        restored
            .prune([DynamicSourceOccurrenceId::new(Uuid::new_v4()).unwrap()])
            .is_err()
    );
    assert!(restored.shares_storage(&warmed));
}

#[test]
fn singleton_metadata_matches_do_not_warm_an_evidence_identity_cache() {
    let (binding, evidence) = captured_static_evidence();
    let singleton = Arc::new(light_engine::ContributionFamilyEvidence::new(vec![
        evidence.entries()[0].clone(),
    ]));
    let mut origins = DynamicSourceOrigins::default();
    let id = origins.bind_static_evidence(binding, &singleton).unwrap();
    let frozen = origins.clone();
    let recreated = Arc::new((*singleton).clone());
    assert_eq!(
        origins.bind_static_evidence(binding, &recreated).unwrap(),
        id
    );
    assert!(origins.static_evidence.is_empty());
    assert!(origins.shares_storage(&frozen));

    let duplicates = Arc::new(light_engine::ContributionFamilyEvidence::new(vec![
        evidence.entries()[0].clone(),
        evidence.entries()[0].clone(),
    ]));
    assert_eq!(
        origins.bind_static_evidence(binding, &duplicates).unwrap(),
        id
    );
    assert_eq!(origins.static_evidence.len(), 1);
    assert_eq!(
        origins.bind_static_evidence(binding, &recreated).unwrap(),
        id
    );
    assert!(origins.static_evidence.is_empty());
}

/// TL-639: the key-only and authored-only retains remove exactly what the record predicate
/// removes, without looking up records they do not need.
#[test]
fn key_and_authored_retains_equal_the_record_retain() {
    let static_key = DynamicSourceBinding::StaticBaseline {
        target: FixtureId::new(),
        owner: ProgrammingOwner::Color,
    };
    let mut catalogue = DynamicSourceOrigins::default();
    let static_id = catalogue
        .bind(
            static_key,
            DynamicSourceOrigin::StaticBaseline {
                sources: vec![DynamicStaticSourceEntry {
                    source: DynamicStaticSource::Playback {
                        source: source(false),
                    },
                    changed_at: DateTime::from_timestamp(3, 4).unwrap(),
                    programmer_order: 0,
                    transition_ordinal: Some(12),
                    authored_cue_id: None,
                    footprint: DynamicStaticFootprint::Whole,
                    role: DynamicStaticRole::CalculationDependency,
                    effective_fields: None,
                }],
            },
        )
        .unwrap();
    let authored = [programmer(), cue(false), programmer()]
        .into_iter()
        .map(|origin| {
            let key = binding(&origin);
            catalogue.bind(key, origin).unwrap();
            key
        })
        .collect::<Vec<_>>();
    let compare = |by_record: &dyn Fn(&DynamicSourceRecord) -> bool,
                   other: &dyn Fn(&mut DynamicSourceOrigins) -> usize| {
        let (mut left, mut right) = (catalogue.clone(), catalogue.clone());
        let removed = left.retain_bindings(by_record);
        assert_eq!(other(&mut right), removed);
        assert_eq!(left.snapshot(), right.snapshot());
        removed
    };
    // Key-only: keep authored, keep the static binding only for its current occurrence.
    for current in [Some(static_id), None] {
        let removed = compare(
            &|record| match record.binding {
                DynamicSourceBinding::StaticBaseline { .. } => {
                    current == Some(record.occurrence_id)
                }
                _ => true,
            },
            &|origins| {
                origins.retain_bindings_by_key(|binding, id| match binding {
                    DynamicSourceBinding::StaticBaseline { .. } => current == Some(id),
                    _ => true,
                })
            },
        );
        assert_eq!(removed, usize::from(current.is_none()));
    }
    // Authored-only: drop Programmer bindings except the first; Cue and static bindings stay.
    let removed = compare(
        &|record| {
            !matches!(record.origin, DynamicSourceOrigin::Programmer { .. })
                || record.binding == authored[0]
        },
        &|origins| {
            origins.retain_authored_bindings(|record| {
                !matches!(record.origin, DynamicSourceOrigin::Programmer { .. })
                    || record.binding == authored[0]
            })
        },
    );
    assert_eq!(removed, 1);
}
