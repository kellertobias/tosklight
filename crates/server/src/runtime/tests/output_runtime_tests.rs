use super::*;

#[tokio::test]
async fn legacy_master_update_publishes_one_typed_change_and_v2_repairs_it() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let show = create_show(&app, &token, "Output runtime show").await;
    open_show_for_output_test(&app, &token, &show).await;

    let denied = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/desks/{}/output-runtime/global-master",
                session.desk.id
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

    let response = put_master(
        &app,
        &token,
        serde_json::json!({"grand_master":0.4,"blackout":true}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["blackout"], true);
    assert!((body["grand_master"].as_f64().unwrap() - 0.4).abs() < 0.000_001);

    let light_application::EventReplay::Events(events) = state.application_events.replay(
        0,
        &light_application::EventFilter::default()
            .with_object(light_application::EventObject::global_output()),
    ) else {
        panic!("global-output change should be retained");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].desk_id, None);
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Http)
    );
    assert!(events[0].correlation_id.is_some());

    let no_change = put_master(
        &app,
        &token,
        serde_json::json!({"grand_master":0.4,"blackout":true}),
    )
    .await;
    assert_eq!(no_change.status(), StatusCode::OK);
    assert_eq!(state.application_events.latest_sequence(), 1);

    let snapshot = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/desks/{}/output-runtime/global-master",
                session.desk.id
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    let snapshot = json(snapshot).await;
    assert_eq!(snapshot["cursor"]["sequence"], 1);
    assert_eq!(snapshot["projection"]["identity"], "global_master");
    assert!((snapshot["projection"]["grand_master"].as_f64().unwrap() - 0.4).abs() < 0.000_001);
    assert_eq!(snapshot["projection"]["blackout"], true);
    assert_eq!(snapshot["projection"]["scope"]["show_id"], show["id"]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn websocket_master_retry_is_idempotent_at_the_typed_boundary() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let command = || WsCommand {
        protocol_version: 1,
        request_id: "global-master-1".into(),
        session_id: session.id,
        expected_revision: None,
        command: "master.set".into(),
        payload: serde_json::json!({"grand_master":0.25}),
    };

    let first = dispatch_ws_command(&state, &session, command());
    let replay = dispatch_ws_command(&state, &session, command());
    assert!(first.ok, "{:?}", first.error);
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(first.payload, replay.payload);
    assert_eq!(state.output_control.lock().options.grand_master, 0.25);
    assert_eq!(state.application_events.latest_sequence(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn put_master(app: &Router, token: &str, payload: serde_json::Value) -> Response {
    app.clone()
        .oneshot(
            Request::put("/api/v1/master")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn open_show_for_output_test(app: &Router, token: &str, show: &serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/v1/shows/{}/open",
                show["id"].as_str().unwrap()
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"transition":"hold_current"}"#))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
