use super::*;

#[tokio::test]
async fn command_line_replace_ws_is_exact_correlated_and_replay_safe() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_default_show(&app, &token).await;
    let revision = state
        .programmers
        .command_line_state(session.id)
        .unwrap()
        .revision;

    let command_payload = serde_json::json!({
        "request_id": "command-line-1",
        "expected_revision": revision,
        "text": "FIXTURE 101",
        "future_field": true
    });
    let response = dispatch_ws_command(
        &state,
        &session,
        interaction_command(
            &session,
            "command-line-1",
            "programmer.command_line.replace",
            command_payload.clone(),
        ),
    );
    assert!(response.ok, "{:?}", response.error);
    let payload = response.payload.unwrap();
    assert_eq!(payload["text"], "FIXTURE 101");
    assert_eq!(payload["revision"], revision + 1);

    let replay = dispatch_ws_command(
        &state,
        &session,
        interaction_command(
            &session,
            "command-line-1",
            "programmer.command_line.replace",
            command_payload,
        ),
    );
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(replay.payload.unwrap()["revision"], revision + 1);
    assert_eq!(
        state
            .programmers
            .command_line_state(session.id)
            .unwrap()
            .revision,
        revision + 1
    );

    let stale = dispatch_ws_command(
        &state,
        &session,
        interaction_command(
            &session,
            "command-line-stale",
            "programmer.command_line.replace",
            serde_json::json!({
                "request_id": "command-line-stale",
                "expected_revision": revision,
                "text": "GROUP"
            }),
        ),
    );
    assert!(!stale.ok);
    assert!(stale.error.unwrap().contains("revision conflict"));
    assert_eq!(
        state
            .programmers
            .command_line_state(session.id)
            .unwrap()
            .visible_text(),
        "FIXTURE 101"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn selection_action_ws_uses_typed_service_and_request_identity() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_default_show(&app, &token).await;
    let fixture_id = state.engine.snapshot().fixtures[0].fixture_id;
    let revision = state.programmers.selection(session.id).unwrap().revision;

    let selection_payload = serde_json::json!({
        "request_id": "selection-1",
        "action": "replace",
        "fixtures": [fixture_id.0],
        "expected_revision": revision,
        "future_field": true
    });
    let response = dispatch_ws_command(
        &state,
        &session,
        interaction_command(
            &session,
            "selection-1",
            "programmer.selection.action",
            selection_payload.clone(),
        ),
    );
    assert!(response.ok, "{:?}", response.error);
    let payload = response.payload.unwrap();
    assert_eq!(payload["request_id"], "selection-1");
    assert_eq!(payload["action"], "replaced");
    assert_eq!(
        payload["selection"]["selected"],
        serde_json::json!([fixture_id.0])
    );
    assert_eq!(payload["replayed"], false);

    let replay = dispatch_ws_command(
        &state,
        &session,
        interaction_command(
            &session,
            "selection-1",
            "programmer.selection.action",
            selection_payload,
        ),
    );
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(replay.payload.unwrap()["replayed"], true);
    assert_eq!(
        state.programmers.selection(session.id).unwrap().revision,
        revision + 1
    );

    let mismatched = dispatch_ws_command(
        &state,
        &session,
        interaction_command(
            &session,
            "selection-envelope",
            "programmer.selection.action",
            serde_json::json!({
                "request_id": "selection-payload",
                "action": "apply_rule",
                "rule": {"type": "all"}
            }),
        ),
    );
    assert!(!mismatched.ok);
    assert!(
        mismatched
            .error
            .unwrap()
            .contains("request_id must match the WebSocket request_id")
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn selection_action_ws_accepts_the_complete_action_union() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_default_show(&app, &token).await;
    let fixture_id = state.engine.snapshot().fixtures[0].fixture_id;
    let show_id = state.active_show.read().as_ref().unwrap().id.0;
    let group = app
        .clone()
        .oneshot(
            Request::put(format!("/api/v1/shows/{show_id}/objects/group/1"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, "0")
                .body(Body::from(
                    serde_json::json!({
                        "name": "WS selection group",
                        "fixtures": [fixture_id.0],
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    if group.status() != StatusCode::OK {
        let status = group.status();
        let body = group.into_body().collect().await.unwrap().to_bytes();
        panic!(
            "group install failed ({status}): {}",
            String::from_utf8_lossy(&body)
        );
    }
    let revision = state.programmers.selection(session.id).unwrap().revision;

    for (request_id, payload, accepted_action) in [
        (
            "selection-replace",
            serde_json::json!({
                "request_id": "selection-replace",
                "action": "replace",
                "fixtures": [fixture_id.0],
                "expected_revision": revision
            }),
            "replaced",
        ),
        (
            "selection-gesture",
            serde_json::json!({
                "request_id": "selection-gesture",
                "action": "gesture",
                "source": {"type": "fixture", "fixture_id": fixture_id.0},
                "remove": true
            }),
            "gesture_applied",
        ),
        (
            "selection-group",
            serde_json::json!({
                "request_id": "selection-group",
                "action": "select_group",
                "group_id": "1",
                "frozen": false,
                "rule": {"type": "all"},
                "expected_revision": revision + 2
            }),
            "group_selected",
        ),
        (
            "selection-rule",
            serde_json::json!({
                "request_id": "selection-rule",
                "action": "apply_rule",
                "rule": {"type": "odd"}
            }),
            "rule_applied",
        ),
    ] {
        let response = dispatch_ws_command(
            &state,
            &session,
            interaction_command(&session, request_id, "programmer.selection.action", payload),
        );
        assert!(response.ok, "{request_id}: {:?}", response.error);
        assert_eq!(response.payload.unwrap()["action"], accepted_action);
    }

    let _ = std::fs::remove_dir_all(data_dir);
}

fn session_for_token(state: &AppState, token: &str) -> Session {
    state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .cloned()
        .unwrap()
}

fn interaction_command(
    session: &Session,
    request_id: &str,
    command: &str,
    payload: serde_json::Value,
) -> WsCommand {
    WsCommand {
        protocol_version: 1,
        request_id: request_id.into(),
        session_id: session.id,
        expected_revision: None,
        command: command.into(),
        payload,
    }
}

async fn open_default_show(app: &Router, token: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/shows/default/open")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
