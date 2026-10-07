use super::*;

#[test]
fn fixed_focus_capture_preserves_typed_mask_and_legacy_normalized_value() {
    let value = DynamicSemanticValue::FixAt {
        value: 0.4,
        timing: light_dynamics::DynamicValueTiming::default(),
    };
    let mask = captured_programming_fixed_mask(&ProgrammingOwner::Focus.key(), &value)
        .unwrap()
        .unwrap();
    assert_eq!(mask.family, AttributeValue::Normalized(0.4));
    assert_eq!(mask.address.component, None);
    assert!(
        captured_programming_fixed_mask(&AttributeKey::intensity(), &value)
            .unwrap()
            .is_none()
    );
}

use light_core::{ProgrammerEditStamp, programming::*};
use light_dynamics::{
    DynamicValueTiming, FamilySampleRank, FamilyTraceContribution, FamilyTraceFootprint,
    FamilyTraceQuery, FamilyTraceRole, FamilyTraceSource,
};

type ProgrammerRow = (Uuid, i16, DynamicAddressValue);

fn programmer_row(
    value: DynamicSemanticValue,
    owner: ProgrammingOwner,
) -> (ProgrammerRow, light_engine::CapturedDynamicProgrammerRow) {
    let programmer_id = ProgrammerId(Uuid::from_u128(77));
    let row = DynamicAddressValue {
        fixture_id: FixtureId(Uuid::from_u128(31)),
        attribute: owner.key(),
        value,
        programmer_order: 3,
        changed_at_millis: 1_000,
    };
    let sidecar = light_engine::CapturedDynamicProgrammerRow {
        programmer_id,
        lane: light_engine::CapturedDynamicProgrammerLane::Live,
        source: light_engine::ContributionSourceId::programmer(programmer_id),
        stamp: Some(ProgrammerEditStamp {
            changed_at: DateTime::from_timestamp_millis(1_000).unwrap(),
            programmer_order: 3,
        }),
        changed_at_millis: row.changed_at_millis,
        programmer_order: row.programmer_order,
    };
    ((programmer_id.0, 100, row), sidecar)
}

fn angle_mask(component: ProgrammingComponent) -> DynamicSemanticValue {
    DynamicSemanticValue::ProgrammingFixAt {
        mask: ProgrammingFamilyFixAt::from_family(
            ProgrammingOwner::Position,
            Some(component),
            AttributeValue::Position(Arc::new(PositionIntent::angles(90.0, 35.0))),
        )
        .unwrap(),
        timing: DynamicValueTiming {
            delay_millis: Some(120),
            fade_millis: Some(800),
        },
    }
}

fn cue_row() -> light_playback::ActiveCueDynamicValue {
    let source = SequenceMasterSource {
        playback_number: Some(1_301),
        playback_identity: Some(PlaybackIdentity::virtual_playback(2, 1_301).unwrap()),
        cue_list_id: CueListId(Uuid::from_u128(42)),
        temporary: false,
    };
    let changed_at = DateTime::from_timestamp(100, 123_456_789).unwrap();
    light_playback::ActiveCueDynamicValue {
        source,
        source_key: light_playback::CueDynamicSourceKey::Normal { source },
        output_enabled: true,
        sequence_master: 0.6,
        snap_sequence_master: 0.7,
        playback_number: source.playback_number,
        cue_list_id: source.cue_list_id,
        authored_cue_id: Uuid::from_u128(43),
        current_cue_id: Uuid::from_u128(44),
        priority: 20,
        changed_at,
        transition_ordinal: 9,
        changed_at_millis: changed_at.timestamp_millis() as u64,
        fixture_id: FixtureId(Uuid::from_u128(31)),
        attribute: ProgrammingOwner::Focus.key(),
        value: DynamicSemanticValue::Static {
            value: AttributeValue::Normalized(0.0),
            timing: DynamicValueTiming {
                delay_millis: Some(300),
                fade_millis: Some(600),
            },
        },
    }
}

#[test]
fn fixed_rows_reuse_occurrences_and_keep_components_lanes_and_changed_assignments_distinct() {
    let (pan, pan_source) = programmer_row(
        angle_mask(ProgrammingComponent::Pan),
        ProgrammingOwner::Position,
    );
    let (tilt, tilt_source) = programmer_row(
        angle_mask(ProgrammingComponent::Tilt),
        ProgrammingOwner::Position,
    );
    let rows = vec![pan.clone(), tilt.clone()];
    let sources = vec![pan_source.clone(), tilt_source];
    let mut origins = DynamicSourceOrigins::default();
    let first = origins
        .reconcile_captured_programming_fixed_sources(&rows, Some(&sources), &[])
        .unwrap();
    assert_eq!(first.len(), 2);
    assert_ne!(first[0].binding, first[1].binding);
    assert_ne!(first[0].occurrence_id, first[1].occurrence_id);
    let frozen = origins.clone();
    assert_eq!(
        first,
        origins
            .reconcile_captured_programming_fixed_sources(&rows, Some(&sources), &[])
            .unwrap()
    );
    assert!(frozen.shares_storage(&origins));
    assert_eq!(
        origins
            .get(first[0].occurrence_id)
            .unwrap()
            .origin
            .authored_controller_id(),
        None
    );

    let mut changed_pan = pan.clone();
    changed_pan.2.programmer_order += 1;
    let mut changed_source = pan_source.clone();
    changed_source.programmer_order += 1;
    changed_source.stamp.as_mut().unwrap().programmer_order += 1;
    let changed = origins
        .reconcile_captured_programming_fixed_sources(
            &[changed_pan, tilt],
            Some(&[changed_source, sources[1].clone()]),
            &[],
        )
        .unwrap();
    assert_ne!(changed[0].occurrence_id, first[0].occurrence_id);
    assert_eq!(changed[1].occurrence_id, first[1].occurrence_id);
    assert_eq!(
        origins.get(first[0].occurrence_id),
        frozen.get(first[0].occurrence_id)
    );
    let DynamicSourceOrigin::Fixed { value, .. } =
        &origins.get(first[0].occurrence_id).unwrap().origin
    else {
        panic!()
    };
    assert_eq!(*value, pan.2.value);

    let mut preload = pan_source;
    preload.lane = light_engine::CapturedDynamicProgrammerLane::Preload;
    preload.source = light_engine::ContributionSourceId::preload(preload.programmer_id);
    let pending = origins
        .reconcile_captured_programming_fixed_sources(&[pan], Some(&[preload]), &[])
        .unwrap();
    assert_ne!(pending[0].binding, first[0].binding);
}

#[test]
fn fixed_cue_identity_retains_exact_time_author_temporary_kind_and_original_zero_payload() {
    let mut row = cue_row();
    let mut origins = DynamicSourceOrigins::default();
    let initial = origins
        .reconcile_captured_programming_fixed_sources(&[], Some(&[]), &[row.clone()])
        .unwrap()[0];
    let record = origins.get(initial.occurrence_id).unwrap();
    let DynamicSourceOrigin::Fixed { stamp, value, .. } = &record.origin else {
        panic!()
    };
    assert_eq!(stamp.exact_changed_at(), Some(row.changed_at));
    assert_eq!(*value, row.value);
    assert!(
        matches!(stamp, DynamicFixedStamp::Cue { authored_cue_id, transition_ordinal: 9, .. } if *authored_cue_id == row.authored_cue_id)
    );
    row.output_enabled = false;
    row.sequence_master = 0.0;
    row.snap_sequence_master = 0.0;
    row.current_cue_id = Uuid::from_u128(45);
    assert_eq!(
        origins
            .reconcile_captured_programming_fixed_sources(&[], Some(&[]), &[row.clone()])
            .unwrap()[0],
        initial
    );
    row.changed_at += chrono::Duration::nanoseconds(1);
    let next = origins
        .reconcile_captured_programming_fixed_sources(&[], Some(&[]), &[row.clone()])
        .unwrap()[0];
    assert_ne!(next.occurrence_id, initial.occurrence_id);
    assert_eq!(next.binding, initial.binding);
    row.source.temporary = true;
    row.source_key = light_playback::CueDynamicSourceKey::Temporary {
        source: row.source,
        kind: TemporaryPlaybackKind::Flash,
    };
    let temporary = origins
        .reconcile_captured_programming_fixed_sources(&[], Some(&[]), &[row])
        .unwrap()[0];
    assert_ne!(temporary.binding, initial.binding);
}

#[test]
fn fixed_release_retires_only_its_binding_and_retained_record_survives_publication_and_pruning() {
    let (mut row, source) = programmer_row(
        angle_mask(ProgrammingComponent::Pan),
        ProgrammingOwner::Position,
    );
    let mut origins = DynamicSourceOrigins::default();
    let fixed = origins
        .reconcile_captured_programming_fixed_sources(&[row.clone()], Some(&[source.clone()]), &[])
        .unwrap()[0];
    let retained = Arc::clone(origins.get(fixed.occurrence_id).unwrap());
    let branch = origins.clone();
    row.2.value = DynamicSemanticValue::ProgrammingRelease {
        component: Some(ProgrammingComponent::Pan),
    };
    assert!(
        origins
            .reconcile_captured_programming_fixed_sources(&[row], Some(&[source]), &[])
            .unwrap()
            .is_empty()
    );
    assert_eq!(origins.binding(&fixed.binding), None);
    assert_eq!(branch.binding(&fixed.binding), Some(fixed.occurrence_id));
    assert_eq!(origins.get(fixed.occurrence_id), Some(&retained));
    assert_eq!(origins.prune([fixed.occurrence_id]).unwrap(), 0);
    assert_eq!(origins.prune([]).unwrap(), 1);
    assert_eq!(branch.get(fixed.occurrence_id), Some(&retained));
}

#[test]
fn missing_fixed_programmer_sidecar_means_unknown_authorship_not_absent_mask() {
    let (row, _) = programmer_row(
        angle_mask(ProgrammingComponent::Pan),
        ProgrammingOwner::Position,
    );
    let mut origins = DynamicSourceOrigins::default();
    assert!(
        origins
            .reconcile_captured_programming_fixed_sources(&[row.clone()], None, &[])
            .unwrap()
            .is_empty()
    );
    // The later compiler reads the original row even when the optional occurrence is unknown.
    let mask = captured_programming_fixed_mask(&row.2.attribute, &row.2.value)
        .unwrap()
        .unwrap();
    assert_eq!(mask.address.component, Some(ProgrammingComponent::Pan));
    assert_eq!(
        mask.family,
        AttributeValue::Position(Arc::new(PositionIntent::angles(90.0, 35.0)))
    );
}

#[test]
fn duplicate_or_inconsistent_fixed_capture_rejects_before_catalogue_mutation() {
    let (row, source) = programmer_row(
        angle_mask(ProgrammingComponent::Pan),
        ProgrammingOwner::Position,
    );
    let mut origins = DynamicSourceOrigins::default();
    origins
        .reconcile_captured_programming_fixed_sources(&[row.clone()], Some(&[source.clone()]), &[])
        .unwrap();
    let original = origins.clone();
    let duplicate = origins.reconcile_captured_programming_fixed_sources(
        &[row.clone(), row.clone()],
        Some(&[source.clone(), source.clone()]),
        &[],
    );
    assert!(duplicate.is_err());
    assert!(origins.shares_storage(&original));
    let mut invalid = source.clone();
    invalid.programmer_order += 1;
    assert!(
        origins
            .reconcile_captured_programming_fixed_sources(
                &[row.clone(), row],
                Some(&[source, invalid]),
                &[]
            )
            .is_err()
    );
    assert!(origins.shares_storage(&original));
}

#[test]
fn fixed_checkpoint_roundtrip_preserves_masks_and_old_version_one_reader_contract() {
    let (row, source) = programmer_row(
        angle_mask(ProgrammingComponent::Pan),
        ProgrammingOwner::Position,
    );
    let mut origins = DynamicSourceOrigins::default();
    let fixed = origins
        .reconcile_captured_programming_fixed_sources(&[row], Some(&[source]), &[cue_row()])
        .unwrap();
    let checkpoint = DynamicRuntimeSourceCheckpoint::capture(Default::default(), &origins).unwrap();
    let json = serde_json::to_value(&checkpoint).unwrap();
    assert_eq!(json["origins"]["version"], 1);
    let restored: DynamicRuntimeSourceCheckpoint = serde_json::from_value(json.clone()).unwrap();
    let (_, loaded) = restored.restore().unwrap();
    assert_eq!(loaded.snapshot(), origins.snapshot());
    loaded
        .validate_reachable(fixed.iter().map(|row| row.occurrence_id))
        .unwrap();
    assert!(
        loaded
            .validate_reachable([DynamicSourceOccurrenceId::new(Uuid::new_v4()).unwrap()])
            .is_err()
    );

    let legacy: DynamicRuntimeSourceCheckpoint = serde_json::from_value(
        serde_json::json!({"runtime": {"global_paused": false, "instances": []}}),
    )
    .unwrap();
    let (_, legacy_origins) = legacy.restore().unwrap();
    assert!(legacy_origins.snapshot().records.is_empty());
    // Earlier valid v1 snapshots with no new variants retain their original shape.
    let old = DynamicSourceOriginsSnapshot {
        version: 1,
        records: vec![],
        bindings: vec![],
    };
    assert!(
        DynamicSourceOrigins::restore(old, [])
            .unwrap()
            .snapshot()
            .records
            .is_empty()
    );

    let mut invalid = checkpoint.origins.unwrap();
    let DynamicSourceBinding::Fixed { component, .. } = &mut invalid.records[0].binding else {
        panic!()
    };
    *component = Some(ProgrammingComponent::Tilt);
    assert!(DynamicSourceOrigins::restore(invalid, []).is_err());
}

#[test]
fn fixed_projection_uses_original_source_and_footprint_without_dynamic_runtime_identity() {
    let (row, source) = programmer_row(
        angle_mask(ProgrammingComponent::Pan),
        ProgrammingOwner::Position,
    );
    let mut origins = DynamicSourceOrigins::default();
    let captured = origins
        .reconcile_captured_programming_fixed_sources(&[row.clone()], Some(&[source]), &[])
        .unwrap()[0];
    let mut query = FamilyTraceQuery {
        sources: vec![FamilyTraceContribution {
            source: FamilyTraceSource {
                // Source projection must resolve the occurrence, never infer a runtime scope.
                rank: FamilySampleRank {
                    priority: 100,
                    changed_at_millis: 1000,
                    changed_at_submillis_nanos: 0,
                    stable_order: 3,
                    identity: light_dynamics::FamilySampleIdentity::Fixed {
                        source: light_dynamics::FamilyFixedSampleSource::Programmer,
                        row_index: 0,
                    },
                },
                footprint: FamilyTraceFootprint::Component(ProgrammingComponent::Pan),
                role: FamilyTraceRole::Authored,
                occurrence: Some(captured.occurrence_id),
            },
            fields: ProgrammingFieldScope::from_component(ProgrammingComponent::Pan),
        }],
        ..Default::default()
    };
    let mut projection = DynamicFamilySourceProjection::default();
    projection
        .project(
            &origins,
            row.2.fixture_id,
            ProgrammingOwner::Position,
            Some(&query),
            None,
        )
        .unwrap();
    let entries = projection.entries().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].record().binding, captured.binding);
    assert_eq!(
        entries[0].record().origin,
        origins.get(captured.occurrence_id).unwrap().origin
    );
    assert_eq!(
        entries[0].footprint(),
        FamilyTraceFootprint::Component(ProgrammingComponent::Pan)
    );
    let retained = projection.clone();
    origins
        .reconcile_captured_programming_fixed_sources(&[], Some(&[]), &[])
        .unwrap();
    origins.prune([]).unwrap();
    assert_eq!(
        retained.entries().unwrap()[0].record().occurrence_id,
        captured.occurrence_id
    );
    let (_, source) = programmer_row(row.2.value.clone(), ProgrammingOwner::Position);
    let fresh = origins
        .reconcile_captured_programming_fixed_sources(&[row.clone()], Some(&[source]), &[])
        .unwrap()[0];
    query.sources[0].source.occurrence = Some(fresh.occurrence_id);
    query.sources[0].source.footprint = FamilyTraceFootprint::Whole;
    assert!(
        projection
            .project(
                &origins,
                row.2.fixture_id,
                ProgrammingOwner::Position,
                Some(&query),
                None
            )
            .is_err()
    );
}

#[test]
fn unavailable_direct_model_keeps_the_original_complete_recipe_and_uv_evidence() {
    use light_core::{
        NativeColorBinding, NativeColorIdentity, NativeColorValue, PhysicalDataQuality,
    };
    let native = NativeColorBinding {
        channel_id: Uuid::from_u128(5),
        function_id: Uuid::from_u128(6),
    };
    let family = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: NativeColorIdentity {
                profile_id: Uuid::from_u128(1),
                profile_revision: 3,
                profile_digest: "unavailable-original-profile".into(),
                mode_id: Uuid::from_u128(2),
                head_id: Uuid::from_u128(3),
                path_id: Uuid::from_u128(4),
                model_revision: 2,
                native_layout_signature: "rgb-wheel-uv".into(),
            },
            channels: vec![
                NativeColorValue {
                    channel_id: native.channel_id,
                    function_id: native.function_id,
                    raw: u32::MAX - 1,
                },
                NativeColorValue {
                    channel_id: Uuid::from_u128(7),
                    function_id: Uuid::from_u128(8),
                    raw: 17,
                },
            ],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 2,
            visible: None,
            uv: Some(PortableUv {
                amount: 0.8,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Unknown,
            limitations: vec!["visible appearance unknown".into()],
        },
    }));
    let value = DynamicSemanticValue::ProgrammingFixAt {
        mask: ProgrammingFamilyFixAt::from_family(
            ProgrammingOwner::Color,
            Some(ProgrammingComponent::NativeColor(native)),
            family.clone(),
        )
        .unwrap(),
        timing: DynamicValueTiming {
            delay_millis: Some(40),
            fade_millis: Some(500),
        },
    };
    let (row, sidecar) = programmer_row(value.clone(), ProgrammingOwner::Color);
    let mut origins = DynamicSourceOrigins::default();
    // Source capture has no model resolver: absence of the original profile is not a missing row.
    let captured = origins
        .reconcile_captured_programming_fixed_sources(&[row], Some(&[sidecar]), &[])
        .unwrap()[0];
    let DynamicSourceOrigin::Fixed { value: stored, .. } =
        &origins.get(captured.occurrence_id).unwrap().origin
    else {
        panic!()
    };
    assert_eq!(*stored, value);
    let mask = captured_programming_fixed_mask(&ProgrammingOwner::Color.key(), stored)
        .unwrap()
        .unwrap();
    assert_eq!(mask.family, family);
    assert_eq!(
        mask.address.component,
        Some(ProgrammingComponent::NativeColor(native))
    );
    let checkpoint = DynamicRuntimeSourceCheckpoint::capture(Default::default(), &origins).unwrap();
    let (_, restored) = serde_json::from_value::<DynamicRuntimeSourceCheckpoint>(
        serde_json::to_value(checkpoint).unwrap(),
    )
    .unwrap()
    .restore()
    .unwrap();
    assert_eq!(restored.snapshot(), origins.snapshot());
}
