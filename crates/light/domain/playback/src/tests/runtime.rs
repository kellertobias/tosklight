use super::*;

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
fn concrete_playbacks_share_a_cuelist_but_keep_independent_runtime() {
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
    assert_eq!(
        runtime
            .iter()
            .find(|item| item.playback_number == Some(1))
            .unwrap()
            .current_cue_number,
        Some(2.0)
    );
    assert_eq!(
        runtime
            .iter()
            .find(|item| item.playback_number == Some(2))
            .unwrap()
            .current_cue_number,
        Some(3.0)
    );
    assert!(engine.go(id).unwrap_err().contains("multiple playbacks"));
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
