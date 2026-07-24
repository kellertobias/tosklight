use super::*;

#[tokio::test]
async fn programmer_values_ws_frame_uses_typed_service_and_relative_intent_once() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .cloned()
        .unwrap();
    open_values_test_show(&app, &token).await;
    let fixture_id = state.engine.snapshot().fixtures[0].fixture_id;
    let absolute_id = "ws-programmer-values-absolute";

    let absolute = dispatch_ws_command(
        &state,
        &session,
        values_command(
            &session,
            absolute_id,
            serde_json::json!({
                "request_id": absolute_id,
                "expected_revision": 0,
                "expected_capture_mode_revision": 0,
                "action": {
                    "type": "apply_intent",
                    "fixture_ids": [fixture_id.0],
                    "attribute": "intensity",
                    "operation": {
                        "type": "absolute_set",
                        "value": {"kind": "normalized", "value": 0.4}
                    },
                    "timing": {"fade": false},
                    "future_field": true
                },
                "future_envelope_field": true
            }),
        ),
    );
    assert!(absolute.ok, "{:?}", absolute.error);
    let payload = absolute.payload.unwrap();
    assert_eq!(payload["request_id"], absolute_id);
    assert_eq!(payload["status"], "changed");
    assert_eq!(payload["revision"], 1);
    assert_eq!(payload["replayed"], false);

    let relative_id = "ws-programmer-values-relative";
    let relative = dispatch_ws_command(
        &state,
        &session,
        values_command(
            &session,
            relative_id,
            serde_json::json!({
                "request_id": relative_id,
                "expected_revision": 1,
                "expected_capture_mode_revision": 0,
                "action": {
                    "type": "apply_intent",
                    "fixture_ids": [fixture_id.0],
                    "attribute": "intensity",
                    "operation": {"type": "relative_step", "delta": 0.1},
                    "timing": {"fade": false}
                }
            }),
        ),
    );
    assert!(relative.ok, "{:?}", relative.error);
    let payload = relative.payload.unwrap();
    assert_eq!(payload["status"], "changed");
    assert_eq!(payload["revision"], 2);
    assert_eq!(
        payload["projection"]["fixture_values"][0]["value"]["value"],
        0.5
    );
    assert_eq!(state.programmers.get(session.id).unwrap().undo.len(), 2);

    let replay = dispatch_ws_command(
        &state,
        &session,
        values_command(
            &session,
            relative_id,
            serde_json::json!({
                "request_id": relative_id,
                "expected_revision": 1,
                "expected_capture_mode_revision": 0,
                "action": {
                    "type": "apply_intent",
                    "fixture_ids": [fixture_id.0],
                    "attribute": "intensity",
                    "operation": {"type": "relative_step", "delta": 0.1},
                    "timing": {"fade": false}
                }
            }),
        ),
    );
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(replay.payload.unwrap()["replayed"], true);
    assert_eq!(state.programmers.get(session.id).unwrap().undo.len(), 2);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn programmer_values_ws_frame_rejects_mismatched_identity_without_mutation() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .cloned()
        .unwrap();
    open_values_test_show(&app, &token).await;

    let response = dispatch_ws_command(
        &state,
        &session,
        values_command(
            &session,
            "ws-envelope",
            serde_json::json!({
                "request_id": "different-payload",
                "expected_revision": 0,
                "expected_capture_mode_revision": 0,
                "action": {"type": "clear"}
            }),
        ),
    );
    assert!(!response.ok);
    assert!(
        response
            .error
            .unwrap()
            .contains("request_id must match the WebSocket request_id")
    );
    assert_eq!(state.programmers.normal_values_revision(session.user.id), 0);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn values_command(session: &Session, request_id: &str, payload: serde_json::Value) -> WsCommand {
    WsCommand {
        protocol_version: 1,
        request_id: request_id.into(),
        session_id: session.id,
        expected_revision: None,
        command: "programmer.values.action".into(),
        payload,
    }
}

async fn open_values_test_show(app: &Router, token: &str) {
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
