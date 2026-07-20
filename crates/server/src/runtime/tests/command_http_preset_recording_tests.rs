fn preset_record_request(
    request_id: &str,
    family: &str,
    number: u32,
    expected_object_revision: u64,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "address": {"family": family, "number": number},
        "name": format!("Preset {number}"),
        "mode": "overwrite",
        "expected_object_revision": expected_object_revision,
    })
}

async fn scenario_with_recordable_value() -> (CommandHttpScenario, String) {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Preset record route").await;
    let fixture = scenario.install_direct_fixture();
    let set = serde_json::json!({
        "request_id": "preset-route-value",
        "expected_revision": 0,
        "expected_capture_mode_revision": 0,
        "action": {
            "type": "set_fixture",
            "fixture_id": fixture.0,
            "attribute": "intensity",
            "value": {"kind": "normalized", "value": 0.5}
        }
    });
    assert_eq!(scenario.values_action(set).await.status(), StatusCode::OK);
    (scenario, show_id)
}

#[tokio::test]
async fn preset_record_route_is_authoritative_replay_safe_and_sparse_on_no_change() {
    let (scenario, show_id) = scenario_with_recordable_value().await;
    let baseline = scenario.state.application_events.latest_sequence();
    let request = preset_record_request("preset-route-record", "mixed", 7, 0);
    let response = scenario
        .preset_recording_action(&show_id, Some(&scenario.token), request.clone())
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let first: light_wire::v2::preset_recording::PresetRecordOutcome =
        serde_json::from_value(json(response).await).unwrap();
    assert_eq!(first.request_id(), "preset-route-record");
    assert!(!first.replayed());
    assert_eq!(first.preset().id, "0.7");
    assert_eq!(first.preset().revision, 1);
    assert_eq!(first.preset().body["number"], 7);
    let event_sequence = first.event_sequence().unwrap();
    assert_eq!(event_sequence, baseline + 1);
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 1);

    let replay = scenario
        .preset_recording_action(&show_id, Some(&scenario.token), request)
        .await;
    let replay: light_wire::v2::preset_recording::PresetRecordOutcome =
        serde_json::from_value(json(replay).await).unwrap();
    assert!(replay.replayed());
    assert_eq!(replay.event_sequence(), Some(event_sequence));
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 1);

    let no_change = scenario
        .preset_recording_action(
            &show_id,
            Some(&scenario.token),
            preset_record_request("preset-route-no-change", "mixed", 7, 1),
        )
        .await;
    let no_change: light_wire::v2::preset_recording::PresetRecordOutcome =
        serde_json::from_value(json(no_change).await).unwrap();
    assert!(matches!(
        &no_change,
        light_wire::v2::preset_recording::PresetRecordOutcome::NoChange { .. }
    ));
    assert_eq!(no_change.event_sequence(), None);
    assert_eq!(no_change.preset().revision, 1);
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preset_record_route_rejects_auth_conflict_forged_values_and_preload_capture() {
    let (scenario, show_id) = scenario_with_recordable_value().await;
    let request = preset_record_request("preset-route-secure", "intensity", 2, 0);
    assert_eq!(
        scenario
            .preset_recording_action(&show_id, None, request.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let mut forged = request.clone();
    forged["values"] = serde_json::json!({"forged": true});
    assert_eq!(
        scenario
            .preset_recording_action(&show_id, Some(&scenario.token), forged)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        scenario
            .preset_recording_action(&show_id, Some(&scenario.token), request)
            .await
            .status(),
        StatusCode::OK
    );
    let conflict = scenario
        .preset_recording_action(
            &show_id,
            Some(&scenario.token),
            preset_record_request("preset-route-conflict", "intensity", 2, 0),
        )
        .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(json(conflict).await["current_revision"], 1);

    assert_eq!(
        scenario
            .press_key(&scenario.token, "PRE", "preset-route-preload")
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .preset_recording_action(
                &show_id,
                Some(&scenario.token),
                preset_record_request("preset-route-preload-record", "mixed", 3, 0),
            )
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn plain_preset_and_group_record_commands_use_typed_capabilities() {
    let (scenario, show_id) = scenario_with_recordable_value().await;
    let response = scenario
        .execute("preset-command-record", Some("RECORD 0.8"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let object = scenario
        .app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/shows/{show_id}/objects/preset/0.8"))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", scenario.token),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(object.status(), StatusCode::OK);
    let group = scenario
        .execute("preset-command-group", Some("RECORD GROUP 8"))
        .await;
    assert_eq!(group.status(), StatusCode::OK);
    let group = json(group).await;
    assert_eq!(group["outcome"], "accepted");
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn osc_record_key_sequence_commits_through_the_typed_preset_capability() {
    let (scenario, show_id) = scenario_with_recordable_value().await;
    let source: SocketAddr = "127.0.0.1:9017".parse().unwrap();
    scenario.state.osc_subscribers.lock().insert(
        "preset-record".into(),
        OscSubscriber {
            desk_alias: "main".into(),
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
    let pressed = [OscArgument::Bool(true)];
    for action in ["record", "digit-0", "dot", "digit-9", "enter"] {
        handle_programmer_osc(
            &scenario.state,
            &format!("/light/main/programmer/{action}"),
            &pressed,
            Some("127.0.0.1:9017"),
        );
    }

    assert_eq!(
        scenario
            .state
            .programmers
            .get(scenario.session.id)
            .unwrap()
            .command_line,
        ""
    );
    let object = scenario
        .app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/shows/{show_id}/objects/preset/0.9"))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", scenario.token),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(object.status(), StatusCode::OK);
    let body = json(object).await;
    assert_eq!(body["body"]["number"], 9);
    assert_eq!(body["body"]["values"].as_object().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn websocket_preset_request_replay_skips_interaction_side_effects() {
    let (scenario, _show_id) = scenario_with_recordable_value().await;
    let command = || WsCommand {
        protocol_version: 1,
        request_id: "preset-ws-replay".into(),
        session_id: scenario.session.id,
        expected_revision: None,
        command: "programmer.execute".into(),
        payload: serde_json::json!({"value":"RECORD 0.10"}),
    };

    let first = dispatch_ws_command(&scenario.state, &scenario.session, command());
    assert!(first.ok, "{:?}", first.error);
    let first_sequence = scenario.state.application_events.latest_sequence();
    let first_history = scenario.history_len();
    assert_eq!(first_history, 1);

    let replay = dispatch_ws_command(&scenario.state, &scenario.session, command());
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(scenario.state.application_events.latest_sequence(), first_sequence);
    assert_eq!(scenario.history_len(), first_history);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
