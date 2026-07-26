async fn post_live_action(
    scenario: &CommandHttpScenario,
    show_id: &str,
    path: &str,
    body: serde_json::Value,
) -> Response {
    scenario
        .app
        .clone()
        .oneshot(
            Request::post(path)
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header("x-tosk-show", show_id)
                .header("x-tosk-desk", scenario.session.desk.id.to_string())
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn programming_align_http_and_websocket_reject_the_same_empty_selection() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario
        .create_and_open_show("Align HTTP and WebSocket convergence")
        .await;
    let request = light_wire::v2::live_action::ProgrammingAlignLiveActionRequest {
        request_id: "align-empty-selection".into(),
        attribute: "intensity".into(),
        mode: light_wire::v2::live_action::ProgrammingAlignMode::Left,
        from: 0.0,
        to: 1.0,
    };
    let websocket = dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            &request.request_id,
            light_wire::v2::live_action::LiveAction::ProgrammingAlign(request.clone()),
        ),
    );
    assert!(!websocket.ok);

    let http = post_live_action(
        &scenario,
        &show_id,
        "/api/v2/programming-align/actions",
        serde_json::json!({
            "attribute":request.attribute,
            "mode":request.mode,
            "from":request.from,
            "to":request.to,
            "future_field":"accepted",
        }),
    )
    .await;
    assert_eq!(http.status(), StatusCode::BAD_REQUEST);
    let error = json(http).await["error"].as_str().unwrap().to_owned();
    assert_eq!(websocket.error.as_deref(), Some(error.as_str()));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn capture_mode_http_and_websocket_publish_one_typed_change_each() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario
        .create_and_open_show("Capture mode HTTP and WebSocket convergence")
        .await;
    let count_changes = |after| {
        let light_application::EventReplay::Events(events) = scenario
            .state
            .application_events
            .replay(after, &light_application::EventFilter::default())
        else {
            panic!("capture-mode events should remain replayable");
        };
        events
            .iter()
            .filter(|event| {
                matches!(
                    event.payload,
                    light_application::ApplicationEvent::Programming(
                        light_application::ProgrammingEvent::CaptureModeChanged(_)
                    )
                )
            })
            .count()
    };
    let baseline = scenario.state.application_events.latest_sequence();
    let websocket_request = light_wire::v2::live_action::ProgrammerCaptureModeLiveActionRequest {
        request_id: "capture-mode-ws".into(),
        blind: Some(true),
        preview: None,
        active_context: Some(Some("capture-mode-test".into())),
    };
    let websocket_request_id = websocket_request.request_id.clone();
    let websocket = dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            websocket_request_id,
            light_wire::v2::live_action::LiveAction::ProgrammerCaptureMode(websocket_request),
        ),
    );
    assert!(websocket.ok, "{:?}", websocket.error);
    assert_eq!(count_changes(baseline), 1);

    let before_http = scenario.state.application_events.latest_sequence();
    let http = post_live_action(
        &scenario,
        &show_id,
        "/api/v2/programmer-capture-mode/actions",
        serde_json::json!({
            "blind":false,
            "preview":true,
            "active_context":null,
            "future_field":"accepted",
        }),
    )
    .await;
    assert_eq!(http.status(), StatusCode::OK);
    let outcome = json(http).await;
    assert!(!outcome["request_id"].as_str().unwrap().is_empty());
    assert_eq!(outcome["blind"], false);
    assert_eq!(outcome["preview"], true);
    assert_eq!(count_changes(before_http), 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn explicit_command_target_and_undo_http_forms_match_websocket_semantics() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario
        .create_and_open_show("Command target and undo HTTP parity")
        .await;
    let target = dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            "target-ws",
            light_wire::v2::live_action::LiveAction::CommandTarget(
                light_wire::v2::live_action::CommandTargetLiveActionRequest {
                    value: light_wire::v2::command_line::CommandTarget::Group,
                },
            ),
        ),
    );
    assert!(target.ok, "{:?}", target.error);
    let target_http = post_live_action(
        &scenario,
        &show_id,
        "/api/v2/command-target/actions",
        serde_json::json!({"value":"FIXTURE","future_field":"accepted"}),
    )
    .await;
    assert_eq!(target_http.status(), StatusCode::OK);
    assert_eq!(json(target_http).await["target"], "FIXTURE");

    let undo_ws = dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            "undo-ws",
            light_wire::v2::live_action::LiveAction::ProgrammerUndo,
        ),
    );
    assert!(undo_ws.ok, "{:?}", undo_ws.error);
    assert_eq!(undo_ws.payload.unwrap()["changed"], false);
    let undo_http = scenario
        .app
        .clone()
        .oneshot(
            Request::get("/api/v2/programmer-undo/actions")
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header("x-tosk-show", &show_id)
                .header("x-tosk-desk", scenario.session.desk.id.to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(undo_http.status(), StatusCode::OK);
    assert_eq!(
        undo_http.headers()[header::CACHE_CONTROL].to_str().unwrap(),
        "no-store"
    );
    assert_eq!(json(undo_http).await["changed"], false);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn fixture_control_http_and_websocket_share_not_found_semantics() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario
        .create_and_open_show("Fixture control HTTP and WebSocket convergence")
        .await;
    let request = light_wire::v2::live_action::FixtureControlLiveActionRequest {
        request_id: "fixture-control-missing".into(),
        fixture_id: Uuid::new_v4(),
        action_id: Uuid::new_v4(),
        active: true,
    };
    let websocket = dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            &request.request_id,
            light_wire::v2::live_action::LiveAction::FixtureControl(request.clone()),
        ),
    );
    assert!(!websocket.ok);

    let http = post_live_action(
        &scenario,
        &show_id,
        "/api/v2/fixture-controls/actions",
        serde_json::json!({
            "fixture_id":request.fixture_id,
            "action_id":request.action_id,
            "active":request.active,
            "future_field":"accepted",
        }),
    )
    .await;
    assert_eq!(http.status(), StatusCode::NOT_FOUND);
    let error = json(http).await["error"].as_str().unwrap().to_owned();
    assert!(error.contains(websocket.error.as_deref().unwrap()));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preset_generation_intent_replay_does_not_repeat_show_mutation_or_events() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario
        .create_and_open_show("Preset generation intent replay")
        .await;
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    assert_eq!(
        scenario
            .put_active_object(
                &show_id,
                "patched_fixture",
                &fixture_id.0.to_string(),
                0,
                serde_json::to_value(fixture).unwrap(),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let show = scenario.state.active_show.read().clone().unwrap();
    let show_revision = ShowStore::open(&show.path)
        .unwrap()
        .portable_revision()
        .unwrap()
        .value();
    let body = serde_json::json!({
        "request_id":"generate-presets-replay",
        "expected_show_revision":show_revision,
        "fixture_ids":[fixture_id.0],
    });
    let first = post_live_action(
        &scenario,
        &show_id,
        "/api/v2/preset-profile-generation/update",
        body.clone(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first = json(first).await;
    assert_eq!(first["replayed"], false);
    assert!(!first["created"].as_array().unwrap().is_empty());
    let event_sequence = scenario.state.application_events.latest_sequence();
    let preset_count = ShowStore::open(&show.path)
        .unwrap()
        .objects("preset")
        .unwrap()
        .len();

    let replay = post_live_action(
        &scenario,
        &show_id,
        "/api/v2/preset-profile-generation/update",
        body,
    )
    .await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["show_revision"], first["show_revision"]);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        event_sequence
    );
    assert_eq!(
        ShowStore::open(&show.path)
            .unwrap()
            .objects("preset")
            .unwrap()
            .len(),
        preset_count
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
