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
fn users_are_isolated() {
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
    assert_eq!(registry.get(first).unwrap().selected.len(), 1);
    assert!(registry.get(second).unwrap().selected.is_empty());
    assert!(registry.get(second).unwrap().values.is_empty());
    registry.set_group(
        first,
        "front".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    assert!(
        registry
            .get(first)
            .unwrap()
            .group_values
            .contains_key("front")
    );
    assert!(registry.get(second).unwrap().group_values.is_empty());
}
#[test]
fn sessions_for_the_same_user_share_values_but_keep_command_lines_local() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first = SessionId::new();
    let second = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(first, user);
    registry.select(first, [fixture]);
    registry.start(second, user);
    assert_eq!(registry.active().len(), 1);
    assert!(registry.get(second).unwrap().selected.is_empty());
    assert!(registry.set_command_line(first, "GROUP 1 +".into()));
    assert!(registry.set_command_line(second, "GROUP 2 +".into()));
    assert!(registry.set_command_target(first, "GROUP".into()));
    assert_eq!(registry.command_target(first), "GROUP");
    assert_eq!(registry.command_target(second), "FIXTURE");
    let mut command_lines = registry
        .active_for_sessions()
        .into_iter()
        .map(|state| state.command_line)
        .collect::<Vec<_>>();
    command_lines.sort();
    assert_eq!(command_lines, ["GROUP 1 +", "GROUP 2 +"]);
    assert_eq!(registry.get(second).unwrap().command_line, "GROUP 2 +");
    registry.disconnect(first);
    assert!(registry.get(second).unwrap().connected);
    registry.disconnect(second);
    assert!(!registry.active()[0].connected);
}

#[test]
fn sessions_share_programmer_values_by_user_and_command_interactions_by_desk() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first = SessionId::new();
    let second = SessionId::new();
    let other_desk_session = SessionId::new();
    let desk = SessionId::new();
    let other_desk = SessionId::new();
    let fixture = FixtureId::new();

    registry.start(first, user);
    registry.start(second, user);
    registry.start(other_desk_session, user);
    assert!(registry.attach_command_context(first, desk));
    assert!(registry.attach_command_context(second, desk));
    assert!(registry.attach_command_context(other_desk_session, other_desk));

    registry.select(first, [fixture]);
    registry.set(
        first,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.75),
    );
    assert_eq!(registry.get(second).unwrap().selected, vec![fixture]);
    assert!(
        registry
            .get(other_desk_session)
            .unwrap()
            .selected
            .is_empty()
    );
    assert_eq!(registry.get(other_desk_session).unwrap().values.len(), 1);

    assert!(registry.set_command_line(first, "GROUP 1 +".into()));
    assert!(registry.set_command_target(first, "GROUP".into()));
    assert_eq!(registry.get(second).unwrap().command_line, "GROUP 1 +");
    assert_eq!(registry.command_target(second), "GROUP");
    assert!(
        registry
            .get(other_desk_session)
            .unwrap()
            .command_line
            .is_empty()
    );
    assert_eq!(registry.command_target(other_desk_session), "FIXTURE");

    assert!(registry.set_command_line(other_desk_session, "FIXTURE 9".into()));
    assert_eq!(registry.get(first).unwrap().command_line, "GROUP 1 +");
}

#[test]
fn command_line_revisions_are_shared_by_desk_and_reject_stale_replacements() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first = SessionId::new();
    let second = SessionId::new();
    let other = SessionId::new();
    let desk = SessionId::new();
    let other_desk = SessionId::new();
    registry.start(first, user);
    registry.start(second, user);
    registry.start(other, user);
    assert!(registry.attach_command_context(first, desk));
    assert!(registry.attach_command_context(second, desk));
    assert!(registry.attach_command_context(other, other_desk));

    let initial = registry.command_line_state(first).unwrap();
    assert_eq!(initial.visible_text(), "FIXTURE");
    assert_eq!(initial.target, CommandTarget::Fixture);
    assert!(initial.pristine);
    assert_eq!(initial.revision, 0);

    let same_default = registry
        .replace_command_line(other, 0, "FIXTURE".into())
        .unwrap();
    assert_eq!(same_default.text, "");
    assert_eq!(same_default.visible_text(), "FIXTURE");
    assert_eq!(same_default.revision, 0);

    let group_default = registry
        .update_command_line(other, |_| ("GROUP".into(), CommandTarget::Group, true))
        .unwrap();
    assert_eq!(group_default.text, "");
    assert_eq!(group_default.visible_text(), "GROUP");
    assert_eq!(group_default.revision, 1);
    assert_eq!(
        registry
            .replace_command_line(other, group_default.revision, "GROUP".into())
            .unwrap()
            .revision,
        group_default.revision
    );

    let changed = registry
        .replace_command_line(first, 0, "GROUP 1 +".into())
        .unwrap();
    assert_eq!(changed.revision, 1);
    assert_eq!(registry.command_line_state(second).unwrap(), changed);
    assert_eq!(registry.command_line_state(other).unwrap(), group_default);

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
fn ordinary_selection_gestures_accumulate_per_desk_until_a_value_lands() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first = SessionId::new();
    let same_desk = SessionId::new();
    let other_desk = SessionId::new();
    let desk_context = SessionId::new();
    let other_context = SessionId::new();
    let first_fixture = FixtureId::new();
    let second_fixture = FixtureId::new();
    let third_fixture = FixtureId::new();
    registry.start(first, user);
    registry.start(same_desk, user);
    registry.start(other_desk, user);
    registry.attach_command_context(first, desk_context);
    registry.attach_command_context(same_desk, desk_context);
    registry.attach_command_context(other_desk, other_context);

    assert!(registry.apply_selection_gesture(
        first,
        vec![SelectionReference::Fixture {
            fixture_id: first_fixture,
        }],
        &HashMap::new(),
    ));
    assert!(registry.apply_selection_gesture(
        same_desk,
        vec![SelectionReference::Fixture {
            fixture_id: second_fixture,
        }],
        &HashMap::new(),
    ));
    assert_eq!(
        registry.get(first).unwrap().selected,
        vec![first_fixture, second_fixture]
    );
    assert!(registry.get(other_desk).unwrap().selected.is_empty());

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
    assert_eq!(registry.get(other_desk).unwrap().values.len(), 1);
}
