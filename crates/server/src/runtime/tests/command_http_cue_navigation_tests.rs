// Public-boundary coverage for the typed CUE navigation family.
//
// Go To and Load reach the Playback application service through the v2 command-line HTTP contract.
// These tests assert the operator-visible contract: authoritative runtime state, exactly one typed
// Playback event per real transition, command-line reset only after success, desk-local selection,
// and convergence with the retained compatibility surfaces.

const CUE_LIST_ID: &str = "5f2b9d64-3f4a-4c21-9b8f-2f8f4b6d1a01";

async fn cue_navigation_scenario() -> (CommandHttpScenario, String) {
    install_cue_navigation_show(CommandHttpScenario::new().await).await
}

/// A frozen clock lets a repeated Go To land on the exact same runtime instant.
async fn frozen_cue_navigation_scenario() -> (CommandHttpScenario, String) {
    let clock = Arc::new(ManualClock::new(chrono::Utc::now()));
    install_cue_navigation_show(CommandHttpScenario::with_clock(clock).await).await
}

async fn install_cue_navigation_show(
    scenario: CommandHttpScenario,
) -> (CommandHttpScenario, String) {
    let show_id = scenario.create_and_open_show("Cue navigation").await;
    let fixture = scenario.install_direct_fixture();
    let cue = |number: f64, level: f64| {
        serde_json::json!({
            "id": Uuid::new_v4(),
            "number": number,
            "name": format!("Cue {number}"),
            "fade_millis": 0,
            "delay_millis": 0,
            "trigger": {"type":"manual"},
            "phasers": [],
            "group_changes": [],
            "changes": [{
                "fixture_id": fixture,
                "attribute": "intensity",
                "value": {"kind":"normalized","value":level},
                "automatic_restore": false
            }]
        })
    };
    let response = scenario
        .put_active_object(
            &show_id,
            "cue_list",
            CUE_LIST_ID,
            0,
            serde_json::json!({
                "id": CUE_LIST_ID,
                "name": "Twin Cuelist",
                "priority": 0,
                "mode": "sequence",
                "looped": false,
                "wrap_mode": "off",
                "restart_mode": "first_cue",
                "cues": [cue(1.0, 0.2), cue(2.0, 0.5), cue(2.5, 0.4), cue(3.0, 0.8)]
            }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK, "{:?}", json(response).await);
    for (number, name) in [(1, "Twin A"), (2, "Twin B")] {
        let response = scenario
            .put_active_object(
                &show_id,
                "playback",
                &number.to_string(),
                0,
                serde_json::json!({
                    "number": number,
                    "name": name,
                    "target": {"type":"cue_list","cue_list_id":CUE_LIST_ID},
                    "buttons": ["go_minus","go","flash"],
                    "fader": "master",
                    "go_activates": true,
                    "auto_off": false,
                    "xfade_millis": 0,
                    "color": "#20c997",
                    "flash_release": "release_all",
                    "protect_from_swap": false
                }),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let response = scenario
        .put_active_object(
            &show_id,
            "playback_page",
            "1",
            0,
            serde_json::json!({"number":1,"name":"Main","slots":{"1":1,"2":2}}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    (scenario, show_id)
}

fn select_playback(scenario: &CommandHttpScenario, desk_id: Uuid, playback: Option<u16>) {
    let show_id = scenario.state.active_show.read().as_ref().unwrap().id;
    scenario
        .state
        .desk
        .lock()
        .set_selected_playback(desk_id, show_id, playback)
        .unwrap();
}

fn active_playback(
    scenario: &CommandHttpScenario,
    playback: u16,
) -> Option<light_playback::ActivePlayback> {
    scenario
        .state
        .engine
        .playback_runtime()
        .into_iter()
        .find(|runtime| runtime.playback_number == Some(playback))
}

fn current_cue(scenario: &CommandHttpScenario, playback: u16) -> Option<f64> {
    active_playback(scenario, playback).and_then(|runtime| runtime.current_cue_number)
}

fn loaded_cue(scenario: &CommandHttpScenario, playback: u16) -> Option<f64> {
    active_playback(scenario, playback).and_then(|runtime| runtime.loaded_cue_number)
}

/// Counts only authoritative typed Playback runtime events published after `baseline`.
fn playback_events(scenario: &CommandHttpScenario, baseline: u64) -> usize {
    let light_application::EventReplay::Events(events) = scenario
        .state
        .application_events
        .replay(baseline, &light_application::EventFilter::default())
    else {
        return usize::MAX;
    };
    events
        .iter()
        .filter(|event| {
            matches!(
                event.payload,
                light_application::ApplicationEvent::Playback(
                    light_application::PlaybackEvent::RuntimeChanged(_)
                )
            )
        })
        .count()
}

/// Counts the temporary v1 `playback_changed` notification.
fn compatibility_notifications(scenario: &CommandHttpScenario) -> usize {
    scenario
        .state
        .audit_events
        .lock()
        .iter()
        .filter(|event| event.kind == "playback_changed")
        .count()
}

/// A reset command line returns to the full editable default target and is pristine again.
async fn assert_command_line_reset(scenario: &CommandHttpScenario) {
    let response = scenario.get().await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["text"], "FIXTURE", "{body}");
    assert_eq!(body["pristine"], true, "{body}");
}

async fn command_line_text(scenario: &CommandHttpScenario) -> String {
    let response = scenario.get().await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await["text"].as_str().unwrap_or("").to_owned()
}

fn history_len(scenario: &CommandHttpScenario) -> usize {
    scenario
        .state
        .command_history
        .lock()
        .get(&scenario.session.desk.id)
        .map_or(0, std::collections::VecDeque::len)
}

#[tokio::test]
async fn selected_and_explicit_go_to_and_load_use_the_typed_playback_action() {
    let (scenario, _show_id) = cue_navigation_scenario().await;
    select_playback(&scenario, scenario.session.desk.id, Some(2));

    // Go To on the desk-selected Playback.
    let baseline = scenario.state.application_events.latest_sequence();
    let response = scenario.execute("go-to-selected", Some("CUE 3")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["outcome"], "accepted");
    assert_eq!(current_cue(&scenario, 2), Some(3.0));
    assert_eq!(playback_events(&scenario, baseline), 1);
    // A successful action clears the shared command line exactly once.
    assert_eq!(body["command_line"]["text"], "FIXTURE");
    assert_command_line_reset(&scenario).await;
    // The v2 path must not fabricate the v1 compatibility notification.
    assert_eq!(compatibility_notifications(&scenario), 0);

    // Load on the desk-selected Playback leaves the current Cue and output untouched.
    let baseline = scenario.state.application_events.latest_sequence();
    let response = scenario.execute("load-selected", Some("CUE CUE 2")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["outcome"], "accepted");
    assert_eq!(current_cue(&scenario, 2), Some(3.0));
    assert_eq!(loaded_cue(&scenario, 2), Some(2.0));
    assert_eq!(playback_events(&scenario, baseline), 1);

    // Explicit pool Go To addresses the other Playback without changing the selection.
    let response = scenario
        .execute("go-to-explicit", Some("CUE SET 1 CUE 3"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["outcome"], "accepted");
    assert_eq!(current_cue(&scenario, 1), Some(3.0));
    let show_id = scenario.state.active_show.read().as_ref().unwrap().id;
    assert_eq!(
        scenario
            .state
            .desk
            .lock()
            .selected_playback(scenario.session.desk.id, show_id)
            .unwrap(),
        Some(2)
    );

    // Explicit Load through an explicit-page address, with a decimal Cue number.
    let response = scenario
        .execute("load-explicit-page", Some("CUE CUE SET 1 . 2 CUE 2.5"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["outcome"], "accepted");
    assert_eq!(loaded_cue(&scenario, 2), Some(2.5));
    assert_eq!(compatibility_notifications(&scenario), 0);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn rejected_navigation_retains_the_command_line_and_mutates_no_runtime() {
    let (scenario, _show_id) = cue_navigation_scenario().await;

    // A missing selection is rejected before any Playback is addressed.
    let revision = json(scenario.get().await).await["revision"].as_u64().unwrap();
    assert_eq!(scenario.put("CUE 2", revision).await.status(), StatusCode::OK);
    let baseline = scenario.state.application_events.latest_sequence();
    let response = scenario.execute("missing-selection", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["outcome"], "rejected");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("no playback is selected"),
        "{body}"
    );
    // The rejected command line is retained for correction, not reset.
    assert_eq!(command_line_text(&scenario).await, "CUE 2");
    assert_eq!(playback_events(&scenario, baseline), 0);

    select_playback(&scenario, scenario.session.desk.id, Some(2));

    // A missing Playback and a missing Cue are both rejected without mutating runtime.
    for (request_id, command, expected) in [
        ("missing-playback", "CUE SET 99 CUE 1", "playback 99 does not exist"),
        ("missing-cue", "CUE 99", "cue does not exist"),
        ("unassigned-slot", "CUE SET 9 . 9 CUE 1", "page 9 slot 9 is not assigned"),
    ] {
        let baseline = scenario.state.application_events.latest_sequence();
        let response = scenario.execute(request_id, Some(command)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json(response).await;
        assert_eq!(body["outcome"], "rejected", "{command}");
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .to_ascii_lowercase()
                .contains(expected),
            "{command}: {body}"
        );
        assert_eq!(playback_events(&scenario, baseline), 0, "{command}");
        assert_eq!(current_cue(&scenario, 2), None, "{command}");
    }
    assert_eq!(compatibility_notifications(&scenario), 0);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn replay_and_semantic_no_change_publish_no_second_transition() {
    let (scenario, _show_id) = frozen_cue_navigation_scenario().await;
    select_playback(&scenario, scenario.session.desk.id, Some(2));

    let baseline = scenario.state.application_events.latest_sequence();
    let history = history_len(&scenario);
    let response = scenario.execute("go-to-once", Some("CUE 3")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["outcome"], "accepted");
    assert_eq!(playback_events(&scenario, baseline), 1);
    let after_first = scenario.state.application_events.latest_sequence();
    let history_after_first = history_len(&scenario);
    assert_eq!(history_after_first, history + 1);

    // Repeating the same request ID must not repeat the runtime transition or the history entry.
    let response = scenario.execute("go-to-once", Some("CUE 3")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["outcome"], "accepted");
    assert_eq!(playback_events(&scenario, after_first), 0);
    assert_eq!(history_len(&scenario), history_after_first);
    assert_eq!(current_cue(&scenario, 2), Some(3.0));

    // A distinct request that resolves to the same runtime instant is a semantic no-change: the
    // frozen clock keeps `activated_at` identical, so nothing about the runtime actually moves.
    let baseline = scenario.state.application_events.latest_sequence();
    let response = scenario.execute("go-to-again", Some("CUE 3")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["outcome"], "accepted");
    assert_eq!(playback_events(&scenario, baseline), 0);
    // A no-change action is still a successful command, so the command line resets.
    assert_command_line_reset(&scenario).await;
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn selection_is_desk_local_and_foreign_or_locked_desks_are_rejected() {
    let (scenario, _show_id) = cue_navigation_scenario().await;
    let second_desk = scenario.state.desk.lock().add_desk("Wing", "wing").unwrap();
    select_playback(&scenario, scenario.session.desk.id, Some(2));
    select_playback(&scenario, second_desk.id, Some(1));

    // The same user on a second desk keeps an independent selected Playback.
    let response = scenario
        .app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"username":"Operator","desk_id":second_desk.id})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let second_token = json(response).await["token"].as_str().unwrap().to_owned();

    let response = scenario.execute("desk-one-go-to", Some("CUE 3")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["outcome"], "accepted");
    assert_eq!(current_cue(&scenario, 2), Some(3.0));
    assert_eq!(current_cue(&scenario, 1), None);

    let response = scenario
        .app
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/v2/desks/{}/command-line/execute",
                second_desk.id
            ))
            .header(header::AUTHORIZATION, format!("Bearer {second_token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"request_id":"desk-two-go-to","command":"CUE 2"}).to_string(),
            ))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["outcome"], "accepted");
    assert_eq!(current_cue(&scenario, 1), Some(2.0));
    assert_eq!(current_cue(&scenario, 2), Some(3.0));

    // The first desk's token cannot drive the second desk's command line.
    let response = scenario
        .app
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/v2/desks/{}/command-line/execute",
                second_desk.id
            ))
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", scenario.token),
            )
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"request_id":"foreign-desk","command":"CUE 1"}).to_string(),
            ))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // An unauthenticated caller cannot navigate at all.
    let response = scenario
        .app
        .clone()
        .oneshot(
            Request::post(format!("{}/execute", scenario.path))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id":"anonymous","command":"CUE 1"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // A locked desk rejects the mutating action without moving the runtime.
    let baseline = scenario.state.application_events.latest_sequence();
    write_desk_lock(
        &scenario.state,
        scenario.session.desk.id,
        &DeskLockConfiguration {
            locked: true,
            ..DeskLockConfiguration::default()
        },
    )
    .unwrap();
    let response = scenario.execute("locked-desk", Some("CUE 1")).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(playback_events(&scenario, baseline), 0);
    assert_eq!(current_cue(&scenario, 2), Some(3.0));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn command_line_websocket_and_osc_navigation_share_the_typed_action() {
    let (scenario, _show_id) = cue_navigation_scenario().await;
    select_playback(&scenario, scenario.session.desk.id, Some(2));

    // The compatibility WebSocket keeps its v1 notification through the same typed action.
    let ws_command = || WsCommand {
        protocol_version: 1,
        request_id: "cue-ws-go-to".into(),
        session_id: scenario.session.id,
        expected_revision: None,
        command: "programmer.execute".into(),
        payload: serde_json::json!({"value":"CUE 3"}),
    };
    let baseline = scenario.state.application_events.latest_sequence();
    let ws = dispatch_ws_command(&scenario.state, &scenario.session, ws_command());
    assert!(ws.ok, "{:?}", ws.error);
    assert_eq!(current_cue(&scenario, 2), Some(3.0));
    assert_eq!(playback_events(&scenario, baseline), 1);
    assert_eq!(compatibility_notifications(&scenario), 1);

    // Replaying the compatibility request repeats neither the transition nor the notification.
    let after = scenario.state.application_events.latest_sequence();
    let replay = dispatch_ws_command(&scenario.state, &scenario.session, ws_command());
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(playback_events(&scenario, after), 0);
    assert_eq!(compatibility_notifications(&scenario), 1);

    // Real OSC keys build the same command and reach the same typed action.
    let source: SocketAddr = "127.0.0.1:9031".parse().unwrap();
    let osc_alias = scenario.session.desk.osc_alias.clone();
    scenario.state.osc_subscribers.lock().insert(
        "cue-navigation-keys".into(),
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
    let baseline = scenario.state.application_events.latest_sequence();
    for action in ["cue", "cue", "digit-2", "enter"] {
        handle_programmer_osc(
            &scenario.state,
            &format!("/light/{osc_alias}/programmer/{action}"),
            &[OscArgument::Bool(true)],
            Some("127.0.0.1:9031"),
        );
    }
    assert_eq!(loaded_cue(&scenario, 2), Some(2.0));
    assert_eq!(current_cue(&scenario, 2), Some(3.0));
    assert_eq!(playback_events(&scenario, baseline), 1);
    assert_command_line_reset(&scenario).await;
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
