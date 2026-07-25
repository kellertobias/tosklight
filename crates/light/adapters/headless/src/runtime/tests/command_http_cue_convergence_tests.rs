#[tokio::test]
async fn command_keyboard_and_websocket_cue_recording_share_the_typed_action() {
    let scenario = CommandHttpScenario::new().await;
    let _show_id = scenario.create_and_open_show("Cue command convergence").await;
    set_cue_record_value(&scenario);
    let initial_revision = active_show_revision(&scenario);
    let compatibility_baseline = scenario.cue_list_compatibility_payloads().len();

    let command = scenario
        .execute(
            "cue-command-record",
            Some("RECORD SET 31 CUE 2.5 TIME 2 DELAY 1"),
        )
        .await;
    assert_eq!(command.status(), StatusCode::OK);
    assert_eq!(json(command).await["outcome"], "accepted");
    let (_, _, recorded) = stored_cue_list(&scenario, 31);
    let cue = recorded.cues.iter().find(|cue| cue.number == 2.5).unwrap();
    assert_eq!(cue.fade_millis, 2_000);
    assert_eq!(
        cue.trigger,
        light_playback::CueTrigger::Wait {
            delay_millis: 1_000
        }
    );
    assert!(
        scenario
            .state
            .engine
            .playback_runtime()
            .iter()
            .all(|runtime| runtime.playback_number != Some(31)),
        "command grammar uses Hold activation policy"
    );

    for (index, key) in ["REC", "SET", "3", "2", "CUE", "1", "ENT"]
        .into_iter()
        .enumerate()
    {
        let response = scenario
            .press_key(&scenario.token, key, &format!("cue-key-{index}"))
            .await;
        assert_eq!(response.status(), StatusCode::OK, "key {key} failed");
    }
    assert!(stored_cue_list(&scenario, 32).2.cues.iter().any(|cue| cue.number == 1.0));

    let source: SocketAddr = "127.0.0.1:9026".parse().unwrap();
    let osc_alias = scenario.session.desk.osc_alias.clone();
    scenario.state.osc_subscribers.lock().insert(
        "cue-record-keys".into(),
        OscSubscriber {
            desk_alias: osc_alias.clone(),
            target: source,
            command_source: source,
            session_id: scenario.session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
    for action in [
        "record", "set", "digit-3", "digit-4", "cue", "digit-1", "enter",
    ] {
        handle_programmer_osc(
            &scenario.state,
            &format!("/light/{osc_alias}/programmer/{action}"),
            &[OscArgument::Bool(true)],
            Some("127.0.0.1:9026"),
        );
    }
    assert!(stored_cue_list(&scenario, 34).2.cues.iter().any(|cue| cue.number == 1.0));

    let ws_command = || WsCommand {
        protocol_version: 1,
        request_id: "cue-ws-record".into(),
        session_id: scenario.session.id,
        expected_revision: None,
        command: "programmer.execute".into(),
        payload: serde_json::json!({"value":"RECORD SET 33 CUE 1"}),
    };
    let ws = dispatch_ws_command(&scenario.state, &scenario.session, ws_command());
    assert!(ws.ok, "{:?}", ws.error);
    assert!(stored_cue_list(&scenario, 33).2.cues.iter().any(|cue| cue.number == 1.0));
    let sequence = scenario.state.application_events.latest_sequence();
    let history = scenario.history_len();
    let compatibility_before_replay = scenario.cue_list_compatibility_payloads().len();
    let replay = dispatch_ws_command(&scenario.state, &scenario.session, ws_command());
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(scenario.state.application_events.latest_sequence(), sequence);
    assert_eq!(scenario.history_len(), history);
    assert_eq!(
        scenario.cue_list_compatibility_payloads().len(),
        compatibility_before_replay
    );
    assert_eq!(compatibility_before_replay, compatibility_baseline + 4);
    assert_eq!(active_show_revision(&scenario), initial_revision + 4);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn real_osc_record_touch_creates_exact_page_target_and_suppresses_control() {
    let scenario = CommandHttpScenario::new().await;
    let _show_id = scenario.create_and_open_show("OSC Cue record").await;
    set_cue_record_value(&scenario);
    let source: SocketAddr = "127.0.0.1:9027".parse().unwrap();
    scenario.state.osc_subscribers.lock().insert(
        "cue-record-touch".into(),
        OscSubscriber {
            desk_alias: scenario.session.desk.osc_alias.clone(),
            target: source,
            command_source: source,
            session_id: scenario.session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
    for (slot, control, press, release) in [
        (
            7,
            "button/1",
            OscArgument::Bool(true),
            Some(OscArgument::Bool(false)),
        ),
        (
            8,
            "label",
            OscArgument::Bool(true),
            Some(OscArgument::Bool(false)),
        ),
        (9, "fader", OscArgument::Float(0.42), None),
    ] {
        assert_osc_surface_records(&scenario, slot, control, press, release);
    }
    let fader_address = "/light/playback/4/9/fader";
    let after_record = scenario.state.application_events.latest_sequence();
    handle_playback_osc(
        &scenario.state,
        fader_address,
        &[OscArgument::Float(0.84)],
        Some("127.0.0.1:9027"),
    );
    assert_eq!(scenario.state.application_events.latest_sequence(), after_record);
    assert_eq!(runtime_for_page_slot(&scenario, 9).master, 1.0);

    assert_osc_surface_records(
        &scenario,
        10,
        "button/3",
        OscArgument::Bool(true),
        None,
    );
    let flash_address = "/light/playback/4/10/button/3";
    handle_playback_osc(
        &scenario.state,
        flash_address,
        &[OscArgument::Bool(true)],
        Some("127.0.0.1:9027"),
    );
    assert!(runtime_for_page_slot(&scenario, 10).flash);
    handle_playback_osc(
        &scenario.state,
        flash_address,
        &[OscArgument::Bool(false)],
        Some("127.0.0.1:9027"),
    );
    assert!(!runtime_for_page_slot(&scenario, 10).flash);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn assert_osc_surface_records(
    scenario: &CommandHttpScenario,
    slot: u8,
    control: &str,
    press: OscArgument,
    release: Option<OscArgument>,
) {
    scenario
        .state
        .programmers
        .set_command_line(scenario.session.id, "RECORD".into());
    let address = format!("/light/playback/4/{slot}/{control}");
    let baseline = scenario.state.application_events.latest_sequence();
    handle_playback_osc(
        &scenario.state,
        &address,
        &[press],
        Some("127.0.0.1:9027"),
    );
    let after_press = scenario.state.application_events.latest_sequence();
    assert_eq!(after_press, baseline + 3);
    let runtime = runtime_for_page_slot(scenario, slot);
    assert_eq!(runtime.current_cue_number, Some(1.0));
    assert_eq!(runtime.master, 1.0, "the fader action must be suppressed");
    assert_eq!(
        scenario
            .state
            .programmers
            .get(scenario.session.id)
            .unwrap()
            .command_line,
        ""
    );
    assert_osc_record_event_order(&scenario.state, baseline);
    if let Some(release) = release {
        handle_playback_osc(
            &scenario.state,
            &address,
            &[release],
            Some("127.0.0.1:9027"),
        );
        assert_eq!(scenario.state.application_events.latest_sequence(), after_press);
    }
}

fn runtime_for_page_slot(
    scenario: &CommandHttpScenario,
    slot: u8,
) -> light_playback::ActivePlayback {
    let playback = scenario
        .state
        .engine
        .snapshot()
        .playback_pages
        .iter()
        .find(|page| page.number == 4)
        .unwrap()
        .slots[&slot];
    scenario
        .state
        .engine
        .playback_runtime_status()
        .into_iter()
        .find(|runtime| runtime.playback.playback_number == Some(playback))
        .unwrap()
        .playback
}

fn stored_cue_list(
    scenario: &CommandHttpScenario,
    playback: u16,
) -> (
    light_playback::PlaybackDefinition,
    light_show::VersionedObject,
    light_playback::CueList,
) {
    let entry = scenario.state.active_show.read().clone().unwrap();
    cue_list_for_playback(
        &ShowStore::open(&entry.path).unwrap(),
        &scenario.state.engine.snapshot(),
        playback,
    )
    .unwrap()
}

fn assert_osc_record_event_order(state: &AppState, baseline: u64) {
    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(baseline, &light_application::EventFilter::default())
    else {
        panic!("OSC Cue record events must remain replayable")
    };
    assert_eq!(events.len(), 3);
    assert!(matches!(
        events[0].payload,
        light_application::ApplicationEvent::Show(_)
    ));
    assert!(matches!(
        events[1].payload,
        light_application::ApplicationEvent::Playback(_)
    ));
    assert!(matches!(
        events[2].payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::InteractionChanged(_)
        )
    ));
}
