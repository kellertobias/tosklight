use super::*;

#[tokio::test]
async fn command_line_replace_ws_is_exact_correlated_and_replay_safe() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_default_show(&app, &token).await;
    let revision = state
        .programming
        .command_line_state(session.id)
        .unwrap()
        .revision;

    let command_payload = serde_json::json!({
        "request_id": "command-line-1",
        "expected_revision": revision,
        "text": "FIXTURE 101",
        "future_field": true
    });
    let response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "command-line-1",
            serde_json::from_value(serde_json::json!({
                "type":"command_line_replace",
                "request":command_payload.clone()
            }))
            .unwrap(),
        ),
    );
    assert!(response.ok, "{:?}", response.error);
    let payload = response.payload.unwrap();
    assert_eq!(payload["text"], "FIXTURE 101");
    assert_eq!(payload["revision"], revision + 1);

    let replay = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "command-line-1",
            serde_json::from_value(serde_json::json!({
                "type":"command_line_replace",
                "request":command_payload
            }))
            .unwrap(),
        ),
    );
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(replay.payload.unwrap()["revision"], revision + 1);
    assert_eq!(
        state
            .programming
            .command_line_state(session.id)
            .unwrap()
            .revision,
        revision + 1
    );

    let stale = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "command-line-stale",
            serde_json::from_value(serde_json::json!({
                "type":"command_line_replace",
                "request":{
                    "request_id": "command-line-stale",
                    "expected_revision": revision,
                    "text": "GROUP"
                }
            }))
            .unwrap(),
        ),
    );
    assert!(!stale.ok);
    assert!(stale.error.unwrap().contains("revision conflict"));
    assert_eq!(
        state
            .programming
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
    let fixture_id = state
        .output
        .snapshot()
        .fixtures
        .iter()
        .find(|fixture| fixture.logical_heads.is_empty())
        .expect("Default Stage must contain a leaf fixture")
        .fixture_id;
    let revision = state.programming.selection(session.id).unwrap().revision;

    let selection_payload = serde_json::json!({
        "request_id": "selection-1",
        "action": "replace",
        "fixtures": [fixture_id.0],
        "expected_revision": revision,
        "future_field": true
    });
    let response = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "selection-1",
            serde_json::from_value(serde_json::json!({
                "type":"programming_selection",
                "request":selection_payload.clone()
            }))
            .unwrap(),
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

    let replay = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "selection-1",
            serde_json::from_value(serde_json::json!({
                "type":"programming_selection",
                "request":selection_payload
            }))
            .unwrap(),
        ),
    );
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(replay.payload.unwrap()["replayed"], true);
    assert_eq!(
        state.programming.selection(session.id).unwrap().revision,
        revision + 1
    );

    let mismatched = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "selection-envelope",
            serde_json::from_value(serde_json::json!({
                "type":"programming_selection",
                "request":{
                    "request_id": "selection-payload",
                    "action": "apply_rule",
                    "rule": {"type": "all"}
                }
            }))
            .unwrap(),
        ),
    );
    assert!(!mismatched.ok);
    assert!(
        mismatched
            .error
            .unwrap()
            .contains("action payload request_id must match the frame request_id")
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
    let fixture_id = state.output.snapshot().fixtures[0].fixture_id;
    let show_id = state.active_show.current().as_ref().unwrap().id.0;
    let group = seed_show_object(
        &state,
        &token,
        &show_id.to_string(),
        "group",
        "1",
        0,
        serde_json::json!({
            "name": "WS selection group",
            "fixtures": [fixture_id.0],
        }),
    )
    .await;
    if group.status() != StatusCode::OK {
        let status = group.status();
        let body = group.into_body().collect().await.unwrap().to_bytes();
        panic!(
            "group install failed ({status}): {}",
            String::from_utf8_lossy(&body)
        );
    }
    let revision = state.programming.selection(session.id).unwrap().revision;

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
        let response = dispatch_live_action(
            &state,
            &session,
            live_action_frame(
                &session,
                request_id,
                serde_json::from_value(serde_json::json!({
                    "type":"programming_selection",
                    "request":payload
                }))
                .unwrap(),
            ),
        );
        assert!(response.ok, "{request_id}: {:?}", response.error);
        assert_eq!(response.payload.unwrap()["action"], accepted_action);
    }

    let _ = std::fs::remove_dir_all(data_dir);
}

fn session_for_token(state: &AppState, token: &str) -> Session {
    state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap()
}

async fn open_default_show(app: &Router, token: &str) {
    let response = app
        .clone()
        .oneshot(open_default_show_request(token))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
