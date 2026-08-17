#[tokio::test]
async fn command_keyboard_and_websocket_cue_recording_share_the_typed_action() {
    let scenario = CommandHttpScenario::new().await;
    let _show_id = scenario
        .create_and_open_show("Cue command convergence")
        .await;
    set_cue_record_value(&scenario);
    let initial_revision = active_show_revision(&scenario);
    let compatibility_baseline = scenario.cue_list_compatibility_payloads().len();

    let command = scenario
        .execute(
            "cue-command-record",
            Some("RECORD CUELIST 31 CUE 2.5 TIME 2 DELAY 1"),
        )
        .await;
    assert_eq!(command.status(), StatusCode::OK);
    assert_eq!(json(command).await["outcome"], "accepted");
    let (_, _, recorded) = stored_cue_list(&scenario, 31);
    let recorded_cue = recorded
        .cues
        .iter()
        .find(|item| item.number == cue("2.5"))
        .unwrap();
    assert_eq!(recorded_cue.fade_millis, 2_000);
    assert_eq!(
        recorded_cue.trigger,
        light_playback::CueTrigger::Wait {
            delay_millis: 1_000
        }
    );
    assert!(
        scenario
            .state
            .output.playback_runtime()
            .iter()
            .all(|runtime| runtime.playback_number != Some(31)),
        "command grammar uses Hold activation policy"
    );

    for (index, key) in ["REC", "CUE", "CUE", "3", "2", "CUE", "1", "ENT"]
        .into_iter()
        .enumerate()
    {
        let response = scenario
            .press_key(&scenario.token, key, &format!("cue-key-{index}"))
            .await;
        assert_eq!(response.status(), StatusCode::OK, "key {key} failed");
    }
    assert!(
        stored_cue_list(&scenario, 32)
            .2
            .cues
            .iter()
            .any(|item| item.number == cue("1"))
    );

    let source: SocketAddr = "127.0.0.1:9026".parse().unwrap();
    let osc_alias = scenario.session.desk.osc_alias.clone();
    scenario.state.integrations.register_osc_subscriber(
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
        "record", "cue", "cue", "digit-3", "digit-4", "cue", "digit-1", "enter",
    ] {
        let address = format!("/light/{osc_alias}/programmer/{action}");
        handle_programmer_osc(
            &scenario.state,
            &address,
            &[OscArgument::Bool(true)],
            Some("127.0.0.1:9026"),
        );
        // Record resolves on release: a bare press only starts the gesture.
        if action == "record" {
            handle_programmer_osc(
                &scenario.state,
                &address,
                &[OscArgument::Bool(false)],
                Some("127.0.0.1:9026"),
            );
        }
    }
    assert!(
        stored_cue_list(&scenario, 34)
            .2
            .cues
            .iter()
            .any(|item| item.number == cue("1"))
    );

    let ws_command = || {
        live_action_frame(
            &scenario.session,
            "cue-ws-record",
            light_wire::v2::live_action::LiveAction::CommandLineExecute(
                light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
                    value: "RECORD CUELIST 33 CUE 1".into(),
                },
            ),
        )
    };
    let ws = dispatch_live_action(&scenario.state, &scenario.session, ws_command());
    assert!(ws.ok, "{:?}", ws.error);
    assert!(
        stored_cue_list(&scenario, 33)
            .2
            .cues
            .iter()
            .any(|item| item.number == cue("1"))
    );
    let sequence = scenario.state.events.latest_sequence();
    let history = scenario.history_len();
    let compatibility_before_replay = scenario.cue_list_compatibility_payloads().len();
    let replay = dispatch_live_action(&scenario.state, &scenario.session, ws_command());
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(
        scenario.state.events.latest_sequence(),
        sequence
    );
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
    scenario.state.installation.update_configuration(|configuration| {
        configuration.start_after_first_recording = true;
    });
    let _show_id = scenario.create_and_open_show("OSC Cue record").await;
    set_cue_record_value(&scenario);
    let source: SocketAddr = "127.0.0.1:9027".parse().unwrap();
    scenario.state.integrations.register_osc_subscriber(
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
    assert_osc_surface_records(
        &scenario,
        7,
        "button/1",
        OscArgument::Bool(true),
        Some(OscArgument::Bool(false)),
    );
    assert_osc_surface_records(
        &scenario,
        8,
        "label",
        OscArgument::Bool(true),
        Some(OscArgument::Bool(false)),
    );
    assert_osc_surface_records(
        &scenario,
        9,
        "button/1",
        OscArgument::Bool(true),
        Some(OscArgument::Bool(false)),
    );
    scenario
        .state
        .programming
        .set_command_line(scenario.session.id, "RECORD".into());
    let fader_address = "/light/playback/4/9/fader";
    handle_playback_osc(
        &scenario.state,
        fader_address,
        &[OscArgument::Float(0.42)],
        Some("127.0.0.1:9027"),
    );
    assert_eq!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .command_line,
        "RECORD"
    );

    assert_osc_surface_records(
        &scenario,
        10,
        "button/1",
        OscArgument::Bool(true),
        Some(OscArgument::Bool(false)),
    );
    scenario
        .state
        .programming
        .set_command_line(scenario.session.id, "RECORD".into());
    let flash_address = "/light/playback/4/10/button/3";
    handle_playback_osc(
        &scenario.state,
        flash_address,
        &[OscArgument::Bool(true)],
        Some("127.0.0.1:9027"),
    );
    assert!(runtime_for_page_slot(&scenario, 10).flash);
    assert_eq!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .command_line,
        "RECORD"
    );
    handle_playback_osc(
        &scenario.state,
        flash_address,
        &[OscArgument::Bool(false)],
        Some("127.0.0.1:9027"),
    );
    assert!(!runtime_for_page_slot(&scenario, 10).flash);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn real_osc_set_touch_selects_current_or_explicit_page_target_and_suppresses_control() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("OSC SET Playback target").await;
    set_cue_record_value(&scenario);
    assert_eq!(
        scenario
            .execute("set-target-record", Some("RECORD CUELIST 41 CUE 1"))
            .await
            .status(),
        StatusCode::OK
    );
    let mapped = dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            "set-target-map",
            light_wire::v2::live_action::LiveAction::CommandLineExecute(
                light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
                    value: "SET 41 AT 4 . 7".into(),
                },
            ),
        ),
    );
    assert!(mapped.ok, "{:?}", mapped.error);
    scenario
        .state
        .installation
        .set_desk_page(
            scenario.session.desk.id,
            light_core::ShowId(Uuid::parse_str(&show_id).unwrap()),
            4,
        )
        .unwrap();
    let source: SocketAddr = "127.0.0.1:9028".parse().unwrap();
    scenario.state.integrations.register_osc_subscriber(
        "set-playback-touch".into(),
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

    scenario
        .state
        .programming
        .set_command_line(scenario.session.id, "SET".into());
    let current_address = format!(
        "/light/{}/page-playback/7/button/1",
        scenario.session.desk.osc_alias
    );
    assert!(handle_playback_osc(
        &scenario.state,
        &current_address,
        &[OscArgument::Bool(true)],
        Some("127.0.0.1:9028"),
    ));
    let current = scenario
        .state
        .events
        .audit_events()
        .into_iter()
        .rev()
        .find(|event| event.kind == "playback_target_selected")
        .unwrap_or_else(|| {
            panic!(
                "missing SET target event; audit kinds: {:?}",
                scenario
                    .state
                    .events
                    .audit_events()
                    .iter()
                    .map(|event| (&event.kind, &event.payload))
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(
        current.payload["target"],
        serde_json::json!({"addressing":"current_page","slot":7})
    );
    assert_eq!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .command_line,
        ""
    );
    assert!(!has_runtime_for_playback(&scenario, 41));
    let after_current_press = scenario.state.events.latest_sequence();
    handle_playback_osc(
        &scenario.state,
        &current_address,
        &[OscArgument::Bool(false)],
        Some("127.0.0.1:9028"),
    );
    assert_eq!(scenario.state.events.latest_sequence(), after_current_press);

    scenario
        .state
        .programming
        .set_command_line(scenario.session.id, "SET".into());
    let explicit_address = "/light/playback/4/7/fader";
    handle_playback_osc(
        &scenario.state,
        explicit_address,
        &[OscArgument::Float(0.25)],
        Some("127.0.0.1:9028"),
    );
    let explicit = scenario
        .state
        .events
        .audit_events()
        .into_iter()
        .rev()
        .find(|event| event.kind == "playback_target_selected")
        .unwrap();
    assert_eq!(
        explicit.payload["target"],
        serde_json::json!({"addressing":"explicit_page","page":4,"slot":7})
    );
    assert!(!has_runtime_for_playback(&scenario, 41));
    let after_explicit_sample = scenario.state.events.latest_sequence();
    handle_playback_osc(
        &scenario.state,
        explicit_address,
        &[OscArgument::Float(0.75)],
        Some("127.0.0.1:9028"),
    );
    assert_eq!(scenario.state.events.latest_sequence(), after_explicit_sample);
    assert!(!has_runtime_for_playback(&scenario, 41));

    assert_eq!(
        scenario
            .execute("set-group-source", Some("RECORD GROUP 4"))
            .await
            .status(),
        StatusCode::OK
    );
    scenario
        .state
        .programming
        .set_command_line(scenario.session.id, "SET GROUP 4".into());
    handle_playback_osc(
        &scenario.state,
        "/light/playback/4/7/label",
        &[OscArgument::Bool(true)],
        Some("127.0.0.1:9028"),
    );
    let snapshot = scenario.state.output.snapshot();
    let group_master_number = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == 4)
        .unwrap()
        .slots[&7];
    assert!(snapshot.playbacks.iter().any(|playback| {
        playback.number == group_master_number
            && matches!(
                &playback.target,
                light_playback::PlaybackTarget::Group { group_id, .. } if group_id.as_str() == "4"
            )
    }));

    assert_eq!(
        scenario
            .execute("set-cuelist-source", Some("RECORD CUELIST 42 CUE 1"))
            .await
            .status(),
        StatusCode::OK
    );
    scenario
        .state
        .programming
        .set_command_line(scenario.session.id, "SET 42".into());
    handle_playback_osc(
        &scenario.state,
        "/light/playback/4/8/label",
        &[OscArgument::Bool(true)],
        Some("127.0.0.1:9028"),
    );
    assert_eq!(
        scenario
            .state
            .output
            .snapshot()
            .playback_pages
            .iter()
            .find(|page| page.number == 4)
            .unwrap()
            .slots[&8],
        42
    );

    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn real_osc_off_touch_uses_internal_off_for_current_and_explicit_page_targets() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("OSC OFF Playback target").await;
    set_cue_record_value(&scenario);
    for (playback, page) in [(41, 4), (42, 5)] {
        assert_eq!(
            scenario
                .execute(
                    &format!("off-target-record-{playback}"),
                    Some(&format!("RECORD CUELIST {playback} CUE 1")),
                )
                .await
                .status(),
            StatusCode::OK
        );
        let mapped = dispatch_live_action(
            &scenario.state,
            &scenario.session,
            live_action_frame(
                &scenario.session,
                &format!("off-target-map-{playback}"),
                light_wire::v2::live_action::LiveAction::CommandLineExecute(
                    light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
                        value: format!("ASSIGN CUELIST {playback} AT PBK {page} . 7"),
                    },
                ),
            ),
        );
        assert!(mapped.ok, "{:?}", mapped.error);
    }
    scenario
        .state
        .installation
        .set_desk_page(
            scenario.session.desk.id,
            light_core::ShowId(Uuid::parse_str(&show_id).unwrap()),
            4,
        )
        .unwrap();
    let source: SocketAddr = "127.0.0.1:9030".parse().unwrap();
    scenario.state.integrations.register_osc_subscriber(
        "off-playback-touch".into(),
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
    for playback in [41, 42] {
        assert_eq!(
            scenario
                .playback_action_for(
                    &scenario.token,
                    scenario.session.desk.id,
                    serde_json::json!({
                        "request_id":format!("off-target-go-{playback}"),
                        "address":{"kind":"playback","playback_number":playback},
                        "action":{"type":"go","pressed":true},
                        "surface":"physical",
                    }),
                )
                .await
                .status(),
            StatusCode::OK
        );
    }
    assert!(playback_is_enabled(&scenario, 41));
    assert!(playback_is_enabled(&scenario, 42));

    scenario
        .state
        .programming
        .set_command_line(scenario.session.id, "OFF".into());
    let current_address = format!(
        "/light/{}/page-playback/7/button/1",
        scenario.session.desk.osc_alias
    );
    assert!(handle_playback_osc(
        &scenario.state,
        &current_address,
        &[OscArgument::Bool(true)],
        Some("127.0.0.1:9030"),
    ));
    assert!(!playback_is_enabled(&scenario, 41));
    assert!(playback_is_enabled(&scenario, 42));
    assert_eq!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .command_line,
        ""
    );
    let current = scenario
        .state
        .events
        .audit_events()
        .into_iter()
        .rev()
        .find(|event| event.kind == "playback_off_targeted")
        .unwrap();
    assert_eq!(
        current.payload["target"],
        serde_json::json!({"addressing":"current_page","slot":7})
    );

    assert_eq!(
        scenario
            .playback_action_for(
                &scenario.token,
                scenario.session.desk.id,
                serde_json::json!({
                    "request_id":"off-target-go-again",
                    "address":{"kind":"playback","playback_number":41},
                    "action":{"type":"go","pressed":true},
                    "surface":"physical",
                }),
            )
            .await
            .status(),
        StatusCode::OK
    );
    assert!(playback_is_enabled(&scenario, 41));
    scenario
        .state
        .installation
        .set_desk_page(
            scenario.session.desk.id,
            light_core::ShowId(Uuid::parse_str(&show_id).unwrap()),
            5,
        )
        .unwrap();
    scenario
        .state
        .programming
        .set_command_line(scenario.session.id, "OFF".into());
    assert!(handle_playback_osc(
        &scenario.state,
        "/light/playback/4/7/fader",
        &[OscArgument::Float(0.25)],
        Some("127.0.0.1:9030"),
    ));
    assert!(!playback_is_enabled(&scenario, 41));
    assert!(playback_is_enabled(&scenario, 42));
    let explicit = scenario
        .state
        .events
        .audit_events()
        .into_iter()
        .rev()
        .find(|event| event.kind == "playback_off_targeted")
        .unwrap();
    assert_eq!(
        explicit.payload["target"],
        serde_json::json!({"addressing":"explicit_page","page":4,"slot":7})
    );
    let after_off = scenario.state.events.latest_sequence();
    handle_playback_osc(
        &scenario.state,
        "/light/playback/4/7/fader",
        &[OscArgument::Float(0.75)],
        Some("127.0.0.1:9030"),
    );
    assert_eq!(scenario.state.events.latest_sequence(), after_off);

    scenario
        .state
        .programming
        .set_command_line(scenario.session.id, "OFF".into());
    assert!(handle_playback_osc(
        &scenario.state,
        &format!(
            "/light/{}/page-playback/7/label",
            scenario.session.desk.osc_alias
        ),
        &[OscArgument::Bool(true)],
        Some("127.0.0.1:9030"),
    ));
    assert!(!playback_is_enabled(&scenario, 42));

    scenario
        .state
        .programming
        .set_command_line(scenario.session.id, "OFF".into());
    assert!(handle_playback_osc(
        &scenario.state,
        &format!(
            "/light/{}/page-playback/7/label",
            scenario.session.desk.osc_alias
        ),
        &[OscArgument::Bool(true)],
        Some("127.0.0.1:9030"),
    ));
    assert!(!playback_is_enabled(&scenario, 42));
    assert_eq!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .command_line,
        ""
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn osc_copy_move_and_delete_do_not_guess_a_whole_playback_mutation() {
    let scenario = CommandHttpScenario::new().await;
    let _show_id = scenario
        .create_and_open_show("OSC unsupported Playback targets")
        .await;
    set_cue_record_value(&scenario);
    assert_eq!(
        scenario
            .execute("unsupported-target-record", Some("RECORD CUELIST 42 CUE 1"))
            .await
            .status(),
        StatusCode::OK
    );
    let mapped = dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            "unsupported-target-map",
            light_wire::v2::live_action::LiveAction::CommandLineExecute(
                light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
                    value: "SET 42 AT 4 . 8".into(),
                },
            ),
        ),
    );
    assert!(mapped.ok, "{:?}", mapped.error);
    let source: SocketAddr = "127.0.0.1:9029".parse().unwrap();
    scenario.state.integrations.register_osc_subscriber(
        "unsupported-playback-touch".into(),
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
    for (index, operation) in ["COPY", "MOVE", "DELETE"].into_iter().enumerate() {
        scenario
            .state
            .programming
            .set_command_line(scenario.session.id, operation.into());
        let value = 0.2 + index as f32 * 0.2;
        handle_playback_osc(
            &scenario.state,
            "/light/playback/4/8/fader",
            &[OscArgument::Float(value)],
            Some("127.0.0.1:9029"),
        );
        assert_eq!(runtime_for_page_slot(&scenario, 8).master, value);
        assert_eq!(
            scenario
                .state
                .programming
                .get(scenario.session.id)
                .unwrap()
                .command_line,
            operation
        );
    }
    assert!(!scenario
        .state
        .events
        .audit_events()
        .iter()
        .any(|event| event.kind == "playback_target_selected"));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn assert_osc_surface_records(
    scenario: &CommandHttpScenario,
    slot: u8,
    control: &str,
    press: OscArgument,
    release: Option<OscArgument>,
) {
    set_cue_record_value(scenario);
    scenario
        .state
        .programming
        .set_command_line(scenario.session.id, "RECORD".into());
    let address = format!("/light/playback/4/{slot}/{control}");
    let baseline = scenario.state.events.latest_sequence();
    handle_playback_osc(&scenario.state, &address, &[press], Some("127.0.0.1:9027"));
    let action_filter = light_application::EventFilter::default()
        .with_capability(light_application::EventCapability::Desk)
        .with_capability(light_application::EventCapability::Playback)
        .with_capability(light_application::EventCapability::Show);
    let light_application::EventReplay::Events(action_events) = scenario
        .state
        .events
        .replay(baseline, &action_filter)
    else {
        panic!("the focused OSC recording action events must remain replayable")
    };
    assert_eq!(action_events.len(), 3);
    let runtime = runtime_for_page_slot(scenario, slot);
    assert_eq!(runtime.current_cue_number, Some(cue("1")));
    assert_eq!(runtime.master, 1.0, "the fader action must be suppressed");
    assert_eq!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .command_line,
        ""
    );
    assert_osc_record_event_order(&scenario.state, baseline);
    let after_press = scenario.state.events.latest_sequence();
    if let Some(release) = release {
        handle_playback_osc(
            &scenario.state,
            &address,
            &[release],
            Some("127.0.0.1:9027"),
        );
        assert_eq!(
            scenario.state.events.latest_sequence(),
            after_press
        );
    }
}

fn runtime_for_page_slot(
    scenario: &CommandHttpScenario,
    slot: u8,
) -> light_playback::ActivePlayback {
    let playback = scenario
        .state
        .output.snapshot()
        .playback_pages
        .iter()
        .find(|page| page.number == 4)
        .unwrap()
        .slots[&slot];
    scenario
        .state
        .output.playback_runtime_status()
        .into_iter()
        .find(|runtime| runtime.playback.playback_number == Some(playback))
        .unwrap()
        .playback
}

fn has_runtime_for_playback(scenario: &CommandHttpScenario, playback: u16) -> bool {
    scenario
        .state
        .output
        .playback_runtime_status()
        .iter()
        .any(|runtime| runtime.playback.playback_number == Some(playback))
}

fn playback_is_enabled(scenario: &CommandHttpScenario, playback: u16) -> bool {
    scenario
        .state
        .output
        .playback_runtime_status()
        .iter()
        .find(|runtime| runtime.playback.playback_number == Some(playback))
        .is_some_and(|runtime| runtime.playback.enabled)
}

fn stored_cue_list(
    scenario: &CommandHttpScenario,
    playback: u16,
) -> (
    light_playback::PlaybackDefinition,
    light_show::VersionedObject,
    light_playback::CueList,
) {
    let entry = scenario.state.active_show.current().clone().unwrap();
    cue_list_for_playback(
        &ActiveShowRepository::open(&entry.path).unwrap(),
        &scenario.state.output.snapshot(),
        playback,
    )
    .unwrap()
}

fn assert_osc_record_event_order(state: &AppState, baseline: u64) {
    let filter = light_application::EventFilter::default()
        .with_capability(light_application::EventCapability::Show)
        .with_capability(light_application::EventCapability::Playback)
        .with_capability(light_application::EventCapability::Desk);
    let light_application::EventReplay::Events(events) = state
        .events
        .replay(baseline, &filter)
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
