use super::*;

#[tokio::test]
async fn command_line_v2_is_revisioned_desk_scoped_and_idempotent() {
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
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            groups: vec![light_programmer::GroupDefinition {
                id: "1".into(),
                name: "Group 1".into(),
                fixtures: vec![fixture_id],
                ..Default::default()
            }],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let path = format!("/api/v2/desks/{}/command-line", session.desk.id);

    let initial = app
        .clone()
        .oneshot(
            Request::get(&path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial.status(), StatusCode::OK);
    assert_eq!(initial.headers()[header::ETAG], "\"0\"");
    let initial = json(initial).await;
    assert_eq!(initial["text"], "FIXTURE");
    assert_eq!(initial["target"], "FIXTURE");
    assert_eq!(initial["pristine"], true);
    assert_eq!(initial["revision"], 0);
    assert!(initial["pending_choice"].is_null());

    let canonical_no_op = app
        .clone()
        .oneshot(
            Request::put(&path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, "\"0\"")
                .body(Body::from(r#"{"text":"FIXTURE"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(canonical_no_op.status(), StatusCode::OK);
    assert_eq!(canonical_no_op.headers()[header::ETAG], "\"0\"");
    assert_eq!(json(canonical_no_op).await["text"], "FIXTURE");

    let replaced = app
        .clone()
        .oneshot(
            Request::put(&path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, "\"0\"")
                .body(Body::from(r#"{"text":"FIXTURE 1"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replaced.status(), StatusCode::OK);
    assert_eq!(replaced.headers()[header::ETAG], "\"1\"");
    assert_eq!(json(replaced).await["text"], "FIXTURE 1");

    let stale = app
        .clone()
        .oneshot(
            Request::put(&path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, "0")
                .body(Body::from(r#"{"text":"FIXTURE 99"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        state
            .programmers
            .command_line_state(session.id)
            .unwrap()
            .visible_text(),
        "FIXTURE 1"
    );

    let execute_path = format!("{path}/execute");
    let request = serde_json::json!({"request_id":"execute-fixture-1"}).to_string();
    let executed = app
        .clone()
        .oneshot(
            Request::post(&execute_path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(request.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(executed.status(), StatusCode::OK);
    let executed_etag = executed.headers()[header::ETAG].clone();
    let executed = json(executed).await;
    assert_eq!(executed["outcome"], "accepted");
    assert_eq!(executed["action"], "executed");
    assert_eq!(executed["applied"], 1);
    assert_eq!(executed["command_line"]["text"], "FIXTURE");
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        vec![fixture_id]
    );

    let replayed = app
        .clone()
        .oneshot(
            Request::post(&execute_path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(request))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replayed.status(), StatusCode::OK);
    assert_eq!(replayed.headers()[header::ETAG], executed_etag);
    assert_eq!(json(replayed).await, executed);
    assert_eq!(
        state
            .command_history
            .lock()
            .get(&session.desk.id)
            .unwrap()
            .len(),
        1
    );

    let reused = app
        .clone()
        .oneshot(
            Request::post(&execute_path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"execute-fixture-1",
                            "command":"GROUP 1 AT BOGUS"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reused.status(), StatusCode::CONFLICT);

    let compatibility_choice = command_http::execute_existing_command(
        &state,
        &session,
        "COPY SET 1 CUE 1 AT SET 2 CUE 2",
        "test",
        Some("compatibility-choice"),
        command_http::ExistingCommandPolicy::Compatibility,
    );
    assert!(matches!(
        compatibility_choice,
        command_http::ExistingCommandOutcome::ChoiceRequired { .. }
    ));

    let compatibility_only = app
        .clone()
        .oneshot(
            Request::post(&execute_path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"pending-choice",
                        "command":"COPY SET 1 CUE 1 AT SET 2 CUE 2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(compatibility_only.status(), StatusCode::OK);
    let compatibility_only = json(compatibility_only).await;
    assert_eq!(compatibility_only["outcome"], "rejected");
    assert!(
        compatibility_only["error"]
            .as_str()
            .unwrap()
            .contains("not yet available through the atomic")
    );
    assert_eq!(
        compatibility_only["command_line"]["text"],
        "COPY SET 1 CUE 1 AT SET 2 CUE 2"
    );

    let selection_before_rejection = state.programmers.selection(session.id).unwrap();
    let programmer_before_rejection = state.programmers.get(session.id).unwrap();
    let rejected_request = serde_json::json!({
        "request_id":"missing-fixture",
        "command":"GROUP 1 AT BOGUS"
    })
    .to_string();
    let rejected = app
        .clone()
        .oneshot(
            Request::post(&execute_path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(rejected_request.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::OK);
    let rejected = json(rejected).await;
    assert_eq!(rejected["outcome"], "rejected");
    assert_eq!(rejected["command_line"]["text"], "GROUP 1 AT BOGUS");
    assert_eq!(
        state.programmers.selection(session.id).unwrap(),
        selection_before_rejection
    );
    let programmer_after_rejection = state.programmers.get(session.id).unwrap();
    assert_eq!(
        serde_json::to_value(programmer_after_rejection.values).unwrap(),
        serde_json::to_value(programmer_before_rejection.values).unwrap()
    );
    assert_eq!(
        serde_json::to_value(programmer_after_rejection.group_values).unwrap(),
        serde_json::to_value(programmer_before_rejection.group_values).unwrap()
    );
    assert_eq!(
        state
            .command_history
            .lock()
            .get(&session.desk.id)
            .unwrap()
            .len(),
        3
    );
    let replayed_rejection = app
        .clone()
        .oneshot(
            Request::post(&execute_path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(rejected_request))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json(replayed_rejection).await, rejected);
    assert_eq!(
        state
            .command_history
            .lock()
            .get(&session.desk.id)
            .unwrap()
            .len(),
        3
    );

    let wrong_desk = app
        .oneshot(
            Request::get(format!("/api/v2/desks/{}/command-line", Uuid::new_v4()))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_desk.status(), StatusCode::FORBIDDEN);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn supplied_command_executes_without_publishing_intermediate_command_text() {
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
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            groups: vec![light_programmer::GroupDefinition {
                id: "1".into(),
                name: "Group 1".into(),
                fixtures: vec![fixture_id],
                ..Default::default()
            }],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let command_line_events_before = state
        .audit_events
        .lock()
        .iter()
        .filter(|event| event.kind == "command_line_changed")
        .count();

    let response = app
        .oneshot(
            Request::post(format!(
                "/api/v2/desks/{}/command-line/execute",
                session.desk.id
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "request_id":"atomic-supplied-line",
                    "command":"GROUP 1 AT 50"
                })
                .to_string(),
            ))
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["outcome"], "accepted");
    assert_eq!(response["applied"], 1);
    assert_eq!(response["command_line"]["text"], "FIXTURE");
    assert_eq!(response["command_line"]["revision"], 0);
    assert_eq!(
        state
            .audit_events
            .lock()
            .iter()
            .filter(|event| event.kind == "command_line_changed")
            .count(),
        command_line_events_before
    );
    assert_eq!(
        state
            .audit_events
            .lock()
            .iter()
            .filter(|event| {
                matches!(
                    event.kind.as_str(),
                    "command_applied" | "programmer_changed"
                ) && event.payload["request_id"] == "atomic-supplied-line"
            })
            .count(),
        2
    );
    let programmer = state.programmers.get(session.id).unwrap();
    assert_eq!(programmer.selected, vec![fixture_id]);
    assert_eq!(
        programmer.group_values["1"][&light_core::AttributeKey::intensity()].value,
        light_core::AttributeValue::Normalized(0.5)
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn sensitive_command_text_is_returned_to_its_writer_but_redacted_from_global_events() {
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
    let path = format!("/api/v2/desks/{}/command-line", session.desk.id);
    let mut events = state.events.subscribe();

    let response = app
        .clone()
        .oneshot(
            Request::put(&path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, "0")
                .body(Body::from(
                    r#"{"text":"FIXTURE 1 TOKEN super-secret-value"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json(response).await["text"],
        "FIXTURE 1 TOKEN super-secret-value"
    );

    let event = events.recv().await.unwrap();
    assert_eq!(event.kind, "command_line_changed");
    assert_eq!(event.payload["text"], "[REDACTED SENSITIVE COMMAND]");
    assert_eq!(event.payload["redacted"], true);
    assert!(!event.payload.to_string().contains("super-secret-value"));
    let audit_event = state.audit_events.lock().back().cloned().unwrap();
    assert_eq!(audit_event.revision, event.revision);
    assert_eq!(audit_event.payload, event.payload);

    let current = app
        .oneshot(
            Request::get(&path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        json(current).await["text"],
        "FIXTURE 1 TOKEN super-secret-value"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn command_line_v2_keys_are_replay_safe_and_put_writers_use_one_atomic_boundary() {
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
    let path = format!("/api/v2/desks/{}/command-line", session.desk.id);
    let keys_path = format!("{path}/keys");
    let digit = serde_json::json!({
        "key":"7",
        "phase":"press",
        "request_id":"digit-7"
    })
    .to_string();

    let pressed = app
        .clone()
        .oneshot(
            Request::post(&keys_path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(digit.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pressed.status(), StatusCode::OK);
    let pressed = json(pressed).await;
    assert_eq!(pressed["outcome"], "accepted");
    assert_eq!(pressed["action"], "edited");
    assert_eq!(pressed["command_line"]["text"], "F7");
    assert_eq!(pressed["command_line"]["revision"], 1);

    let replayed = app
        .clone()
        .oneshot(
            Request::post(&keys_path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(digit))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json(replayed).await["command_line"]["revision"], 1);

    let second_login = app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username":"Operator",
                        "desk_id":session.desk.id,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_login.status(), StatusCode::OK);
    let second_token = json(second_login).await["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let second_session_same_request_id = app
        .clone()
        .oneshot(
            Request::post(&keys_path)
                .header(header::AUTHORIZATION, format!("Bearer {second_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "key":"7",
                        "phase":"press",
                        "request_id":"digit-7"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_session_same_request_id = json(second_session_same_request_id).await;
    assert_eq!(
        second_session_same_request_id["command_line"]["text"],
        "F77"
    );
    assert_eq!(
        second_session_same_request_id["command_line"]["revision"],
        2
    );

    let oversized = app
        .clone()
        .oneshot(
            Request::post(format!("{path}/execute"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"oversized",
                        "command":"X".repeat(40 * 1024),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let first = app.clone().oneshot(
        Request::put(&path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::IF_MATCH, "2")
            .body(Body::from(r#"{"text":"FIXTURE 1"}"#))
            .unwrap(),
    );
    let second = app.clone().oneshot(
        Request::put(&path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::IF_MATCH, "2")
            .body(Body::from(r#"{"text":"FIXTURE 2"}"#))
            .unwrap(),
    );
    let (first, second) = tokio::join!(first, second);
    let statuses = [first.unwrap().status(), second.unwrap().status()];
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::OK)
            .count(),
        1
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::CONFLICT)
            .count(),
        1
    );
    assert_eq!(
        state
            .programmers
            .command_line_state(session.id)
            .unwrap()
            .revision,
        3
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
