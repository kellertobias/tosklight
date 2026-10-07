//! TL-639 round 2: the head lookups answer exactly as the per-head maps they replaced, for a
//! frame read by number (with unnumbered values beside it) and for values handed over by name.
use super::*;
use crate::contribution::{EngineContribution, EngineContributionResolver, PlaybackEvidenceCache};
use chrono::Utc;
use light_core::{CueListId, MergeMode, TimedValue};
use light_playback::{PlaybackContribution, SequenceMasterSource};

fn timed(fixture: FixtureId, attribute: &str, level: f32) -> TimedValue {
    TimedValue {
        fixture_id: fixture,
        attribute: AttributeKey(attribute.into()),
        value: AttributeValue::Normalized(level),
        priority: 0,
        changed_at: Utc::now(),
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    }
}

/// A value with a sequence master (a played Cue) or without one (the Programmer).
fn contribution(
    fixture: FixtureId,
    attribute: &str,
    level: f32,
    master: Option<f32>,
) -> EngineContribution {
    match master {
        Some(scale) => EngineContribution::from_playback(
            PlaybackContribution {
                value: timed(fixture, attribute, level),
                family_evidence: None,
                authored_target: true,
                transition_ordinal: 1,
                sequence_master: scale,
                source: SequenceMasterSource {
                    playback_number: Some(1),
                    playback_identity: None,
                    cue_list_id: CueListId::new(),
                    temporary: false,
                },
                address: None,
                pending_transition: None,
            },
            &mut PlaybackEvidenceCache::default(),
        ),
        None => EngineContribution::unscaled(timed(fixture, attribute, level)),
    }
}

fn assert_like_the_maps(index: &ProfileValueIndex<'_>, fixture: FixtureId) {
    let values = index.values(fixture);
    for name in [
        "intensity",
        "pan",
        "red",
        "color",
        "unnumbered",
        "unmastered",
        "absent",
    ] {
        let attribute = AttributeKey(name.into());
        assert_eq!(
            index.head_value(fixture, &attribute),
            values.get(&attribute),
            "value of {name}"
        );
    }
    let mut keys = Vec::new();
    index.for_each_head_attribute(fixture, |attribute| keys.push(attribute.clone()));
    keys.sort_by(|left, right| left.0.cmp(&right.0));
    keys.dedup();
    let mut expected = values.keys().cloned().collect::<Vec<_>>();
    expected.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(keys, expected);
}

#[test]
fn head_lookups_answer_like_the_head_maps_they_replaced() {
    let fixture_id = FixtureId::new();
    let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, &["intensity", "pan", "red"]);
    let slots = std::sync::Arc::new(crate::SlotTable::compile(1, std::slice::from_ref(&fixture)));
    let channels = crate::ChannelSlotIndex::compile(std::slice::from_ref(&fixture), &slots);
    let contributions = || {
        [
            contribution(fixture_id, "intensity", 0.4, Some(0.5)),
            contribution(fixture_id, "pan", 0.2, None),
            contribution(fixture_id, "unnumbered", 0.7, Some(0.25)),
            contribution(fixture_id, "unmastered", 0.1, None),
        ]
    };
    let mut resolver = EngineContributionResolver::unpooled(&slots);
    resolver.extend(contributions());
    let resolved = resolver.finish();
    let frame = crate::FrameValues::from_frame(resolved.frame.expect("a dense frame"));
    let dense = ProfileValueIndex::new(&frame, &channels);
    assert!(matches!(dense, ProfileValueIndex::Dense { .. }));
    assert_like_the_maps(&dense, fixture_id);

    // The same values handed over by name.
    let mut by_name = crate::ResolvedValues::default();
    for value in contributions() {
        let (fixture, attribute) = (value.fixture_id(), value.attribute().clone());
        by_name.insert((fixture, attribute), value.timed_value().value.clone());
    }
    let named = crate::FrameValues::from_maps(by_name, Default::default());
    let scanned = ProfileValueIndex::new(&named, &channels);
    assert!(matches!(scanned, ProfileValueIndex::Scanned { .. }));
    assert_like_the_maps(&scanned, fixture_id);
}
