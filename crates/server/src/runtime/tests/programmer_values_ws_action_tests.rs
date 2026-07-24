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

#[tokio::test]
async fn preload_lifecycle_and_values_ws_frames_use_exact_typed_authority() {
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
    let enter_id = "ws-preload-enter";
    let enter = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: enter_id.into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.preload.lifecycle.action".into(),
            payload: serde_json::json!({
                "request_id":enter_id,
                "expected_capture_mode_revision":0,
                "expected_values_revision":0,
                "expected_queue_revision":0,
                "expected_selection_revision":0,
                "action":{"type":"enter","future_action_field":true},
                "future_request_field":true
            }),
        },
    );
    assert!(enter.ok, "{:?}", enter.error);
    let enter_payload = enter.payload.unwrap();
    assert_eq!(enter_payload["status"], "changed");
    assert_eq!(enter_payload["capture_mode"]["revision"], 1);

    let fixture_id = state.engine.snapshot().fixtures[0].fixture_id;
    let values_id = "ws-preload-values";
    let values = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: values_id.into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.preload.values.action".into(),
            payload: serde_json::json!({
                "request_id":values_id,
                "expected_revision":0,
                "expected_capture_mode_revision":1,
                "action":{
                    "type":"set_fixture",
                    "fixture_id":fixture_id.0,
                    "attribute":"intensity",
                    "value":{"kind":"normalized","value":0.6},
                    "timing":{"fade":false},
                    "future_action_field":true
                }
            }),
        },
    );
    assert!(values.ok, "{:?}", values.error);
    let values_payload = values.payload.unwrap();
    assert_eq!(values_payload["status"], "changed");
    assert_eq!(values_payload["revision"], 1);
    let replay = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: values_id.into(),
            session_id: session.id,
            expected_revision: None,
            command: "programmer.preload.values.action".into(),
            payload: serde_json::json!({
                "request_id":values_id,
                "expected_revision":0,
                "expected_capture_mode_revision":1,
                "action":{
                    "type":"set_fixture",
                    "fixture_id":fixture_id.0,
                    "attribute":"intensity",
                    "value":{"kind":"normalized","value":0.6},
                    "timing":{"fade":false},
                    "future_action_field":true
                }
            }),
        },
    );
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(replay.payload.unwrap()["replayed"], true);
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
        .oneshot(open_default_show_request(token))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
