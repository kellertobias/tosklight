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
    registry.start(session, UserId::new());

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
    registry.start(session, UserId::new());

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
    registry.start(session, UserId::new());

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
    registry.start(session, UserId::new());
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
