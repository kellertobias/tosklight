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
    assert_eq!(event.payload["addressing"], "current_page");
    assert_eq!(event.payload["slot"], 6);
    assert_eq!(event.payload["desk_id"], session.desk.id.to_string());

    assert_eq!(
        execute_programmer_command(&state, &session, "ASSIGN VPBK 1001").unwrap(),
        0
    );
    let event = state.events.audit_events().last().cloned().unwrap();
    assert_eq!(event.payload["addressing"], "virtual");
    assert_eq!(event.payload["playback"], 1001);
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
        id: show_id,
        name: "Group Master Command".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
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
    let playback_number = *page.slots.get(&2).unwrap();
    let playback = store
        .objects("playback")
        .unwrap()
        .into_iter()
        .find(|object| object.id == playback_number.to_string())
        .unwrap();
    let playback =
        serde_json::from_value::<light_playback::PlaybackDefinition>(playback.body).unwrap();
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
    assert_eq!(page.slots.get(&2), Some(&playback_number));

    assert_eq!(
        execute_programmer_command(&state, &session, "ASSIGN GROUP 4 AT VPBK 1001").unwrap(),
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
            .virtual_playbacks
            .get(&1001)
            .map(|playback| &playback.target),
        Some(light_playback::PlaybackTarget::Group { group_id, .. }) if group_id == "4"
    ));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn assign_cuelist_uses_pool_identity_for_physical_and_virtual_targets() {
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
        id: show_id,
        name: "Assign Cuelist Command".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };
    let store = ShowStore::open(&show_path).unwrap();
    let cue_list = light_playback::CueList {
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
        number: 4,
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
        .put_object("playback", "4", &serde_json::to_value(&source).unwrap(), 0)
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
        execute_programmer_command(&state, &session, "ASSIGN CUELIST 4 AT VPBK 1001").unwrap(),
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
    let physical_number = physical_page.slots[&6];
    let playback: light_playback::PlaybackDefinition = serde_json::from_value(
        ShowStore::open(&show_path)
            .unwrap()
            .objects("playback")
            .unwrap()
            .into_iter()
            .find(|object| object.id == physical_number.to_string())
            .unwrap()
            .body,
    )
    .unwrap();
    assert_eq!(playback.target, source.target);
    let virtual_page: light_playback::PlaybackPage = serde_json::from_value(
        pages
            .iter()
            .find(|object| object.id == "1")
            .unwrap()
            .body
            .clone(),
    )
    .unwrap();
    assert_eq!(virtual_page.virtual_playbacks[&1001].target, source.target);
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
            &{
                let mut group = serde_json::to_value(light_programmer::GroupDefinition {
                    id: "3".into(),
                    name: "Kept name".into(),
                    fixtures: fixtures[..2].to_vec(),
                    ..Default::default()
                })
                .unwrap();
                group["master"] = serde_json::json!(0.4);
                group
            },
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
