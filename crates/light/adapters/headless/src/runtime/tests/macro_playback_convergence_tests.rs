use super::*;

#[tokio::test]
async fn macro_playback_http_and_ui_ws_actions_converge_on_one_execution_service() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap();
    let show = create_show(&app, &token, "Macro Playback convergence").await;
    let opened = app
        .clone()
        .oneshot(open_show_request(
            &token,
            show["id"].as_str().expect("created show id"),
        ))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let show_id = state.active_show.current().as_ref().unwrap().id;
    let macro_id = Uuid::from_u128(71);

    let created = app
        .clone()
        .oneshot(
            Request::post("/api/v2/macros/actions")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-tosk-show", show_id.0.to_string())
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "create-macro-playback-target",
                        "action": {
                            "type": "create",
                            "definition": {
                                "id": macro_id,
                                "number": 71,
                                "name": "Clear programmer",
                                "source": "Clear",
                                "presentation": {"color": "#8f3541"}
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK, "{}", json(created).await);

    install_macro_playback(&state, macro_id);
    let http = app
        .clone()
        .oneshot(playback_action_request(
            &token,
            show_id,
            session.desk.id,
            "http-macro-playback",
        ))
        .await
        .unwrap();
    let http_status = http.status();
    let http = json(http).await;
    assert_eq!(http_status, StatusCode::OK, "{http}");
    assert_eq!(http["projection"]["target"], "macro");
    assert_eq!(http["projection"]["macro_id"], macro_id.to_string());

    let first = wait_for_macro_executions(&state, session.desk.id, 1).await;
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].macro_id, macro_id);
    assert_eq!(first[0].desk_id, session.desk.id);
    assert_eq!(first[0].session_id, session.id.0);
    assert_eq!(
        first[0].trigger,
        light_application::CommandMacroTrigger::Playback { playback_number: 7 }
    );

    let ws_id = "ui-ws-macro-playback";
    let ws = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            ws_id,
            serde_json::from_value(serde_json::json!({
                "type": "playback",
                "request": {
                    "request_id": ws_id,
                    "address": {"kind": "playback", "playback_number": 7},
                    "action": {"type": "go", "pressed": true},
                    "surface": "virtual"
                }
            }))
            .unwrap(),
        ),
    );
    assert!(ws.ok, "{:?}", ws.error);

    let both = wait_for_macro_executions(&state, session.desk.id, 2).await;
    assert_eq!(both.len(), 2);
    assert!(both.iter().all(|execution| {
        execution.macro_id == macro_id
            && execution.trigger
                == light_application::CommandMacroTrigger::Playback { playback_number: 7 }
    }));

    let editor_ws_id = "ui-ws-macro-editor";
    let editor_ws = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            editor_ws_id,
            serde_json::from_value(serde_json::json!({
                "type": "macro",
                "request": {
                    "type": "run",
                    "macro_id": macro_id,
                    "source_revision": 1,
                    "trigger": {"type": "web_socket"}
                }
            }))
            .unwrap(),
        ),
    );
    assert!(editor_ws.ok, "{:?}", editor_ws.error);
    let editor_runs = wait_for_macro_executions(&state, session.desk.id, 3).await;
    assert!(editor_runs.iter().any(|execution| {
        execution.macro_id == macro_id
            && execution.trigger == light_application::CommandMacroTrigger::WebSocket
    }));

    let command_line = app
        .clone()
        .oneshot(
            Request::post("/api/v2/command-line/execute")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-desk", session.desk.id.to_string())
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "command-line-macro-start",
                        "command": "MACRO 71"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        command_line.status(),
        StatusCode::OK,
        "{}",
        json(command_line).await
    );
    let all = wait_for_macro_executions(&state, session.desk.id, 4).await;
    assert!(all.iter().any(|execution| {
        execution.macro_id == macro_id
            && execution.trigger == light_application::CommandMacroTrigger::CommandLine
    }));
    let edit = app
        .clone()
        .oneshot(
            Request::post("/api/v2/command-line/execute")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-desk", session.desk.id.to_string())
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "command-line-macro-edit",
                        "command": "SET MACRO 71"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(edit.status(), StatusCode::OK, "{}", json(edit).await);
    assert!(state.events.audit_events().iter().any(|event| {
        event.kind == "desk_action"
            && event.payload["action"] == "open-object-editor"
            && event.payload["value"] == macro_id.to_string()
    }));
    assert_eq!(
        wait_for_macro_executions(&state, session.desk.id, 4)
            .await
            .len(),
        4,
        "SET opens the editor without starting another execution"
    );
    wait_for_terminal_macro_events(&state, 4).await;
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn wait_for_terminal_macro_events(state: &AppState, expected: usize) {
    for _ in 0..100 {
        let terminal = state
            .events
            .audit_events()
            .into_iter()
            .filter(|event| event.kind == "macro_execution_changed")
            .filter(|event| {
                matches!(
                    event
                        .payload
                        .get("state")
                        .and_then(serde_json::Value::as_str),
                    Some("succeeded" | "failed" | "cancelled")
                )
            })
            .count();
        if terminal >= expected {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("Macro terminal execution events did not reach {expected}")
}

fn install_macro_playback(state: &AppState, macro_id: Uuid) {
    let target = light_playback::PlaybackTarget::Macro { macro_id };
    state
        .output
        .replace_snapshot(EngineSnapshot {
            playbacks: vec![light_playback::PlaybackDefinition {
                number: 7,
                name: "Macro 71".into(),
                target: target.clone(),
                buttons: light_playback::PlaybackDefinition::default_buttons(&target),
                button_count: 1,
                fader: light_playback::PlaybackDefinition::default_fader(&target),
                has_fader: false,
                footprint: light_playback::PlaybackFootprint::Normal,
                go_activates: true,
                auto_off: false,
                xfade_millis: 0,
                color: "#8f3541".into(),
                flash_release: light_playback::FlashReleaseMode::default(),
                protect_from_swap: false,
                presentation_icon: None,
                presentation_image: None,
            }]
            .into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
}

fn playback_action_request(
    token: &str,
    show_id: light_core::ShowId,
    desk_id: Uuid,
    request_id: &str,
) -> Request<Body> {
    Request::post("/api/v2/playback-actions")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-tosk-show", show_id.0.to_string())
        .header("x-tosk-desk", desk_id.to_string())
        .body(Body::from(
            serde_json::json!({
                "request_id": request_id,
                "address": {"kind": "playback", "playback_number": 7},
                "action": {"type": "go", "pressed": true},
                "surface": "virtual"
            })
            .to_string(),
        ))
        .unwrap()
}

async fn wait_for_macro_executions(
    state: &AppState,
    desk_id: Uuid,
    expected: usize,
) -> Vec<light_application::CommandMacroExecutionSnapshot> {
    for _ in 0..100 {
        let snapshot = state.macros.snapshot(desk_id);
        let executions = snapshot.recent;
        if executions.len() >= expected {
            return executions;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("Macro executions did not reach {expected}")
}
