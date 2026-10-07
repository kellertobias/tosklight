use super::*;
use crate::runtime::dynamic_source_origins::*;
use light_core::programming::ProgrammingOwner;

pub(super) fn checkpoint(ordinal: u64) -> DynamicRuntimeSourceCheckpoint {
    let mut origins = DynamicSourceOrigins::default();
    let binding = DynamicSourceBinding::StaticBaseline {
        target: light_core::FixtureId::new(),
        owner: ProgrammingOwner::Focus,
    };
    origins
        .bind(
            binding,
            DynamicSourceOrigin::StaticBaseline {
                sources: vec![DynamicStaticSourceEntry {
                    source: DynamicStaticSource::Playback {
                        source: DynamicSequenceSource {
                            playback_number: Some(1),
                            playback_identity: Some(
                                light_playback::PlaybackIdentity::physical(1).unwrap(),
                            ),
                            cue_list_id: light_core::CueListId::new(),
                            temporary: false,
                        },
                    },
                    changed_at: chrono::DateTime::from_timestamp(100, 33).unwrap(),
                    programmer_order: 0,
                    transition_ordinal: Some(ordinal),
                    authored_cue_id: None,
                    footprint: DynamicStaticFootprint::Whole,
                    role: DynamicStaticRole::Authored,
                    effective_fields: None,
                }],
            },
        )
        .unwrap();
    origins.unbind(&binding);
    DynamicRuntimeSourceCheckpoint {
        runtime: Default::default(),
        origins: Some(origins.snapshot()),
    }
}

#[test]
fn cold_startup_restore_reserves_unbound_catalogue_history_before_publishing() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let runtime = Mutex::new(light_dynamics::DynamicRuntime::default());
    let publication = crate::runtime::DynamicSnapshotPublication::new(engine.snapshot());
    let origins = Arc::new(arc_swap::ArcSwap::from_pointee(
        DynamicSourceOrigins::default(),
    ));
    // This is the same cold restore entry point used by bootstrap, without starting workers.
    OutputResource::restore_dynamic_source_state(
        &engine,
        &runtime,
        &origins,
        &publication,
        checkpoint(501),
    )
    .unwrap();
    assert_eq!(engine.playback_source_occurrence_watermark(), 501);
    assert_eq!(origins.load().playback_source_occurrence_watermark(), 501);
    assert!(origins.load().snapshot().bindings.is_empty());
    OutputResource::restore_dynamic_source_state(
        &engine,
        &runtime,
        &origins,
        &publication,
        checkpoint(9),
    )
    .unwrap();
    assert_eq!(engine.playback_source_occurrence_watermark(), 501);
    assert_eq!(origins.load().playback_source_occurrence_watermark(), 9);
}

#[test]
fn explicit_runtime_restore_reserves_history_and_rejects_invalid_catalogue_before_reserving() {
    let (state, data_dir) = test_state();
    state
        .output
        .restore_dynamic_source_checkpoint(checkpoint(601))
        .unwrap();
    assert_eq!(
        state.output.engine().playback_source_occurrence_watermark(),
        601
    );
    let mut malformed = checkpoint(999);
    malformed.origins.as_mut().unwrap().records[0].occurrence_id =
        light_dynamics::DynamicSourceOccurrenceId::new(Uuid::new_v4()).unwrap();
    let binding = malformed.origins.as_ref().unwrap().records[0].binding;
    malformed
        .origins
        .as_mut()
        .unwrap()
        .bindings
        .push(DynamicSourceActiveBinding {
            binding,
            occurrence_id: light_dynamics::DynamicSourceOccurrenceId::new(Uuid::new_v4()).unwrap(),
        });
    assert!(
        state
            .output
            .restore_dynamic_source_checkpoint(malformed)
            .is_err()
    );
    assert_eq!(
        state.output.engine().playback_source_occurrence_watermark(),
        601
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
