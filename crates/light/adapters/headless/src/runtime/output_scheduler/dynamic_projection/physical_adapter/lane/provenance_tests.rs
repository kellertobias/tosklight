//! Real lane ownership tests; the seam adapter does not claim physical fitting accuracy.
use super::super::test_adapter::{SeamAdapter, SeamContinuity, SeamQuality, SeamValues};
use super::*;
use crate::runtime::dynamic_source_origins::{DynamicSourceBinding, DynamicSourceOrigins};
use light_core::{ProgrammerEditStamp, ProgrammerId, programming::ProgrammingTraceField};
use light_dynamics::{
    FamilySampleIdentity, FamilySampleRank, FamilyTraceFootprint, FamilyTraceQuery,
};
use light_engine::{
    ContributionFamilyEntry, ContributionFamilyEvidence, ContributionFamilyFootprint,
    ContributionFamilyRole, ContributionSourceId, Engine,
};
use light_programmer::ProgrammerRegistry;

fn target() -> FixtureId {
    FixtureId(uuid::Uuid::from_u128(17))
}
fn token(engine: &Engine) -> CapturedFrameToken {
    engine
        .prepare_output_frame(Default::default())
        .frame_token()
}
fn provenance(priority: i16) -> PhysicalProvenance {
    let fields = ProgrammingFieldScope::new([ProgrammingTraceField::Uv]);
    let evidence = Arc::new(ContributionFamilyEvidence::new(
        (0..2)
            .map(|index| {
                ContributionFamilyEntry::new(
                    ContributionSourceId::programmer_group(
                        ProgrammerId::new(),
                        format!("source{index}"),
                    ),
                    ProgrammerEditStamp {
                        changed_at: chrono::DateTime::from_timestamp(10, 0).unwrap(),
                        programmer_order: index,
                    },
                    ContributionFamilyFootprint::Whole,
                    ContributionFamilyRole::Authored,
                )
                .with_effective_fields(fields.clone())
            })
            .collect::<Vec<_>>(),
    ));
    let mut origins = DynamicSourceOrigins::default();
    let id = origins
        .bind_static_evidence(
            DynamicSourceBinding::StaticBaseline {
                target: target(),
                owner: ProgrammingOwner::Color,
            },
            &evidence,
        )
        .unwrap();
    let mut sources = DynamicFamilySourceProjection::default();
    sources
        .project(
            &origins,
            target(),
            ProgrammingOwner::Color,
            Some(&FamilyTraceQuery {
                base_fields: fields.clone(),
                ..Default::default()
            }),
            Some(id),
        )
        .unwrap();
    assert_eq!(sources.entries().unwrap().len(), 2);
    PhysicalProvenance::new(
        fields.clone(),
        sources,
        Some(
            (0..2)
                .map(|index| FamilyControlContribution {
                    rank: FamilySampleRank {
                        priority,
                        changed_at_millis: 10,
                        changed_at_submillis_nanos: 0,
                        stable_order: index,
                        identity: FamilySampleIdentity::Fixed {
                            source: light_dynamics::FamilyFixedSampleSource::Programmer,
                            row_index: index as usize,
                        },
                    },
                    footprint: FamilyTraceFootprint::Whole,
                    fields: fields.clone(),
                })
                .collect(),
        ),
    )
}
fn resolution() -> PhysicalResolution<SeamAdapter> {
    PhysicalResolution {
        writes: Vec::new(),
        requested: SeamValues {
            primary: 0.2,
            secondary: 0.3,
        },
        achieved: SeamValues {
            primary: 0.2,
            secondary: 0.3,
        },
        quality: SeamQuality {
            clipped: false,
            previous: None,
        },
        continuity: SeamContinuity {
            primary: 0.2,
            frames: 1,
        },
    }
}
fn stage(
    lane: &PhysicalAdapterLane<SeamAdapter>,
    token: &CapturedFrameToken,
    provenance: PhysicalProvenance,
) -> PhysicalHeadResult<SeamAdapter> {
    lane.stage_resolution(
        token,
        target(),
        ProgrammingOwner::Color,
        AttributeValue::Normalized(0.2),
        provenance,
        FamilyProjectionMetadata {
            changed_at: None,
            evidence: FamilyProjectionEvidence::Replace {
                origin: None,
                family_evidence: None,
            },
        },
        resolution(),
    )
    .unwrap()
    .1
}
fn same_storage(actual: &PhysicalProvenance, expected: &PhysicalProvenance) {
    let a = actual.sources.entries().unwrap();
    let b = expected.sources.entries().unwrap();
    assert!(!a.is_empty() && !b.is_empty());
    assert_eq!(
        a.as_ptr(),
        b.as_ptr(),
        "completed source entries must share storage"
    );
    let a = actual.controls.as_ref().unwrap();
    let b = expected.controls.as_ref().unwrap();
    assert!(!a.is_empty() && !b.is_empty());
    assert_eq!(
        a.as_ptr(),
        b.as_ptr(),
        "completed control contributions must share storage"
    );
}
#[test]
fn provenance_storage_shared_by_real_staging_sidecar_and_acceptance() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let lane = PhysicalAdapterLane::live(SeamAdapter::new(&[ProgrammingOwner::Color]));
    let token = token(&engine);
    lane.begin(&token).unwrap();
    let row = stage(&lane, &token, provenance(1));
    {
        let state = lane.state.borrow();
        same_storage(
            &state.staged.as_ref().unwrap().entries[0].2,
            &row.provenance,
        );
    }
    assert!(lane.accept(&token));
    same_storage(
        &lane.state.borrow().committed[&(target(), ProgrammingOwner::Color)].provenance,
        &row.provenance,
    );
}

#[test]
fn provenance_fresh_frames_preserve_retained_evidence_and_debug_shape() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let lane = PhysicalAdapterLane::live(SeamAdapter::new(&[ProgrammingOwner::Color]));
    let first = token(&engine);
    lane.begin(&first).unwrap();
    let old = stage(&lane, &first, provenance(1));
    let debug = format!(
        "PhysicalProvenance {{ fields: {:?}, sources: {:?}, controls: {:?} }}",
        old.provenance.fields, old.provenance.sources, old.provenance.controls
    );
    assert_eq!(format!("{:?}", old.provenance), debug);
    assert!(lane.accept(&first));
    let second = token(&engine);
    lane.begin(&second).unwrap();
    let fresh = stage(&lane, &second, provenance(2));
    assert_ne!(
        old.provenance.sources.entries().unwrap().as_ptr(),
        fresh.provenance.sources.entries().unwrap().as_ptr()
    );
    assert_eq!(
        old.provenance.controls.as_ref().unwrap()[0].rank.priority,
        1
    );
    assert_eq!(
        fresh.provenance.controls.as_ref().unwrap()[0].rank.priority,
        2
    );
    assert_eq!(format!("{:?}", old.provenance), debug);
    for (original, current) in old
        .provenance
        .sources
        .entries()
        .unwrap()
        .iter()
        .zip(fresh.provenance.sources.entries().unwrap())
    {
        assert!(!Arc::ptr_eq(original.record(), current.record()));
    }
    // Rollback never installs the new frame's evidence.
    lane.abandon();
    same_storage(
        &lane.state.borrow().committed[&(target(), ProgrammingOwner::Color)].provenance,
        &old.provenance,
    );
    let retry = token(&engine);
    lane.begin(&retry).unwrap();
    let retry_row = stage(&lane, &retry, provenance(3));
    assert!(lane.accept(&retry));
    same_storage(
        &lane.state.borrow().committed[&(target(), ProgrammingOwner::Color)].provenance,
        &retry_row.provenance,
    );
    assert_eq!(format!("{:?}", old.provenance), debug);
}

#[test]
fn provenance_hold_release_and_last_owner_lifetime_keep_original_evidence() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let lane = PhysicalAdapterLane::live(SeamAdapter::new(&[ProgrammingOwner::Color]));
    let first = token(&engine);
    lane.begin(&first).unwrap();
    let old = stage(&lane, &first, provenance(1));
    let weak_record = Arc::downgrade(old.provenance.sources.entries().unwrap()[0].record());
    assert!(lane.accept(&first));
    let hold = token(&engine);
    lane.begin(&hold).unwrap();
    let held = lane
        .stage_held_resolution(
            &hold,
            target(),
            ProgrammingOwner::Color,
            AttributeValue::Normalized(0.2),
            provenance(9),
            old.metadata.clone(),
            resolution(),
        )
        .unwrap()
        .1;
    assert!(lane.accept(&hold));
    same_storage(
        &lane.state.borrow().committed[&(target(), ProgrammingOwner::Color)].provenance,
        &old.provenance,
    );
    assert_eq!(
        held.provenance.controls.as_ref().unwrap()[0].rank.priority,
        9
    );
    let release = token(&engine);
    lane.begin(&release).unwrap();
    assert!(lane.accept(&release));
    let released = lane.released();
    assert_eq!(released.len(), 1);
    same_storage(&released[0].last_provenance, &old.provenance);
    assert_eq!(released[0].last_token, first);
    drop(lane);
    drop(old);
    assert!(
        weak_record.upgrade().is_some(),
        "the retained release owns original evidence"
    );
    drop(released);
    assert!(
        weak_record.upgrade().is_none(),
        "the last owner releases the original source record"
    );
}

#[test]
fn provenance_worker_staging_merge_and_truncation_share_completed_storage() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let lane = PhysicalAdapterLane::live(SeamAdapter::new(&[ProgrammingOwner::Color]));
    let token = token(&engine);
    lane.begin(&token).unwrap();
    let missed = std::cell::Cell::new(false);
    let (row, mut staging) = lane.with_shared(|shared| {
        let worker = LaneWorker::new(shared, &missed, LaneStaging::default());
        let row = LaneAccess::stage(
            &worker,
            &token,
            target(),
            ProgrammingOwner::Color,
            AttributeValue::Normalized(0.2),
            provenance(5),
            FamilyProjectionMetadata {
                changed_at: None,
                evidence: FamilyProjectionEvidence::Replace {
                    origin: None,
                    family_evidence: None,
                },
            },
            resolution(),
        )
        .unwrap()
        .1;
        (row, worker.into_staging())
    });
    lane.merge_staging(&token, &mut staging).unwrap();
    assert!(staging.is_empty());
    same_storage(
        &lane.state.borrow().staged.as_ref().unwrap().entries[0].2,
        &row.provenance,
    );
    assert!(lane.accept(&token));
    same_storage(
        &lane.state.borrow().committed[&(target(), ProgrammingOwner::Color)].provenance,
        &row.provenance,
    );
    assert!(!missed.get());
    let next = self::token(&engine);
    lane.begin(&next).unwrap();
    lane.with_shared(|shared| {
        let worker = LaneWorker::new(shared, &missed, LaneStaging::default());
        let mark = worker.staging().mark();
        let row = LaneAccess::stage(
            &worker,
            &next,
            target(),
            ProgrammingOwner::Color,
            AttributeValue::Normalized(0.2),
            provenance(6),
            row.metadata.clone(),
            resolution(),
        )
        .unwrap()
        .1;
        worker.staging().truncate(mark);
        assert!(worker.staging().is_empty());
        assert_eq!(
            row.provenance.controls.as_ref().unwrap()[0].rank.priority,
            6
        );
    });
    lane.abandon();
    same_storage(
        &lane.state.borrow().committed[&(target(), ProgrammingOwner::Color)].provenance,
        &row.provenance,
    );
}

#[test]
fn provenance_debug_matches_original_derived_struct_and_layout_is_one_arc() {
    fn original_debug(value: &super::PhysicalProvenance, pretty: bool) -> String {
        #[derive(Debug)]
        struct PhysicalProvenance<'a> {
            fields: &'a ProgrammingFieldScope,
            sources: &'a DynamicFamilySourceProjection,
            controls: &'a Option<Vec<FamilyControlContribution>>,
        }
        let original = PhysicalProvenance {
            fields: &value.fields,
            sources: &value.sources,
            controls: &value.controls,
        };
        if pretty {
            format!("{original:#?}")
        } else {
            format!("{original:?}")
        }
    }
    let value = provenance(7);
    assert_eq!(format!("{value:?}"), original_debug(&value, false));
    assert_eq!(format!("{value:#?}"), original_debug(&value, true));
    assert_eq!(
        std::mem::size_of::<PhysicalProvenance>(),
        std::mem::size_of::<usize>()
    );
    eprintln!(
        "provenance layout: wrapper={} data={} alignment={}",
        std::mem::size_of::<PhysicalProvenance>(),
        std::mem::size_of_val(&*value),
        std::mem::align_of_val(&*value)
    );
}
