#[test]
fn invalid_active_show_enters_recovery_instead_of_aborting_startup() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let entry = ShowEntry {
        id: light_core::ShowId::new(),
        name: "Damaged Show".into(),
        path: std::env::temp_dir()
            .join(format!("missing-{}.show", Uuid::new_v4()))
            .display()
            .to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };
    let error = compile_active_show_for_startup(&engine, &entry)
        .expect("invalid show should enter recovery mode");
    assert!(error.contains("might be corrupted or incompatible"));
    assert!(error.contains("Damaged Show"));
    assert_eq!(engine.snapshot().fixtures.len(), 0);
}
#[test]
fn repeated_group_command_freezes_membership_while_live_reference_refreshes() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    let first = light_core::FixtureId::new();
    let second = light_core::FixtureId::new();
    let third = light_core::FixtureId::new();
    let snapshot = |members| EngineSnapshot {
        groups: vec![light_programmer::GroupDefinition {
            id: "1".into(),
            name: "Group 1".into(),
            fixtures: members,
            ..Default::default()
        }],
        ..Default::default()
    };
    state
        .engine
        .replace_snapshot(snapshot(vec![first, second]))
        .unwrap();
    assert_eq!(
        execute_programmer_command(&state, &session, "GROUP GROUP 1").unwrap(),
        2
    );
    state
        .engine
        .replace_snapshot(snapshot(vec![first, second, third]))
        .unwrap();
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        vec![first, second]
    );
    assert!(execute_programmer_command(&state, &session, "GROUP GROUP 2").is_err());
    execute_programmer_command(&state, &session, "GROUP 1").unwrap();
    state
        .engine
        .replace_snapshot(snapshot(vec![third]))
        .unwrap();
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        vec![third]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mixed_selection_sources_dereference_only_the_addressed_term_and_replay_left_to_right() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    attach_session_command_context(&state, &session);

    let show_path = data_dir.join("shows/mixed-selection.show");
    let show_id = default_show::initialise(&show_path).unwrap();
    let entry = ShowEntry {
        id: show_id,
        name: "Mixed selection".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };
    let mut snapshot = load_engine_snapshot(&entry).unwrap();
    let fixture = |number| {
        snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture_number == Some(number))
            .unwrap()
            .fixture_id
    };
    let fixtures = [1, 2, 3, 4, 5, 6, 101, 102, 103]
        .into_iter()
        .map(fixture)
        .collect::<Vec<_>>();
    snapshot.groups = vec![
        light_programmer::GroupDefinition {
            id: "3".into(),
            name: "Front".into(),
            fixtures: fixtures[..4].to_vec(),
            ..Default::default()
        },
        light_programmer::GroupDefinition {
            id: "5".into(),
            name: "Back".into(),
            fixtures: fixtures[4..8].to_vec(),
            ..Default::default()
        },
    ];
    state.engine.replace_snapshot(snapshot.clone()).unwrap();

    assert_eq!(
        execute_programmer_command(&state, &session, "DEGRP 3 + G5").unwrap(),
        8
    );
    let mixed = state.programmers.get(session.id).unwrap();
    assert_eq!(mixed.selected, fixtures[..8]);
    let Some(light_programmer::SelectionExpression::Sources { items }) = mixed.selection_expression
    else {
        panic!("mixed command must retain ordered sources")
    };
    assert_eq!(items.len(), 5);
    assert!(
        items[..4]
            .iter()
            .all(|item| matches!(item, light_programmer::SelectionReference::Fixture { .. }))
    );
    assert_eq!(
        items[4],
        light_programmer::SelectionReference::LiveGroup {
            group_id: "5".into()
        }
    );

    snapshot.groups[1].fixtures = vec![fixtures[8], fixtures[4]];
    state.engine.replace_snapshot(snapshot).unwrap();
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        vec![
            fixtures[0],
            fixtures[1],
            fixtures[2],
            fixtures[3],
            fixtures[8],
            fixtures[4]
        ]
    );

    execute_programmer_command(&state, &session, "G3 - F2 + F2").unwrap();
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        vec![fixtures[0], fixtures[2], fixtures[3], fixtures[1]]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn set_group_requests_properties_only_for_the_originating_desk() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            groups: vec![light_programmer::GroupDefinition {
                id: "4".into(),
                name: "Center Spot".into(),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        execute_programmer_command(&state, &session, "SET GROUP 4").unwrap(),
        0
    );
    let event = state.audit_events.lock().back().cloned().unwrap();
    assert_eq!(event.kind, "group_configuration_requested");
    assert_eq!(event.payload["group_id"], "4");
    assert_eq!(event.payload["desk_id"], session.desk.id.to_string());
    assert!(execute_programmer_command(&state, &session, "SET GROUP 99").is_err());
    assert!(execute_programmer_command(&state, &session, "SET GROUP 4 EXTRA").is_err());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn record_group_supports_overwrite_merge_subtract_and_empty_source_delete() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    let show_path = data_dir.join("shows/record-group.show");
    let show_id = initialise_show(&show_path, "Record Group").unwrap();
    let entry = ShowEntry {
        id: show_id,
        name: "Record Group".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };
    let fixtures = (0..4)
        .map(|_| light_core::FixtureId::new())
        .collect::<Vec<_>>();
    let store = ShowStore::open(&show_path).unwrap();
    store
        .put_object(
            "group",
            "3",
            &serde_json::to_value(light_programmer::GroupDefinition {
                id: "3".into(),
                name: "Kept name".into(),
                fixtures: fixtures[..2].to_vec(),
                master: 0.4,
                ..Default::default()
            })
            .unwrap(),
            0,
        )
        .unwrap();
    *state.active_show.write() = Some(entry.clone());
    state
        .engine
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();

    state.programmers.select_expression(
        session.id,
        fixtures[..3].to_vec(),
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "3".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );
    execute_programmer_command(&state, &session, "RECORD GROUP 3").unwrap();
    let read_group = || {
        let object = ShowStore::open(&show_path)
            .unwrap()
            .objects("group")
            .unwrap()
            .into_iter()
            .find(|object| object.id == "3")
            .unwrap();
        serde_json::from_value::<light_programmer::GroupDefinition>(object.body).unwrap()
    };
    let overwritten = read_group();
    assert_eq!(overwritten.fixtures, fixtures[..3]);
    assert_eq!(overwritten.name, "Kept name");
    assert_eq!(overwritten.master, 0.4);
    assert!(overwritten.derived_from.is_none());

    let group_3_revision = ShowStore::open(&show_path)
        .unwrap()
        .objects("group")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "3")
        .unwrap()
        .revision;
    ShowStore::open(&show_path)
        .unwrap()
        .put_object(
            "group",
            "4",
            &serde_json::to_value(light_programmer::GroupDefinition {
                id: "4".into(),
                name: "Derived from 3".into(),
                derived_from: Some(light_programmer::DerivedGroup {
                    source_group_id: "3".into(),
                    rule: light_programmer::SelectionRule::All,
                }),
                ..Default::default()
            })
            .unwrap(),
            0,
        )
        .unwrap();
    state
        .engine
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();
    state.programmers.select_expression(
        session.id,
        fixtures[..3].to_vec(),
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "4".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );
    execute_programmer_command(&state, &session, "RECORD GROUP 3").unwrap();
    assert!(read_group().derived_from.is_none());
    assert!(
        ShowStore::open(&show_path)
            .unwrap()
            .objects("group")
            .unwrap()
            .into_iter()
            .find(|object| object.id == "3")
            .unwrap()
            .revision
            > group_3_revision
    );

    state.programmers.select(session.id, []);
    assert!(execute_programmer_command(&state, &session, "RECORD - GROUP 3").is_err());
    execute_programmer_command(&state, &session, "DELETE GROUP 4").unwrap();

    state
        .programmers
        .select(session.id, [fixtures[2], fixtures[3]]);
    execute_programmer_command(&state, &session, "RECORD + GROUP 3").unwrap();
    assert_eq!(read_group().fixtures, fixtures);

    state
        .programmers
        .select(session.id, [fixtures[1], fixtures[3]]);
    execute_programmer_command(&state, &session, "RECORD - GROUP 3").unwrap();
    assert_eq!(read_group().fixtures, vec![fixtures[0], fixtures[2]]);

    state.programmers.select(session.id, []);
    execute_programmer_command(&state, &session, "RECORD - GROUP 3").unwrap();
    assert!(
        ShowStore::open(&show_path)
            .unwrap()
            .objects("group")
            .unwrap()
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
