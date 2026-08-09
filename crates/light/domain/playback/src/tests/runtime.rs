use super::*;

#[test]
fn runtime_timing_projects_effective_phases_and_retains_completed_trigger_until_retrigger() {
    let outgoing = FixtureId::new();
    let incoming = FixtureId::new();
    let mut first = Cue::new(1.0);
    first.out_delay_millis = Some(200);
    first.out_fade_millis = Some(1_800);
    first.changes.push(value(outgoing, "intensity", 1.0));
    first.changes.push(value(incoming, "intensity", 0.0));
    let mut second = Cue::new(2.0);
    second.delay_millis = 100;
    second.fade_millis = 900;
    second.out_delay_millis = Some(200);
    second.out_fade_millis = Some(1_800);
    second.changes.push(value(outgoing, "intensity", 0.0));
    second.changes.push(value(incoming, "intensity", 1.0));
    let mut third = Cue::new(3.0);
    third.trigger = CueTrigger::Wait { delay_millis: 300 };
    let third_id = third.id;
    let cue_list = list(vec![first, second, third]);
    let id = cue_list.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go_at(id, started).unwrap();
    engine.go_at(id, started).unwrap();

    let status = engine.runtime_status().remove(0);
    let timing = status.cue_timing.unwrap();
    assert_eq!(
        (
            timing.in_delay_millis,
            timing.in_fade_millis,
            timing.out_delay_millis,
            timing.out_fade_millis,
            timing.completion_millis,
        ),
        (100, 900, 200, 1_800, 2_000)
    );
    let trigger = timing.active_trigger.unwrap();
    assert_eq!(trigger.cue_id, third_id);
    assert_eq!(trigger.kind, CueTriggerTimingKind::Wait);
    assert_eq!(trigger.started_at, started);
    assert_eq!(trigger.duration_millis, 300);

    engine.tick(started + ChronoDuration::milliseconds(2_300), None);
    let timing = engine.runtime_status().remove(0).cue_timing.unwrap();
    assert_eq!(timing.cue_id, third_id);
    assert_eq!(timing.completed_trigger_cue_id, Some(third_id));

    engine
        .jump_at(id, 3.0, started + ChronoDuration::milliseconds(2_400))
        .unwrap();
    let status = engine.runtime_status().remove(0);
    assert_eq!(
        status.playback.activated_at,
        started + ChronoDuration::milliseconds(2_400)
    );
    assert_eq!(
        status.cue_timing.unwrap().completed_trigger_cue_id,
        None,
        "a manual same-Cue retrigger starts a fresh timing generation"
    );
}

#[test]
fn runtime_timing_associates_link_delay_with_its_source_row() {
    let mut source = Cue::new(1.0);
    let skipped = Cue::new(2.0);
    let destination = Cue::new(3.0);
    source.trigger = CueTrigger::Link {
        cue_id: destination.id,
        delay_millis: 250,
    };
    let source_id = source.id;
    let destination_id = destination.id;
    let cue_list = list(vec![source, skipped, destination]);
    let id = cue_list.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go_at(id, started).unwrap();

    let trigger = engine
        .runtime_status()
        .remove(0)
        .cue_timing
        .unwrap()
        .active_trigger
        .unwrap();
    assert_eq!(trigger.cue_id, source_id);
    assert_eq!(trigger.kind, CueTriggerTimingKind::Link);
    assert_eq!(trigger.started_at, started);

    engine.tick(started + ChronoDuration::milliseconds(250), None);
    let timing = engine.runtime_status().remove(0).cue_timing.unwrap();
    assert_eq!(timing.cue_id, destination_id);
    assert_eq!(timing.completed_trigger_cue_id, Some(source_id));
}

#[test]
fn active_cue_dynamic_projection_tracks_fat_and_honors_release() {
    let fixture = FixtureId::new();
    let mut one = Cue::new(1.0);
    one.dynamic_changes.push(CueDynamicChange {
        fixture_id: fixture,
        attribute: AttributeKey::intensity(),
        value: light_dynamics::DynamicSemanticValue::FixAt {
            value: 0.4,
            timing: light_dynamics::DynamicValueTiming {
                fade_millis: Some(200),
                delay_millis: Some(50),
            },
        },
        automatic_restore: false,
    });
    let two = Cue::new(2.0);
    let mut three = Cue::new(3.0);
    three.dynamic_changes.push(CueDynamicChange {
        fixture_id: fixture,
        attribute: AttributeKey::intensity(),
        value: light_dynamics::DynamicSemanticValue::Release,
        automatic_restore: false,
    });
    let cue_list = list(vec![one, two, three]);
    let id = cue_list.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go_at(id, started).unwrap();
    let first = engine.active_cue_dynamic_values();
    assert_eq!(first.len(), 1);
    assert!(matches!(
        first[0].value,
        light_dynamics::DynamicSemanticValue::FixAt { value: 0.4, .. }
    ));

    engine
        .go_at(id, started + ChronoDuration::milliseconds(1))
        .unwrap();
    assert_eq!(engine.active_cue_dynamic_values().len(), 1);
    engine
        .go_at(id, started + ChronoDuration::milliseconds(2))
        .unwrap();
    assert!(engine.active_cue_dynamic_values().is_empty());
}

#[test]
fn active_cue_dynamic_projection_preserves_tracking_order_after_indexed_updates() {
    let first_fixture = FixtureId::new();
    let second_fixture = FixtureId::new();
    let value = |fixture_id, value| CueDynamicChange {
        fixture_id,
        attribute: AttributeKey::intensity(),
        value: light_dynamics::DynamicSemanticValue::FixAt {
            value,
            timing: light_dynamics::DynamicValueTiming::default(),
        },
        automatic_restore: false,
    };
    let mut one = Cue::new(1.0);
    one.dynamic_changes.push(value(first_fixture, 0.1));
    one.dynamic_changes.push(value(second_fixture, 0.2));
    one.dynamic_changes.push(value(first_fixture, 0.3));
    let mut two = Cue::new(2.0);
    two.dynamic_changes.push(CueDynamicChange {
        fixture_id: first_fixture,
        attribute: AttributeKey::intensity(),
        value: light_dynamics::DynamicSemanticValue::Release,
        automatic_restore: false,
    });
    two.dynamic_changes.push(value(first_fixture, 0.4));
    let cue_list = list(vec![one, two]);
    let id = cue_list.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go_at(id, started).unwrap();

    let first = engine.active_cue_dynamic_values();
    assert_eq!(
        first
            .iter()
            .map(|candidate| candidate.fixture_id)
            .collect::<Vec<_>>(),
        vec![first_fixture, second_fixture]
    );
    assert!(matches!(
        first[0].value,
        light_dynamics::DynamicSemanticValue::FixAt { value: 0.3, .. }
    ));

    engine
        .go_at(id, started + ChronoDuration::milliseconds(1))
        .unwrap();
    assert_eq!(
        engine
            .active_cue_dynamic_values()
            .iter()
            .map(|candidate| candidate.fixture_id)
            .collect::<Vec<_>>(),
        vec![second_fixture, first_fixture]
    );
}

#[test]
fn ltp_intensity_can_select_a_newer_lower_value() {
    let fixture = FixtureId::new();
    let mut high = Cue::new(1.0);
    high.changes.push(value(fixture, "intensity", 0.8));
    let mut low = Cue::new(1.0);
    low.changes.push(value(fixture, "intensity", 0.2));
    let mut high = list(vec![high]);
    high.intensity_priority_mode = IntensityPriorityMode::Ltp;
    let mut low = list(vec![low]);
    low.intensity_priority_mode = IntensityPriorityMode::Ltp;
    let high_id = high.id;
    let low_id = low.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(high).unwrap();
    engine.register(low).unwrap();
    engine.go_at(high_id, started).unwrap();
    engine
        .go_at(low_id, started + ChronoDuration::milliseconds(1))
        .unwrap();
    assert_eq!(
        resolve(engine.contributions_at(started + ChronoDuration::milliseconds(1)))
            [&(fixture, AttributeKey::intensity())],
        AttributeValue::Normalized(0.2)
    );
}

#[test]
fn equal_timestamp_transitions_receive_persistable_monotonic_order() {
    let fixture = FixtureId::new();
    let mut high = Cue::new(1.0);
    high.changes.push(value(fixture, "pan", 0.8));
    let mut low = Cue::new(1.0);
    low.changes.push(value(fixture, "pan", 0.2));
    let high = list(vec![high]);
    let low = list(vec![low]);
    let high_id = high.id;
    let low_id = low.id;
    let started = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(high.clone()).unwrap();
    engine.register(low.clone()).unwrap();
    engine.go_at(high_id, started).unwrap();
    engine.go_at(low_id, started).unwrap();

    let high_order = engine
        .contributions_with_context_at(started, |_, _| false)
        .into_iter()
        .find(|value| value.source.cue_list_id == high_id)
        .unwrap()
        .transition_ordinal;
    let low_order = engine
        .contributions_with_context_at(started, |_, _| false)
        .into_iter()
        .find(|value| value.source.cue_list_id == low_id)
        .unwrap()
        .transition_ordinal;
    assert!(low_order > high_order);

    let persisted = engine.runtime();
    let mut restored = PlaybackEngine::default();
    restored.register(high).unwrap();
    restored.register(low).unwrap();
    restored.restore_active(persisted);
    restored.jump_at(high_id, 1.0, started).unwrap();
    assert!(
        restored
            .active()
            .iter()
            .find(|playback| playback.cue_list_id == high_id)
            .unwrap()
            .transition_ordinal
            > low_order
    );
}

#[test]
fn concrete_playbacks_share_one_cuelist_runtime() {
    let fixture = FixtureId::new();
    let mut one = Cue::new(1.0);
    one.changes.push(value(fixture, "intensity", 0.1));
    let mut two = Cue::new(2.0);
    two.changes.push(value(fixture, "intensity", 0.5));
    let mut three = Cue::new(3.0);
    three.changes.push(value(fixture, "intensity", 0.9));
    let cue_list = list(vec![one, two, three]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.register_definition(definition(2, id)).unwrap();
    engine.goto_playback(1, 2.0).unwrap();
    engine.goto_playback(2, 3.0).unwrap();
    let runtime = engine.runtime();
    assert_eq!(runtime.len(), 1);
    assert_eq!(runtime[0].current_cue_number, Some(3.0));
    assert_eq!(engine.playback_runtime(1), engine.playback_runtime(2));
    assert_eq!(engine.go(id).unwrap().current_cue_number, Some(3.0));
}

#[test]
fn load_is_silent_consumed_by_go_and_cleared_by_off() {
    let fixture = FixtureId::new();
    let mut one = Cue::new(1.0);
    one.changes.push(value(fixture, "intensity", 0.1));
    let mut two = Cue::new(2.0);
    two.changes.push(value(fixture, "intensity", 0.5));
    let mut three = Cue::new(3.0);
    three.changes.push(value(fixture, "intensity", 0.9));
    let cue_list = list(vec![one, two, three]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.load_playback(1, 2.0).unwrap();
    assert!(engine.active().is_empty());
    assert!(engine.contributions().is_empty());
    assert_eq!(engine.runtime()[0].loaded_cue_number, Some(2.0));
    engine.go_playback(1).unwrap();
    assert_eq!(engine.active()[0].current_cue_number, Some(2.0));
    assert_eq!(engine.active()[0].loaded_cue_number, None);
    engine.go_playback(1).unwrap();
    assert_eq!(engine.active()[0].current_cue_number, Some(3.0));
    engine.load_playback(1, 1.0).unwrap();
    engine.back_playback(1).unwrap();
    assert_eq!(
        engine.active()[0].loaded_cue_number,
        Some(1.0),
        "GO minus deliberately preserves Load"
    );
    engine.off(1).unwrap();
    assert_eq!(engine.runtime()[0].loaded_cue_number, None);
}

#[test]
fn loaded_feedback_tracks_stable_identity_through_renumber_and_deletion() {
    let original = list(vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)]);
    let id = original.id;
    let loaded_id = original.cues[1].id;
    let mut engine = PlaybackEngine::default();
    engine.register(original.clone()).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.load_playback(1, 2.0).unwrap();
    let status = engine.runtime_status().remove(0);
    assert_eq!(
        (
            status.normal_next_cue_number,
            status.effective_next_cue_number,
            status.effective_next_is_loaded
        ),
        (Some(1.0), Some(2.0), true)
    );

    let mut renumbered = original.clone();
    renumbered.cues[1].number = 8.0;
    renumbered
        .cues
        .sort_by(|left, right| left.number.total_cmp(&right.number));
    let active = engine.active_for_snapshot(&[renumbered.clone()], Utc::now());
    let mut restored = PlaybackEngine::default();
    restored.register(renumbered.clone()).unwrap();
    restored.register_definition(definition(1, id)).unwrap();
    restored.restore_active(active);
    let status = restored.runtime_status().remove(0);
    assert_eq!(status.playback.loaded_cue_id, Some(loaded_id));
    assert_eq!(status.effective_next_cue_number, Some(8.0));

    renumbered.cues.retain(|cue| cue.id != loaded_id);
    let active = restored.active_for_snapshot(&[renumbered.clone()], Utc::now());
    let mut deleted = PlaybackEngine::default();
    deleted.register(renumbered).unwrap();
    deleted.register_definition(definition(1, id)).unwrap();
    deleted.restore_active(active);
    let status = deleted.runtime_status().remove(0);
    assert_eq!(status.playback.loaded_cue_id, None);
    assert!(!status.effective_next_is_loaded);
    assert_eq!(status.effective_next_cue_number, Some(1.0));
}

#[test]
fn link_feedback_uses_stable_effective_next_and_load_overrides_it() {
    let mut source = Cue::new(1.0);
    let sequential = Cue::new(2.0);
    let destination = Cue::new(3.0);
    source.trigger = CueTrigger::Link {
        cue_id: destination.id,
        delay_millis: 0,
    };
    let mut cue_list = list(vec![source, sequential, destination.clone()]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list.clone()).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    engine.go_playback(1).unwrap();
    let status = engine.runtime_status().remove(0);
    assert_eq!(status.normal_next_cue_number, Some(2.0));
    assert_eq!(status.effective_next_cue_id, Some(destination.id));

    engine.load_playback(1, 2.0).unwrap();
    let status = engine.runtime_status().remove(0);
    assert_eq!(status.effective_next_cue_number, Some(2.0));
    assert!(status.effective_next_is_loaded);

    cue_list.cues[2].number = 30.0;
    let active = engine.active_for_snapshot(&[cue_list.clone()], Utc::now());
    let mut restored = PlaybackEngine::default();
    restored.register(cue_list).unwrap();
    restored.register_definition(definition(1, id)).unwrap();
    restored.restore_active(active);
    restored.off(1).unwrap();
    restored.go_playback(1).unwrap();
    let status = restored.runtime_status().remove(0);
    assert_eq!(status.effective_next_cue_id, Some(destination.id));
    assert_eq!(status.effective_next_cue_number, Some(30.0));
}
