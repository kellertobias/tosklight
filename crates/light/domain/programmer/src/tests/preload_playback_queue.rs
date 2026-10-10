use super::*;

fn queue(registry: &ProgrammerRegistry, session: SessionId, number: u16) {
    assert!(registry.queue_preload_playback_action(
        session,
        number,
        None,
        PreloadPlaybackQueueAction::Go,
        PreloadPlaybackQueueSurface::Virtual,
    ));
}

#[test]
fn persisted_queue_actions_accept_old_json_and_omit_an_unknown_page() {
    let legacy = serde_json::json!({
        "playback_number": 7,
        "action": "go",
        "surface": "virtual",
    });
    let action: PreloadPlaybackAction = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(action.page, None);
    assert_eq!(action.origin_desk_id, None);
    assert_eq!(serde_json::to_value(&action).unwrap(), legacy);

    let with_page = PreloadPlaybackAction {
        page: Some(3),
        ..action
    };
    assert_eq!(serde_json::to_value(with_page).unwrap()["page"], 3);
}

#[test]
fn captured_origin_round_trips_with_the_persisted_queue_action() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let desk_id = uuid::Uuid::new_v4();
    registry.start(session);

    assert!(registry.queue_preload_playback_action_with_origin(
        session,
        7,
        Some(3),
        PreloadPlaybackQueueAction::Go,
        PreloadPlaybackQueueSurface::Physical,
        Some(desk_id),
    ));

    let queued = registry.preload_playback_actions(session).unwrap();
    assert_eq!(queued[0].origin_desk_id, Some(desk_id));
    let encoded = serde_json::to_value(&queued[0]).unwrap();
    assert_eq!(encoded["origin_desk_id"], desk_id.to_string());
    assert_eq!(
        serde_json::from_value::<PreloadPlaybackAction>(encoded).unwrap(),
        queued[0]
    );
}

#[test]
fn append_retains_order_and_duplicates_and_advances_generation() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);

    queue(&registry, session, 7);
    queue(&registry, session, 7);

    let actions = registry.preload_playback_actions(session).unwrap();
    assert_eq!(
        actions
            .iter()
            .map(|item| item.playback_number)
            .collect::<Vec<_>>(),
        [7, 7]
    );
    assert_eq!(registry.preload_playback_queue_generation(session), Some(2));
}

#[test]
fn drain_clear_and_release_advance_only_when_queue_changes() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);

    queue(&registry, session, 1);
    assert_eq!(registry.take_preload_playback_actions(session).len(), 1);
    assert_eq!(registry.preload_playback_queue_generation(session), Some(2));
    assert!(registry.take_preload_playback_actions(session).is_empty());
    assert_eq!(registry.preload_playback_queue_generation(session), Some(2));

    queue(&registry, session, 2);
    assert!(registry.clear_preload_pending(session));
    assert_eq!(registry.preload_playback_queue_generation(session), Some(4));
    assert!(registry.clear_preload_pending(session));
    assert_eq!(registry.preload_playback_queue_generation(session), Some(4));

    queue(&registry, session, 3);
    assert!(registry.release_preload(session));
    assert_eq!(registry.preload_playback_queue_generation(session), Some(6));
    assert!(!registry.release_preload(session));
    assert_eq!(registry.preload_playback_queue_generation(session), Some(6));
}

#[test]
fn undo_redo_and_failed_transaction_track_exact_queue_state() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    queue(&registry, session, 4);

    assert!(registry.undo(session));
    assert!(
        registry
            .preload_playback_actions(session)
            .unwrap()
            .is_empty()
    );
    assert_eq!(registry.preload_playback_queue_generation(session), Some(2));
    assert!(registry.redo(session));
    assert_eq!(registry.preload_playback_actions(session).unwrap().len(), 1);
    assert_eq!(registry.preload_playback_queue_generation(session), Some(3));

    let result = registry.with_transaction(session, || {
        queue(&registry, session, 5);
        Err::<(), _>("reject")
    });
    assert_eq!(result, Err("reject"));
    assert_eq!(registry.preload_playback_actions(session).unwrap().len(), 1);
    assert_eq!(registry.preload_playback_queue_generation(session), Some(3));
}

#[test]
fn inspection_removal_keeps_dynamic_and_group_release_order_and_live_state() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    let fixture = FixtureId::new();
    let attribute = AttributeKey::intensity();
    {
        let mut states = registry.state.write();
        let state = states.as_mut().unwrap();
        state.preload_dynamic_pending = Arc::new(
            [9, 2, 7]
                .map(|programmer_order| light_dynamics::DynamicAddressValue {
                    fixture_id: fixture,
                    attribute: attribute.clone(),
                    programmer_order,
                    changed_at_millis: 0,
                    value: light_dynamics::DynamicSemanticValue::Release,
                })
                .into(),
        );
        state.preload_group_release_pending = [8, 1, 6]
            .map(|programmer_order| crate::GroupReleaseProgrammerValue {
                group_id: "3".into(),
                attribute: attribute.clone(),
                programmer_order,
                changed_at_millis: 0,
            })
            .into();
    }
    let before = registry.get(session).unwrap();
    let content = registry.preload_pending_values(session).unwrap();
    assert_eq!(
        content
            .dynamic_values
            .iter()
            .map(|v| v.programmer_order)
            .collect::<Vec<_>>(),
        [9, 2, 7]
    );
    assert_eq!(
        content
            .group_release_values
            .iter()
            .map(|v| v.programmer_order)
            .collect::<Vec<_>>(),
        [8, 1, 6]
    );
    assert!(registry.remove_preload_dynamic_value(session, 1));
    assert!(registry.remove_preload_group_release(session, 1));
    let after = registry.get(session).unwrap();
    assert_eq!(
        after
            .preload_dynamic_pending
            .iter()
            .map(|v| v.programmer_order)
            .collect::<Vec<_>>(),
        [9, 7]
    );
    assert_eq!(
        after
            .preload_group_release_pending
            .iter()
            .map(|v| v.programmer_order)
            .collect::<Vec<_>>(),
        [8, 6]
    );
    assert_eq!(after.values, before.values);
    assert_eq!(after.dynamic_values, before.dynamic_values);
    assert_eq!(after.group_release_values, before.group_release_values);
    let depth = registry.undo_depth(session).unwrap();
    assert!(!registry.remove_preload_dynamic_value(session, 99));
    assert!(!registry.remove_preload_group_release(session, 99));
    assert_eq!(registry.undo_depth(session), Some(depth));
}

#[test]
fn inspector_static_removal_preserves_coexisting_release_rows_and_nonempty_live() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    let fixture = FixtureId::new();
    let attribute = AttributeKey::intensity();
    registry.set(
        session,
        fixture,
        attribute.clone(),
        AttributeValue::Normalized(0.6),
    );
    registry.arm_preload(session, true);
    assert!(registry.apply_preload_values(
        session,
        &[
            crate::PreloadProgrammerValueMutation::SetFixture {
                fixture_id: fixture,
                attribute: attribute.clone(),
                value: AttributeValue::Normalized(0.8),
                timing: Default::default()
            },
            crate::PreloadProgrammerValueMutation::SetGroup {
                group_id: "3".into(),
                attribute: attribute.clone(),
                value: AttributeValue::Normalized(0.4),
                timing: Default::default()
            },
        ]
    ));
    {
        let mut states = registry.state.write();
        let state = states.as_mut().unwrap();
        state.preload_dynamic_pending = Arc::new(vec![light_dynamics::DynamicAddressValue {
            fixture_id: fixture,
            attribute: attribute.clone(),
            programmer_order: 90,
            changed_at_millis: 0,
            value: light_dynamics::DynamicSemanticValue::Release,
        }]);
        state
            .preload_group_release_pending
            .push(crate::GroupReleaseProgrammerValue {
                group_id: "3".into(),
                attribute: attribute.clone(),
                programmer_order: 91,
                changed_at_millis: 0,
            });
    }
    let before = registry.get(session).unwrap();
    assert!(!before.values.is_empty());
    assert!(registry.apply_preload_values(
        session,
        &[
            crate::PreloadProgrammerValueMutation::RemoveFixtureValue {
                fixture_id: fixture,
                attribute: attribute.clone()
            },
            crate::PreloadProgrammerValueMutation::RemoveGroupValue {
                group_id: "3".into(),
                attribute: attribute.clone()
            },
        ]
    ));
    let after = registry.get(session).unwrap();
    assert!(after.preload_pending.is_empty());
    assert!(after.preload_group_pending.is_empty());
    assert_eq!(
        after.preload_dynamic_pending,
        before.preload_dynamic_pending
    );
    assert_eq!(
        after.preload_group_release_pending,
        before.preload_group_release_pending
    );
    assert_eq!(after.values, before.values);
    assert!(registry.undo(session));
    assert_eq!(
        registry.get(session).unwrap().preload_pending,
        before.preload_pending
    );
}
