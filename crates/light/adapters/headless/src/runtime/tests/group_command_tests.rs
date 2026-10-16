#[test]
fn invalid_active_show_enters_recovery_instead_of_aborting_startup() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let entry = ShowEntry {
        is_base_show: false,
        id: light_core::ShowId::new(),
        name: "Damaged Show".into(),
        path: std::env::temp_dir()
            .join(format!("missing-{}.show", Uuid::new_v4()))
            .display()
            .to_string(),
        revision: 0,
        updated_at: String::new(),
        created_at: None,
        last_loaded_at: None,
        revision_copy: None,
    };
    let error = compile_active_show_for_startup(&engine, &entry, &std::env::temp_dir(), 5)
        .expect("invalid show should enter recovery mode");
    assert!(error.contains("might be corrupted or incompatible"));
    assert!(error.contains("Damaged Show"));
    assert_eq!(engine.snapshot().fixtures.len(), 0);
}
#[test]
fn repeated_group_command_freezes_membership_while_live_reference_refreshes() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let first = light_core::FixtureId::new();
    let second = light_core::FixtureId::new();
    let third = light_core::FixtureId::new();
    let snapshot = |members| EngineSnapshot {
        groups: vec![light_programmer::GroupDefinition {
            id: "1".into(),
            name: "Group 1".into(),
            fixtures: members,
            ..Default::default()
        }]
        .into(),
        ..Default::default()
    };
    state
        .output
        .replace_snapshot(snapshot(vec![first, second]))
        .unwrap();
    assert_eq!(
        execute_programmer_command(&state, &session, "DEGROUP 1").unwrap(),
        2
    );
    state
        .output
        .replace_snapshot(snapshot(vec![first, second, third]))
        .unwrap();
    assert_eq!(
        state.programming.get(session.id).unwrap().selected,
        vec![first, second]
    );
    assert!(execute_programmer_command(&state, &session, "DEGROUP 2").is_err());
    assert_eq!(
        execute_programmer_command(&state, &session, "DEGRP 1").unwrap(),
        3,
        "the legacy DEGRP spelling remains a compatible input alias"
    );
    // GROUP GROUP is not a command in any string surface; the keypad's second Group press
    // replaces GROUP with DEGROUP instead of appending a second word.
    assert!(
        execute_programmer_command(&state, &session, "GROUP GROUP 1")
            .unwrap_err()
            .contains("DEGROUP")
    );
    execute_programmer_command(&state, &session, "GROUP 1").unwrap();
    state
        .output
        .replace_snapshot(snapshot(vec![third]))
        .unwrap();
    assert_eq!(
        state.programming.get(session.id).unwrap().selected,
        vec![third]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mixed_selection_sources_dereference_only_the_addressed_term_and_replay_left_to_right() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    attach_session_command_context(&state, &session);

    let show_path = data_dir.join("shows/mixed-selection.show");
    let show_id = default_show::initialise_legacy_test_show(&show_path).unwrap();
    let entry = ShowEntry {
        is_base_show: false,
        id: show_id,
        name: "Mixed selection".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        created_at: None,
        last_loaded_at: None,
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
    ]
    .into();
    state.output.replace_snapshot(snapshot.clone()).unwrap();

    assert_eq!(
        execute_programmer_command(&state, &session, "DEGROUP 3 + G5").unwrap(),
        8
    );
    let mixed = state.programming.get(session.id).unwrap();
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

    std::sync::Arc::make_mut(&mut snapshot.groups)[1].fixtures = vec![fixtures[8], fixtures[4]];
    state.output.replace_snapshot(snapshot).unwrap();
    assert_eq!(
        state.programming.get(session.id).unwrap().selected,
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
        state.programming.get(session.id).unwrap().selected,
        vec![fixtures[0], fixtures[2], fixtures[3], fixtures[1]]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn set_group_requests_properties_only_for_the_originating_desk() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    state
        .output
        .replace_snapshot(EngineSnapshot {
            groups: vec![light_programmer::GroupDefinition {
                id: "4".into(),
                name: "Center Spot".into(),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        execute_programmer_command(&state, &session, "SET GROUP 4").unwrap(),
        0
    );
    let event = state.events.audit_events().last().cloned().unwrap();
    assert_eq!(event.kind, "group_configuration_requested");
    assert_eq!(event.payload["group_id"], "4");
    assert_eq!(event.payload["desk_id"], session.desk.id.to_string());
    assert!(execute_programmer_command(&state, &session, "SET GROUP 99").is_err());
    assert!(execute_programmer_command(&state, &session, "SET GROUP 4 EXTRA").is_err());

    assert_eq!(
        execute_programmer_command(&state, &session, "ASSIGN PBK 6").unwrap(),
        0
    );
    let event = state.events.audit_events().last().cloned().unwrap();
    assert_eq!(event.kind, "playback_configuration_requested");
    assert_eq!(event.payload["address"], serde_json::json!({"page":1,"position":6}));
    assert_eq!(event.payload["desk_id"], session.desk.id.to_string());

    assert_eq!(
        execute_programmer_command(&state, &session, "ASSIGN PBK 4 . 1000").unwrap(),
        0
    );
    let event = state.events.audit_events().last().cloned().unwrap();
    assert_eq!(event.payload["address"], serde_json::json!({"page":4,"position":1000}));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn assign_group_at_page_slot_assigns_a_group_master_and_set_rejects_assignment() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.sessions.insert_session(session.clone());
    state.programming.start(session.id);
    let show_path = data_dir.join("shows/group-master-command.show");
    let show_id = initialise_show(&show_path, "Group Master Command").unwrap();
    let entry = ShowEntry {
        is_base_show: false,
        id: show_id,
        name: "Group Master Command".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        created_at: None,
        last_loaded_at: None,
        revision_copy: None,
    };
    let store = ShowStore::open(&show_path).unwrap();
    store
        .put_object(
            "group",
            "4",
            &serde_json::to_value(light_programmer::GroupDefinition {
                id: "4".into(),
                name: "Center Spot".into(),
                ..Default::default()
            })
            .unwrap(),
            0,
        )
        .unwrap();
    let group_revision = store
        .objects("group")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "4")
        .unwrap()
        .revision;
    state.active_show.replace_current(Some(entry.clone()));
    state
        .output
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();

    let rejected =
        execute_programmer_command(&state, &session, "SET GROUP 4 AT 1 . 2").unwrap_err();
    assert!(rejected.contains("use ASSIGN"));
    assert_eq!(
        execute_programmer_command(&state, &session, "ASSIGN GROUP 4 AT PBK 1 . 2").unwrap(),
        1
    );

    let store = ShowStore::open(&show_path).unwrap();
    let page = store
        .objects("playback_page")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "1")
        .unwrap();
    let page = serde_json::from_value::<light_playback::PlaybackPage>(page.body).unwrap();
    let playback = page.assignments.get(&2).unwrap();
    assert!(matches!(
        playback.target,
        light_playback::PlaybackTarget::Group { ref group_id, .. } if group_id == "4"
    ));
    assert_eq!(
        store
            .objects("group")
            .unwrap()
            .into_iter()
            .find(|object| object.id == "4")
            .unwrap()
            .revision,
        group_revision,
        "Group Master assignment must not mutate Group data",
    );
    assert_eq!(page.assignments[&2].target, playback.target);

    assert_eq!(
        execute_programmer_command(&state, &session, "ASSIGN GROUP 4 AT PBK 1 . 1000").unwrap(),
        1
    );
    let virtual_page = ShowStore::open(&show_path)
        .unwrap()
        .objects("playback_page")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "1")
        .map(|object| serde_json::from_value::<light_playback::PlaybackPage>(object.body).unwrap())
        .unwrap();
    assert!(matches!(
        virtual_page
            .assignments
            .get(&1000)
            .map(|playback| &playback.target),
        Some(light_playback::PlaybackTarget::Group { group_id, .. }) if group_id == "4"
    ));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn assign_cuelist_uses_pool_identity_for_explicit_page_targets() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.sessions.insert_session(session.clone());
    state.programming.start(session.id);
    let show_path = data_dir.join("shows/assign-cuelist-command.show");
    let show_id = initialise_show(&show_path, "Assign Cuelist Command").unwrap();
    let entry = ShowEntry {
        is_base_show: false,
        id: show_id,
        name: "Assign Cuelist Command".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        created_at: None,
        last_loaded_at: None,
        revision_copy: None,
    };
    let store = ShowStore::open(&show_path).unwrap();
    let cue_list = light_playback::CueList {
        pool_number: Some(4),
        legacy_pool_aliases: Vec::new(),
        id: light_core::CueListId::new(),
        name: "Main".into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![light_playback::Cue::new(cue("1"))],
    };
    store
        .put_object(
            "cue_list",
            &cue_list.id.0.to_string(),
            &serde_json::to_value(&cue_list).unwrap(),
            0,
        )
        .unwrap();
    let target = light_playback::PlaybackTarget::CueList {
        cue_list_id: cue_list.id,
    };
    let source = light_playback::PlaybackDefinition {
        name: "Main".into(),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#123456".into(),
        flash_release: light_playback::FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
        target,
    };
    store
        .put_object("playback_page", "1", &serde_json::json!({"number":1,"name":"Main","assignments":{"4":source}}), 0)
        .unwrap();
    state.active_show.replace_current(Some(entry.clone()));
    state
        .output
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();

    assert_eq!(
        execute_programmer_command(&state, &session, "ASSIGN CUELIST 4 AT PBK 2 . 6").unwrap(),
        1
    );
    assert_eq!(
        execute_programmer_command(&state, &session, "ASSIGN CUELIST 4 AT PBK 1 . 1000").unwrap(),
        1
    );
    let pages = ShowStore::open(&show_path)
        .unwrap()
        .objects("playback_page")
        .unwrap();
    let physical_page: light_playback::PlaybackPage = serde_json::from_value(
        pages
            .iter()
            .find(|object| object.id == "2")
            .unwrap()
            .body
            .clone(),
    )
    .unwrap();
    assert_eq!(physical_page.assignments[&6].target, source.target);
    let virtual_page: light_playback::PlaybackPage = serde_json::from_value(
        pages
            .iter()
            .find(|object| object.id == "1")
            .unwrap()
            .body
            .clone(),
    )
    .unwrap();
    assert_eq!(virtual_page.assignments[&1000].target, source.target);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn record_group_supports_overwrite_merge_subtract_and_empty_source_delete() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let show_path = data_dir.join("shows/record-group.show");
    let show_id = initialise_show(&show_path, "Record Group").unwrap();
    let entry = ShowEntry {
        is_base_show: false,
        id: show_id,
        name: "Record Group".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        created_at: None,
        last_loaded_at: None,
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
                ..Default::default()
            }).unwrap(),
            0,
        )
        .unwrap();
    state.active_show.replace_current(Some(entry.clone()));
    state
        .output
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();

    state.programming.select_expression(
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
    assert!(overwritten.derived_from.is_none());
    assert_eq!(
        overwritten.source,
        Some(light_programmer::GroupFixtureSource::Explicit {
            fixture_ids: fixtures[..3].to_vec(),
        })
    );

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
                source: Some(light_programmer::GroupFixtureSource::References {
                    references: vec![light_programmer::GroupReference {
                        group_id: "3".into(),
                        rule: light_programmer::SelectionRule::All,
                    }],
                }),
                ..Default::default()
            })
            .unwrap(),
            0,
        )
        .unwrap();
    state
        .output
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();
    state.programming.select_expression(
        session.id,
        fixtures[..3].to_vec(),
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "4".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );
    execute_programmer_command(&state, &session, "RECORD GROUP 3").unwrap();
    let cycle_safe = read_group();
    assert!(cycle_safe.derived_from.is_none());
    assert_eq!(
        cycle_safe.source,
        Some(light_programmer::GroupFixtureSource::Explicit {
            fixture_ids: fixtures[..3].to_vec(),
        })
    );
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

    state.programming.select(session.id, []);
    assert!(execute_programmer_command(&state, &session, "RECORD - GROUP 3").is_err());
    execute_programmer_command(&state, &session, "DELETE GROUP 4").unwrap();

    state
        .programming
        .select(session.id, [fixtures[2], fixtures[3]]);
    execute_programmer_command(&state, &session, "RECORD + GROUP 3").unwrap();
    let merged = read_group();
    assert_eq!(merged.fixtures, fixtures);
    assert_eq!(
        merged.source,
        Some(light_programmer::GroupFixtureSource::Explicit {
            fixture_ids: fixtures.clone(),
        })
    );

    state
        .programming
        .select(session.id, [fixtures[1], fixtures[3]]);
    execute_programmer_command(&state, &session, "RECORD - GROUP 3").unwrap();
    let subtracted = read_group();
    assert_eq!(subtracted.fixtures, vec![fixtures[0], fixtures[2]]);
    assert_eq!(
        subtracted.source,
        Some(light_programmer::GroupFixtureSource::Explicit {
            fixture_ids: vec![fixtures[0], fixtures[2]],
        })
    );

    state.programming.select(session.id, []);
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

#[test]
fn multi_point_spread_with_more_points_than_selection_is_rejected_without_mutation() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "spread-reject".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let first = light_core::FixtureId::new();
    let second = light_core::FixtureId::new();
    state
        .output
        .replace_snapshot(EngineSnapshot {
            groups: vec![light_programmer::GroupDefinition {
                id: "1".into(),
                name: "Group 1".into(),
                fixtures: vec![first, second],
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    let error =
        execute_programmer_command(&state, &session, "GROUP 1 AT 0 THRU 50 THRU 100").unwrap_err();
    assert!(error.contains("control points"), "{error}");
    // No partial mutation: neither per-fixture values nor a group value landed.
    let programmer = state.programming.get(session.id).unwrap();
    assert!(programmer.values.is_empty());
    assert!(programmer.group_values.is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn documented_simple_timing_preserves_group_owner_fixture_values_and_atomic_refusal() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "test".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    attach_session_command_context(&state, &session);
    let path = data_dir.join("shows/simple-timing.show");
    let show_id = default_show::initialise_legacy_test_show(&path).unwrap();
    let entry = ShowEntry {
        is_base_show: false,
        id: show_id,
        name: "Simple timing".into(),
        path: path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        created_at: None,
        last_loaded_at: None,
        revision_copy: None,
    };
    let mut snapshot = load_engine_snapshot(&entry).unwrap();
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_number == Some(1))
        .unwrap()
        .fixture_id;
    snapshot.groups = vec![light_programmer::GroupDefinition {
        id: "91".into(),
        name: "Timed group".into(),
        fixtures: vec![fixture],
        ..Default::default()
    }]
    .into();
    state.output.replace_snapshot(snapshot).unwrap();
    for command in [
        "G91 AT 25 FADE 0 DELAY 0",
        "G91 AT 25% FADE 0s DELAY 0s",
        "G91 AT 25 TIME 0 TIME TIME 0",
    ] {
        execute_programmer_command(&state, &session, command).unwrap();
        let programmer = state.programming.get(session.id).unwrap();
        let value = &programmer.group_values["91"][&light_core::AttributeKey::intensity()];
        assert_eq!(value.value.normalized(), Some(0.25));
        assert_eq!(value.fade_millis, Some(0));
        assert_eq!(value.delay_millis, Some(0));
        assert!(
            !programmer
                .values
                .iter()
                .any(|value| value.fixture_id == fixture)
        );
        assert!(
            programmer
                .selection_expression
                .unwrap()
                .live_group_owners()
                .contains(&"91".to_owned())
        );
    }
    execute_programmer_command(&state, &session, "F1 AT 75% FADE 0 DELAY 0").unwrap();
    let programmer = state.programming.get(session.id).unwrap();
    let value = programmer
        .values
        .iter()
        .find(|value| {
            value.fixture_id == fixture && value.attribute == light_core::AttributeKey::intensity()
        })
        .unwrap();
    assert_eq!(value.value.normalized(), Some(0.75));
    assert_eq!(value.fade_millis, Some(0));
    assert_eq!(value.delay_millis, Some(0));
    execute_programmer_command(&state, &session, "AT 49.5% TIME 0 DELAY 0").unwrap();
    let programmer = state.programming.get(session.id).unwrap();
    assert_eq!(
        programmer
            .values
            .iter()
            .find(|value| value.fixture_id == fixture)
            .unwrap()
            .value
            .normalized(),
        Some(0.495)
    );
    execute_programmer_command(&state, &session, "G91").unwrap();
    execute_programmer_command(&state, &session, "AT 12.5% FADE 0 DELAY 0").unwrap();
    let programmer = state.programming.get(session.id).unwrap();
    assert_eq!(
        programmer.group_values["91"][&light_core::AttributeKey::intensity()]
            .value
            .normalized(),
        Some(0.125)
    );
    assert_eq!(
        programmer
            .values
            .iter()
            .find(|value| value.fixture_id == fixture)
            .unwrap()
            .value
            .normalized(),
        Some(0.495)
    );
    for command in [
        "G91 AT 50 FADE nope",
        "F1 AT 50 TIME 86401",
        "G91 AT 50 DELAY -1",
        "G91 AT 50 FADE 0ms",
        "G91 AT 50 TIME 0 THRU 5",
        "F1 AT 50 DELAY 0 THRU 5",
    ] {
        let before = serde_json::to_value(state.programming.get(session.id).unwrap()).unwrap();
        assert!(
            execute_programmer_command(&state, &session, command).is_err(),
            "{command}"
        );
        assert_eq!(
            serde_json::to_value(state.programming.get(session.id).unwrap()).unwrap(),
            before,
            "{command}"
        );
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn protected_dynamic_group_delete_returns_operator_message_without_changing_show() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(), token: "test".into(), connected: true, desk: test_control_desk(),
    };
    state.programming.start(session.id);
    attach_session_command_context(&state, &session);
    let show_path = data_dir.join("shows/protected-dynamic-group.show");
    let show_id = default_show::initialise_legacy_test_show(&show_path).unwrap();
    let entry = ShowEntry { is_base_show:false, id:show_id, name:"Protected dependency".into(),
        path:show_path.display().to_string(), revision:0, updated_at:String::new(),
        created_at:None, last_loaded_at:None, revision_copy:None };
    let mut dynamic = command_test_dynamic(Uuid::new_v4(), 1);
    dynamic.target_binding = light_dynamics::DynamicTargetBinding::LiveGroup { group_id:"991".into() };
    let store = ShowStore::open(&show_path).unwrap();
    store.put_object("group", "991", &serde_json::json!({"id":"991","name":"Protected","fixtures":[]}), 0).unwrap();
    store.put_object("dynamic", &dynamic.id.to_string(), &serde_json::to_value(&dynamic).unwrap(), 0).unwrap();
    let before = store.portable_document().unwrap();
    state.active_show.replace_current(Some(entry.clone()));
    state.output.replace_snapshot(load_engine_snapshot(&entry).unwrap()).unwrap();
    let runtime_before = state.output.snapshot();
    // The raw legacy dispatcher still reaches the final engine integrity guard. Keep that
    // safety proof separately from the authoritative HTTP/WS/OSC command service below.
    assert_eq!(
        execute_programmer_command(&state, &session, "DELETE GROUP 991").unwrap_err(),
        "snapshot validation failed: Dynamic 1 references a missing Group"
    );
    let expected = "Group 991 cannot be deleted: Dynamic 1 depends on it. Retarget that Dynamic first.";
    for (source, action_source) in [
        ("software", light_application::ActionSource::UserInterface),
        ("http", light_application::ActionSource::Http),
    ] {
        let context = light_application::ActionContext::operator(
            session.desk.id, session.id.0, action_source,
        );
        let result = command_http::run_service_with_source(
            &state, &session, context,
            light_application::ProgrammingCommand::Execute {
                command: Some("DELETE GROUP 991".into()),
                policy: light_application::ExecutionPolicy::AtomicProgrammer,
                origin: light_application::CommandOrigin::CommandLine,
            }, source,
        ).unwrap();
        assert!(matches!(result.outcome,
            light_application::ProgrammingOutcome::Rejected { ref error } if error == expected
        ), "{source}: {:?}", result.outcome);
        assert_eq!(result.command_line.visible_text(), "DELETE GROUP 991");
    }
    state.programming.set_command_line(session.id, "DELETE GROUP 991".into());
    let event_start = state.events.audit_events().len();
    assert_eq!(command_http::route_osc_command_key_outcome(
        &state, &session, "desk", "enter", Some("protected-dynamic-group-delete")
    ), Some(true));
    let events = state.events.audit_events();
    assert!(events[event_start..].iter().any(|event|
        event.kind == "programmer_command_rejected" && event.payload["error"] == expected
            && event.payload["source"] == "osc"
    ), "OSC must publish the same actionable operator rejection");
    assert_eq!(state.programming.get(session.id).unwrap().command_line, "DELETE GROUP 991");
    let after = store.portable_document().unwrap();
    assert_eq!(after.revision(), before.revision());
    for object in before.objects() {
        let retained = after.object(object.key().kind(), object.key().id()).unwrap();
        assert_eq!(retained.revision(), object.revision());
        assert_eq!(retained.body(), object.body());
    }
    assert!(std::sync::Arc::ptr_eq(&runtime_before, &state.output.snapshot()));
    assert!(state.output.snapshot().groups.iter().any(|g|g.id == "991"));
    drop(store); drop(state);
    let _ = std::fs::remove_dir_all(data_dir);
}
