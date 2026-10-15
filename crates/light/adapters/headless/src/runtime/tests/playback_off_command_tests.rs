// TL-786: real command ingress, using unified addresses and existing Off authority.
async fn tl786_running_pair() -> CommandHttpScenario {
    tl786_install_pair(CommandHttpScenario::new().await).await
}
async fn tl786_install_pair(scenario: CommandHttpScenario) -> CommandHttpScenario {
    let show = scenario.create_and_open_show("TL786 OFF command").await;
    let mut fixture = super::template_dimmer("TL786 dimmer", 1);
    fixture.fixture_number = Some(1);
    let response = scenario
        .put_active_object(
            &show,
            "patched_fixture",
            &fixture.fixture_id.0.to_string(),
            0,
            serde_json::to_value(fixture).unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    for command in [
        "FIXTURE 1 AT 50",
        "RECORD CUELIST 41 CUE 1",
        "RECORD CUELIST 42 CUE 1",
        "ASSIGN CUELIST 41 AT PBK 3.16",
        "ASSIGN CUELIST 42 AT PBK 4.16",
    ] {
        let response = json(scenario.execute(command, Some(command)).await).await;
        assert_eq!(response["outcome"], "accepted", "{command}: {response}");
    }
    for index in 0..2 {
        assert_eq!(
            scenario
                .press_key(&scenario.token, "CLR", &format!("tl786-clear-{index}"))
                .await
                .status(),
            StatusCode::OK
        );
    }
    scenario
        .state
        .installation
        .set_playback_surface_page(scenario.session.desk.id, "main", 3)
        .unwrap();
    for page in [3, 4] {
        tl786_go(&scenario, page).await;
    }
    scenario
}
async fn tl786_go(scenario: &CommandHttpScenario, page: u8) {
    let response = scenario
        .playback_action_for(
            &scenario.token,
            scenario.session.desk.id,
            serde_json::json!({"address":{"kind":"playback","page":page,"position":16},
            "action":{"type":"go","pressed":true},
            "control_source":{"surface_id":"main","control_id":1}}),
        )
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "page {page}: {}",
        json(response).await
    );
    assert!(playback_is_enabled(
        scenario,
        cue_programming_test_address(page, 16)
    ));
}
fn tl786_document(scenario: &CommandHttpScenario) -> light_show::PortableShowDocument {
    let show = scenario.state.active_show.current().clone().unwrap();
    ShowStore::open(&show.path)
        .unwrap()
        .portable_document()
        .unwrap()
}
fn tl786_runtime(scenario: &CommandHttpScenario, page: u8) -> light_playback::ActivePlayback {
    scenario
        .state
        .output
        .playback_runtime_status_at(cue_programming_test_address(page, 16))
        .unwrap()
        .playback
}
fn tl786_assert_stopped(scenario: &CommandHttpScenario, page: u8) {
    assert!(
        !tl786_runtime(scenario, page).enabled,
        "OFF must disable the retained addressed Playback runtime"
    );
}
#[tokio::test]
async fn tl786_actual_http_off_targets_explicit_page_without_authored_writes() {
    let scenario = tl786_running_pair().await;
    let before = tl786_document(&scenario);
    let neighbor = tl786_runtime(&scenario, 4);
    let response = json(scenario.execute("tl786-http", Some("OFF PBK 3.16")).await).await;
    assert_eq!(response["outcome"], "accepted", "{response}");
    tl786_assert_stopped(&scenario, 3);
    assert_eq!(tl786_runtime(&scenario, 4), neighbor);
    assert_eq!(tl786_document(&scenario), before);
    let repeated = json(
        scenario
            .execute("tl786-http-repeat", Some("OFF PBK 3.16"))
            .await,
    )
    .await;
    assert_eq!(repeated["outcome"], "accepted", "{repeated}");
    assert_eq!(tl786_document(&scenario), before);
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}
#[tokio::test]
async fn tl786_actual_software_off_pbk_digits_enter_uses_same_authority() {
    let scenario = tl786_running_pair().await;
    let before = tl786_document(&scenario);
    let neighbor = tl786_runtime(&scenario, 4);
    for (index, key) in ["OFF", "PBK", "3", ".", "1", "6", "ENT"]
        .into_iter()
        .enumerate()
    {
        let response = scenario
            .press_key(&scenario.token, key, &format!("tl786-key-{index}"))
            .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{key}: {}",
            json(response).await
        );
    }
    tl786_assert_stopped(&scenario, 3);
    assert_eq!(tl786_runtime(&scenario, 4), neighbor);
    assert_eq!(tl786_document(&scenario), before);
    assert!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .command_line
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}
#[tokio::test]
async fn tl786_ws_command_and_selected_surface_page_match_explicit_off() {
    let scenario = tl786_running_pair().await;
    let desk = scenario.session.desk.id;
    let mut surfaces = scenario.state.installation.playback_surfaces(desk).unwrap();
    let mut selected = surfaces["main"].clone();
    selected.page_source = light_show::PlaybackPageSource::Own { page: 4 };
    surfaces.insert("pane:off-test".into(), selected);
    scenario
        .state
        .installation
        .put_playback_surfaces(desk, &surfaces)
        .unwrap();
    scenario
        .state
        .installation
        .select_playback_surface(desk, "pane:off-test")
        .unwrap();
    let before = tl786_document(&scenario);
    let explicit_neighbor = tl786_runtime(&scenario, 4);
    let explicit = json(
        scenario
            .execute("tl786-explicit-selected-other", Some("OFF PBK 3.16"))
            .await,
    )
    .await;
    assert_eq!(explicit["outcome"], "accepted", "{explicit}");
    tl786_assert_stopped(&scenario, 3);
    assert_eq!(tl786_runtime(&scenario, 4), explicit_neighbor);
    tl786_go(&scenario, 3).await;
    scenario
        .state
        .installation
        .select_playback_surface(desk, "pane:off-test")
        .unwrap();
    let neighbor = tl786_runtime(&scenario, 3);
    let response = dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            "tl786-ws",
            light_wire::v2::live_action::LiveAction::CommandLineExecute(
                light_wire::v2::live_action::CommandLineExecuteLiveActionRequest { pool_tap: None,
                    value: "OFF PBK 16".into(),
                },
            ),
        ),
    );
    assert!(response.ok, "{:?}", response.error);
    tl786_assert_stopped(&scenario, 4);
    assert_eq!(tl786_runtime(&scenario, 3), neighbor);
    assert_eq!(tl786_document(&scenario), before);
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}
fn tl786_subscribe(scenario: &CommandHttpScenario) {
    let source: SocketAddr = "127.0.0.1:9786".parse().unwrap();
    scenario.state.integrations.register_osc_subscriber(
        "tl786-keys".into(),
        OscSubscriber {
            capability: light_core::SurfaceCapability::Programming,
            path: "desk".into(),
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
}
fn tl786_osc_keys(scenario: &CommandHttpScenario, source: &str) {
    for key in [
        "off", "playback", "digit-3", "dot", "digit-1", "digit-6", "enter",
    ] {
        let address = format!("/light/desk/programmer/{key}");
        for pressed in [true, false] {
            handle_programmer_osc(
                &scenario.state,
                &address,
                &[OscArgument::Bool(pressed)],
                Some(source),
            );
        }
    }
}
#[tokio::test]
async fn tl786_subscribed_osc_off_keypad_and_releases_preserve_peer() {
    let scenario = tl786_running_pair().await;
    let before = tl786_document(&scenario);
    let neighbor = tl786_runtime(&scenario, 4);
    tl786_osc_keys(&scenario, "127.0.0.1:9786"); // unsubscribed has no authority
    assert!(playback_is_enabled(
        &scenario,
        cue_programming_test_address(3, 16)
    ));
    tl786_subscribe(&scenario);
    tl786_osc_keys(&scenario, "127.0.0.1:9787"); // wrong remote has no authority
    assert!(playback_is_enabled(
        &scenario,
        cue_programming_test_address(3, 16)
    ));
    tl786_osc_keys(&scenario, "127.0.0.1:9786");
    assert!(
        !scenario
            .state
            .events
            .audit_events()
            .iter()
            .any(|event| { event.kind == "desk_action" && event.payload["action"] == "off" }),
        "authoritative OSC Off must not also ask the GUI to append OFF"
    );
    tl786_assert_stopped(&scenario, 3);
    assert_eq!(tl786_runtime(&scenario, 4), neighbor);
    assert_eq!(tl786_document(&scenario), before);
    let history = scenario.history_len();
    handle_programmer_osc(
        &scenario.state,
        "/light/desk/programmer/enter",
        &[OscArgument::Bool(false)],
        Some("127.0.0.1:9786"),
    );
    assert_eq!(
        scenario.history_len(),
        history,
        "key release must not repeat Off"
    );
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}
#[tokio::test]
async fn tl786_invalid_targets_never_stop_neighbors_or_change_show() {
    let scenario = tl786_running_pair().await;
    let before = tl786_document(&scenario);
    let runtimes = [tl786_runtime(&scenario, 3), tl786_runtime(&scenario, 4)];
    for (index, command) in [
        "OFF",
        "OFF PBK",
        "OFF PBK 0",
        "OFF PBK 0.16",
        "OFF PBK 100.16",
        "OFF PBK 3.1001",
        "OFF PBK 3.16 EXTRA",
        "OFF VPBK 16",
        "OFF VPLBK 16",
        "OFF PBK 3.999",
    ]
    .into_iter()
    .enumerate()
    {
        let response = json(
            scenario
                .execute(&format!("tl786-invalid-{index}"), Some(command))
                .await,
        )
        .await;
        assert_eq!(response["outcome"], "rejected", "{command}: {response}");
        assert!(
            !response.to_string().contains("fixture number is invalid"),
            "{command}: {response}"
        );
        assert_eq!(
            [tl786_runtime(&scenario, 3), tl786_runtime(&scenario, 4)],
            runtimes
        );
        assert_eq!(tl786_document(&scenario), before);
    }
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}
#[tokio::test]
async fn tl786_typed_off_uses_canonical_preload_queue_until_go() {
    tl786_preload_off(true).await;
}
#[tokio::test]
async fn tl786_direct_off_uses_identical_canonical_preload_release() {
    tl786_preload_off(false).await;
}
async fn tl786_preload_off(typed: bool) {
    let clock = Arc::new(ManualClock::new(chrono::Utc::now()));
    let scenario = tl786_install_pair(CommandHttpScenario::with_clock(clock.clone()).await).await;
    let before = tl786_document(&scenario);
    let neighbor = tl786_runtime(&scenario, 4);
    let registry = scenario.state.programming.programmers();
    let request = lifecycle_request(
        "tl786-enter",
        registry.capture_mode_revision(),
        registry.preload_values_revision(),
        registry.preload_playback_queue_revision(),
        registry.selection(scenario.session.id).unwrap().revision,
        serde_json::json!({"type":"enter"}),
    );
    assert_eq!(
        scenario.preload_lifecycle_action(request).await.status(),
        StatusCode::OK
    );
    if typed {
        let response = json(
            scenario
                .execute("tl786-capture-off", Some("OFF PBK 3.16"))
                .await,
        )
        .await;
        assert_eq!(response["outcome"], "accepted", "{response}");
    } else {
        let response = scenario
            .playback_action_for(
                &scenario.token,
                scenario.session.desk.id,
                serde_json::json!({"address":{"kind":"playback","page":3,"position":16},
                "action":{"type":"off","pressed":true},
                "control_source":{"surface_id":"main","control_id":1}}),
            )
            .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{}",
            json(response).await
        );
    }
    assert!(playback_is_enabled(
        &scenario,
        cue_programming_test_address(3, 16)
    ));
    let queue = json(scenario.preload_playback_queue_snapshot().await).await;
    assert_eq!(
        queue["projection"]["actions"].as_array().unwrap().len(),
        1,
        "{queue}"
    );
    assert_eq!(queue["projection"]["actions"][0]["action"], "off");
    assert_eq!(
        queue["projection"]["actions"][0]["address"],
        serde_json::json!({"page":3,"position":16})
    );
    let current = tl786_document(&scenario);
    let request = lifecycle_go(
        "tl786-go",
        registry.capture_mode_revision(),
        registry.preload_values_revision(),
        registry.preload_playback_queue_revision(),
        registry.selection(scenario.session.id).unwrap().revision,
        current.id().0,
        current.revision().value(),
        scenario.state.events.latest_sequence(),
    );
    let response = scenario.preload_lifecycle_action(request).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        json(response).await
    );
    let fading = tl786_runtime(&scenario, 3);
    let transition = fading
        .master_transition
        .as_ref()
        .expect("GO must begin canonical timed Off");
    assert!(transition.release_after);
    assert_eq!(transition.to, 0.0);
    assert_eq!(
        transition.duration_millis,
        scenario
            .state
            .installation
            .configuration()
            .programmer_fade_millis
    );
    assert_eq!(tl786_runtime(&scenario, 4), neighbor);
    clock.advance_millis(transition.duration_millis as i64 + 1);
    scenario.state.output.render(Default::default()).unwrap();
    tl786_assert_stopped(&scenario, 3);
    assert_eq!(tl786_runtime(&scenario, 4), neighbor);
    let cleared = json(scenario.preload_playback_queue_snapshot().await).await;
    assert!(
        cleared["projection"]["actions"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{cleared}"
    );
    assert_eq!(tl786_document(&scenario), before);
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}

#[tokio::test]
async fn tl786_osc_double_off_opens_running_output_without_gui_command_echo() {
    let scenario = tl786_running_pair().await;
    let before = tl786_document(&scenario);
    let runtimes = [tl786_runtime(&scenario, 3), tl786_runtime(&scenario, 4)];
    let send = |source: &str, pressed| {
        handle_programmer_osc(
            &scenario.state,
            "/light/desk/programmer/off",
            &[OscArgument::Bool(pressed)],
            Some(source),
        );
    };
    let opened = || {
        scenario
            .state
            .events
            .audit_events()
            .iter()
            .filter(|event| {
                event.kind == "desk_action" && event.payload["action"] == "running-output"
            })
            .count()
    };
    send("127.0.0.1:9786", true);
    assert_eq!(opened(), 0, "unsubscribed cannot open an operator window");
    tl786_subscribe(&scenario);
    send("127.0.0.1:9786", true);
    send("127.0.0.1:9786", false);
    let first = scenario
        .state
        .programming
        .command_line_state(scenario.session.id)
        .unwrap();
    assert_eq!(first.visible_text(), "OFF");
    assert_eq!(opened(), 0);
    let mut guest = scenario
        .state
        .integrations
        .osc_subscriber_for_source("127.0.0.1:9786".parse().unwrap())
        .unwrap();
    guest.capability = light_core::SurfaceCapability::PlaybackOnly;
    guest.command_source = "127.0.0.1:9788".parse().unwrap();
    guest.target = guest.command_source;
    scenario
        .state
        .integrations
        .register_osc_subscriber("tl786-guest".into(), guest);
    send("127.0.0.1:9788", true);
    send("127.0.0.1:9788", false);
    assert_eq!(opened(), 0, "PlaybackOnly cannot open an operator window");
    assert_eq!(
        scenario
            .state
            .programming
            .command_line_state(scenario.session.id)
            .unwrap(),
        first
    );
    send("127.0.0.1:9787", true);
    send("127.0.0.1:9787", false);
    assert_eq!(opened(), 0, "a different remote has no authority");
    send("127.0.0.1:9786", true);
    send("127.0.0.1:9786", false);
    assert_eq!(
        opened(),
        1,
        "second Off must open Running & Output exactly once"
    );
    assert_eq!(
        scenario
            .state
            .programming
            .command_line_state(scenario.session.id)
            .unwrap(),
        first,
        "the second press and releases must not duplicate command edits"
    );
    assert_eq!(
        [tl786_runtime(&scenario, 3), tl786_runtime(&scenario, 4)],
        runtimes
    );
    assert_eq!(tl786_document(&scenario), before);
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}
