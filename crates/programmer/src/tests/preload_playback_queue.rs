use super::*;

fn queue(registry: &ProgrammerRegistry, session: SessionId, number: u16) {
    assert!(registry.queue_preload_playback_action(
        session,
        number,
        PreloadPlaybackQueueAction::Go,
        PreloadPlaybackQueueSurface::Virtual,
    ));
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
