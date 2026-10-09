use super::*;
use light_core::programming::{
    ColorAllocation, ColorAuthoringModel, ColorComponent, ColorIntent, ColorProgram,
    ProgrammingTraceField as F, VirtualColorAuthoringV1,
};

fn focus(number: f64, fixture: FixtureId, level: f32, fade: u64) -> Cue {
    let mut cue = Cue::new(cue_number(number));
    cue.fade_millis = fade;
    cue.changes.push(value(fixture, "focus", level));
    cue
}

fn sample(engine: &PlaybackEngine, at: DateTime<Utc>) -> PlaybackContribution {
    engine
        .contributions_with_context(at, None)
        .into_iter()
        .next()
        .unwrap()
}

fn evidence(engine: &PlaybackEngine, at: DateTime<Utc>) -> Arc<PlaybackFamilyEvidence> {
    sample(engine, at)
        .family_evidence
        .expect("known family evidence")
}

#[test]
fn equal_endpoints_and_repeated_interruption_retain_distinct_actions_and_shared_phase_arcs() {
    let fixture = FixtureId::new();
    let cues = vec![
        focus(1.0, fixture, 0.2, 0),
        focus(2.0, fixture, 0.2, 1_000),
        focus(3.0, fixture, 0.8, 1_000),
    ];
    let cue_ids: Vec<_> = cues.iter().map(|cue| cue.id).collect();
    let cue_list = list(cues);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    let start = Utc::now();
    engine.go_at(id, start).unwrap();
    let first = evidence(&engine, start);
    engine.go_at(id, start).unwrap();
    assert!(Arc::ptr_eq(&first, &evidence(&engine, start)));
    let half = start + ChronoDuration::milliseconds(500);
    let middle = evidence(&engine, half);
    assert_eq!(middle.entries().len(), 2);
    assert_eq!(
        sample(&engine, half).value.value,
        AttributeValue::Normalized(0.2)
    );
    assert_eq!(
        middle.entries()[0].occurrence.authored_cue_id,
        Some(cue_ids[0])
    );
    assert_eq!(
        middle.entries()[1].occurrence.authored_cue_id,
        Some(cue_ids[1])
    );
    assert_ne!(
        middle.entries()[0].occurrence.action_ordinal,
        middle.entries()[1].occurrence.action_ordinal
    );
    assert!(
        middle
            .entries()
            .iter()
            .all(|entry| entry.occurrence.action_changed_at == start)
    );
    assert!(Arc::ptr_eq(
        &middle,
        &evidence(&engine, start + ChronoDuration::milliseconds(700))
    ));
    engine.go_at(id, half).unwrap();
    assert!(Arc::ptr_eq(&middle, &evidence(&engine, half)));
    let interrupted = evidence(&engine, start + ChronoDuration::milliseconds(1_000));
    assert_eq!(interrupted.entries().len(), 3);
    let level = sample(&engine, start + ChronoDuration::milliseconds(1_000))
        .value
        .value
        .normalized()
        .unwrap();
    assert!((level - 0.5).abs() < 0.0001);
    let endpoint = evidence(&engine, start + ChronoDuration::milliseconds(1_500));
    assert_eq!(endpoint.entries().len(), 1);
    assert_eq!(
        endpoint.entries()[0].occurrence.authored_cue_id,
        Some(cue_ids[2])
    );
}

#[test]
fn tracked_target_reuses_original_author_while_arbitration_clock_advances() {
    let fixture = FixtureId::new();
    let first = focus(1.0, fixture, 0.4, 0);
    let authored = first.id;
    let cue_list = list(vec![
        first,
        Cue::new(cue_number(2.0)),
        Cue::new(cue_number(3.0)),
    ]);
    let id = cue_list.id;
    let start = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go_at(id, start).unwrap();
    let original = evidence(&engine, start);
    for step in [1, 2] {
        let now = start + ChronoDuration::seconds(step);
        engine.go_at(id, now).unwrap();
        let current = sample(&engine, now);
        assert_eq!(current.value.changed_at, now);
        assert!(Arc::ptr_eq(
            &original,
            current.family_evidence.as_ref().unwrap()
        ));
        assert_eq!(
            original.entries()[0].occurrence.authored_cue_id,
            Some(authored)
        );
        assert_eq!(original.entries()[0].occurrence.action_changed_at, start);
    }
}

#[test]
fn goto_and_back_capture_current_values_and_keep_their_original_sources() {
    let fixture = FixtureId::new();
    let cue_list = list(vec![
        focus(1.0, fixture, 0.0, 0),
        focus(2.0, fixture, 1.0, 1_000),
        focus(3.0, fixture, 0.0, 1_000),
    ]);
    let id = cue_list.id;
    let start = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go_at(id, start).unwrap();
    engine.go_at(id, start).unwrap();
    let half = start + ChronoDuration::milliseconds(500);
    let before = sample(&engine, half);
    engine.jump_at(id, cue_number(3.0), half).unwrap();
    let after = sample(&engine, half);
    assert_eq!(after.value.value, before.value.value);
    assert!(Arc::ptr_eq(
        after.family_evidence.as_ref().unwrap(),
        before.family_evidence.as_ref().unwrap()
    ));
    let next = start + ChronoDuration::milliseconds(750);
    let before_back = sample(&engine, next);
    engine.back_at(id, next).unwrap();
    assert_eq!(sample(&engine, next).value.value, before_back.value.value);
    assert!(Arc::ptr_eq(
        &evidence(&engine, next),
        before_back.family_evidence.as_ref().unwrap()
    ));
    assert_eq!(
        evidence(&engine, start + ChronoDuration::milliseconds(1_250))
            .entries()
            .len(),
        4
    );
}

#[test]
fn pause_resume_and_deleted_hold_restore_keep_the_original_action_clock() {
    let fixture = FixtureId::new();
    let first = focus(1.0, fixture, 0.0, 0);
    let second = focus(2.0, fixture, 1.0, 1_000);
    let third = focus(3.0, fixture, 0.8, 1_000);
    let original = list(vec![first.clone(), second, third.clone()]);
    let id = original.id;
    let start = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(original).unwrap();
    engine.go_at(id, start).unwrap();
    engine.go_at(id, start).unwrap();
    let paused = start + ChronoDuration::milliseconds(250);
    engine.pause_at(id, paused).unwrap();
    let before = evidence(&engine, paused);
    let resumed = start + ChronoDuration::seconds(1);
    engine.go_at(id, resumed).unwrap();
    assert_eq!(
        sample(&engine, resumed).value.value,
        AttributeValue::Normalized(0.25)
    );
    assert!(Arc::ptr_eq(&before, &evidence(&engine, resumed)));
    assert!(
        before
            .entries()
            .iter()
            .all(|entry| entry.occurrence.action_changed_at == start)
    );
    let runtime = engine.runtime();
    assert_ne!(runtime[0].activated_at, start);
    assert_eq!(
        runtime[0]
            .source_history
            .as_ref()
            .unwrap()
            .action_changed_at(),
        start
    );
    let mut replacement = list(vec![first, third]);
    replacement.id = id;
    let held = engine.active_for_snapshot(&[replacement.clone()], resumed);
    let wire = serde_json::to_value(held).unwrap();
    let held: Vec<ActivePlayback> = serde_json::from_value(wire).unwrap();
    let mut restored = PlaybackEngine::default();
    restored.register(replacement).unwrap();
    restored.restore_active(held);
    let held_sample = sample(&restored, resumed + ChronoDuration::seconds(5));
    assert_eq!(held_sample.value.value, AttributeValue::Normalized(0.25));
    assert_eq!(
        held_sample.family_evidence.as_ref().unwrap().entries(),
        before.entries()
    );
    restored
        .go_at(id, resumed + ChronoDuration::seconds(5))
        .unwrap();
    assert_eq!(
        evidence(&restored, resumed + ChronoDuration::milliseconds(5_500))
            .entries()
            .len(),
        3
    );
}

#[test]
fn automatic_wait_uses_the_same_captured_evidence_and_phase_cache() {
    let fixture = FixtureId::new();
    let mut second = focus(2.0, fixture, 0.8, 1_000);
    second.trigger = CueTrigger::Wait { delay_millis: 100 };
    let cue_list = list(vec![focus(1.0, fixture, 0.2, 0), second]);
    let id = cue_list.id;
    let start = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go_at(id, start).unwrap();
    let original = evidence(&engine, start);
    engine.tick(start + ChronoDuration::milliseconds(100), None);
    assert!(Arc::ptr_eq(
        &original,
        &evidence(&engine, start + ChronoDuration::milliseconds(100))
    ));
    let half = evidence(&engine, start + ChronoDuration::milliseconds(600));
    assert_eq!(half.entries().len(), 2);
    assert!(Arc::ptr_eq(
        &half,
        &evidence(&engine, start + ChronoDuration::milliseconds(700))
    ));
}

#[test]
fn preview_navigation_does_not_change_live_occurrences_or_cached_phase_evidence() {
    let fixture = FixtureId::new();
    let cue_list = list(vec![
        focus(1.0, fixture, 0.0, 0),
        focus(2.0, fixture, 1.0, 1_000),
    ]);
    let id = cue_list.id;
    let start = Utc::now();
    let mut live = PlaybackEngine::default();
    live.register(cue_list).unwrap();
    live.go_at(id, start).unwrap();
    let original = evidence(&live, start);
    let live_ordinal = live.next_transition_ordinal;
    let mut preview = live.fork_for_preview(start);
    preview.go_at(id, start).unwrap();
    assert_eq!(
        evidence(&preview, start + ChronoDuration::milliseconds(500))
            .entries()
            .len(),
        2
    );
    assert_eq!(live.next_transition_ordinal, live_ordinal);
    assert!(Arc::ptr_eq(
        &original,
        &evidence(&live, start + ChronoDuration::seconds(1))
    ));
}

#[test]
fn manual_legs_are_exact_and_unproven_restored_targets_stay_unknown() {
    let fixture = FixtureId::new();
    let cue_list = list(vec![
        focus(1.0, fixture, 0.2, 0),
        focus(2.0, fixture, 0.8, 1_000),
    ]);
    let id = cue_list.id;
    let start = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(start));
    let mut engine = PlaybackEngine::with_clock(clock);
    engine.register(cue_list.clone()).unwrap();
    let mut playback = definition(1, id);
    playback.fader = PlaybackFaderMode::XFade;
    engine.register_definition(playback).unwrap();
    engine.go_playback(1).unwrap();
    let _ = evidence(&engine, start);
    let ordinal = engine.active()[0].transition_ordinal;
    engine.set_manual_xfade(1, 0.5).unwrap();
    assert_eq!(engine.active()[0].transition_ordinal, ordinal);
    assert_eq!(evidence(&engine, start).entries().len(), 2);
    engine.set_manual_xfade(1, 1.0).unwrap();
    assert_eq!(evidence(&engine, start).entries().len(), 1);

    let mut engine = PlaybackEngine::default();
    engine.register(cue_list.clone()).unwrap();
    engine.go_at(id, start).unwrap();
    let _ = evidence(&engine, start);
    let wire = serde_json::to_value(engine.runtime()).unwrap();
    let restored: Vec<ActivePlayback> = serde_json::from_value(wire).unwrap();
    assert_eq!(
        restored[0]
            .source_history
            .as_ref()
            .unwrap()
            .action_changed_at(),
        start
    );
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.restore_active(restored);
    assert!(sample(&engine, start).family_evidence.is_none());
    engine.go_at(id, start).unwrap();
    assert!(
        sample(&engine, start + ChronoDuration::milliseconds(500))
            .family_evidence
            .is_none()
    );
    assert!(
        sample(&engine, start + ChronoDuration::milliseconds(1_000))
            .family_evidence
            .is_some()
    );
}

#[test]
fn retained_rows_decode_legacy_values_and_reject_invalid_evidence_scopes() {
    let fixture = FixtureId::new();
    let cue_list = list(vec![focus(1.0, fixture, 0.2, 0)]);
    let id = cue_list.id;
    let start = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go_at(id, start).unwrap();
    let sample = sample(&engine, start);
    let legacy = serde_json::to_value(&sample.value).unwrap();
    let row: PlaybackRetainedValue = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(serde_json::to_value(&row).unwrap(), legacy);
    assert!(row.family_evidence.is_none());
    let row = PlaybackRetainedValue::from(sample);
    let wire = serde_json::to_value(&row).unwrap();
    assert_eq!(
        serde_json::from_value::<PlaybackRetainedValue>(wire.clone()).unwrap(),
        row
    );
    let mut wrong = wire;
    wrong["family_evidence"]["entries"][0]["effective_fields"] =
        serde_json::json!([{"kind":"pan"}]);
    assert!(serde_json::from_value::<PlaybackRetainedValue>(wrong).is_err());
}

#[test]
fn semantic_color_evidence_uses_rebuilt_recipe_fields_and_held_allocation() {
    let fixture = FixtureId::new();
    let mut a = ColorIntent::default();
    VirtualColorAuthoringV1
        .set_base_component(&mut a, ColorComponent::Amber, 0.7)
        .unwrap();
    let mut b = ColorIntent::default();
    b.allocation = ColorAllocation::PreferWhite;
    let mut first = Cue::new(cue_number(1.0));
    first.changes.push(CueChange::set(
        fixture,
        AttributeKey("color".into()),
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent: a })),
    ));
    let mut second = Cue::new(cue_number(2.0));
    second.fade_millis = 1_000;
    second.changes.push(CueChange::set(
        fixture,
        AttributeKey("color".into()),
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent: b })),
    ));
    let cue_list = list(vec![first, second]);
    let id = cue_list.id;
    let start = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go_at(id, start).unwrap();
    engine.go_at(id, start).unwrap();
    let middle = evidence(&engine, start + ChronoDuration::milliseconds(500));
    assert_eq!(middle.entries().len(), 2);
    for entry in middle.entries() {
        assert!(entry.effective_fields.contains(F::ColorRecipeRed));
        assert!(!entry.effective_fields.contains(F::ColorRecipeAmber));
    }
    assert!(middle.entries()[0].effective_fields.contains(F::Allocation));
    assert!(!middle.entries()[1].effective_fields.contains(F::Allocation));
}

#[test]
fn cold_generation_is_pinned_before_observation_but_address_cow_preserves_it() {
    struct NoAddresses;
    impl light_core::FrameAddressResolver for NoAddresses {
        fn generation(&self) -> u64 {
            2
        }
        fn frame_address(
            &self,
            _: FixtureId,
            _: &AttributeKey,
        ) -> Option<light_core::FrameAddress> {
            None
        }
    }
    let cue_list = list(vec![focus(1.0, FixtureId::new(), 0.4, 0)]);
    let id = cue_list.id;
    let at = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list.clone()).unwrap();
    engine.go_at(id, at).unwrap();
    let _preview = engine.clone();
    engine.resolve_frame_addresses(&NoAddresses);
    assert!(sample(&engine, at).family_evidence.is_some());
    let mut cold = PlaybackEngine::default();
    cold.register(cue_list.clone()).unwrap();
    cold.go_at(id, at).unwrap();
    cold.register(cue_list).unwrap();
    assert!(sample(&cold, at).family_evidence.is_none());
}

#[test]
fn original_control_source_is_pinned_before_first_observation() {
    let cue_list = list(vec![focus(1.0, FixtureId::new(), 0.4, 0)]);
    let id = cue_list.id;
    let at = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(at));
    let mut engine = PlaybackEngine::with_clock(clock);
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.register_definition(definition(2, id)).unwrap();
    engine.go_playback(1).unwrap();
    engine.set_virtual_master(2, 0.5).unwrap();
    let first = evidence(&engine, at);
    assert_eq!(
        first.entries()[0].occurrence.source.playback_number,
        Some(1)
    );
    assert_eq!(sample(&engine, at).source.playback_number, Some(2));
    let temporary = engine
        .temporary_playback_at(PlaybackIdentity::physical(2).unwrap(), 1.0, false)
        .unwrap();
    assert!(temporary.source_history.is_some());
}

mod legs;

#[test]
fn restored_history_rejects_invalid_ordinal_and_cannot_recover_tracked_authorship() {
    let cue_list = list(vec![
        focus(1.0, FixtureId::new(), 0.4, 0),
        Cue::new(cue_number(2.0)),
    ]);
    let id = cue_list.id;
    let at = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list.clone()).unwrap();
    engine.go_at(id, at).unwrap();
    let mut wire = serde_json::to_value(engine.runtime()).unwrap();
    wire[0]["source_history"]["action_ordinal"] = serde_json::json!(0);
    assert!(serde_json::from_value::<Vec<ActivePlayback>>(wire).is_err());
    let restored: Vec<ActivePlayback> =
        serde_json::from_value(serde_json::to_value(engine.runtime()).unwrap()).unwrap();
    let mut resumed = PlaybackEngine::default();
    resumed.register(cue_list).unwrap();
    resumed.restore_active(restored);
    resumed.go_at(id, at + ChronoDuration::seconds(1)).unwrap();
    assert!(
        resumed.active()[0]
            .source_history
            .as_ref()
            .unwrap()
            .action_ordinal()
            > engine.active()[0]
                .source_history
                .as_ref()
                .unwrap()
                .action_ordinal()
    );
    assert!(
        sample(&resumed, at + ChronoDuration::seconds(1))
            .family_evidence
            .is_none()
    );
}

#[test]
fn generated_restoration_and_unresolved_group_rows_are_unknown() {
    for group in [false, true] {
        let mut cue = focus(1.0, FixtureId::new(), 0.4, 0);
        if group {
            cue.group_changes.push(GroupCueChange {
                preset_reference: None,
                group_id: "1".into(),
                attribute: AttributeKey("focus".into()),
                value: Some(AttributeValue::Normalized(0.4)),
                automatic_restore: false,
                fade_millis: None,
                delay_millis: None,
            });
        } else {
            cue.changes[0].automatic_restore = true;
        }
        let cue_list = list(vec![cue]);
        let id = cue_list.id;
        let at = Utc::now();
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine.go_at(id, at).unwrap();
        assert!(sample(&engine, at).family_evidence.is_none());
    }
}

#[test]
fn equal_native_trace_requirement_is_unknown_while_actual_conversion_hold_keeps_from() {
    use light_core::programming::{NativeColorRecipe, PortableColorEstimate};
    use light_core::{NativeColorIdentity, NativeColorValue, PhysicalDataQuality};
    let fixture = FixtureId::new();
    let native = |raw| {
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
            recipe: NativeColorRecipe {
                source: NativeColorIdentity {
                    profile_id: Uuid::from_u128(1),
                    mode_id: Uuid::from_u128(2),
                    head_id: Uuid::from_u128(3),
                    path_id: Uuid::from_u128(4),
                    profile_revision: 1,
                    profile_digest: "source-test".into(),
                    native_layout_signature: "single-channel".into(),
                    model_revision: 1,
                },
                channels: vec![NativeColorValue {
                    channel_id: Uuid::from_u128(10),
                    function_id: Uuid::from_u128(11),
                    raw,
                }],
                spreads: vec![],
            },
            portable: PortableColorEstimate {
                model_revision: 1,
                visible: None,
                uv: None,
                quality: PhysicalDataQuality::Unknown,
                limitations: vec![],
            },
        }))
    };
    for to_raw in [20, 80] {
        let mut first = Cue::new(cue_number(1.0));
        first.changes.push(CueChange::set(
            fixture,
            AttributeKey("color".into()),
            native(20),
        ));
        let mut second = Cue::new(cue_number(2.0));
        second.fade_millis = 1_000;
        second.changes.push(CueChange::set(
            fixture,
            AttributeKey("color".into()),
            native(to_raw),
        ));
        let cue_list = list(vec![first, second]);
        let id = cue_list.id;
        let at = Utc::now();
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine.go_at(id, at).unwrap();
        let original = evidence(&engine, at);
        engine.go_at(id, at).unwrap();
        let middle = sample(&engine, at + ChronoDuration::milliseconds(500));
        assert_eq!(middle.value.value, native(20));
        if to_raw == 20 {
            assert!(middle.family_evidence.is_none());
        } else {
            assert!(Arc::ptr_eq(
                &original,
                middle.family_evidence.as_ref().unwrap()
            ));
        }
        assert_eq!(
            evidence(&engine, at + ChronoDuration::milliseconds(1_000))
                .entries()
                .len(),
            1
        );
    }
}
