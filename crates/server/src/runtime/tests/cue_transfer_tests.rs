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
fn legacy_choice_selection_resets_the_authoritative_command_once() {
    let scenario = CueTransferScenario::new();
    let dispatch = |request_id: &str, value: &str| {
        dispatch_ws_command(
            &scenario.state,
            &scenario.session,
            WsCommand {
                protocol_version: 1,
                request_id: request_id.into(),
                session_id: scenario.session.id,
                expected_revision: None,
                command: "programmer.execute".into(),
                payload: serde_json::json!({"value":value}),
            },
        )
    };
    let pending = dispatch("pending-copy", "COPY SET 1 CUE 2 AT SET 2 CUE 2");
    assert!(pending.ok);
    let pending = pending.payload.unwrap()["pending_choice"].clone();
    assert_eq!(pending["type"], "cue_move_copy");
    let authoritative = scenario
        .state
        .programmers
        .command_line_state(scenario.session.id)
        .unwrap();
    assert_eq!(
        authoritative.pending_choice.as_ref().unwrap().choice_id,
        serde_json::from_value::<Uuid>(pending["choice_id"].clone()).unwrap()
    );
    let before = scenario.state.application_events.latest_sequence();

    assert!(dispatch("plain-copy", "COPY PLAIN SET 1 CUE 2 AT SET 2 CUE 2").ok);

    let command = scenario
        .state
        .programmers
        .command_line_state(scenario.session.id)
        .unwrap();
    assert_eq!(command.visible_text(), "FIXTURE");
    assert!(command.pristine);
    assert!(command.pending_choice.is_none());
    let persisted = scenario
        .state
        .desk
        .lock()
        .persisted_sessions()
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
        scenario.state.application_events.replay(before, &filter)
    else {
        panic!("accepted choice should publish one retained command event")
    };
    assert_eq!(events.len(), 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[test]
fn cue_copy_preserves_extensions_on_duplicate_id_destination_cues() {
    let scenario = CueTransferScenario::new();
    let store = ShowStore::open(&scenario.show_path).unwrap();
    let (_, destination_object, _) =
        cue_list_for_playback(&store, &scenario.state.engine.snapshot(), 2).unwrap();
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
    let entry = scenario.state.active_show.read().clone().unwrap();
    scenario
        .state
        .engine
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();

    let response = dispatch_cue_transfer(
        &scenario,
        "copy-duplicate-destination",
        "COPY PLAIN SET 1 CUE 2 AT SET 2 CUE 2",
    );
    assert!(response.ok, "Cue copy failed: {:?}", response.error);

    let (_, destination_object, destination) = cue_list_for_playback(
        &ShowStore::open(&scenario.show_path).unwrap(),
        &scenario.state.engine.snapshot(),
        2,
    )
    .unwrap();
    assert_eq!(
        destination
            .cues
            .iter()
            .map(|cue| cue.number)
            .collect::<Vec<_>>(),
        vec![1.0, 2.0, 3.0]
    );
    let cues = destination_object.body["cues"].as_array().unwrap();
    assert_eq!(cues[0]["future_cue_metadata"]["position"], "first");
    assert_eq!(cues[2]["future_cue_metadata"]["position"], "second");
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn verify_pending_cue_transfer_choice(
    scenario: &CueTransferScenario,
    before: &CueTransferBaseline,
) {
    let response = dispatch_ws_command(
        &scenario.state,
        &scenario.session,
        WsCommand {
            protocol_version: 1,
            request_id: "pending-copy".into(),
            session_id: scenario.session.id,
            expected_revision: None,
            command: "programmer.execute".into(),
            payload: serde_json::json!({"value":"COPY SET 1 CUE 2 AT SET 2 CUE 2"}),
        },
    );
    assert!(response.ok, "pending transfer failed: {:?}", response.error);
    let pending = &response.payload.unwrap()["pending_choice"];
    assert_eq!(pending["type"], "cue_move_copy");
    assert_eq!(pending["options"][0]["label"], "Plain Copy");
    assert_eq!(pending["options"][1]["label"], "Status Copy");
    assert_eq!(pending["cancel_label"], "Cancel");
    let authoritative = scenario
        .state
        .programmers
        .command_line_state(scenario.session.id)
        .unwrap();
    assert_eq!(
        authoritative.pending_choice.as_ref().unwrap().command,
        pending["command"].as_str().unwrap()
    );
    assert!(execute_programmer_command(
        &scenario.state,
        &scenario.session,
        "COPY SET 1 CUE 2 AT SET 2 CUE 2"
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
        .programmers
        .command_line_state(scenario.session.id)
        .is_some_and(|command| command.pending_choice.is_some());
    let command = format!(
        "{} {} SET 1 CUE 2 AT SET 2 CUE 2",
        case.operation, case.mode
    );
    let response = dispatch_cue_transfer(scenario, "explicit-transfer", &command);
    assert!(response.ok, "Cue transfer failed: {:?}", response.error);
    let store = ShowStore::open(&scenario.show_path).unwrap();
    let (_, source_object, source) =
        cue_list_for_playback(&store, &scenario.state.engine.snapshot(), 1).unwrap();
    let (_, destination_object, destination) =
        cue_list_for_playback(&store, &scenario.state.engine.snapshot(), 2).unwrap();
    assert_eq!(
        store.portable_revision().unwrap().value(),
        before.show_revision + 1
    );
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        before.event_sequence + u64::from(had_pending_choice) + 1
    );
    assert_eq!(
        cue_transfer_backup_count(&scenario.data_dir),
        before.backup_count + 1
    );
    let runtime = scenario.state.engine.snapshot();
    assert_eq!(runtime.revision, before.show_revision + 1);
    assert!(!Arc::ptr_eq(&runtime, &before.runtime));
    assert_eq!(source_object.body["future_cuelist_metadata"]["list"], 0);
    assert_eq!(
        destination_object.body["future_cuelist_metadata"]["list"],
        1
    );
    if case.moves {
        assert_eq!(source.cues.len(), 2);
        assert!(source.cues.iter().all(|cue| cue.number != 2.0));
        assert!(source_object.revision > before.source_revision);
        let remaining = source.state_at_number(3.0);
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
        destination.cues.iter().map(|cue| cue.number).collect::<Vec<_>>(),
        vec![1.0, 2.0]
    );
    let transferred = destination.cues.iter().find(|cue| cue.number == 2.0).unwrap();
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
    dispatch_ws_command(
        &scenario.state,
        &scenario.session,
        WsCommand {
            protocol_version: 1,
            request_id: request_id.into(),
            session_id: scenario.session.id,
            expected_revision: None,
            command: "programmer.execute".into(),
            payload: serde_json::json!({"value":value}),
        },
    )
}

fn verify_transferred_state(
    scenario: &CueTransferScenario,
    destination: &light_playback::CueList,
    status: bool,
) {
    let replayed = destination.state_at_number(2.0);
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
        }],
        ..Default::default()
    };
    let pool = ["SET", "25", "CUE", "2", ".", "5"].map(String::from);
    let (address, used) = parse_playback_address(&pool, true, &snapshot).unwrap();
    assert_eq!((address.playback, address.cue, used), (25, Some(2.5), 6));
    assert_eq!(
        address.application_address(),
        light_application::PlaybackAddress::Pool(25)
    );
    let pool_only = ["SET", "25"].map(String::from);
    let (address, used) = parse_playback_address(&pool_only, true, &snapshot).unwrap();
    assert_eq!((address.playback, address.cue, used), (25, None, 2));
    let page = ["SET", "4", ".", "7", "CUE", "12"].map(String::from);
    let (address, used) = parse_playback_address(&page, true, &snapshot).unwrap();
    assert_eq!((address.playback, address.cue, used), (25, Some(12.0), 6));
    assert_eq!(
        address.application_address(),
        light_application::PlaybackAddress::ExplicitPage { page: 4, slot: 7 }
    );
    let page_only = ["SET", "4", ".", "7"].map(String::from);
    let (address, used) = parse_playback_address(&page_only, true, &snapshot).unwrap();
    assert_eq!((address.playback, address.cue, used), (25, None, 4));
    let old_entangled = ["SET", "4", "SET", "7", ".", "12"].map(String::from);
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
            },
            light_playback::PlaybackPage {
                number: 4,
                name: "Page 4".into(),
                slots: HashMap::from([(7, 25)]),
            },
        ],
        ..Default::default()
    };
    let current = ["SET", "7", "CUE", "2", ".", "5"].map(String::from);
    let explicit = ["SET", "1", ".", "7", "CUE", "2", ".", "5"].map(String::from);

    let page_one = parse_update_playback_address(&current, 1, &snapshot).unwrap();
    let page_four = parse_update_playback_address(&current, 4, &snapshot).unwrap();
    let pinned = parse_update_playback_address(&explicit, 4, &snapshot).unwrap();

    assert_eq!((page_one.playback, page_one.cue), (11, Some(2.5)));
    assert_eq!((page_four.playback, page_four.cue), (25, Some(2.5)));
    assert_eq!((pinned.playback, pinned.cue), (11, Some(2.5)));
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
