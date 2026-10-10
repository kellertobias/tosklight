use super::*;
use light_core::CueListId;
use light_playback::SequenceMasterSource;

/// A show of one fixture that declares the attributes these tests arbitrate.
fn resolved(
    fixture_id: FixtureId,
    attributes: &[&str],
    contributions: impl IntoIterator<Item = EngineContribution>,
) -> crate::FrameValues {
    let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, attributes);
    let slots = std::sync::Arc::new(crate::SlotTable::compile(1, std::slice::from_ref(&fixture)));
    let mut resolver = EngineContributionResolver::unpooled(&slots);
    resolver.extend(contributions);
    resolver.finish().named_values()
}

fn playback_value(
    fixture_id: FixtureId,
    value: f32,
    merge_mode: MergeMode,
    changed_at: DateTime<Utc>,
    transition_ordinal: u64,
) -> EngineContribution {
    EngineContribution::from_playback(
        PlaybackContribution {
            replacement_projection: None,
            authored_target: true,
            family_evidence: None,
            value: TimedValue {
                fixture_id,
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(value),
                priority: 10,
                changed_at,
                programmer_order: 0,
                merge_mode,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            },
            transition_ordinal,
            sequence_master: 1.0,
            source: SequenceMasterSource {
                playback_number: None,
                playback_identity: None,
                cue_list_id: CueListId::new(),
                temporary: false,
            },
            address: None,
            pending_transition: None,
        },
        &mut PlaybackEvidenceCache::default(),
    )
}

#[test]
fn playback_endpoint_value_proof_does_not_reconstruct_missing_historical_evidence() {
    let fixture = FixtureId::new();
    let mut contribution = playback_value(fixture, 0.5, MergeMode::Ltp, Utc::now(), 17);
    contribution.value.attribute = AttributeKey("focus".into());
    let values = [contribution];
    let index = ResolvedContributionIndex::new(&values);
    assert_eq!(
        index.value(fixture, &AttributeKey("focus".into())),
        Some(&AttributeValue::Normalized(0.5))
    );
    assert!(
        index
            .family_evidence(fixture, &AttributeKey("focus".into()))
            .is_none()
    );
}

/// A value the compiled patch could not number must still reach the boundary and projection,
/// and must not cost the rest of the frame its dense reading. One unrecognised name from a
/// hardware surface or an HTTP client is a lookup, not a slower desk.
/// The slot table is compiled from the patch and from nothing else, so an inbound surface —
/// OSC, HTTP, anything that names an attribute freely — cannot grow it. That is the bound the
/// item asks for, and it is zero rather than a limit: a name the patch never declared is an
/// overflow entry on the frame that made it, discarded with that frame.
#[test]
fn an_inbound_name_cannot_grow_the_slot_table() {
    let fixture_id = FixtureId::new();
    let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, &["intensity"]);
    let slots = std::sync::Arc::new(crate::SlotTable::compile(1, std::slice::from_ref(&fixture)));
    let numbered = slots.len();
    for index in 0..64 {
        let mut resolver = EngineContributionResolver::unpooled(&slots);
        resolver.add_borrowed_unscaled(
            fixture_id,
            &AttributeKey(format!("inbound{index}").into()),
            &AttributeValue::Normalized(0.5),
            0,
            Utc::now(),
            MergeMode::Ltp,
        );
        let _ = resolver.finish();
    }
    assert_eq!(
        slots.len(),
        numbered,
        "sixty-four names the patch never declared leave the numbering exactly as it was"
    );
}

#[test]
fn a_value_the_patch_never_numbered_costs_itself_a_lookup_not_the_frame() {
    let fixture_id = FixtureId::new();
    let undeclared = AttributeKey("neverPatched".into());
    let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, &["intensity"]);
    let slots = std::sync::Arc::new(crate::SlotTable::compile(1, std::slice::from_ref(&fixture)));
    let mut resolver = EngineContributionResolver::unpooled(&slots);
    resolver.add_borrowed_unscaled(
        fixture_id,
        &undeclared,
        &AttributeValue::Normalized(0.42),
        0,
        Utc::now(),
        MergeMode::Ltp,
    );
    let mut resolved = resolver.finish();
    let values = resolved.named_values();
    assert_eq!(
        values.value(fixture_id, &undeclared),
        Some(&AttributeValue::Normalized(0.42)),
        "an operator's value is never lost to a name the patch did not declare"
    );
    assert!(
        values.is_dense(),
        "one unnumbered name costs that value a lookup, not the whole frame its dense reading"
    );
    assert!(values.has_unnumbered_values());
    assert_eq!(
        values.values()[&(fixture_id, undeclared)],
        AttributeValue::Normalized(0.42),
        "and it is there when the boundary asks for everything by name"
    );
}

#[test]
fn equal_timestamp_playback_ltp_uses_transition_order() {
    let fixture_id = FixtureId::new();
    let at = Utc::now();
    let resolved = resolved(
        fixture_id,
        &["intensity"],
        [
            playback_value(fixture_id, 0.8, MergeMode::Ltp, at, 4),
            playback_value(fixture_id, 0.2, MergeMode::Ltp, at, 5),
        ],
    );
    assert_eq!(
        resolved[&(fixture_id, AttributeKey::intensity())],
        AttributeValue::Normalized(0.2)
    );
}

#[test]
fn equal_timestamp_playback_htp_ignores_transition_order() {
    let fixture_id = FixtureId::new();
    let at = Utc::now();
    let resolved = resolved(
        fixture_id,
        &["intensity"],
        [
            playback_value(fixture_id, 0.8, MergeMode::Htp, at, 4),
            playback_value(fixture_id, 0.2, MergeMode::Htp, at, 5),
        ],
    );
    assert_eq!(
        resolved[&(fixture_id, AttributeKey::intensity())],
        AttributeValue::Normalized(0.8)
    );
}

#[test]
fn equal_timestamp_non_playback_ltp_does_not_use_playback_order() {
    let fixture_id = FixtureId::new();
    let at = Utc::now();
    let value = |normalized| {
        EngineContribution::unscaled(TimedValue {
            fixture_id,
            attribute: AttributeKey("pan".into()),
            value: AttributeValue::Normalized(normalized),
            priority: 10,
            changed_at: at,
            programmer_order: 0,
            merge_mode: MergeMode::Ltp,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        })
    };
    let resolved = resolved(fixture_id, &["pan"], [value(0.8), value(0.2)]);
    assert_eq!(
        resolved[&(fixture_id, AttributeKey("pan".into()))],
        AttributeValue::Normalized(0.8)
    );
}
