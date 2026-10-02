use super::*;
use light_core::programming::{ColorComponent, ProgrammingTraceField as Field};
use light_dynamics::{FamilySampleRank, FamilyTraceContribution, FamilyTraceSource};

fn scope(fields: impl IntoIterator<Item = Field>) -> ProgrammingFieldScope {
    ProgrammingFieldScope::new(fields)
}

fn query(fields: impl IntoIterator<Item = Field>) -> FamilyTraceQuery {
    FamilyTraceQuery {
        sources: Vec::new(),
        base_fields: scope(fields),
        base_dependency_fields: scope([]),
    }
}

fn static_entry(fields: Option<ProgrammingFieldScope>) -> DynamicStaticSourceEntry {
    DynamicStaticSourceEntry {
        source: DynamicStaticSource::Programmer {
            programmer_id: ProgrammerId::new(),
            lane: DynamicStaticProgrammerLane::Group("warm front".into()),
        },
        changed_at: DateTime::from_timestamp(100, 17).unwrap(),
        programmer_order: 83,
        transition_ordinal: None,
        authored_cue_id: None,
        footprint: DynamicStaticFootprint::Component(ProgrammingComponent::Color(
            ColorComponent::Amber,
        )),
        role: DynamicStaticRole::Authored,
        effective_fields: fields,
    }
}

fn bind_static(
    origins: &mut DynamicSourceOrigins,
    target: FixtureId,
    sources: Vec<DynamicStaticSourceEntry>,
) -> DynamicSourceOccurrenceId {
    origins
        .bind(
            DynamicSourceBinding::StaticBaseline {
                target,
                owner: ProgrammingOwner::Color,
            },
            DynamicSourceOrigin::StaticBaseline { sources },
        )
        .unwrap()
}

fn leaf(
    id: Option<DynamicSourceOccurrenceId>,
    fields: ProgrammingFieldScope,
) -> FamilyTraceContribution {
    FamilyTraceContribution {
        source: FamilyTraceSource {
            rank: FamilySampleRank {
                priority: 100,
                changed_at_millis: 1000,
                changed_at_submillis_nanos: 0,
                stable_order: 1,
                identity: light_dynamics::FamilySampleIdentity::Dynamic {
                    instance_id: Uuid::new_v4(),
                    controller_id: Uuid::new_v4(),
                    lane_id: Uuid::new_v4(),
                },
            },
            footprint: FamilyTraceFootprint::Component(ProgrammingComponent::Color(
                ColorComponent::Red,
            )),
            role: FamilyTraceRole::CalculationDependency,
            occurrence: id,
        },
        fields,
    }
}

#[test]
fn converted_static_scope_keeps_original_author_and_distinct_query_relationship() {
    let target = FixtureId::new();
    let mut origins = DynamicSourceOrigins::default();
    let original = static_entry(Some(scope([
        Field::ColorXyz,
        Field::ColorRecipeRed,
        Field::ColorRecipeGreen,
    ])));
    let mut ultraviolet = static_entry(Some(scope([Field::Uv])));
    ultraviolet.footprint =
        DynamicStaticFootprint::Component(ProgrammingComponent::Color(ColorComponent::Uv));
    let id = bind_static(&mut origins, target, vec![original.clone(), ultraviolet]);
    let mut request = query([Field::ColorRecipeRed]);
    request.base_dependency_fields = scope([Field::ColorRecipeGreen]);
    request
        .sources
        .push(leaf(Some(id), scope([Field::ColorRecipeRed])));
    let mut result = DynamicFamilySourceProjection::default();
    result
        .project(
            &origins,
            target,
            ProgrammingOwner::Color,
            Some(&request),
            Some(id),
        )
        .unwrap();
    let entries = result.entries().unwrap();
    assert_eq!(
        entries.len(),
        2,
        "ordinary base and Current dependency remain separate"
    );
    for entry in entries {
        assert_eq!(entry.static_source(), Some(&original));
        assert!(Arc::ptr_eq(entry.record(), origins.get(id).unwrap()));
        assert_eq!(
            entry.footprint(),
            FamilyTraceFootprint::Component(ProgrammingComponent::Color(ColorComponent::Amber))
        );
        assert!(!entry.fields().contains(Field::ColorRecipeAmber));
        assert!(!entry.fields().contains(Field::Uv));
    }
    assert_eq!(entries[0].role(), FamilyTraceRole::Authored);
    assert_eq!(entries[0].fields(), &scope([Field::ColorRecipeRed]));
    assert_eq!(
        entries[1].relationship(),
        FamilyTraceRole::CalculationDependency
    );
    assert_eq!(entries[1].role(), FamilyTraceRole::CalculationDependency);
    assert_eq!(
        entries[1].fields(),
        &scope([Field::ColorRecipeRed, Field::ColorRecipeGreen])
    );
    assert_eq!(
        entries[1].static_source().unwrap().role,
        DynamicStaticRole::Authored
    );
}

#[test]
fn a_stored_dependency_cannot_become_authored_by_surviving_as_ordinary_underlay() {
    let target = FixtureId::new();
    let mut origins = DynamicSourceOrigins::default();
    let mut source = static_entry(Some(scope([Field::ColorRecipeRed])));
    source.role = DynamicStaticRole::CalculationDependency;
    let id = bind_static(&mut origins, target, vec![source]);
    let mut result = DynamicFamilySourceProjection::default();
    result
        .project(
            &origins,
            target,
            ProgrammingOwner::Color,
            Some(&query([Field::ColorRecipeRed])),
            Some(id),
        )
        .unwrap();
    let entry = &result.entries().unwrap()[0];
    assert_eq!(entry.relationship(), FamilyTraceRole::Authored);
    assert_eq!(entry.role(), FamilyTraceRole::CalculationDependency);
}

#[test]
fn transfer_identity_scope_and_known_empty_have_distinct_non_blocking_results() {
    let target = FixtureId::new();
    let mut origins = DynamicSourceOrigins::default();
    let id = bind_static(
        &mut origins,
        target,
        vec![static_entry(Some(scope([Field::ColorRecipeRed])))],
    );
    let mut result = DynamicFamilySourceProjection::default();
    result
        .project(&origins, target, ProgrammingOwner::Color, None, Some(id))
        .unwrap();
    assert_eq!(result.unknown(), Some(DynamicFamilySourceUnknown::Transfer));
    result
        .project(
            &origins,
            target,
            ProgrammingOwner::Color,
            Some(&query([Field::ColorRecipeRed])),
            None,
        )
        .unwrap();
    assert_eq!(result.unknown(), Some(DynamicFamilySourceUnknown::Identity));
    let legacy = bind_static(&mut origins, target, vec![static_entry(None)]);
    result
        .project(
            &origins,
            target,
            ProgrammingOwner::Color,
            Some(&query([Field::ColorRecipeRed])),
            Some(legacy),
        )
        .unwrap();
    assert_eq!(
        result.unknown(),
        Some(DynamicFamilySourceUnknown::FieldScope)
    );
    // The generated zero Amber has no inherited source; that is known empty, not missing data.
    result
        .project(
            &origins,
            target,
            ProgrammingOwner::Color,
            Some(&query([Field::ColorRecipeAmber])),
            Some(id),
        )
        .unwrap();
    assert!(result.entries().unwrap().is_empty());
    result
        .project(
            &origins,
            target,
            ProgrammingOwner::Color,
            Some(&query([])),
            None,
        )
        .unwrap();
    assert!(
        result.entries().unwrap().is_empty(),
        "a cut baseline needs no identity"
    );
    let mut mixed = query([Field::ColorRecipeRed]);
    mixed.sources.push(leaf(None, scope([Field::Uv])));
    result
        .project(
            &origins,
            target,
            ProgrammingOwner::Color,
            Some(&mixed),
            Some(id),
        )
        .unwrap();
    assert_eq!(result.unknown(), Some(DynamicFamilySourceUnknown::Identity));
    assert!(
        result.entries.is_empty(),
        "a partial set must not be published as complete"
    );
}

#[test]
fn projection_retains_original_records_across_branch_edits_pruning_and_scratch_reuse() {
    let target = FixtureId::new();
    let mut live = DynamicSourceOrigins::default();
    let original = static_entry(Some(scope([Field::ColorRecipeRed])));
    let old = bind_static(&mut live, target, vec![original.clone()]);
    let mut preview = live.clone();
    let mut edited = original.clone();
    edited.programmer_order += 1;
    let next = bind_static(&mut preview, target, vec![edited]);
    let mut scratch = DynamicFamilySourceProjection::default();
    let request = query([Field::ColorRecipeRed]);
    scratch
        .project(
            &live,
            target,
            ProgrammingOwner::Color,
            Some(&request),
            Some(old),
        )
        .unwrap();
    let published = scratch.clone();
    scratch
        .project(
            &preview,
            target,
            ProgrammingOwner::Color,
            Some(&request),
            Some(next),
        )
        .unwrap();
    assert_eq!(published.entries().unwrap()[0].record().occurrence_id, old);
    assert_eq!(scratch.entries().unwrap()[0].record().occurrence_id, next);
    assert!(live.get(next).is_none());
    live.retain_bindings(|_| false);
    live.prune([]).unwrap();
    assert!(live.get(old).is_none());
    assert_eq!(
        published.entries().unwrap()[0].static_source(),
        Some(&original)
    );
}

#[test]
fn standalone_dynamic_playback_keeps_its_exact_record_without_fabricating_a_cue_source() {
    let target = FixtureId::new();
    let mut origins = DynamicSourceOrigins::default();
    let mut source = leaf(None, scope([Field::ColorRecipeRed]));
    source.source.role = FamilyTraceRole::Authored;
    let rank = source.source.rank;
    let id = origins
        .bind(
            DynamicSourceBinding::Authored {
                instance_id: rank.dynamic_identity().unwrap().instance_id,
                controller_id: rank.dynamic_identity().unwrap().controller_id,
                lane_id: rank.dynamic_identity().unwrap().lane_id,
                target,
            },
            DynamicSourceOrigin::Playback {
                identity: PlaybackIdentity::physical(4).unwrap(),
                activated_at: DateTime::from_timestamp(100, 500).unwrap(),
            },
        )
        .unwrap();
    source.source.occurrence = Some(id);
    let mut request = query([]);
    request.sources.push(source);
    let mut result = DynamicFamilySourceProjection::default();
    result
        .project(
            &origins,
            target,
            ProgrammingOwner::Color,
            Some(&request),
            None,
        )
        .unwrap();
    let entry = &result.entries().unwrap()[0];
    assert!(Arc::ptr_eq(entry.record(), origins.get(id).unwrap()));
    assert!(entry.static_source().is_none());
    assert!(matches!(
        entry.record().origin,
        DynamicSourceOrigin::Playback { .. }
    ));
    assert_eq!(entry.role(), FamilyTraceRole::Authored);
    assert_eq!(
        entry.footprint(),
        FamilyTraceFootprint::Component(ProgrammingComponent::Color(ColorComponent::Red))
    );
}

#[test]
fn invalid_target_or_owner_cannot_publish_a_partial_projection() {
    let target = FixtureId::new();
    let mut origins = DynamicSourceOrigins::default();
    let id = bind_static(
        &mut origins,
        target,
        vec![static_entry(Some(scope([Field::ColorRecipeRed])))],
    );
    let mut scratch = DynamicFamilySourceProjection::default();
    let request = query([Field::ColorRecipeRed]);
    scratch
        .project(
            &origins,
            target,
            ProgrammingOwner::Color,
            Some(&request),
            Some(id),
        )
        .unwrap();
    let published = scratch.clone();
    assert!(
        scratch
            .project(
                &origins,
                FixtureId::new(),
                ProgrammingOwner::Color,
                Some(&request),
                Some(id)
            )
            .is_err()
    );
    assert!(scratch.entries().is_none());
    assert_eq!(published.entries().unwrap().len(), 1);
    assert!(
        scratch
            .project(
                &origins,
                target,
                ProgrammingOwner::Position,
                Some(&request),
                Some(id)
            )
            .is_err()
    );
    assert!(scratch.entries().is_none());
}
