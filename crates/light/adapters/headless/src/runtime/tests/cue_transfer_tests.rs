#[test]
fn cue_move_copy_requires_a_choice_and_preserves_plain_status_and_move_copy_axes() {
    for case in [
        CueTransferCase { operation: "COPY", mode: "PLAIN", moves: false, status: false },
        CueTransferCase { operation: "MOVE", mode: "PLAIN", moves: true, status: false },
        CueTransferCase { operation: "COPY", mode: "STATUS", moves: false, status: true },
        CueTransferCase { operation: "MOVE", mode: "STATUS", moves: true, status: true },
    ] {
        let scenario = CueTransferScenario::new();
        let mut before = scenario.baseline();
        if case.operation == "COPY" && case.mode == "PLAIN" {
            verify_pending_cue_transfer_choice(&scenario, &before);
            before = scenario.baseline();
        }
        execute_and_verify_cue_transfer(&scenario, &before, case);
        let _ = std::fs::remove_dir_all(scenario.data_dir);
    }
}

#[test]
fn typed_choice_selection_resets_the_authoritative_command_once() {
    let scenario = CueTransferScenario::new();
    let dispatch = |request_id: &str, value: &str| {
        dispatch_live_action(
            &scenario.state,
            &scenario.session,
            live_action_frame(
                &scenario.session,
                request_id,
                light_wire::v2::live_action::LiveAction::CommandLineExecute(
                    light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
                        value: value.into(),
                    },
                ),
            ),
        )
    };
    let pending = dispatch(
        "pending-copy",
        "COPY CUELIST 1 CUE 2 AT CUELIST 2 CUE 2",
    );
    assert!(pending.ok);
    let pending = pending.payload.unwrap()["pending_choice"].clone();
    assert_eq!(pending["type"], "cue_move_copy");
    let authoritative = scenario
        .state
        .programming
        .command_line_state(scenario.session.id)
        .unwrap();
    assert_eq!(
        authoritative
            .pending_choice
            .as_ref()
            .unwrap()
            .cue_move_copy()
            .unwrap()
            .choice_id,
        serde_json::from_value::<Uuid>(pending["choice_id"].clone()).unwrap()
    );
    let before = scenario.state.events.latest_sequence();

    assert!(
        dispatch(
            "plain-copy",
            "COPY PLAIN CUELIST 1 CUE 2 AT CUELIST 2 CUE 2"
        )
        .ok
    );

    let command = scenario
        .state
        .programming
        .command_line_state(scenario.session.id)
        .unwrap();
    assert_eq!(command.visible_text(), "FIXTURE");
    assert!(command.pristine);
    assert!(command.pending_choice.is_none());
    let persisted = scenario
        .state
        .installation.persisted_sessions()
        .unwrap()
        .into_iter()
        .find(|session| session.id == scenario.session.id)
        .unwrap();
    let persisted: light_programmer::ProgrammerState =
        serde_json::from_str(&persisted.programmer_json).unwrap();
    assert!(persisted.command_line.is_empty());
    let filter = light_application::EventFilter::for_desk(scenario.session.desk.id).with_object(
        light_application::EventObject::programming_command_line(scenario.session.desk.id),
    );
    let light_application::EventReplay::Events(events) =
        scenario.state.events.replay(before, &filter)
    else {
        panic!("accepted choice should publish one retained command event")
    };
    assert_eq!(events.len(), 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[test]
fn cue_copy_preserves_extensions_on_duplicate_id_destination_cues() {
    let scenario = CueTransferScenario::new();
    let store = ActiveShowRepository::open(&scenario.show_path).unwrap();
    let (_, destination_object, _) =
        cue_list_for_playback(&store, &scenario.state.output.snapshot(), 2).unwrap();
    let mut body = destination_object.body;
    let cues = body["cues"].as_array_mut().unwrap();
    cues[0]["future_cue_metadata"] = serde_json::json!({"position": "first"});
    let mut duplicate = cues[0].clone();
    duplicate["number"] = serde_json::json!(3.0);
    duplicate["future_cue_metadata"] = serde_json::json!({"position": "second"});
    cues.push(duplicate);
    store
        .put_object(
            "cue_list",
            body["id"].as_str().unwrap(),
            &body,
            destination_object.revision,
        )
        .unwrap();
    let entry = scenario.state.active_show.current().clone().unwrap();
    scenario
        .state
        .output.replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();

    let response = dispatch_cue_transfer(
        &scenario,
        "copy-duplicate-destination",
        "COPY PLAIN CUELIST 1 CUE 2 AT CUELIST 2 CUE 2",
    );
    assert!(response.ok, "Cue copy failed: {:?}", response.error);

    let (_, destination_object, destination) = cue_list_for_playback(
        &ActiveShowRepository::open(&scenario.show_path).unwrap(),
        &scenario.state.output.snapshot(),
        2,
    )
    .unwrap();
    assert_eq!(
        destination
            .cues
            .iter()
            .map(|cue| cue.number.to_string())
            .collect::<Vec<_>>(),
        vec!["1", "2", "3"]
    );
    let cues = destination_object.body["cues"].as_array().unwrap();
    assert_eq!(cues[0]["future_cue_metadata"]["position"], "first");
    assert_eq!(cues[2]["future_cue_metadata"]["position"], "second");
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[test]
fn selected_cuelist_shorthand_resolves_once_from_the_initiating_desk() {
    let scenario = CueTransferScenario::new();
    let show_id = scenario.state.active_show.current().as_ref().unwrap().id;
    scenario
        .state
        .installation
        .set_selected_playback(scenario.session.desk.id, show_id, Some(1))
        .unwrap();

    let response = dispatch_cue_transfer(
        &scenario,
        "selected-cuelist-copy",
        "COPY PLAIN CUE 2 AT CUE 4",
    );
    assert!(response.ok, "selected Cuelist copy failed: {:?}", response.error);

    let store = ActiveShowRepository::open(&scenario.show_path).unwrap();
    let (_, _, list) = cue_list_for_playback(&store, &scenario.state.output.snapshot(), 1).unwrap();
    assert_eq!(
        list.cues
            .iter()
            .map(|cue| cue.number.to_string())
            .collect::<Vec<_>>(),
        vec!["1", "2", "3", "4"]
    );
    assert_ne!(
        list.cues
            .iter()
            .find(|cue| cue.number.to_string() == "4")
            .unwrap()
            .id,
        scenario.source_cue_id,
        "Copy must create a new Cue identity"
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[test]
fn pool_object_move_copy_requires_typed_compatible_addresses_and_is_atomic() {
    let scenario = CueTransferScenario::new();
    let store = ActiveShowRepository::open(&scenario.show_path).unwrap();
    let mut group = light_programmer::GroupDefinition {
        id: "10".into(),
        name: "Source Group".into(),
        ..Default::default()
    };
    group
        .fixtures
        .extend([scenario.fixtures[0], scenario.fixtures[1]]);
    store
        .put_object("group", "10", &serde_json::to_value(group).unwrap(), 0)
        .unwrap();
    let entry = scenario.state.active_show.current().clone().unwrap();
    scenario
        .state
        .output
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();

    execute_programmer_command(
        &scenario.state,
        &scenario.session,
        "COPY GROUP 10 AT GROUP 11",
    )
    .unwrap();
    execute_programmer_command(
        &scenario.state,
        &scenario.session,
        "MOVE GROUP 11 AT GROUP 12",
    )
    .unwrap();
    let groups = ActiveShowRepository::open(&scenario.show_path)
        .unwrap()
        .objects("group")
        .unwrap();
    assert!(groups.iter().any(|object| object.id == "10"));
    assert!(!groups.iter().any(|object| object.id == "11"));
    assert_eq!(
        groups
            .iter()
            .find(|object| object.id == "12")
            .unwrap()
            .body["id"],
        "12"
    );

    execute_programmer_command(
        &scenario.state,
        &scenario.session,
        "COPY CUELIST 1 AT CUELIST 3",
    )
    .unwrap();
    let copied = ActiveShowRepository::open(&scenario.show_path)
        .unwrap()
        .objects("playback")
        .unwrap();
    let source_target = copied
        .iter()
        .find(|object| object.id == "1")
        .unwrap()
        .body["target"]["cue_list_id"]
        .clone();
    let copied_target = copied
        .iter()
        .find(|object| object.id == "3")
        .unwrap()
        .body["target"]["cue_list_id"]
        .clone();
    assert_ne!(source_target, copied_target);

    let page = light_playback::PlaybackPage {
        number: 1,
        name: "Main".into(),
        slots: std::collections::HashMap::from([(1, 3)]),
        virtual_playbacks: std::collections::HashMap::new(),
    };
    ActiveShowRepository::open(&scenario.show_path)
        .unwrap()
        .put_object(
            "playback_page",
            "1",
            &serde_json::to_value(page).unwrap(),
            0,
        )
        .unwrap();

    execute_programmer_command(
        &scenario.state,
        &scenario.session,
        "MOVE CUELIST 3 AT CUELIST 4",
    )
    .unwrap();
    let moved = ActiveShowRepository::open(&scenario.show_path)
        .unwrap()
        .objects("playback")
        .unwrap();
    assert!(!moved.iter().any(|object| object.id == "3"));
    assert_eq!(
        moved
            .iter()
            .find(|object| object.id == "4")
            .unwrap()
            .body["target"]["cue_list_id"],
        copied_target
    );
    let page = ActiveShowRepository::open(&scenario.show_path)
        .unwrap()
        .objects("playback_page")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "1")
        .unwrap();
    assert_eq!(page.body["slots"]["1"], 4);

    let before = ActiveShowRepository::open(&scenario.show_path)
        .unwrap()
        .portable_revision()
        .unwrap();
    let error = execute_programmer_command(
        &scenario.state,
        &scenario.session,
        "COPY CUELIST 1 AT GROUP 20",
    )
    .unwrap_err();
    assert!(error.contains("incompatible"));
    assert_eq!(
        ActiveShowRepository::open(&scenario.show_path)
            .unwrap()
            .portable_revision()
            .unwrap(),
        before
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn verify_pending_cue_transfer_choice(
    scenario: &CueTransferScenario,
    before: &CueTransferBaseline,
) {
    let response = dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            "pending-copy",
            light_wire::v2::live_action::LiveAction::CommandLineExecute(
                light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
                    value: "COPY CUELIST 1 CUE 2 AT CUELIST 2 CUE 2".into(),
                },
            ),
        ),
    );
    assert!(response.ok, "pending transfer failed: {:?}", response.error);
    let pending = &response.payload.unwrap()["pending_choice"];
    assert_eq!(pending["type"], "cue_move_copy");
    assert_eq!(pending["options"][0]["label"], "Plain Copy");
    assert_eq!(pending["options"][1]["label"], "Status Copy");
    assert_eq!(pending["cancel_label"], "Cancel");
    let authoritative = scenario
        .state
        .programming
        .command_line_state(scenario.session.id)
        .unwrap();
    assert_eq!(
        authoritative
            .pending_choice
            .as_ref()
            .unwrap()
            .cue_move_copy()
            .unwrap()
            .command,
        pending["command"].as_str().unwrap()
    );
    assert!(execute_programmer_command(
        &scenario.state,
        &scenario.session,
        "COPY CUELIST 1 CUE 2 AT CUELIST 2 CUE 2"
    )
    .is_err());
    let unchanged = scenario.baseline();
    assert_eq!(unchanged.source_body, before.source_body);
    assert_eq!(unchanged.destination_body, before.destination_body);
}

fn execute_and_verify_cue_transfer(
    scenario: &CueTransferScenario,
    before: &CueTransferBaseline,
    case: CueTransferCase,
) {
    let had_pending_choice = scenario
        .state
        .programming
        .command_line_state(scenario.session.id)
        .is_some_and(|command| command.pending_choice.is_some());
    let command = format!(
        "{} {} CUELIST 1 CUE 2 AT CUELIST 2 CUE 2",
        case.operation, case.mode
    );
    let response = dispatch_cue_transfer(scenario, "explicit-transfer", &command);
    assert!(response.ok, "Cue transfer failed: {:?}", response.error);
    let store = ActiveShowRepository::open(&scenario.show_path).unwrap();
    let (_, source_object, source) =
        cue_list_for_playback(&store, &scenario.state.output.snapshot(), 1).unwrap();
    let (_, destination_object, destination) =
        cue_list_for_playback(&store, &scenario.state.output.snapshot(), 2).unwrap();
    assert_eq!(
        store.portable_revision().unwrap().value(),
        before.show_revision + 1
    );
    let show_filter = light_application::EventFilter::default()
        .with_capability(light_application::EventCapability::Show);
    let light_application::EventReplay::Events(show_events) = scenario
        .state
        .events
        .replay(before.event_sequence, &show_filter)
    else {
        panic!("the focused Cue transfer Show event must remain replayable")
    };
    assert_eq!(show_events.len(), 1);
    if had_pending_choice {
        let command_filter =
            light_application::EventFilter::for_desk(scenario.session.desk.id).with_object(
                light_application::EventObject::programming_command_line(
                    scenario.session.desk.id,
                ),
            );
        let light_application::EventReplay::Events(command_events) = scenario
            .state
            .events
            .replay(before.event_sequence, &command_filter)
        else {
            panic!("the focused pending-choice command-line event must remain replayable")
        };
        assert_eq!(command_events.len(), 1);
    }
    assert_eq!(
        cue_transfer_backup_count(&scenario.data_dir),
        before.backup_count + 1
    );
    let runtime = scenario.state.output.snapshot();
    assert_eq!(runtime.revision, before.show_revision + 1);
    assert!(!Arc::ptr_eq(&runtime, &before.runtime));
    assert_eq!(source_object.body["future_cuelist_metadata"]["list"], 0);
    assert_eq!(
        destination_object.body["future_cuelist_metadata"]["list"],
        1
    );
    if case.moves {
        assert_eq!(source.cues.len(), 2);
        assert!(source.cues.iter().all(|item| item.number != cue("2")));
        assert!(source_object.revision > before.source_revision);
        let remaining = source.state_at_number(&cue("3"));
        assert_eq!(
            remaining.get(&(
                scenario.fixtures[0],
                light_core::AttributeKey::intensity()
            )),
            Some(&light_core::AttributeValue::Normalized(0.0))
        );
        assert!(!remaining.contains_key(&(
            scenario.fixtures[1],
            light_core::AttributeKey::intensity()
        )));
    } else {
        assert_eq!(source_object.body, before.source_body);
        assert_eq!(source_object.revision, before.source_revision);
    }
    assert!(destination_object.revision > before.destination_revision);
    assert_eq!(
        destination
            .cues
            .iter()
            .map(|cue| cue.number.to_string())
            .collect::<Vec<_>>(),
        vec!["1", "2"]
    );
    let transferred = destination.cues.iter().find(|item| item.number == cue("2")).unwrap();
    assert_eq!(transferred.id == scenario.source_cue_id, case.moves);
    assert_eq!(transferred.changes.len(), if case.status { 2 } else { 1 });
    assert_eq!(
        transferred.group_changes.len(),
        if case.status { 2 } else { 1 }
    );
    assert!(transferred.changes.iter().all(|change| {
        change.fixture_id != scenario.fixtures[2]
    }));
    assert!(transferred.group_changes.iter().all(|change| change.group_id != "3"));
    let transferred_raw = destination_object.body["cues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cue| cue["id"] == transferred.id.to_string())
        .unwrap();
    assert_eq!(
        transferred_raw["future_cue_metadata"]["owner"],
        "newer-desk"
    );
    verify_transferred_state(scenario, &destination, case.status);
}

fn dispatch_cue_transfer(
    scenario: &CueTransferScenario,
    request_id: &str,
    value: &str,
) -> WsResponse {
    dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            request_id,
            light_wire::v2::live_action::LiveAction::CommandLineExecute(
                light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
                    value: value.into(),
                },
            ),
        ),
    )
}

fn verify_transferred_state(
    scenario: &CueTransferScenario,
    destination: &light_playback::CueList,
    status: bool,
) {
    let replayed = destination.state_at_number(&cue("2"));
    let intensity = light_core::AttributeKey::intensity();
    assert_eq!(
        replayed.get(&(scenario.fixtures[0], intensity.clone())),
        Some(&light_core::AttributeValue::Normalized(if status { 1.0 } else { 0.0 }))
    );
    for fixture in &scenario.fixtures[1..] {
        assert_eq!(
            replayed.get(&(*fixture, intensity.clone())),
            Some(&light_core::AttributeValue::Normalized(1.0))
        );
    }
}

#[test]
fn cue_addresses_use_cue_for_pool_and_page_playbacks() {
    let snapshot = EngineSnapshot {
        playback_pages: vec![light_playback::PlaybackPage {
            number: 4,
            name: "Page 4".into(),
            slots: HashMap::from([(7, 25)]),
            virtual_playbacks: HashMap::new(),
        }].into(),
        ..Default::default()
    };
    let pool = ["PBK", "25", "CUE", "2", ".", "5"].map(String::from);
    let (address, used) = parse_playback_address(&pool, true, &snapshot).unwrap();
    assert_eq!((address.playback, address.cue.clone(), used), (25, Some(cue("2.5")), 6));
    assert_eq!(
        address.application_address(),
        light_application::PlaybackAddress::Pool(25)
    );
    let pool_only = ["PBK", "25"].map(String::from);
    let (address, used) = parse_playback_address(&pool_only, true, &snapshot).unwrap();
    assert_eq!((address.playback, address.cue, used), (25, None, 2));
    let page = ["PBK", "4", ".", "7", "CUE", "12"].map(String::from);
    let (address, used) = parse_playback_address(&page, true, &snapshot).unwrap();
    assert_eq!((address.playback, address.cue.clone(), used), (25, Some(cue("12")), 6));
    assert_eq!(
        address.application_address(),
        light_application::PlaybackAddress::ExplicitPage { page: 4, slot: 7 }
    );
    let page_only = ["PBK", "4", ".", "7"].map(String::from);
    let (address, used) = parse_playback_address(&page_only, true, &snapshot).unwrap();
    assert_eq!((address.playback, address.cue, used), (25, None, 4));
    let old_entangled = ["PBK", "4", "PBK", "7", ".", "12"].map(String::from);
    let (_, used) = parse_playback_address(&old_entangled, true, &snapshot).unwrap();
    assert_ne!(used, old_entangled.len());
}

#[test]
fn update_addresses_keep_current_page_and_explicit_page_distinct() {
    let snapshot = EngineSnapshot {
        playback_pages: vec![
            light_playback::PlaybackPage {
                number: 1,
                name: "Page 1".into(),
                slots: HashMap::from([(7, 11)]),
                virtual_playbacks: HashMap::new(),
            },
            light_playback::PlaybackPage {
                number: 4,
                name: "Page 4".into(),
                slots: HashMap::from([(7, 25)]),
                virtual_playbacks: HashMap::new(),
            },
        ].into(),
        ..Default::default()
    };
    let current = ["PBK", "7", "CUE", "2", ".", "5"].map(String::from);
    let explicit = ["PBK", "1", ".", "7", "CUE", "2", ".", "5"].map(String::from);

    let page_one = parse_update_playback_address(&current, 1, &snapshot).unwrap();
    let page_four = parse_update_playback_address(&current, 4, &snapshot).unwrap();
    let pinned = parse_update_playback_address(&explicit, 4, &snapshot).unwrap();

    assert_eq!((page_one.playback, page_one.cue.clone()), (11, Some(cue("2.5"))));
    assert_eq!((page_four.playback, page_four.cue.clone()), (25, Some(cue("2.5"))));
    assert_eq!((pinned.playback, pinned.cue.clone()), (11, Some(cue("2.5"))));
    assert_eq!(
        page_one.application_address(),
        light_application::PlaybackAddress::CurrentPage { slot: 7 }
    );
    assert_eq!(
        page_four.application_address(),
        light_application::PlaybackAddress::CurrentPage { slot: 7 }
    );
    assert_eq!(
        pinned.application_address(),
        light_application::PlaybackAddress::ExplicitPage { page: 1, slot: 7 }
    );
}
