use super::*;

#[test]
fn selection_revision_identifies_operations_but_ignores_value_changes() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());

    registry.select(session, [fixture]);
    let first = registry.selection(session).unwrap();
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    assert_eq!(
        registry.selection(session).unwrap().revision,
        first.revision
    );

    registry.select(session, [fixture]);
    let reselection = registry.selection(session).unwrap();
    assert!(reselection.revision > first.revision);
    assert_eq!(reselection.selected, first.selected);
    assert_eq!(reselection.expression, first.expression);
}

#[test]
fn persisted_programmer_omits_live_undo_and_redo_history() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session, UserId::new());
    registry.select(session, [FixtureId::new()]);

    let encoded = serde_json::to_value(registry.get(session).unwrap()).unwrap();

    assert!(encoded.get("undo").is_none());
    assert!(encoded.get("redo").is_none());
    let restored: ProgrammerState = serde_json::from_value(encoded).unwrap();
    assert!(restored.undo.is_empty());
    assert!(restored.redo.is_empty());
}

#[test]
fn restored_session_aliases_are_retained_but_not_reported_as_live_connections() {
    let user = UserId::new();
    let old_session = SessionId::new();
    let old_fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let source = ProgrammerRegistry::default();
    source.start(old_session, user);
    source.select(old_session, old_fixtures);
    let mut persisted = source.get(old_session).unwrap();
    persisted.connected = false;

    let restored = ProgrammerRegistry::default();
    restored.restore(persisted);
    assert!(
        restored.get(old_session).is_some(),
        "durable state remains available"
    );
    assert!(
        restored.active_programmer_lifecycles().is_empty(),
        "a historical browser session is not a current connection"
    );

    let current_session = SessionId::new();
    let current_fixtures = [FixtureId::new(), FixtureId::new()];
    restored.start(current_session, user);
    restored.select(current_session, current_fixtures);
    let lifecycle = restored.programmer_lifecycle(user).unwrap();
    assert_eq!(lifecycle.connected_sessions.len(), 1);
    assert_eq!(lifecycle.connected_sessions[0].session_id, current_session);
    assert_eq!(lifecycle.selected_fixture_count, 2);
}

#[test]
fn revisioned_selection_replacement_cannot_overwrite_a_concurrent_change() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let first = FixtureId::new();
    let second = FixtureId::new();
    registry.start(session, UserId::new());
    let expected = registry.selection(session).unwrap().revision;

    let accepted = registry
        .replace_selection_if_revision(session, expected, [first], SelectionExpression::Static)
        .unwrap();
    let rejected = registry.replace_selection_if_revision(
        session,
        expected,
        [second],
        SelectionExpression::Static,
    );

    assert_eq!(accepted.selected, vec![first]);
    assert_eq!(
        rejected,
        Err(SelectionReplaceError::RevisionConflict {
            expected,
            actual: accepted.revision,
        })
    );
    assert_eq!(registry.selection(session).unwrap(), accepted);
}

#[test]
fn selection_projection_versions_the_gesture_boundary() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let first = FixtureId::new();
    let second = FixtureId::new();
    registry.start(session, UserId::new());

    assert!(registry.apply_selection_gesture(
        session,
        vec![SelectionReference::Fixture { fixture_id: first }],
        &HashMap::new(),
    ));
    let open = registry.selection(session).unwrap();
    assert!(open.gesture_open);

    assert!(registry.finish_selection_gesture(session));
    let closed = registry.selection(session).unwrap();
    assert!(!closed.gesture_open);
    assert!(closed.revision > open.revision);
    assert_eq!(closed.selected, open.selected);
    assert_eq!(closed.expression, open.expression);
    assert!(!registry.finish_selection_gesture(session));
    assert_eq!(
        registry.selection(session).unwrap().revision,
        closed.revision
    );

    assert!(registry.apply_selection_gesture(
        session,
        vec![SelectionReference::Fixture { fixture_id: second }],
        &HashMap::new(),
    ));
    assert_eq!(registry.selection(session).unwrap().selected, vec![second]);
}

/// One desk, one Programmer. A connection presenting a different identity — a second screen, an
/// OSC client, an attached hardware surface — joins the Programmer the desk already has instead of
/// opening a private one beside it.
#[test]
fn a_connection_presenting_another_identity_joins_the_one_desk_programmer() {
    let registry = ProgrammerRegistry::default();
    let first = SessionId::new();
    let second = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(first, UserId::new());
    registry.start(second, UserId::new());
    registry.select(first, [fixture]);
    registry.set(
        first,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    );

    assert_eq!(registry.active().len(), 1, "a desk has one Programmer");
    assert_eq!(registry.get(second).unwrap().selected.len(), 1);
    assert_eq!(
        registry.get(second).unwrap().values[0].value,
        AttributeValue::Normalized(1.0)
    );
    assert_eq!(
        registry.get(first).unwrap().user_id,
        registry.get(second).unwrap().user_id,
        "both connections operate the same authority"
    );

    registry.set_group(
        first,
        "front".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    assert!(
        registry
            .get(second)
            .unwrap()
            .group_values
            .contains_key("front")
    );
}

#[test]
/// Two connections are two views of one desk: the same values, the same selection, and the same
/// command line, because the operator typing on either is typing on the desk.
fn sessions_share_values_selection_and_one_command_line() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first = SessionId::new();
    let second = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(first, user);
    registry.select(first, [fixture]);
    registry.start(second, user);
    assert_eq!(registry.active().len(), 1);
    assert_eq!(registry.get(second).unwrap().selected, vec![fixture]);
    assert!(registry.set_command_line(first, "GROUP 1 +".into()));
    assert_eq!(registry.get(second).unwrap().command_line, "GROUP 1 +");
    assert!(registry.set_command_line(second, "GROUP 2 +".into()));
    assert_eq!(registry.get(first).unwrap().command_line, "GROUP 2 +");
    assert!(registry.set_command_target(first, "GROUP".into()));
    assert_eq!(registry.command_target(first), "GROUP");
    assert_eq!(registry.command_target(second), "GROUP");
    let command_lines = registry
        .active_for_sessions()
        .into_iter()
        .map(|state| state.command_line)
        .collect::<Vec<_>>();
    assert_eq!(command_lines, ["GROUP 2 +", "GROUP 2 +"]);
    registry.disconnect(first);
    assert!(registry.get(second).unwrap().connected);
    registry.disconnect(second);
    assert!(!registry.active()[0].connected);
}

/// The compatibility projection keyed by identity still answers, and now answers with every
/// connection: a foreign identity is no longer foreign, so nothing is filtered out of the desk.
#[test]
fn the_identity_projection_returns_every_connection_to_the_one_desk() {
    let registry = ProgrammerRegistry::default();
    let first = SessionId::new();
    let second = SessionId::new();
    let arriving_as_someone_else = SessionId::new();
    registry.start(first, UserId::new());
    registry.start(second, UserId::new());
    registry.start(arriving_as_someone_else, UserId::new());
    let desk_user = registry.get(first).unwrap().user_id;
    registry.set(
        arriving_as_someone_else,
        FixtureId::new(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );

    let rows = registry.active_for_user_sessions(desk_user);

    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| row.user_id == desk_user));
    assert!(
        rows.iter().all(|row| row.values.len() == 1),
        "every connection observes the value any of them set"
    );
}

#[test]
/// A surface that asks to attach to some other interaction context joins this desk's instead.
/// There is no other context to attach to, so a saved hardware configuration naming one keeps
/// working and simply lands where every other surface already is.
fn every_surface_shares_the_desk_values_selection_and_command_interactions() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first = SessionId::new();
    let second = SessionId::new();
    let hardware = SessionId::new();
    let saved_context = SessionId::new();
    let fixture = FixtureId::new();

    registry.start(first, user);
    registry.start(second, user);
    registry.start(hardware, user);
    assert!(registry.attach_command_context(hardware, saved_context));

    registry.select(first, [fixture]);
    registry.set(
        first,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.75),
    );
    assert_eq!(registry.get(second).unwrap().selected, vec![fixture]);
    assert_eq!(registry.get(hardware).unwrap().selected, vec![fixture]);
    assert_eq!(registry.get(hardware).unwrap().values.len(), 1);

    assert!(registry.set_command_line(first, "GROUP 1 +".into()));
    assert!(registry.set_command_target(first, "GROUP".into()));
    assert_eq!(registry.get(second).unwrap().command_line, "GROUP 1 +");
    assert_eq!(registry.command_target(second), "GROUP");
    assert_eq!(registry.get(hardware).unwrap().command_line, "GROUP 1 +");
    assert_eq!(registry.command_target(hardware), "GROUP");

    assert!(registry.set_command_line(hardware, "FIXTURE 9".into()));
    assert_eq!(registry.get(first).unwrap().command_line, "FIXTURE 9");
}

#[test]
fn one_command_line_is_shared_by_every_surface_and_rejects_stale_replacements() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first = SessionId::new();
    let second = SessionId::new();
    let third = SessionId::new();
    registry.start(first, user);
    registry.start(second, user);
    registry.start(third, user);

    let initial = registry.command_line_state(first).unwrap();
    assert_eq!(initial.visible_text(), "FIXTURE");
    assert_eq!(initial.target, CommandTarget::Fixture);
    assert!(initial.pristine);
    assert_eq!(initial.revision, 0);

    // Replacing the line with what it already says is not a change, so the revision holds and a
    // surface reconciling against it is not made stale for nothing.
    let same_default = registry
        .replace_command_line(third, 0, "FIXTURE".into())
        .unwrap();
    assert_eq!(same_default.text, "");
    assert_eq!(same_default.visible_text(), "FIXTURE");
    assert_eq!(same_default.revision, 0);

    let changed = registry
        .replace_command_line(first, 0, "GROUP 1 +".into())
        .unwrap();
    assert_eq!(changed.revision, 1);
    assert_eq!(registry.command_line_state(second).unwrap(), changed);
    assert_eq!(
        registry.command_line_state(third).unwrap(),
        changed,
        "every surface is looking at the one command line"
    );

    assert_eq!(
        registry.replace_command_line(second, 0, "GROUP 2".into()),
        Err(CommandLineReplaceError::RevisionConflict {
            expected: 0,
            actual: 1,
        })
    );
    assert_eq!(
        registry.command_line_state(first).unwrap().text,
        "GROUP 1 +"
    );

    let edited = registry
        .update_command_line(second, |current| {
            (format!("{} F2", current.text), current.target, false)
        })
        .unwrap();
    assert_eq!(edited.text, "GROUP 1 + F2");
    assert_eq!(edited.revision, 2);
}

#[test]
fn pending_command_choices_are_revisioned_shared_by_every_surface_and_cleared_by_edits() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first = SessionId::new();
    let peer = SessionId::new();
    let other = SessionId::new();
    for session in [first, peer, other] {
        registry.start(session, user);
    }
    let command = "COPY SET 1 CUE 1 AT SET 2 CUE 2";
    let edited = registry
        .replace_command_line(first, 0, command.into())
        .unwrap();
    assert!(edited.pending_choice.is_none());

    let choice = CueMoveCopyChoice {
        choice_id: uuid::Uuid::from_u128(1),
        show_id: uuid::Uuid::from_u128(2),
        show_revision: 3,
        operation: CueTransferOperation::Copy,
        command: command.into(),
        options: vec![],
        cancel_label: "Cancel".into(),
    };
    let pending_choice = PendingCommandChoice::CueMoveCopy(choice.clone());
    let pending = registry
        .set_pending_command_choice(first, Some(pending_choice.clone()))
        .unwrap();
    assert_eq!(pending.revision, edited.revision + 1);
    assert_eq!(pending.pending_choice.as_deref(), Some(&pending_choice));
    assert_eq!(registry.command_line_state(peer).unwrap(), pending);
    assert_eq!(
        registry.command_line_state(other).unwrap(),
        pending,
        "a choice the desk is waiting on is put to every surface, not one of them"
    );

    let repeated = registry
        .set_pending_command_choice(peer, Some(PendingCommandChoice::CueMoveCopy(choice)))
        .unwrap();
    assert_eq!(repeated.revision, pending.revision);
    assert!(
        serde_json::to_value(&repeated)
            .unwrap()
            .get("pending_choice")
            .is_none()
    );
    let unchanged = registry
        .replace_command_line(peer, repeated.revision, command.into())
        .unwrap();
    assert_eq!(unchanged, repeated);
    let cleared = registry
        .replace_command_line(peer, repeated.revision, format!("{command} "))
        .unwrap();
    assert!(cleared.pending_choice.is_none());
    assert_eq!(cleared.revision, repeated.revision + 1);
}

#[test]
fn concurrent_command_line_replacements_have_one_cas_winner() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session, UserId::new());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let mut workers = Vec::new();
    for command in ["FIXTURE 1", "GROUP 1"] {
        let registry = registry.clone();
        let barrier = std::sync::Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            registry.replace_command_line(session, 0, command.into())
        }));
    }
    barrier.wait();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(
                result,
                Err(CommandLineReplaceError::RevisionConflict { .. })
            ))
            .count(),
        1
    );
    assert_eq!(registry.command_line_state(session).unwrap().revision, 1);
}

#[test]
fn ordered_selection_sources_remove_and_readd_left_to_right_and_stay_live() {
    let first = FixtureId::new();
    let second = FixtureId::new();
    let third = FixtureId::new();
    let fourth = FixtureId::new();
    let mut groups = HashMap::from([(
        "3".into(),
        GroupDefinition {
            id: "3".into(),
            name: "Group 3".into(),
            fixtures: vec![first, second, third],
            ..Default::default()
        },
    )]);
    let sources = vec![
        SelectionReference::LiveGroup {
            group_id: "3".into(),
        },
        SelectionReference::RemoveFixture { fixture_id: second },
        SelectionReference::Fixture { fixture_id: second },
        SelectionReference::Fixture { fixture_id: fourth },
    ];
    assert_eq!(
        resolve_selection_references(&sources, &groups),
        vec![first, third, second, fourth]
    );

    groups.get_mut("3").unwrap().fixtures = vec![third, first];
    assert_eq!(
        resolve_selection_references(&sources, &groups),
        vec![third, first, second, fourth]
    );
}

#[test]
fn ordinary_selection_gestures_accumulate_across_the_desk_until_a_value_lands() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first = SessionId::new();
    let second_surface = SessionId::new();
    let third_surface = SessionId::new();
    let first_fixture = FixtureId::new();
    let second_fixture = FixtureId::new();
    let third_fixture = FixtureId::new();
    registry.start(first, user);
    registry.start(second_surface, user);
    registry.start(third_surface, user);

    assert!(registry.apply_selection_gesture(
        first,
        vec![SelectionReference::Fixture {
            fixture_id: first_fixture,
        }],
        &HashMap::new(),
    ));
    assert!(registry.apply_selection_gesture(
        second_surface,
        vec![SelectionReference::Fixture {
            fixture_id: second_fixture,
        }],
        &HashMap::new(),
    ));
    // A gesture continued on another surface continues the same gesture, because the operator
    // pressing either is building one selection on one desk.
    assert_eq!(
        registry.get(first).unwrap().selected,
        vec![first_fixture, second_fixture]
    );
    assert_eq!(
        registry.get(third_surface).unwrap().selected,
        vec![first_fixture, second_fixture]
    );

    registry.set(
        first,
        first_fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    assert!(registry.apply_selection_gesture(
        first,
        vec![SelectionReference::Fixture {
            fixture_id: third_fixture,
        }],
        &HashMap::new(),
    ));
    assert_eq!(registry.get(first).unwrap().selected, vec![third_fixture]);
    assert_eq!(registry.get(third_surface).unwrap().values.len(), 1);
}
