use super::*;

fn sequence() -> DynamicSequenceSource {
    DynamicSequenceSource {
        playback_number: Some(4),
        playback_identity: Some(PlaybackIdentity::physical(4).unwrap()),
        cue_list_id: CueListId::new(),
        temporary: false,
    }
}

fn entry(ordinal: Option<u64>) -> DynamicStaticSourceEntry {
    DynamicStaticSourceEntry {
        source: DynamicStaticSource::Playback { source: sequence() },
        changed_at: DateTime::from_timestamp(100, 99).unwrap(),
        programmer_order: 0,
        transition_ordinal: ordinal,
        authored_cue_id: Some(Uuid::new_v4()),
        footprint: DynamicStaticFootprint::Whole,
        role: DynamicStaticRole::Authored,
        effective_fields: None,
    }
}

#[test]
fn playback_watermark_includes_unbound_history_and_excludes_other_ordinal_domains() {
    let mut origins = DynamicSourceOrigins::default();
    let target = FixtureId::new();
    let binding = DynamicSourceBinding::StaticBaseline {
        target,
        owner: ProgrammingOwner::Focus,
    };
    let old = origins
        .bind(
            binding,
            DynamicSourceOrigin::StaticBaseline {
                sources: vec![entry(Some(71))],
            },
        )
        .unwrap();
    let mut programmer = entry(Some(u64::MAX));
    programmer.source = DynamicStaticSource::Programmer {
        programmer_id: ProgrammerId::new(),
        lane: DynamicStaticProgrammerLane::Live,
    };
    programmer.authored_cue_id = None;
    let latest = origins
        .bind(
            binding,
            DynamicSourceOrigin::StaticBaseline {
                sources: vec![entry(Some(3)), entry(None), programmer],
            },
        )
        .unwrap();
    assert_ne!(old, latest);
    let cue = DynamicSourceOrigin::Cue {
        source: sequence(),
        temporary_kind: None,
        cue_id: Uuid::new_v4(),
        instance_link: Uuid::new_v4(),
        changed_at: DateTime::from_timestamp(100, 99).unwrap(),
        transition_ordinal: u64::MAX,
    };
    origins
        .bind(
            DynamicSourceBinding::Authored {
                instance_id: Uuid::new_v4(),
                controller_id: cue.authored_controller_id().unwrap(),
                target,
                lane_id: Uuid::new_v4(),
            },
            cue,
        )
        .unwrap();
    assert_eq!(origins.playback_source_occurrence_watermark(), 71);
    let serialized = serde_json::to_string(&origins.snapshot()).unwrap();
    let mut restored =
        DynamicSourceOrigins::restore(serde_json::from_str(&serialized).unwrap(), [old]).unwrap();
    assert_eq!(restored.playback_source_occurrence_watermark(), 71);
    restored.prune([]).unwrap();
    assert_eq!(restored.playback_source_occurrence_watermark(), 3);
}

#[test]
fn playback_watermark_keeps_exhaustion_and_missing_legacy_ordinals_distinct() {
    let mut origins = DynamicSourceOrigins::default();
    assert_eq!(origins.playback_source_occurrence_watermark(), 0);
    let binding = DynamicSourceBinding::StaticBaseline {
        target: FixtureId::new(),
        owner: ProgrammingOwner::Focus,
    };
    origins
        .bind(
            binding,
            DynamicSourceOrigin::StaticBaseline {
                sources: vec![entry(None)],
            },
        )
        .unwrap();
    assert_eq!(origins.playback_source_occurrence_watermark(), 0);
    origins
        .bind(
            binding,
            DynamicSourceOrigin::StaticBaseline {
                sources: vec![entry(Some(u64::MAX))],
            },
        )
        .unwrap();
    assert_eq!(origins.playback_source_occurrence_watermark(), u64::MAX);
}
