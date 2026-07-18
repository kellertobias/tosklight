use super::*;

#[test]
fn rejected_transaction_rolls_back_before_a_same_user_mutation_can_run() {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let registry = Arc::new(ProgrammerRegistry::default());
    let user = UserId::new();
    let transaction_session = SessionId::new();
    let concurrent_session = SessionId::new();
    let unrelated_session = SessionId::new();
    let fixture = FixtureId::new();
    let unrelated_fixture = FixtureId::new();
    registry.start(transaction_session, user);
    registry.start(concurrent_session, user);
    registry.start(unrelated_session, UserId::new());
    registry.set(
        transaction_session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.1),
    );
    registry.set_command_line(transaction_session, "FIXTURE 1".into());

    let transaction_gate = registry.mutation_gate(transaction_session);
    let concurrent_gate = registry.mutation_gate(concurrent_session);
    assert!(Arc::ptr_eq(&transaction_gate, &concurrent_gate));

    let (transaction_entered_tx, transaction_entered_rx) = mpsc::channel();
    let (reject_tx, reject_rx) = mpsc::channel();
    let transaction_registry = Arc::clone(&registry);
    let transaction_thread = thread::spawn(move || {
        let result = transaction_registry.with_transaction(transaction_session, || {
            transaction_registry.set(
                transaction_session,
                fixture,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.5),
            );
            transaction_registry.set_command_line(transaction_session, "FIXTURE 1 AT BOGUS".into());
            transaction_entered_tx.send(()).unwrap();
            reject_rx.recv().unwrap();
            Err::<(), _>("rejected")
        });
        assert_eq!(result, Err("rejected"));
    });

    transaction_entered_rx.recv().unwrap();
    assert!(concurrent_gate.try_lock().is_none());
    registry.set(
        unrelated_session,
        unrelated_fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.7),
    );
    assert_eq!(
        registry.get(unrelated_session).unwrap().values[0].value,
        AttributeValue::Normalized(0.7)
    );

    let (mutation_attempted_tx, mutation_attempted_rx) = mpsc::channel();
    let (mutation_finished_tx, mutation_finished_rx) = mpsc::channel();
    let concurrent_registry = Arc::clone(&registry);
    let concurrent_thread = thread::spawn(move || {
        mutation_attempted_tx.send(()).unwrap();
        concurrent_registry.set(
            concurrent_session,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.9),
        );
        concurrent_registry.set_command_line(concurrent_session, "GROUP 2".into());
        mutation_finished_tx.send(()).unwrap();
    });

    mutation_attempted_rx.recv().unwrap();
    assert!(matches!(
        mutation_finished_rx.recv_timeout(Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    reject_tx.send(()).unwrap();
    transaction_thread.join().unwrap();
    mutation_finished_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    concurrent_thread.join().unwrap();

    let programmer = registry.get(transaction_session).unwrap();
    assert_eq!(programmer.values.len(), 1);
    assert_eq!(programmer.values[0].value, AttributeValue::Normalized(0.9));
    assert_eq!(
        registry
            .command_line_state(transaction_session)
            .unwrap()
            .text,
        "FIXTURE 1"
    );
    assert_eq!(
        registry
            .command_line_state(concurrent_session)
            .unwrap()
            .text,
        "GROUP 2"
    );
}

#[test]
fn staged_transaction_is_invisible_until_one_successful_commit() {
    use std::sync::mpsc;
    use std::thread;

    let registry = Arc::new(ProgrammerRegistry::default());
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());
    registry.select(session, [fixture]);
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.25),
    );
    let before = registry.get(session).unwrap();
    let (staged_tx, staged_rx) = mpsc::channel();
    let (commit_tx, commit_rx) = mpsc::channel();
    let worker_registry = Arc::clone(&registry);
    let worker = thread::spawn(move || {
        worker_registry.with_staged_transaction(session, |staged| {
            staged.set(
                session,
                fixture,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.75),
            );
            staged.select(session, [fixture, FixtureId::new()]);
            staged_tx.send(()).unwrap();
            commit_rx.recv().unwrap();
            Ok::<_, String>(())
        })
    });

    staged_rx.recv().unwrap();
    assert_eq!(
        serde_json::to_value(registry.get(session).unwrap()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    commit_tx.send(()).unwrap();
    worker.join().unwrap().unwrap();

    let after = registry.get(session).unwrap();
    assert_ne!(
        serde_json::to_value(&after).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(after.selected.len(), 2);
    assert_eq!(
        after
            .values
            .iter()
            .find(|value| value.fixture_id == fixture)
            .and_then(|value| value.value.normalized()),
        Some(0.75)
    );
}

#[test]
fn staged_command_is_one_undo_step_even_when_helpers_checkpoint_internally() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let original = FixtureId::new();
    let first = FixtureId::new();
    let second = FixtureId::new();
    registry.start(session, UserId::new());
    registry.select(session, [original]);
    registry.set(
        session,
        original,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.25),
    );
    let before = registry.get(session).unwrap();
    let undo_before = before.undo.len();

    registry
        .with_staged_command(session, |staged| {
            staged.select(session, [first, second]);
            staged.set_many_faded_with_timing(
                session,
                [
                    (
                        first,
                        AttributeKey::intensity(),
                        AttributeValue::Normalized(0.5),
                    ),
                    (
                        second,
                        AttributeKey::intensity(),
                        AttributeValue::Normalized(0.75),
                    ),
                ],
                Some(1_000),
                None,
            );
            Ok::<_, String>(())
        })
        .unwrap();

    assert_eq!(registry.get(session).unwrap().undo.len(), undo_before + 1);
    assert!(registry.undo(session));
    let restored = registry.get(session).unwrap();
    assert_eq!(restored.selected, before.selected);
    assert_eq!(
        serde_json::to_value(restored.values).unwrap(),
        serde_json::to_value(before.values).unwrap()
    );
}

#[test]
fn rejected_staged_transaction_never_changes_live_state() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());
    let before = serde_json::to_value(registry.get(session).unwrap()).unwrap();

    let result = registry.with_staged_transaction(session, |staged| {
        staged.set(
            session,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
        Err::<(), _>("rejected".to_owned())
    });

    assert_eq!(result, Err("rejected".to_owned()));
    assert_eq!(
        serde_json::to_value(registry.get(session).unwrap()).unwrap(),
        before
    );
}

#[test]
fn projected_reads_cannot_mix_old_state_with_a_staged_commit() {
    use std::thread;
    use std::time::Duration;

    let registry = Arc::new(ProgrammerRegistry::default());
    let session = SessionId::new();
    let old_fixture = FixtureId::new();
    let new_fixture = FixtureId::new();
    registry.start(session, UserId::new());
    registry.select(session, [old_fixture]);
    registry.set_command_line(session, "FIXTURE 1".into());

    let staged = registry.detached_session(session).unwrap();
    staged.select(session, [new_fixture]);
    staged.set_command_line(session, "GROUP 2".into());

    // Block the reader on its second projection lock after it has acquired the state lock.
    let command_guard = registry.command_states.write();
    let reader_registry = Arc::clone(&registry);
    let reader = thread::spawn(move || reader_registry.get(session).unwrap());
    for _ in 0..100 {
        if registry.states.try_write().is_none() {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(
        registry.states.try_write().is_none(),
        "the reader never acquired the state projection lock"
    );

    let commit_registry = Arc::clone(&registry);
    let commit = thread::spawn(move || commit_registry.commit_detached_session(session, &staged));
    drop(command_guard);

    let projected = reader.join().unwrap();
    assert_eq!(projected.selected, vec![old_fixture]);
    assert_eq!(projected.command_line, "FIXTURE 1");
    assert!(commit.join().unwrap());
    let committed = registry.get(session).unwrap();
    assert_eq!(committed.selected, vec![new_fixture]);
    assert_eq!(committed.command_line, "GROUP 2");
}

#[test]
fn unknown_sessions_share_one_fallback_gate_without_growing_the_user_registry() {
    let registry = ProgrammerRegistry::default();
    let first = registry.mutation_gate(SessionId::new());
    for _ in 0..1_000 {
        let session = SessionId::new();
        assert!(!registry.clear(session));
        assert!(Arc::ptr_eq(&first, &registry.mutation_gate(session)));
    }
    assert!(registry.mutation_gates.read().is_empty());
}

#[test]
fn reset_preserves_the_per_user_gate_across_session_aliases() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first_session = SessionId::new();
    let alias_session = SessionId::new();
    registry.start(first_session, user);
    registry.start(alias_session, user);

    let before = registry.mutation_gate(alias_session);
    registry.reset_all();

    let restored_session = SessionId::new();
    registry.start(restored_session, user);
    let after = registry.mutation_gate(restored_session);
    assert!(Arc::ptr_eq(&before, &after));
}
