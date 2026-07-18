#[test]
fn cue_move_copy_requires_a_choice_and_preserves_plain_status_and_move_copy_axes() {
    for case in [
        CueTransferCase { operation: "COPY", mode: "PLAIN", moves: false, status: false },
        CueTransferCase { operation: "MOVE", mode: "PLAIN", moves: true, status: false },
        CueTransferCase { operation: "COPY", mode: "STATUS", moves: false, status: true },
        CueTransferCase { operation: "MOVE", mode: "STATUS", moves: true, status: true },
    ] {
        let scenario = CueTransferScenario::new();
        let before = scenario.baseline();
        if case.operation == "COPY" && case.mode == "PLAIN" {
            verify_pending_cue_transfer_choice(&scenario, &before);
        }
        execute_and_verify_cue_transfer(&scenario, &before, case);
        let _ = std::fs::remove_dir_all(scenario.data_dir);
    }
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
    assert!(response.ok);
    let pending = &response.payload.unwrap()["pending_choice"];
    assert_eq!(pending["type"], "cue_move_copy");
    assert_eq!(pending["options"][0]["label"], "Plain Copy");
    assert_eq!(pending["options"][1]["label"], "Status Copy");
    assert_eq!(pending["cancel_label"], "Cancel");
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
    execute_programmer_command(
        &scenario.state,
        &scenario.session,
        &format!(
            "{} {} SET 1 CUE 2 AT SET 2 CUE 2",
            case.operation, case.mode
        ),
    )
    .unwrap();
    let store = ShowStore::open(&scenario.show_path).unwrap();
    let (_, source_object, source) =
        cue_list_for_playback(&store, &scenario.state.engine.snapshot(), 1).unwrap();
    let (_, destination_object, destination) =
        cue_list_for_playback(&store, &scenario.state.engine.snapshot(), 2).unwrap();
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
    verify_transferred_state(scenario, &destination, case.status);
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
    let pool_only = ["SET", "25"].map(String::from);
    let (address, used) = parse_playback_address(&pool_only, true, &snapshot).unwrap();
    assert_eq!((address.playback, address.cue, used), (25, None, 2));
    let page = ["SET", "4", ".", "7", "CUE", "12"].map(String::from);
    let (address, used) = parse_playback_address(&page, true, &snapshot).unwrap();
    assert_eq!((address.playback, address.cue, used), (25, Some(12.0), 6));
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
}
