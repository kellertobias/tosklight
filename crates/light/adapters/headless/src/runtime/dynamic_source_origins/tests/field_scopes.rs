use super::*;
use light_core::programming::{ColorComponent, ProgrammingTraceField as Field};
use light_engine::{
    ContributionFamilyEntry, ContributionFamilyEvidence, ContributionFamilyFootprint as Footprint,
    ContributionFamilyRole as Role, ContributionSourceId,
};

fn amber_conversion() -> (DynamicSourceBinding, ContributionFamilyEntry) {
    (
        DynamicSourceBinding::StaticBaseline {
            target: FixtureId::new(),
            owner: ProgrammingOwner::Color,
        },
        ContributionFamilyEntry::new(
            ContributionSourceId::programmer_group(ProgrammerId::new(), "warm front"),
            light_core::ProgrammerEditStamp {
                changed_at: DateTime::from_timestamp(10, 123_456).unwrap(),
                programmer_order: 71,
            },
            Footprint::Component(ProgrammingComponent::Color(ColorComponent::Amber)),
            Role::CalculationDependency,
        )
        .with_effective_fields(ProgrammingFieldScope::new([
            Field::ColorXyz,
            Field::ColorRecipeRed,
            Field::ColorRecipeGreen,
            Field::ColorRecipeBlue,
        ])),
    )
}

#[test]
fn converted_field_scope_round_trips_without_rewriting_original_authorship() {
    let (binding, entry) = amber_conversion();
    let evidence = Arc::new(ContributionFamilyEvidence::new(vec![entry.clone()]));
    let mut origins = DynamicSourceOrigins::default();
    let id = origins.bind_static_evidence(binding, &evidence).unwrap();
    let wire = serde_json::to_value(origins.snapshot()).unwrap();
    let restored =
        DynamicSourceOrigins::restore(serde_json::from_value(wire.clone()).unwrap(), [id]).unwrap();
    let DynamicSourceOrigin::StaticBaseline { sources } = &restored.get(id).unwrap().origin else {
        panic!()
    };
    let converted = sources[0].family_entry();
    assert_eq!(converted.source(), entry.source());
    assert_eq!(converted.footprint(), entry.footprint());
    assert_eq!(converted.role(), Role::CalculationDependency);
    assert_eq!(converted.effective_fields(), entry.effective_fields());
    assert!(
        !converted
            .effective_fields()
            .unwrap()
            .contains(Field::ColorRecipeAmber)
    );

    // Older snapshots have only the original edit footprint. Preserve that absence rather
    // than pretending an Amber edit had already been converted to RGB.
    let mut legacy = wire;
    legacy["records"][0]["origin"]["sources"][0]
        .as_object_mut()
        .unwrap()
        .remove("effective_fields");
    let legacy =
        DynamicSourceOrigins::restore(serde_json::from_value(legacy).unwrap(), [id]).unwrap();
    let DynamicSourceOrigin::StaticBaseline { sources } = &legacy.get(id).unwrap().origin else {
        panic!()
    };
    assert!(sources[0].family_entry().effective_fields().is_none());
    assert_eq!(sources[0].family_entry().footprint(), entry.footprint());
}

#[test]
fn changed_effective_scope_invalidates_singleton_fast_path_and_multi_source_cache() {
    let (binding, entry) = amber_conversion();
    let singleton = Arc::new(ContributionFamilyEvidence::new(vec![entry.clone()]));
    let mut origins = DynamicSourceOrigins::default();
    let original = origins.bind_static_evidence(binding, &singleton).unwrap();
    let changed_entry = entry
        .clone()
        .with_effective_fields(ProgrammingFieldScope::new([
            Field::ColorXyz,
            Field::ColorRecipeRed,
        ]));
    let changed = Arc::new(ContributionFamilyEvidence::new(vec![changed_entry.clone()]));
    let narrowed = origins.bind_static_evidence(binding, &changed).unwrap();
    assert_ne!(
        original, narrowed,
        "same source stamp does not imply the same affected fields"
    );
    assert!(origins.static_evidence.is_empty());

    let companion = ContributionFamilyEntry::new(
        ContributionSourceId::preload(ProgrammerId::new()),
        entry.stamp(),
        Footprint::Whole,
        Role::Authored,
    );
    let multi = Arc::new(ContributionFamilyEvidence::new(vec![
        entry,
        companion.clone(),
    ]));
    let old_multi = origins.bind_static_evidence(binding, &multi).unwrap();
    let next_multi = Arc::new(ContributionFamilyEvidence::new(vec![
        changed_entry,
        companion,
    ]));
    let new_multi = origins.bind_static_evidence(binding, &next_multi).unwrap();
    assert_ne!(old_multi, new_multi);
    let frozen = origins.clone();
    assert_eq!(
        origins.bind_static_evidence(binding, &next_multi).unwrap(),
        new_multi
    );
    assert!(origins.shares_storage(&frozen));
    assert!(
        origins.get(original).is_some(),
        "held history remains addressable"
    );
}

#[test]
fn reordered_duplicate_wire_fields_reuse_the_saved_occurrence() {
    let (binding, entry) = amber_conversion();
    let evidence = Arc::new(ContributionFamilyEvidence::new(vec![entry]));
    let mut origins = DynamicSourceOrigins::default();
    let id = origins.bind_static_evidence(binding, &evidence).unwrap();
    let mut wire = serde_json::to_value(origins.snapshot()).unwrap();
    let fields = wire["records"][0]["origin"]["sources"][0]["effective_fields"]
        .as_array_mut()
        .unwrap();
    fields.reverse();
    fields.push(fields[0].clone());
    let mut restored =
        DynamicSourceOrigins::restore(serde_json::from_value(wire).unwrap(), [id]).unwrap();
    assert_eq!(restored.snapshot(), origins.snapshot());
    let frozen = restored.clone();
    assert_eq!(
        restored.bind_static_evidence(binding, &evidence).unwrap(),
        id
    );
    assert!(restored.shares_storage(&frozen));
}

#[test]
fn cross_family_and_nil_native_scopes_cannot_replace_valid_history() {
    let (binding, entry) = amber_conversion();
    let evidence = Arc::new(ContributionFamilyEvidence::new(vec![entry.clone()]));
    let mut origins = DynamicSourceOrigins::default();
    let id = origins.bind_static_evidence(binding, &evidence).unwrap();
    let frozen = origins.clone();
    for bad_field in [Field::Pan, Field::NativeColorChannel(Uuid::nil())] {
        let invalid = Arc::new(ContributionFamilyEvidence::new(vec![
            entry
                .clone()
                .with_effective_fields(ProgrammingFieldScope::new([bad_field])),
        ]));
        assert!(origins.bind_static_evidence(binding, &invalid).is_err());
        assert!(origins.shares_storage(&frozen));
        assert_eq!(origins.binding(&binding), Some(id));

        let mut wire = serde_json::to_value(origins.snapshot()).unwrap();
        wire["records"][0]["origin"]["sources"][0]["effective_fields"] =
            serde_json::to_value(ProgrammingFieldScope::new([bad_field])).unwrap();
        let candidate = serde_json::from_value::<DynamicSourceOriginsSnapshot>(wire);
        assert!(
            candidate.is_err() || DynamicSourceOrigins::restore(candidate.unwrap(), [id]).is_err()
        );
        assert!(origins.shares_storage(&frozen));
    }
}

#[test]
fn authored_cue_metadata_is_retained_validated_and_part_of_cached_identity() {
    let binding = DynamicSourceBinding::StaticBaseline {
        target: FixtureId::new(),
        owner: ProgrammingOwner::Focus,
    };
    let cue = Uuid::new_v4();
    let entry = ContributionFamilyEntry::new(
        ContributionSourceId::playback(source(false).sequence_master_source()),
        light_core::ProgrammerEditStamp {
            changed_at: DateTime::from_timestamp(3, 4).unwrap(),
            programmer_order: 0,
        },
        Footprint::Whole,
        Role::Authored,
    )
    .with_transition_ordinal(Some(17))
    .with_authored_cue_id(Some(cue))
    .with_effective_fields(ProgrammingFieldScope::new([Field::Focus]));
    let evidence = |entry| Arc::new(ContributionFamilyEvidence::new(vec![entry]));
    let mut origins = DynamicSourceOrigins::default();
    let original = origins
        .bind_static_evidence(binding, &evidence(entry.clone()))
        .unwrap();
    let changed = origins
        .bind_static_evidence(
            binding,
            &evidence(entry.clone().with_authored_cue_id(Some(Uuid::new_v4()))),
        )
        .unwrap();
    assert_ne!(original, changed);
    let restored = DynamicSourceOrigins::restore(
        serde_json::from_value(serde_json::to_value(origins.snapshot()).unwrap()).unwrap(),
        [original, changed],
    )
    .unwrap();
    let DynamicSourceOrigin::StaticBaseline { sources } = &restored.get(original).unwrap().origin
    else {
        panic!()
    };
    assert_eq!(sources[0].family_entry().authored_cue_id(), Some(cue));
    // Historical Cue IDs may belong to deleted Cues. Validate the ID, not today's Cue list.
    let frozen = origins.clone();
    assert!(
        origins
            .bind_static_evidence(
                binding,
                &evidence(entry.with_authored_cue_id(Some(Uuid::nil())))
            )
            .is_err()
    );
    assert!(origins.shares_storage(&frozen));
    let wrong_source = ContributionFamilyEntry::new(
        ContributionSourceId::programmer(ProgrammerId::new()),
        light_core::ProgrammerEditStamp {
            changed_at: DateTime::from_timestamp(3, 4).unwrap(),
            programmer_order: 0,
        },
        Footprint::Whole,
        Role::Authored,
    )
    .with_authored_cue_id(Some(cue));
    assert!(
        origins
            .bind_static_evidence(binding, &evidence(wrong_source))
            .is_err()
    );
    assert!(origins.shares_storage(&frozen));
}
