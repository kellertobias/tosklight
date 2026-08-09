use super::*;

async fn get_json(app: &Router, path: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    (status, json(response).await)
}

#[tokio::test]
async fn runtime_v2_readiness_and_bootstrap_expose_the_current_contract() {
    let (state, data_dir) = test_state();
    let app = router(state);

    let (status, readiness) = get_json(&app, "/api/v2/readiness").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(readiness["status"], "ready");

    let (status, bootstrap) = get_json(&app, "/api/v2/bootstrap").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bootstrap["api_version"], "v2");
    assert!(
        bootstrap["users"]
            .as_array()
            .unwrap()
            .iter()
            .any(|user| { user["name"] == "Operator" && user["enabled"] == true })
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn runtime_v2_session_tolerates_unknown_fields_and_can_close_itself() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "Operator",
                        "future_client_hint": {"ignored": true}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let session = json(response).await;
    assert_eq!(session["user"]["name"], "Operator");
    assert!(session["client_id"].as_str().is_some());
    let session_id = session["session_id"].as_str().unwrap();
    let token = session["token"].as_str().unwrap();

    let close = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/v2/sessions/{session_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(close.status(), StatusCode::NO_CONTENT);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn runtime_v2_session_rejects_a_malformed_known_field() {
    let (state, data_dir) = test_state();
    let response = router(state)
        .oneshot(
            Request::post("/api/v2/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"username":7}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let message = String::from_utf8_lossy(&body);
    assert!(message.contains("username") || message.contains("string"));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn runtime_v2_preserves_diagnostics_auth_and_recovery_reporting() {
    let (state, data_dir) = test_state();
    state
        .active_show
        .set_error(Some("malformed active show".into()));
    let app = router(state);

    let unauthorized = app
        .clone()
        .oneshot(
            Request::get("/api/v2/diagnostics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let (token, _) = login(&app, "Operator").await;
    let diagnostics = app
        .clone()
        .oneshot(
            Request::get("/api/v2/diagnostics")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(diagnostics.status(), StatusCode::OK);
    let diagnostics = json(diagnostics).await;
    assert!(diagnostics["snapshot_revision"].is_number());
    assert!(diagnostics["extensions"]["extensions_directory"].is_string());
    let extensions = app
        .clone()
        .oneshot(
            Request::get("/api/v2/extensions")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(extensions.status(), StatusCode::OK);
    let extensions = json(extensions).await;
    assert!(extensions["packages"].is_array());
    let rescan = app
        .clone()
        .oneshot(
            Request::post("/api/v2/extensions/rescan")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"request_id":"runtime-test-rescan","future_field":true}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rescan.status(), StatusCode::OK);
    assert_eq!(
        json(rescan).await["extensions_directory"],
        extensions["extensions_directory"]
    );
    let performance = app
        .clone()
        .oneshot(
            Request::get("/api/v2/diagnostics/performance")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(performance.status(), StatusCode::OK);
    let performance = json(performance).await;
    assert!(performance["output"].is_object());
    assert!(performance["programmer_action_timing"].is_array());
    assert!(performance["visualization"].is_object());
    assert!(
        performance.get("active_programmers").is_none(),
        "the bounded performance route must not serialize active Programmer state"
    );

    let (_, readiness) = get_json(&app, "/api/v2/readiness").await;
    assert_eq!(readiness["recovery_mode"], true);
    assert_eq!(readiness["active_show_error"], "malformed active show");
    let (_, bootstrap) = get_json(&app, "/api/v2/bootstrap").await;
    assert_eq!(bootstrap["active_show_error"], "malformed active show");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn authenticated_programmer_http_actions_expose_shared_tick_budget_timing() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/programmer-capture-mode/actions")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"blind":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    for header in [
        "x-tosk-action-id",
        "x-tosk-received-output-tick",
        "x-tosk-ack-output-tick",
        "x-tosk-action-wall-micros",
        "x-tosk-output-frame-hz",
        "x-tosk-action-budget-ticks",
        "x-tosk-action-within-budget",
    ] {
        assert!(
            response.headers().get(header).is_some(),
            "Programmer response is missing {header}"
        );
    }
    assert_eq!(
        response.headers()["x-tosk-action-budget-ticks"],
        "2",
        "the <=60 Hz contract allows two configured output ticks"
    );
    assert_eq!(response.headers()["x-tosk-action-within-budget"], "true");

    let diagnostics = app
        .oneshot(
            Request::get("/api/v2/diagnostics")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let diagnostics = json(diagnostics).await;
    let measurements = diagnostics["programmer_action_timing"]
        .as_array()
        .expect("Programmer timing diagnostics");
    assert_eq!(measurements.len(), 1);
    assert_eq!(measurements[0]["source"], "http");
    assert_eq!(measurements[0]["action"], "capture_mode");
    assert_eq!(measurements[0]["acknowledgement_within_budget"], true);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn visualization_snapshot_reports_authoritative_source_and_route_costs() {
    let (state, data_dir) = test_state();
    let rendered = {
        let _activation = state.active_show.acquire().await;
        state
            .output
            .render_with_playback_events(
                &state.active_show.output_projection(),
                &state.playback.render_capability(),
                state.output.render_options(),
            )
            .expect("render one authoritative output frame")
    };
    state.output.render_frames_and_publish(
        &rendered,
        light_wire::v2::visualization::VisualizationScope {
            show_id: state.active_show.current().map(|show| show.id.0),
        },
    );
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;

    let visualization = app
        .clone()
        .oneshot(
            Request::get("/api/v2/output/visualization")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(visualization.status(), StatusCode::OK);
    let visualization = json(visualization).await;
    assert_eq!(visualization["source_frame"], 1);
    assert!(
        visualization["scope"].get("show_id").is_some(),
        "visualization snapshots carry explicit Show scope even with no active Show"
    );
    assert!(
        chrono::DateTime::parse_from_rfc3339(
            visualization["source_timestamp"]
                .as_str()
                .expect("source timestamp")
        )
        .is_ok()
    );

    let dynamic_stack = app
        .clone()
        .oneshot(
            Request::get("/api/v2/output/visualization?dynamic_stack_only=true")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(dynamic_stack.status(), StatusCode::OK);
    let dynamic_stack = json(dynamic_stack).await;
    assert_eq!(dynamic_stack["values"], serde_json::json!([]));
    assert_eq!(
        dynamic_stack["profile_output_values"],
        serde_json::json!([])
    );
    assert!(dynamic_stack["dynamic_stack"].is_array());

    let diagnostics = app
        .oneshot(
            Request::get("/api/v2/diagnostics")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(diagnostics.status(), StatusCode::OK);
    let visualization_metrics = &json(diagnostics).await["visualization"];
    assert_eq!(visualization_metrics["snapshot_requests"], 2);
    assert_eq!(visualization_metrics["snapshot_source_frame"], 1);
    assert!(
        visualization_metrics["snapshot_projection_micros"].is_number(),
        "projection is measured independently"
    );
    assert!(
        visualization_metrics["snapshot_serialization_micros"].is_number(),
        "payload serialization is measured independently"
    );
    assert!(
        visualization_metrics["snapshot_payload_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0)
    );
    assert!(visualization_metrics["snapshot_source_age_millis"].is_number());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_visualizer_session_is_read_only_and_claims_no_operator_state() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());

    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"username": "Operator", "role": "visualizer"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let session = json(response).await;
    assert_eq!(session["role"], "visualizer");
    let token = session["token"].as_str().unwrap().to_owned();
    let session_id = session["session_id"].as_str().unwrap().to_owned();

    let session_uuid: uuid::Uuid = session_id.parse().unwrap();

    // Reads succeed.
    let read = app
        .clone()
        .oneshot(
            Request::get("/api/v2/output/dmx")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);

    // Connecting and reading must not have started a programmer for the visualizer, which is
    // what owning a command-line context requires.
    assert!(
        !state
            .programming
            .attach_command_context(SessionId(session_uuid), SessionId(session_uuid)),
        "a visualizer session must not own a programmer or a command-line context"
    );

    // Every mutation is refused at the transport.
    let mutation = app
        .clone()
        .oneshot(
            Request::post("/api/v2/patch/fixtures")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id": "viz-1", "fixtures": []}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mutation.status(), StatusCode::FORBIDDEN);

    let _ = std::fs::remove_dir_all(data_dir);
}
