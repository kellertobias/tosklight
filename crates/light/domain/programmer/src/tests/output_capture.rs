use super::*;
use light_dynamics::{DynamicSemanticValue, DynamicValueTiming};

#[test]
fn output_capture_keeps_normal_pending_active_and_release_sources_at_one_edit() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session);
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    );
    assert!(registry.apply_dynamic_values(
        session,
        &[DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: AttributeKey("pan".into()),
            value: DynamicSemanticValue::FixAt {
                value: 0.3,
                timing: DynamicValueTiming::default(),
            },
        }],
        None,
    ));
    registry.arm_preload(session, true);
    assert!(registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetFixture {
            fixture_id: fixture,
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(0.4),
            timing: Default::default(),
        }],
    ));
    assert!(registry.set_preload_group(
        session,
        "front".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    ));
    registry.serialized(|| {
        let mut states = registry.state.write();
        let state = states.as_mut().unwrap();
        Arc::make_mut(&mut state.preload_released_colors)
            .groups
            .insert(
                "front".into(),
                GroupProgrammerValue {
                    value: AttributeValue::Normalized(0.6),
                    changed_at: registry.clock.now(),
                    programmer_order: 1,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                },
            );
        registry.mark_preload_values_changed();
    });

    let captured = registry.capture_output_sources();
    assert_eq!(captured.output_states.len(), 1);
    assert_eq!(captured.output_states[0].values.len(), 1);
    assert_eq!(captured.normal_dynamics.len(), 1);
    assert_eq!(captured.normal_dynamics[0].2.len(), 1);
    let preload = captured.preload.as_ref().unwrap();
    assert_eq!(preload.pending.fixture_values.len(), 1);
    assert_eq!(preload.pending.group_values.len(), 1);
    assert_eq!(preload.pending.released_colors.groups.len(), 1);
    let repeated = registry.capture_output_sources();
    assert!(Arc::ptr_eq(
        &preload.pending,
        &repeated.preload.as_ref().unwrap().pending
    ));

    // Exit capture mode before editing Live; ordinary Set while armed belongs to pending.
    registry.arm_preload(session, false);
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.9),
    );
    let normal_edited = registry.capture_output_sources();
    assert!(Arc::ptr_eq(
        &preload.pending,
        &normal_edited.preload.as_ref().unwrap().pending
    ));
    assert_eq!(
        captured.output_states[0].values[0].value.normalized(),
        Some(0.2)
    );
    assert_eq!(
        normal_edited.output_states[0].values[0].value.normalized(),
        Some(0.9)
    );

    registry.arm_preload(session, true);
    assert!(registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetFixture {
            fixture_id: fixture,
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(0.8),
            timing: Default::default(),
        }],
    ));
    let pending_edited = registry.capture_output_sources();
    assert!(!Arc::ptr_eq(
        &preload.pending,
        &pending_edited.preload.as_ref().unwrap().pending
    ));
    assert_eq!(
        preload.pending.fixture_values[0].value.normalized(),
        Some(0.4)
    );
    assert_eq!(
        pending_edited
            .preload
            .as_ref()
            .unwrap()
            .pending
            .fixture_values[0]
            .value
            .normalized(),
        Some(0.8)
    );
    assert_eq!(preload.pending.released_colors.groups.len(), 1);
    registry.serialized(|| {
        let mut states = registry.state.write();
        let state = states.as_mut().unwrap();
        Arc::make_mut(&mut state.preload_released_colors)
            .groups
            .get_mut("front")
            .unwrap()
            .value = AttributeValue::Normalized(0.7);
        registry.mark_preload_values_changed();
    });
    let released_edited = registry.capture_output_sources();
    assert_eq!(
        preload.pending.released_colors.groups["front"]
            .value
            .normalized(),
        Some(0.6)
    );
    assert_eq!(
        released_edited
            .preload
            .as_ref()
            .unwrap()
            .pending
            .released_colors
            .groups["front"]
            .value
            .normalized(),
        Some(0.7)
    );
}

#[test]
fn priority_and_preload_activation_refresh_the_pending_cache_and_active_sources() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session);
    registry.arm_preload(session, true);
    assert!(registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetFixture {
            fixture_id: fixture,
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(0.4),
            timing: Default::default(),
        }],
    ));
    let before = registry.capture_output_sources();
    assert!(registry.set_priority(session, 7));
    let priority = registry.capture_output_sources();
    assert!(!Arc::ptr_eq(
        &before.preload.as_ref().unwrap().pending,
        &priority.preload.as_ref().unwrap().pending
    ));
    assert_eq!(priority.priority, Some(7));
    assert_eq!(
        priority.preload.as_ref().unwrap().pending.fixture_values[0].priority,
        7
    );

    assert!(registry.activate_preload(session));
    let active = registry.capture_output_sources();
    assert!(
        active
            .preload
            .as_ref()
            .unwrap()
            .pending
            .fixture_values
            .is_empty()
    );
    assert_eq!(active.preload.as_ref().unwrap().active_values.len(), 1);
    assert_eq!(active.output_states[0].preload_active.len(), 1);
}

#[test]
fn try_capture_retains_last_frame_when_mutation_gate_is_owned_elsewhere() {
    let registry = ProgrammerRegistry::default();
    registry.start(SessionId::new());
    let entered = Arc::new(std::sync::Barrier::new(2));
    let release = Arc::new(std::sync::Barrier::new(2));
    let other = registry.clone();
    let other_entered = Arc::clone(&entered);
    let other_release = Arc::clone(&release);
    let editing = std::thread::spawn(move || {
        other.serialized(|| {
            other_entered.wait();
            other_release.wait();
        });
    });
    entered.wait();
    assert!(registry.try_capture_output_sources().is_none());
    release.wait();
    editing.join().unwrap();
    assert!(registry.try_capture_output_sources().is_some());
}

#[test]
fn output_capture_reuses_playback_queue_across_reads_and_value_only_edits() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session);
    assert!(registry.queue_preload_playback_action(
        session,
        7,
        Some(3),
        PreloadPlaybackQueueAction::Go,
        PreloadPlaybackQueueSurface::Virtual,
    ));
    let captured = registry.capture_output_sources();
    let repeated = registry.capture_output_sources();
    assert!(Arc::ptr_eq(
        &captured.preload_playback_actions,
        &repeated.preload_playback_actions,
    ));
    assert_eq!(captured.preload_playback_queue_generation, 1);

    registry.arm_preload(session, false);
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    );
    let normal_edit = registry.capture_output_sources();
    assert!(Arc::ptr_eq(
        &captured.preload_playback_actions,
        &normal_edit.preload_playback_actions,
    ));
    registry.arm_preload(session, true);
    assert!(registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetFixture {
            fixture_id: fixture,
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(0.4),
            timing: Default::default(),
        }],
    ));
    let pending_edit = registry.capture_output_sources();
    assert!(!Arc::ptr_eq(
        &captured.preload.as_ref().unwrap().pending,
        &pending_edit.preload.as_ref().unwrap().pending,
    ));
    assert!(Arc::ptr_eq(
        &captured.preload_playback_actions,
        &pending_edit.preload_playback_actions,
    ));
    assert!(registry.set_priority(session, 7));
    let priority_edit = registry.capture_output_sources();
    assert!(Arc::ptr_eq(
        &captured.preload_playback_actions,
        &priority_edit.preload_playback_actions,
    ));
    assert_eq!(
        priority_edit.preload_playback_queue_generation,
        captured.preload_playback_queue_generation,
    );
    assert!(!Arc::ptr_eq(
        &pending_edit.preload.as_ref().unwrap().pending,
        &priority_edit.preload.as_ref().unwrap().pending,
    ));
    assert_eq!(
        pending_edit
            .preload
            .as_ref()
            .unwrap()
            .pending
            .fixture_values[0]
            .priority,
        100,
    );
    assert_eq!(
        priority_edit
            .preload
            .as_ref()
            .unwrap()
            .pending
            .fixture_values[0]
            .priority,
        7,
    );
    assert_eq!(registry.update_priority(session, 7), Some(false));
    let unchanged_priority = registry.capture_output_sources();
    assert!(Arc::ptr_eq(
        &priority_edit.preload.as_ref().unwrap().pending,
        &unchanged_priority.preload.as_ref().unwrap().pending,
    ));
    assert!(Arc::ptr_eq(
        &captured.preload_playback_actions,
        &unchanged_priority.preload_playback_actions,
    ));
}

#[test]
fn cold_restore_and_reset_discard_both_cached_sources_even_with_reused_generations() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session);
    registry.arm_preload(session, true);
    assert!(registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetFixture {
            fixture_id: fixture,
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(0.4),
            timing: Default::default(),
        }],
    ));
    assert!(registry.queue_preload_playback_action(
        session,
        7,
        None,
        PreloadPlaybackQueueAction::Go,
        PreloadPlaybackQueueSurface::Physical,
    ));
    let before = registry.capture_output_sources();
    let mut replacement = registry.get(session).unwrap();
    replacement.preload_pending[0].value = AttributeValue::Normalized(0.8);
    replacement.preload_playback_pending[0].playback_number = 9;
    registry.restore(replacement);
    let restored = registry.capture_output_sources();
    assert_eq!(restored.identity, before.identity);
    assert_eq!(
        restored.preload_values_generation,
        before.preload_values_generation
    );
    assert_eq!(
        restored.preload_playback_queue_generation,
        before.preload_playback_queue_generation,
    );
    assert_eq!(before.preload_playback_actions[0].playback_number, 7);
    assert_eq!(restored.preload_playback_actions[0].playback_number, 9);
    assert_eq!(
        before.preload.as_ref().unwrap().pending.fixture_values[0]
            .value
            .normalized(),
        Some(0.4),
    );
    assert_eq!(
        restored.preload.as_ref().unwrap().pending.fixture_values[0]
            .value
            .normalized(),
        Some(0.8),
    );

    registry.reset_all();
    let reset = registry.capture_output_sources();
    assert!(reset.identity.is_none());
    assert!(reset.preload.is_none());
    assert!(reset.preload_playback_actions.is_empty());
    assert_eq!(reset.preload_playback_queue_generation, 0);
    registry.start(session);
    let fresh = registry.capture_output_sources();
    assert!(fresh.preload_playback_actions.is_empty());
    assert!(
        fresh
            .preload
            .as_ref()
            .unwrap()
            .pending
            .fixture_values
            .is_empty()
    );
    assert_ne!(fresh.identity, before.identity);
    assert_eq!(restored.preload_playback_actions[0].playback_number, 9);
}

#[test]
fn queue_only_edits_refresh_capture_without_copying_values_or_changing_older_frames() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let origin = uuid::Uuid::new_v4();
    registry.start(session);
    let empty = registry.capture_output_sources();
    for _ in 0..2 {
        assert!(registry.queue_preload_playback_action_with_origin(
            session,
            7,
            Some(3),
            PreloadPlaybackQueueAction::Go,
            PreloadPlaybackQueueSurface::Physical,
            Some(origin),
        ));
    }
    let queued = registry.capture_output_sources();
    assert_eq!(queued.preload_playback_queue_generation, 2);
    assert_eq!(
        queued.preload_values_generation,
        empty.preload_values_generation
    );
    assert_eq!(
        queued.normal_values_generation,
        empty.normal_values_generation
    );
    assert!(Arc::ptr_eq(
        &empty.preload.as_ref().unwrap().pending,
        &queued.preload.as_ref().unwrap().pending,
    ));
    assert!(!Arc::ptr_eq(
        &empty.preload_playback_actions,
        &queued.preload_playback_actions,
    ));
    assert!(empty.preload_playback_actions.is_empty());
    assert_eq!(queued.preload_playback_actions.len(), 2);
    assert_eq!(
        queued.preload_playback_actions[0],
        queued.preload_playback_actions[1]
    );
    let action = &queued.preload_playback_actions[0];
    assert_eq!(action.playback_number, 7);
    assert_eq!(action.page, Some(3));
    assert_eq!(action.origin_desk_id, Some(origin));
    assert_eq!(action.action, PreloadPlaybackQueueAction::Go);
    assert_eq!(action.surface, PreloadPlaybackQueueSurface::Physical);

    let drained = registry.take_preload_playback_actions(session);
    assert_eq!(
        drained.as_slice(),
        queued.preload_playback_actions.as_slice()
    );
    let after = registry.capture_output_sources();
    assert_eq!(after.preload_playback_queue_generation, 3);
    assert!(after.preload_playback_actions.is_empty());
    assert_eq!(queued.preload_playback_actions.len(), 2);
    assert!(Arc::ptr_eq(
        &empty.preload.as_ref().unwrap().pending,
        &after.preload.as_ref().unwrap().pending,
    ));
}
